//! The first real connection (docs/VPN.md, «Pendiente» 3), on a Windows of the continuous integration, as an
//! administrator: the job «de verdad» of `.github/workflows/vpn.yml`, which runs only when launched by hand. Every step
//! has its own time limit:
//!
//! 1. before connecting: the server's name resolved (the public address expected outside), this computer's public
//!    address, its gateway and its address on the network, and what the gateway and a public resolver answer;
//! 2. the whole connection, `guardiana_apps::tunel_win::conecta` (sign-up, tunnel with everything inside, cut-off switch
//!    on, the server's first handshake: if it does not come in 8 s the tunnel and its switch go, and the computer stays
//!    as it was);
//! 3. with it: names answered by the tunnel's resolver; the public address an echo service sees over HTTPS is the
//!    server's; the gateway, and a public resolver asked from the network card (outside the tunnel), get nothing out,
//!    and Windows says the filter cut them: to the program (WSAEACCES, 10013) for a TCP connection, and in its own
//!    record for a UDP datagram, which it drops without telling the sender (Security events 5157 and 5152, which the
//!    workflow turns on; the first run, 3 Nov 2026, saw no answer and no WSAEACCES for UDP, which proves nothing by
//!    itself);
//! 4. disconnected, always (also when a check fails or panics; and at the time limit a watchdog closes the tunnel and
//!    ends the process), and the network is back: the public address is this computer's again and the adapter is gone.
//!
//! If the process dies half-way, Windows itself removes the filters (a dynamic session) and the adapter with it.
//!
//! `GUARDIANA_WG_DLL`: the path of `wireguard.dll`. `GUARDIANA_VPN_SERVIDOR`: the server (by default
//! `vpn.guardianagroup.com`). Exit code 0 when every check holds.

use std::process::ExitCode;

#[cfg(not(windows))]
fn main() -> ExitCode {
    eprintln!("vpn_de_verdad: solo en Windows (WireGuardNT y el Filtering Platform)");
    ExitCode::from(2)
}

#[cfg(windows)]
fn main() -> ExitCode {
    de_verdad::principal()
}

/// The whole run, Windows only.
#[cfg(windows)]
mod de_verdad {
    use std::io::Read;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
    use std::path::{Path, PathBuf};
    use std::process::{Command, ExitCode, Stdio};
    use std::sync::atomic::{AtomicU16, Ordering};
    use std::sync::{Arc, LazyLock, Mutex, PoisonError};
    use std::time::{Duration, Instant};

    use guardiana_apps::tunel_win::{self, Conexion};

    /// WinSock's «an attempt was made to access a socket in a way forbidden by its access permissions»: what a
    /// connection the Filtering Platform blocks gets back, before anything is sent.
    const WSAEACCES: i32 = 10013;
    /// The longest the tunnel stays up, whatever happens: then the watchdog closes it and ends the process.
    const PLAZO_TUNEL: Duration = Duration::from_secs(150);
    /// How long an answer, or a connection, is waited for.
    const PLAZO_RED: Duration = Duration::from_secs(4);
    /// The longest a program run from here (curl, PowerShell) may take.
    const PLAZO_PROGRAMA: Duration = Duration::from_secs(40);
    /// Public resolvers, asked from the network card: they answer before the tunnel, and with it nothing gets out.
    const RESOLUTORES: [Ipv4Addr; 2] = [Ipv4Addr::new(1, 1, 1, 1), Ipv4Addr::new(8, 8, 8, 8)];
    /// Services that answer, over HTTPS, with the address they see; the first that answers counts.
    const ECOS: [&str; 2] = ["api.ipify.org", "checkip.amazonaws.com"];
    /// A name the tunnel's resolver must answer.
    const NOMBRE: &str = "guardianagroup.com";

    static ARRANQUE: LazyLock<Instant> = LazyLock::new(Instant::now);
    static ID: AtomicU16 = AtomicU16::new(0x4741);

    /// One line of the log, with the seconds since the start: if the runner loses GitHub meanwhile, the log still
    /// says where each step stood.
    fn linea(texto: impl std::fmt::Display) {
        println!("[{:>6.1} s] {texto}", ARRANQUE.elapsed().as_secs_f32());
    }

    /// The checks and whether each one held.
    #[derive(Default)]
    struct Informe {
        hechas: Vec<(bool, String)>,
    }

    impl Informe {
        fn pon(&mut self, bien: bool, que: impl Into<String>) {
            let que = que.into();
            linea(format!("{} {que}", if bien { "BIEN" } else { "MAL " }));
            self.hechas.push((bien, que));
        }

        fn todo_bien(&self) -> bool {
            self.hechas.iter().all(|(bien, _)| *bien)
        }
    }

    /// Why nothing came back from a probe.
    #[derive(Debug, PartialEq, Eq)]
    enum Fallo {
        /// Windows refused to send it: the Filtering Platform cut it before it left (WSAEACCES).
        Filtro,
        /// Sent, as far as Windows said, and nothing came back in time.
        Plazo,
        /// Something else, as Windows said it.
        Otro(String),
    }

    impl std::fmt::Display for Fallo {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Filtro => {
                    f.write_str("cortado por el filtro antes de salir (WSAEACCES, 10013)")
                }
                Self::Plazo => write!(f, "sin respuesta en {} s", PLAZO_RED.as_secs()),
                Self::Otro(e) => f.write_str(e),
            }
        }
    }

    fn fallo(e: &std::io::Error) -> Fallo {
        if e.raw_os_error() == Some(WSAEACCES) {
            Fallo::Filtro
        } else if matches!(
            e.kind(),
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
        ) {
            Fallo::Plazo
        } else {
            Fallo::Otro(e.to_string())
        }
    }

    /// The open connection, shared with the watchdog: whoever takes it closes it.
    type Abierta = Arc<Mutex<Option<Conexion>>>;

    /// Closes the connection if it is still open; whether this call closed it.
    fn cierra(abierta: &Mutex<Option<Conexion>>) -> bool {
        let c = abierta
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        match c {
            Some(c) => {
                c.tunel.cierra();
                true
            }
            None => false,
        }
    }

    /// Closes the tunnel when dropped: whatever ends the checks, a panic included.
    struct AlSalir(Abierta);

    impl Drop for AlSalir {
        fn drop(&mut self) {
            if cierra(&self.0) {
                linea("desconectada");
            }
        }
    }

    /// At [`PLAZO_TUNEL`], if the tunnel is still open, closes it and ends the process.
    fn vigilante(abierta: Abierta) {
        std::thread::spawn(move || {
            let hasta = Instant::now() + PLAZO_TUNEL;
            while Instant::now() < hasta {
                std::thread::sleep(Duration::from_millis(250));
                if abierta
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .is_none()
                {
                    return;
                }
            }
            linea(format!(
                "vigilante: más de {} s con el túnel puesto; se cierra y se sale",
                PLAZO_TUNEL.as_secs()
            ));
            cierra(&abierta);
            std::process::exit(3);
        });
    }

    /// Runs `programa` with `args`, at most [`PLAZO_PROGRAMA`]; its standard output when it ends well.
    fn ejecuta(programa: &Path, args: &[String]) -> Result<String, String> {
        let nombre = programa.file_name().map_or_else(
            || programa.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let mut hijo = Command::new(programa)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("{nombre}: {e}"))?;
        let hasta = Instant::now() + PLAZO_PROGRAMA;
        loop {
            match hijo.try_wait() {
                Ok(Some(estado)) => {
                    // as bytes: PowerShell writes in the console's code page, and a line that is not UTF-8 must
                    // not come back empty
                    let mut salida = Vec::new();
                    let mut errores = Vec::new();
                    if let Some(mut s) = hijo.stdout.take() {
                        let _ = s.read_to_end(&mut salida);
                    }
                    if let Some(mut s) = hijo.stderr.take() {
                        let _ = s.read_to_end(&mut errores);
                    }
                    let salida = String::from_utf8_lossy(&salida).into_owned();
                    let errores = String::from_utf8_lossy(&errores);
                    return if estado.success() {
                        Ok(salida)
                    } else {
                        Err(format!("{nombre} ({estado}): {}", recorta(&errores)))
                    };
                }
                Ok(None) if Instant::now() < hasta => {
                    std::thread::sleep(Duration::from_millis(100))
                }
                Ok(None) => {
                    let _ = hijo.kill();
                    let _ = hijo.wait();
                    return Err(format!("{nombre}: más de {} s", PLAZO_PROGRAMA.as_secs()));
                }
                Err(e) => return Err(format!("{nombre}: {e}")),
            }
        }
    }

    /// At most a line of 200 characters, for the log.
    fn recorta(texto: &str) -> String {
        let una: String = texto.split_whitespace().collect::<Vec<_>>().join(" ");
        una.chars().take(200).collect()
    }

    fn system32() -> PathBuf {
        std::env::var_os("SystemRoot")
            .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from)
            .join("System32")
    }

    fn powershell(orden: &str) -> Result<String, String> {
        ejecuta(
            &system32().join(r"WindowsPowerShell\v1.0\powershell.exe"),
            &[
                "-NoProfile".to_owned(),
                "-NonInteractive".to_owned(),
                "-Command".to_owned(),
                orden.to_owned(),
            ],
        )
    }

    /// The first IPv4 address of `nombre`, from the system's resolver.
    fn resuelve_v4(nombre: &str) -> Result<Ipv4Addr, String> {
        let todas: Vec<SocketAddr> = (nombre, 443)
            .to_socket_addrs()
            .map_err(|e| e.to_string())?
            .collect();
        todas
            .iter()
            .find_map(|a| match a.ip() {
                IpAddr::V4(v4) => Some(v4),
                IpAddr::V6(_) => None,
            })
            .ok_or_else(|| format!("sin dirección IPv4 ({} en total)", todas.len()))
    }

    /// The default gateway, read before the tunnel takes the default route.
    fn puerta_de_enlace() -> Result<Ipv4Addr, String> {
        let salida = powershell(
            "Get-NetRoute -AddressFamily IPv4 -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue | Sort-Object RouteMetric | Select-Object -ExpandProperty NextHop",
        )?;
        salida
            .lines()
            .filter_map(|l| l.trim().parse::<Ipv4Addr>().ok())
            .find(|ip| !ip.is_unspecified())
            .ok_or_else(|| format!("ninguna en «{}»", recorta(&salida)))
    }

    /// A DNS question for the A records of `nombre`.
    fn pregunta(id: u16, nombre: &str) -> Vec<u8> {
        let mut m = Vec::with_capacity(32 + nombre.len());
        m.extend_from_slice(&id.to_be_bytes());
        // recursion desired; one question
        m.extend_from_slice(&[0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0]);
        for parte in nombre.trim_end_matches('.').split('.') {
            m.push(u8::try_from(parte.len()).unwrap_or(63).min(63));
            m.extend(parte.bytes().take(63));
        }
        // the root, type A, class IN
        m.extend_from_slice(&[0, 0, 1, 0, 1]);
        m
    }

    fn u16_en(m: &[u8], i: usize) -> Option<u16> {
        Some(u16::from_be_bytes([*m.get(i)?, *m.get(i + 1)?]))
    }

    /// Where the name that starts at `i` ends.
    fn tras_el_nombre(m: &[u8], mut i: usize) -> Option<usize> {
        for _ in 0..128 {
            let largo = *m.get(i)?;
            if largo & 0xC0 == 0xC0 {
                return Some(i + 2);
            }
            if largo == 0 {
                return Some(i + 1);
            }
            i += 1 + usize::from(largo);
        }
        None
    }

    /// The A records of an answer to question `id`; what is wrong with it otherwise.
    fn respuesta(m: &[u8], id: u16) -> Result<Vec<Ipv4Addr>, String> {
        let corta = || "respuesta incompleta".to_owned();
        if u16_en(m, 0).ok_or_else(corta)? != id {
            return Err("respuesta a otra pregunta".to_owned());
        }
        let banderas = u16_en(m, 2).ok_or_else(corta)?;
        if banderas & 0x8000 == 0 {
            return Err("no es una respuesta".to_owned());
        }
        let codigo = banderas & 0x000F;
        if codigo != 0 {
            return Err(format!("el resolutor contestó con el código {codigo}"));
        }
        let preguntas = u16_en(m, 4).ok_or_else(corta)?;
        let respuestas = u16_en(m, 6).ok_or_else(corta)?;
        let mut i = 12;
        for _ in 0..preguntas {
            i = tras_el_nombre(m, i).ok_or_else(corta)? + 4;
        }
        let mut direcciones = Vec::new();
        for _ in 0..respuestas {
            i = tras_el_nombre(m, i).ok_or_else(corta)?;
            let tipo = u16_en(m, i).ok_or_else(corta)?;
            let largo = usize::from(u16_en(m, i + 8).ok_or_else(corta)?);
            let datos = m.get(i + 10..i + 10 + largo).ok_or_else(corta)?;
            if let (1, [a, b, c, d]) = (tipo, datos) {
                direcciones.push(Ipv4Addr::new(*a, *b, *c, *d));
            }
            i += 10 + largo;
        }
        Ok(direcciones)
    }

    /// Asks `servidor` for the A records of `nombre` from a socket bound to `desde` (`0.0.0.0` lets the routes choose;
    /// the network card's own address keeps it on the card, outside the tunnel).
    fn pregunta_dns(
        desde: Ipv4Addr,
        servidor: SocketAddr,
        nombre: &str,
    ) -> Result<Vec<Ipv4Addr>, Fallo> {
        let s = UdpSocket::bind((desde, 0)).map_err(|e| fallo(&e))?;
        s.set_read_timeout(Some(PLAZO_RED)).map_err(|e| fallo(&e))?;
        let id = ID.fetch_add(1, Ordering::Relaxed);
        s.send_to(&pregunta(id, nombre), servidor)
            .map_err(|e| fallo(&e))?;
        let mut buf = [0u8; 1500];
        for _ in 0..8 {
            let (n, de) = s.recv_from(&mut buf).map_err(|e| fallo(&e))?;
            if de == servidor {
                return respuesta(buf.get(..n).unwrap_or_default(), id).map_err(Fallo::Otro);
            }
        }
        Err(Fallo::Otro("solo contestaron otros".to_owned()))
    }

    /// What Windows recorded, in the last minute, of a connection or a packet the Filtering Platform blocked from
    /// `origen` to `destino` (Security events 5157 and 5152, audited when the workflow turns «Filtering Platform
    /// Connection» and «Packet Drop» on): direction, protocol, layer, filter and program, when there is one. Asked a few
    /// times, because Windows writes the record a moment later.
    fn bloqueo_anotado(origen: Ipv4Addr, destino: SocketAddr) -> Result<Option<String>, String> {
        let orden = r#"$desde = (Get-Date).AddSeconds(-60); Get-WinEvent -FilterHashtable @{ LogName = 'Security'; Id = 5157, 5152; StartTime = $desde } -ErrorAction SilentlyContinue | ForEach-Object { $e = $_; $d = @{}; ([xml]$e.ToXml()).Event.EventData.Data | ForEach-Object { $d[$_.Name] = $_.'#text' }; if ($d.SourceAddress -eq '__ORIGEN__' -and $d.DestAddress -eq '__IP__' -and $d.DestPort -eq '__PUERTO__') { "suceso $($e.Id): direccion $($d.Direction), protocolo $($d.Protocol), capa $($d.LayerName), filtro $($d.FilterRTID), $($d.Application)" } } | Select-Object -First 1"#
            .replace("__ORIGEN__", &origen.to_string())
            .replace("__IP__", &destino.ip().to_string())
            .replace("__PUERTO__", &destino.port().to_string());
        for intento in 0..4 {
            if intento > 0 {
                std::thread::sleep(Duration::from_secs(1));
            }
            let salida = powershell(&orden)?;
            let s = salida.trim();
            if !s.is_empty() {
                return Ok(Some(recorta(s)));
            }
        }
        Ok(None)
    }

    /// A TCP connection to `a`, at most [`PLAZO_RED`].
    fn tcp(a: SocketAddr) -> Result<&'static str, Fallo> {
        match TcpStream::connect_timeout(&a, PLAZO_RED) {
            Ok(_) => Ok("conectado"),
            Err(e) => Err(fallo(&e)),
        }
    }

    fn como_texto<T: std::fmt::Display>(r: &Result<T, Fallo>) -> String {
        match r {
            Ok(v) => format!("contesta ({v})"),
            Err(e) => e.to_string(),
        }
    }

    fn lista(ips: &[Ipv4Addr]) -> String {
        ips.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The address an echo service sees, over HTTPS with Windows' own curl. With `fija`, curl is given the service's
    /// address and asks no resolver.
    fn eco(servicio: &str, fija: Option<Ipv4Addr>) -> Result<IpAddr, String> {
        let mut args: Vec<String> = [
            "-sS",
            "--ssl-no-revoke",
            "--connect-timeout",
            "8",
            "--max-time",
            "15",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        if let Some(ip) = fija {
            args.push("--resolve".to_owned());
            args.push(format!("{servicio}:443:{ip}"));
        }
        args.push(format!("https://{servicio}/"));
        let salida = ejecuta(&system32().join("curl.exe"), &args)?;
        let texto = salida.trim();
        texto
            .parse::<IpAddr>()
            .map_err(|_| format!("contestó «{}»", recorta(texto)))
    }

    /// This computer's public address, from the first echo service that answers. With `resolutor`, each service's
    /// name is asked there (through the tunnel) and handed to curl.
    fn ip_publica(resolutor: Option<SocketAddr>) -> Result<IpAddr, String> {
        let mut fallos = Vec::new();
        for servicio in ECOS {
            let fija = match resolutor {
                Some(r) => match pregunta_dns(Ipv4Addr::UNSPECIFIED, r, servicio) {
                    Ok(ips) => match ips.first() {
                        Some(ip) => Some(*ip),
                        None => {
                            fallos.push(format!("{servicio}: el resolutor no dio dirección"));
                            continue;
                        }
                    },
                    Err(e) => {
                        fallos.push(format!("{servicio}: el resolutor: {e}"));
                        continue;
                    }
                },
                None => None,
            };
            match eco(servicio, fija) {
                Ok(ip) => return Ok(ip),
                Err(e) => fallos.push(format!("{servicio}: {e}")),
            }
        }
        Err(fallos.join("; "))
    }

    /// What the network looked like before the tunnel.
    struct Antes {
        publica: Result<IpAddr, String>,
        puerta: Option<Ipv4Addr>,
        lan: Option<Ipv4Addr>,
        puerta_tcp: Option<Result<&'static str, Fallo>>,
        dns_fuera: Vec<(Ipv4Addr, Result<String, Fallo>)>,
    }

    fn antes() -> Antes {
        let publica = ip_publica(None);
        linea(format!(
            "antes: la dirección pública del equipo: {}",
            publica
                .as_ref()
                .map_or_else(Clone::clone, ToString::to_string)
        ));
        let puerta = match puerta_de_enlace() {
            Ok(p) => Some(p),
            Err(e) => {
                linea(format!("antes: la puerta de enlace: {e}"));
                None
            }
        };
        let lan = guardiana_core::tunel_dns::lan();
        linea(format!(
            "antes: puerta de enlace {}, dirección de la red {}",
            puerta.map_or_else(|| "ninguna".to_owned(), |p| p.to_string()),
            lan.map_or_else(|| "ninguna".to_owned(), |l| l.to_string())
        ));
        let puerta_tcp = puerta.map(|p| tcp(SocketAddr::new(IpAddr::V4(p), 80)));
        if let (Some(p), Some(r)) = (puerta, puerta_tcp.as_ref()) {
            linea(format!("antes: TCP a {p}:80: {}", como_texto(r)));
        }
        let dns_fuera = match lan {
            Some(l) => RESOLUTORES
                .iter()
                .map(|r| {
                    let x = pregunta_dns(l, SocketAddr::new(IpAddr::V4(*r), 53), NOMBRE)
                        .map(|ips| lista(&ips));
                    linea(format!("antes: DNS a {r}:53 desde {l}: {}", como_texto(&x)));
                    (*r, x)
                })
                .collect(),
            None => Vec::new(),
        };
        Antes {
            publica,
            puerta,
            lan,
            puerta_tcp,
            dns_fuera,
        }
    }

    /// The checks with the tunnel up, each bounded.
    fn con_el_tunel(informe: &mut Informe, abierta: &Abierta, esperada: Ipv4Addr, antes: &Antes) {
        let (corte, desvio, extremo, direccion) = {
            let g = abierta.lock().unwrap_or_else(PoisonError::into_inner);
            match g.as_ref() {
                Some(c) => (
                    c.tunel.con_corte(),
                    c.tunel.con_dns_del_tunel(),
                    Some(c.extremo),
                    Some(c.direccion),
                ),
                None => (false, false, None, None),
            }
        };
        informe.pon(
            corte && desvio,
            format!(
                "el corte puesto ({corte}) y los nombres solo al resolutor del túnel ({desvio}); dirección en el túnel {}, servidor en {}",
                direccion.map_or_else(|| "?".to_owned(), |d| d.to_string()),
                extremo.map_or_else(|| "?".to_owned(), |e| e.to_string())
            ),
        );
        if let Ok(rutas) = powershell(
            "Get-NetRoute -AddressFamily IPv4 -DestinationPrefix '0.0.0.0/0' | ForEach-Object { \"$($_.InterfaceAlias) $($_.NextHop) $($_.RouteMetric)\" }",
        ) {
            linea(format!("     rutas por defecto: {}", recorta(&rutas)));
        }
        // the tunnel's resolver answers
        let resolutor = guardiana_core::tunel_dns::actual();
        match resolutor {
            Some(r) => match pregunta_dns(Ipv4Addr::UNSPECIFIED, r, NOMBRE) {
                Ok(ips) if !ips.is_empty() => informe.pon(
                    true,
                    format!(
                        "el resolutor del túnel ({r}) contesta: {NOMBRE} = {}",
                        lista(&ips)
                    ),
                ),
                Ok(_) => informe.pon(
                    false,
                    format!("el resolutor del túnel ({r}) contesta sin direcciones"),
                ),
                Err(e) => informe.pon(false, format!("el resolutor del túnel ({r}): {e}")),
            },
            None => informe.pon(false, "no hay resolutor del túnel en vigor"),
        }
        // the public address outside is the server's
        match ip_publica(resolutor) {
            Ok(ip) => informe.pon(
                ip == IpAddr::V4(esperada),
                format!(
                    "la dirección pública que ve un eco por HTTPS: {ip} (la del servidor: {esperada}; la del equipo antes: {})",
                    antes.publica.as_ref().map_or_else(Clone::clone, ToString::to_string)
                ),
            ),
            Err(e) => informe.pon(false, format!("el eco por HTTPS, por el túnel: {e}")),
        }
        // the gateway, on the network card: cut before it leaves
        match antes.puerta {
            Some(p) => {
                let a = SocketAddr::new(IpAddr::V4(p), 80);
                let r = tcp(a);
                let bien = r == Err(Fallo::Filtro);
                // Windows' own record of it: the check that the record works, for the UDP ones below
                if let Some(l) = antes.lan {
                    match bloqueo_anotado(l, a) {
                        Ok(Some(anotado)) => linea(format!("     Windows lo anotó: {anotado}")),
                        Ok(None) => linea(
                            "     Windows no anotó ningún bloqueo (¿está el registro puesto?)",
                        ),
                        Err(e) => linea(format!("     el registro de Windows: {e}")),
                    }
                }
                informe.pon(
                    bien,
                    format!(
                        "TCP a la puerta de enlace {a}: {} (antes: {})",
                        como_texto(&r),
                        antes
                            .puerta_tcp
                            .as_ref()
                            .map_or_else(|| "sin medir".to_owned(), como_texto)
                    ),
                );
            }
            None => informe.pon(
                false,
                "no se supo la puerta de enlace: no se puede comprobar que se corta",
            ),
        }
        // a plain DNS query to a public resolver from the network card, outside the tunnel: cut before it leaves
        match antes.lan {
            Some(l) => {
                for (r, antes_r) in &antes.dns_fuera {
                    let destino = SocketAddr::new(IpAddr::V4(*r), 53);
                    let x = pregunta_dns(l, destino, NOMBRE).map(|ips| lista(&ips));
                    // UDP: Windows drops the datagram without telling the program, so «cut by the filter» is read
                    // in its own record; no answer alone proves nothing (the query could have left and its answer
                    // been lost)
                    let (bien, como) = match &x {
                        Err(Fallo::Filtro) => (true, como_texto(&x)),
                        Err(Fallo::Plazo) => match bloqueo_anotado(l, destino) {
                            Ok(Some(anotado)) => (
                                true,
                                format!(
                                    "{}; Windows anotó que lo cortó el filtro: {anotado}",
                                    como_texto(&x)
                                ),
                            ),
                            Ok(None) => (
                                false,
                                format!("{}, y Windows no anotó ningún bloqueo", como_texto(&x)),
                            ),
                            Err(e) => (
                                false,
                                format!("{}; el registro de Windows: {e}", como_texto(&x)),
                            ),
                        },
                        _ => (false, como_texto(&x)),
                    };
                    informe.pon(
                        bien,
                        format!(
                            "DNS a {r}:53 desde la tarjeta ({l}), fuera del túnel: {como} (antes: {})",
                            como_texto(antes_r)
                        ),
                    );
                }
            }
            None => informe.pon(
                false,
                "no se supo la dirección de la red: no se puede preguntar desde la tarjeta",
            ),
        }
        // the same query through the tunnel, and another program's HTTPS with the system's resolver: what a person
        // gets with the VPN on (said, not counted: it depends on the server's way out, not on the switch)
        if let Some(r) = RESOLUTORES.first() {
            let x = pregunta_dns(
                Ipv4Addr::UNSPECIFIED,
                SocketAddr::new(IpAddr::V4(*r), 53),
                NOMBRE,
            )
            .map(|ips| lista(&ips));
            linea(format!(
                "     DNS a {r}:53 por el túnel: {}",
                como_texto(&x)
            ));
        }
        match ejecuta(
            &system32().join("curl.exe"),
            &[
                "-sS".to_owned(),
                "--ssl-no-revoke".to_owned(),
                "--max-time".to_owned(),
                "15".to_owned(),
                "-o".to_owned(),
                "NUL".to_owned(),
                "-w".to_owned(),
                "%{http_code} %{remote_ip}".to_owned(),
                "https://api.github.com/zen".to_owned(),
            ],
        ) {
            Ok(s) => linea(format!(
                "     HTTPS a api.github.com, con el DNS del sistema: {}",
                recorta(&s)
            )),
            Err(e) => linea(format!(
                "     HTTPS a api.github.com, con el DNS del sistema: {e}"
            )),
        }
        let estado = abierta
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .and_then(|c| c.tunel.estado());
        match estado {
            Some(e) => linea(format!(
                "     el servidor: {} bytes enviados, {} recibidos, último saludo {}",
                e.enviados,
                e.recibidos,
                e.ultimo_saludo_ms
                    .map_or_else(|| "ninguno".to_owned(), |s| format!("{s} ms"))
            )),
            None => linea("     el controlador no dijo nada del servidor"),
        }
    }

    /// After disconnecting: the network is back.
    fn despues(informe: &mut Informe, esperada: Ipv4Addr, antes: &Antes) {
        informe.pon(
            guardiana_core::tunel_dns::actual().is_none(),
            "los nombres vuelven a los resolutores de la red (sin resolutor del túnel en vigor)",
        );
        match powershell(
            "@(Get-NetAdapter -IncludeHidden -ErrorAction SilentlyContinue | Where-Object { $_.Name -eq 'GUARDIANA' }).Count",
        ) {
            Ok(n) => informe.pon(n.trim() == "0", format!("adaptadores «GUARDIANA» que quedan: {}", n.trim())),
            Err(e) => informe.pon(false, format!("no se pudo mirar el adaptador: {e}")),
        }
        // the public address is this computer's again; a few tries, while Windows settles its routes
        let mut publica = ip_publica(None);
        for _ in 0..5 {
            if publica.is_ok() {
                break;
            }
            std::thread::sleep(Duration::from_secs(3));
            publica = ip_publica(None);
        }
        match (&publica, &antes.publica) {
            (Ok(ahora), Ok(antes_ip)) => informe.pon(
                ahora == antes_ip && *ahora != IpAddr::V4(esperada),
                format!(
                    "la dirección pública vuelve a ser la del equipo: {ahora} (antes {antes_ip})"
                ),
            ),
            (Ok(ahora), Err(_)) => informe.pon(
                *ahora != IpAddr::V4(esperada),
                format!("la dirección pública, ya sin túnel: {ahora}"),
            ),
            (Err(e), _) => informe.pon(
                false,
                format!("sin túnel, la dirección pública no se pudo ver: {e}"),
            ),
        }
        if let Some(p) = antes.puerta {
            let r = tcp(SocketAddr::new(IpAddr::V4(p), 80));
            let bien = r != Err(Fallo::Filtro);
            informe.pon(
                bien,
                format!(
                    "TCP a la puerta de enlace {p}:80, ya sin corte: {} (antes: {})",
                    como_texto(&r),
                    antes
                        .puerta_tcp
                        .as_ref()
                        .map_or_else(|| "sin medir".to_owned(), como_texto)
                ),
            );
        }
        if let Some(l) = antes.lan {
            for (r, antes_r) in &antes.dns_fuera {
                let x = pregunta_dns(l, SocketAddr::new(IpAddr::V4(*r), 53), NOMBRE)
                    .map(|ips| lista(&ips));
                let bien = match (&x, antes_r) {
                    (Ok(_), _) => true,
                    // it did not answer before either: only that the filter no longer cuts it
                    (Err(e), Err(_)) => *e != Fallo::Filtro,
                    (Err(_), Ok(_)) => false,
                };
                informe.pon(
                    bien,
                    format!(
                        "DNS a {r}:53 desde la tarjeta, ya sin corte: {} (antes: {})",
                        como_texto(&x),
                        como_texto(antes_r)
                    ),
                );
            }
        }
    }

    /// The whole run; the exit code says whether every check held.
    pub(crate) fn principal() -> ExitCode {
        let _ = LazyLock::force(&ARRANQUE);
        let servidor = std::env::var("GUARDIANA_VPN_SERVIDOR")
            .unwrap_or_else(|_| "vpn.guardianagroup.com".to_owned());
        let Some(dll) = std::env::var_os("GUARDIANA_WG_DLL").map(PathBuf::from) else {
            linea("falta GUARDIANA_WG_DLL, la ruta de wireguard.dll");
            return ExitCode::from(2);
        };
        let exe = match std::env::current_exe() {
            Ok(e) => e,
            Err(e) => {
                linea(format!("no se sabe dónde está este programa: {e}"));
                return ExitCode::from(2);
            }
        };
        let mut informe = Informe::default();
        linea(format!("la primera conexión de verdad, con {servidor}"));
        let esperada = match resuelve_v4(&servidor) {
            Ok(ip) => {
                linea(format!(
                    "antes: {servidor} = {ip}: la dirección que se debe ver fuera"
                ));
                ip
            }
            Err(e) => {
                informe.pon(false, format!("el nombre {servidor} no resuelve: {e}"));
                return ExitCode::FAILURE;
            }
        };
        let antes = antes();

        let inicio = Instant::now();
        linea(
            "conectando: alta, túnel con todo dentro, corte puesto y el primer saludo del servidor",
        );
        let conexion = tunel_win::conecta(&servidor, &dll, &exe);
        let segundos = inicio.elapsed().as_secs_f32();
        match conexion {
            Ok(c) => {
                informe.pon(
                    true,
                    format!("conectada en {segundos:.1} s, el servidor contestó"),
                );
                let abierta: Abierta = Arc::new(Mutex::new(Some(c)));
                vigilante(Arc::clone(&abierta));
                {
                    // disconnected when this block ends, however it ends
                    let _al_salir = AlSalir(Arc::clone(&abierta));
                    con_el_tunel(&mut informe, &abierta, esperada, &antes);
                }
            }
            // `conecta` leaves nothing open on any error: the tunnel and its switch are already gone
            Err(e) => informe.pon(false, format!("la conexión, tras {segundos:.1} s: {e}")),
        }
        despues(&mut informe, esperada, &antes);

        let mal: Vec<&String> = informe
            .hechas
            .iter()
            .filter(|(bien, _)| !*bien)
            .map(|(_, que)| que)
            .collect();
        if informe.todo_bien() {
            linea(format!(
                "todo bien: {} comprobaciones",
                informe.hechas.len()
            ));
            ExitCode::SUCCESS
        } else {
            linea(format!(
                "{} de {} comprobaciones fallaron:",
                mal.len(),
                informe.hechas.len()
            ));
            for que in mal {
                linea(format!("  - {que}"));
            }
            ExitCode::FAILURE
        }
    }
}
