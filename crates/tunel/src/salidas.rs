//! The VPN's exits (the owner, 3 Nov 2026, 09:25: «me ha conectado, pero no sé a dónde… Todas las VPN te dicen a
//! dónde… Haz una VPN de verdad y de calidad»; docs/VPN.md, «Las salidas»): the table of GUARDIANA's servers, each a
//! real exit in its own country (ronda 91: no virtual locations), what is measured of each from this computer, and the
//! address a website sees, asked of GUARDIANA's own server (never a third party).
//!
//! - [`SALIDAS`]: id, country, city, continent, provider, name and time zone of each.
//! - [`mide`]: whether an exit is there (its name resolves and its sign-up answers) and how far (the median of three TCP
//!   connections to its HTTPS, in milliseconds). [`mas_rapida`] picks the nearest of the available ones.
//! - [`ip_vista`]: `GET https://<exit>/vpn/alta`, which GUARDIANA's server answers with the address it sees from whoever
//!   asks: from inside the tunnel, its own public one (what websites see), from outside, the computer's own.
//! - [`desfase_min`]: the exit's offset from UTC now, with its summer time, to tell whether the computer's clock gives the
//!   person away.

use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

/// One exit of GUARDIANA's VPN.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Salida {
    /// Its short id (`hel`).
    pub id: &'static str,
    /// Its country, two letters (ISO 3166-1).
    pub pais: &'static str,
    /// Its city.
    pub ciudad: &'static str,
    /// Its continent (`europa`, `america`).
    pub continente: &'static str,
    /// Who rents the machine.
    pub proveedor: &'static str,
    /// Its name: the sign-up and the tunnel's endpoint.
    pub nombre: &'static str,
    /// Its time zone (IANA).
    pub zona: &'static str,
}

/// GUARDIANA's exits (ronda 91, phase 1). Helsinki is the first one up (docs/VPN.md); a name that does not resolve yet
/// is simply not available.
pub const SALIDAS: [Salida; 3] = [
    Salida {
        id: "hel",
        pais: "FI",
        ciudad: "Helsinki",
        continente: "europa",
        proveedor: "Hetzner",
        nombre: "vpn.guardianagroup.com",
        zona: "Europe/Helsinki",
    },
    Salida {
        id: "nbg",
        pais: "DE",
        ciudad: "Núremberg",
        continente: "europa",
        proveedor: "Hetzner",
        nombre: "vpn-nbg.guardianagroup.com",
        zona: "Europe/Berlin",
    },
    Salida {
        id: "mia",
        pais: "US",
        ciudad: "Miami",
        continente: "america",
        proveedor: "Vultr",
        nombre: "vpn-mia.guardianagroup.com",
        zona: "America/New_York",
    },
];

/// The «fastest» choice: the available exit with the lowest measured time.
pub const RAPIDA: &str = "rapida";

/// The exit with id `id`.
#[must_use]
pub fn por_id(id: &str) -> Option<&'static Salida> {
    SALIDAS.iter().find(|s| s.id == id)
}

/// The exit called `nombre` (its sign-up's or its endpoint's name).
#[must_use]
pub fn por_nombre(nombre: &str) -> Option<&'static Salida> {
    SALIDAS
        .iter()
        .find(|s| s.nombre.eq_ignore_ascii_case(nombre))
}

/// What was measured of one exit from this computer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Medida {
    /// Its name resolves and its sign-up answers.
    pub disponible: bool,
    /// The median of three TCP connections to its HTTPS, in milliseconds; `None` when none connected.
    pub ms: Option<u32>,
}

/// The median of `ms` (the lower middle one of an even count); `None` when empty.
#[must_use]
pub fn mediana(ms: &[u32]) -> Option<u32> {
    let mut v = ms.to_vec();
    v.sort_unstable();
    v.get(v.len().checked_sub(1)? / 2).copied()
}

/// The available exit with the lowest time, of `medidas` (by id); `None` when none is available.
#[must_use]
pub fn mas_rapida<'a>(medidas: &[(&'a str, Medida)]) -> Option<&'a str> {
    medidas
        .iter()
        .filter(|(_, m)| m.disponible)
        .min_by_key(|(_, m)| m.ms.unwrap_or(u32::MAX))
        .map(|(id, _)| *id)
}

/// Whether an HTTP answer to `GET /vpn/alta` is the sign-up's own: its echo (200), or what it answers to what it does
/// not take (400, 405 from a server of before the echo, 429 when busy). The web server in front says 502 to 504 when
/// the sign-up is down, and 404 where there is no VPN.
#[must_use]
pub fn es_del_alta(codigo: u16) -> bool {
    matches!(codigo, 200 | 400 | 405 | 429)
}

/// The address in the sign-up's echo (`{"v":1,"ip":"…"}`); `None` when it says none or is something else.
#[must_use]
pub fn ip_del_eco(texto: &str) -> Option<IpAddr> {
    let v: serde_json::Value = serde_json::from_str(texto).ok()?;
    v.get("ip")?.as_str()?.parse().ok()
}

/// How long one TCP connection to the exit's HTTPS may take.
const PLAZO_TCP: Duration = Duration::from_secs(2);
/// How long the sign-up's answer may take.
const PLAZO_HTTPS: Duration = Duration::from_secs(6);

/// `GET https://{nombre}/vpn/alta`: its code and its text (at most 4 KiB). No redirections, no proxy of the environment
/// (it would tell where from), the server's certificate checked against its name.
fn pregunta_al_alta(nombre: &str) -> Option<(u16, String)> {
    let config = ureq::config::Config::builder()
        .timeout_global(Some(PLAZO_HTTPS))
        .http_status_as_error(false)
        .max_redirects(0)
        .proxy(None)
        .build();
    let agente = ureq::Agent::new_with_config(config);
    let url = format!("https://{nombre}/vpn/alta");
    let r = agente
        .get(url.as_str())
        .header("Accept", "application/json")
        .call()
        .ok()?;
    let codigo = r.status().as_u16();
    let texto = r
        .into_body()
        .with_config()
        .limit(4096)
        .read_to_string()
        .unwrap_or_default();
    Some((codigo, texto))
}

/// Measures the exit `s` from here, now: its name resolved, three TCP connections to its HTTPS (the median, in
/// milliseconds) and its sign-up asked. With the VPN on, this would go through the tunnel: the caller measures only with
/// the VPN off.
#[must_use]
pub fn mide(s: &Salida) -> Medida {
    let Some(direccion) = (s.nombre, 443)
        .to_socket_addrs()
        .ok()
        .and_then(|mut v| v.find(SocketAddr::is_ipv4))
    else {
        return Medida::default();
    };
    let tiempos: Vec<u32> = (0..3)
        .filter_map(|_| {
            let antes = Instant::now();
            TcpStream::connect_timeout(&direccion, PLAZO_TCP)
                .ok()
                .map(|_| u32::try_from(antes.elapsed().as_millis()).unwrap_or(u32::MAX))
        })
        .collect();
    let disponible = pregunta_al_alta(s.nombre).is_some_and(|(codigo, _)| es_del_alta(codigo));
    Medida {
        disponible,
        ms: mediana(&tiempos),
    }
}

/// The address a website sees from this computer, asked of GUARDIANA's own server called `nombre` (its echo, `GET
/// /vpn/alta`): with the VPN connected the request goes through the tunnel and the server answers its own public
/// address; with it off, the computer's. `None` when the server does not say (a server of before the echo, or no
/// answer).
#[must_use]
pub fn ip_vista(nombre: &str) -> Option<IpAddr> {
    match pregunta_al_alta(nombre)? {
        (200, texto) => ip_del_eco(&texto),
        _ => None,
    }
}

/// Days since 1970-01-01 of the civil date `y-m-d` (proleptic Gregorian).
fn dias_desde_1970(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil year of the instant `ms` (milliseconds since 1970, UTC).
fn anio_de(ms: i64) -> i64 {
    let dias = ms.div_euclid(86_400_000);
    // the year whose 1 January is the last one not after `dias`
    let mut y = 1970 + dias.div_euclid(366);
    while dias_desde_1970(y + 1, 1, 1) <= dias {
        y += 1;
    }
    y
}

/// The day of the month of the `n`-th Sunday (1-based; 0 is the last) of month `m` of year `y`.
fn domingo(y: i64, m: i64, n: i64) -> i64 {
    // 1970-01-01 was a Thursday: day 0 is weekday 4 (Sunday 0)
    let dia_semana = |d: i64| (dias_desde_1970(y, m, d) + 4).rem_euclid(7);
    if n == 0 {
        let ultimo = if m == 12 {
            dias_desde_1970(y + 1, 1, 1)
        } else {
            dias_desde_1970(y, m + 1, 1)
        } - dias_desde_1970(y, m, 1);
        let mut d = ultimo;
        while dia_semana(d) != 0 {
            d -= 1;
        }
        d
    } else {
        let mut d = 1;
        while dia_semana(d) != 0 {
            d += 1;
        }
        d + 7 * (n - 1)
    }
}

/// Milliseconds since 1970 of `y-m-d` at `h` o'clock UTC.
fn ms_utc(y: i64, m: i64, d: i64, h: i64) -> i64 {
    (dias_desde_1970(y, m, d) * 24 + h) * 3_600_000
}

/// The offset from UTC of the time zone `zona` (one of [`SALIDAS`]' zones) at the instant `ahora_ms`, in minutes, with
/// its summer time: the European Union's (last Sunday of March to last Sunday of October, at 01:00 UTC) or the United
/// States' (second Sunday of March at 02:00 to first Sunday of November at 02:00, local time). `None` for another zone.
#[must_use]
pub fn desfase_min(zona: &str, ahora_ms: i64) -> Option<i32> {
    let y = anio_de(ahora_ms);
    match zona {
        "Europe/Helsinki" | "Europe/Berlin" => {
            let base = if zona == "Europe/Helsinki" { 120 } else { 60 };
            let empieza = ms_utc(y, 3, domingo(y, 3, 0), 1);
            let acaba = ms_utc(y, 10, domingo(y, 10, 0), 1);
            Some(if (empieza..acaba).contains(&ahora_ms) {
                base + 60
            } else {
                base
            })
        }
        "America/New_York" => {
            // 02:00 local: 07:00 UTC in winter time, 06:00 UTC in summer time
            let empieza = ms_utc(y, 3, domingo(y, 3, 2), 7);
            let acaba = ms_utc(y, 11, domingo(y, 11, 1), 6);
            Some(if (empieza..acaba).contains(&ahora_ms) {
                -240
            } else {
                -300
            })
        }
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn la_tabla_de_salidas() {
        assert_eq!(SALIDAS.len(), 3);
        for s in SALIDAS {
            assert!(crate::alta::es_servidor(s.nombre), "{}", s.nombre);
            assert_eq!(s.pais.len(), 2);
            assert!(matches!(s.continente, "europa" | "america"));
            assert!(desfase_min(s.zona, 0).is_some(), "{}", s.zona);
            assert_eq!(por_id(s.id), Some(&s));
            assert_eq!(por_nombre(&s.nombre.to_ascii_uppercase()), Some(&s));
        }
        // Helsinki: the first one, and the name the program connects to by default
        assert_eq!(SALIDAS[0].id, "hel");
        assert_eq!(SALIDAS[0].nombre, "vpn.guardianagroup.com");
        assert_eq!(por_id(RAPIDA), None);
        assert_eq!(por_id("xyz"), None);
    }

    #[test]
    fn la_mas_rapida_de_las_disponibles() {
        let m = |disponible, ms| Medida { disponible, ms };
        assert_eq!(
            mas_rapida(&[
                ("hel", m(true, Some(48))),
                ("nbg", m(true, Some(31))),
                ("mia", m(false, Some(5)))
            ]),
            Some("nbg")
        );
        // an available exit with no time still counts, last
        assert_eq!(
            mas_rapida(&[("hel", m(true, None)), ("mia", m(false, None))]),
            Some("hel")
        );
        assert_eq!(mas_rapida(&[("hel", m(false, Some(1)))]), None);
        assert_eq!(mas_rapida(&[]), None);
        assert_eq!(mediana(&[30, 10, 20]), Some(20));
        assert_eq!(mediana(&[40, 10]), Some(10));
        assert_eq!(mediana(&[7]), Some(7));
        assert_eq!(mediana(&[]), None);
    }

    #[test]
    fn lo_que_contesta_el_alta() {
        for c in [200, 400, 405, 429] {
            assert!(es_del_alta(c), "{c}");
        }
        for c in [404, 500, 502, 503, 504, 301] {
            assert!(!es_del_alta(c), "{c}");
        }
        assert_eq!(
            ip_del_eco("{\"v\":1,\"ip\":\"62.238.17.121\"}"),
            Some("62.238.17.121".parse().unwrap())
        );
        assert_eq!(
            ip_del_eco("{\"v\":1,\"ip\":\"2001:db8::7\"}"),
            Some("2001:db8::7".parse().unwrap())
        );
        assert_eq!(ip_del_eco("{\"v\":1,\"ip\":null}"), None);
        assert_eq!(ip_del_eco("Method Not Allowed"), None);
    }

    /// The exits' offsets now, with their summer times, against dates whose offset is known.
    #[test]
    fn la_hora_de_cada_salida() {
        let ms = |y, m, d, h| ms_utc(y, m, d, h);
        // 3 Nov 2026: winter time in Europe and, since 1 Nov, in the United States too
        let hoy = ms(2026, 11, 3, 12);
        assert_eq!(desfase_min("Europe/Helsinki", hoy), Some(120));
        assert_eq!(desfase_min("Europe/Berlin", hoy), Some(60));
        assert_eq!(desfase_min("America/New_York", hoy), Some(-300));
        // summer
        let julio = ms(2026, 7, 1, 12);
        assert_eq!(desfase_min("Europe/Helsinki", julio), Some(180));
        assert_eq!(desfase_min("Europe/Berlin", julio), Some(120));
        assert_eq!(desfase_min("America/New_York", julio), Some(-240));
        // the changes of 2026: Europe, 29 Mar and 25 Oct at 01:00 UTC; the United States, 8 Mar and 1 Nov
        assert_eq!(domingo(2026, 3, 0), 29);
        assert_eq!(domingo(2026, 10, 0), 25);
        assert_eq!(domingo(2026, 3, 2), 8);
        assert_eq!(domingo(2026, 11, 1), 1);
        assert_eq!(
            desfase_min("Europe/Berlin", ms(2026, 3, 29, 1) - 1),
            Some(60)
        );
        assert_eq!(desfase_min("Europe/Berlin", ms(2026, 3, 29, 1)), Some(120));
        assert_eq!(
            desfase_min("America/New_York", ms(2026, 11, 1, 6) - 1),
            Some(-240)
        );
        assert_eq!(
            desfase_min("America/New_York", ms(2026, 11, 1, 6)),
            Some(-300)
        );
        assert_eq!(desfase_min("America/Bogota", hoy), None);
        // the civil calendar underneath
        assert_eq!(dias_desde_1970(1970, 1, 1), 0);
        assert_eq!(dias_desde_1970(2000, 3, 1), 11_017);
        assert_eq!(anio_de(ms(2024, 12, 31, 23)), 2024);
        assert_eq!(anio_de(ms(2025, 1, 1, 0)), 2025);
    }
}
