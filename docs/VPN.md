# La VPN de GUARDIANA (en preparación)

> «Otra sección aparte, que sea la VPN… del máximo nivel… tecnología nunca antes vista.» «Y siempre código abierto.»
> (el dueño, 1 nov 2026, 16:21 y 16:36)

Investigación de la agencia: ronda 87 (Gemini y la revisión de Claude), en el repositorio de la web,
`herramientas/agencia-rondas/ronda87/`.

## Lo que la hace distinta

Una VPN que es a la vez el DNS de la casa, el antivirus y el cortafuegos del mismo programa:

- **El túnel y el escudo hablan entre sí.** Un programa que el antivirus marca pierde el túnel y la red al instante.
- **Aviso de lo que sale fuera del túnel**, con el nombre del programa, su firma y el destino; y una prueba de fugas que
  la persona lanza cuando quiere (DNS, IPv6, WebRTC, la ruta de DHCP que usa TunnelVision, CVE-2024-3661).
- **La ruta, en el mapa de Órbita**: por qué países y cables pasa, con las fuentes de Órbita.
- **Postcuántica**: intercambio híbrido (clásico + ML-KEM) que alimenta la clave precompartida de WireGuard, al estilo
  de Rosenpass y de Mullvad.
- **Dos operadores** en el doble salto: entrada y salida en proveedores y países distintos.

## Fases

1. **Túnel y escudo en Windows.** WireGuard en Rust (GotaTun, de Mullvad, o boringtun) con el adaptador Wintun;
   el cortafuegos de Windows (WFP, `crates/apps/src/cortafuegos.rs`) como interruptor de corte: nada sale si no es por el
   túnel, salvo hacia el propio servidor; el DNS de la casa dentro del túnel; IPv6 y WebRTC cerrados fuera del túnel.
   Servidores: los actuales (Núremberg, Helsinki y Miami) con WireGuard, y un alta anónima de la llave pública del equipo
   (sin cuenta y sin guardar a quién pertenece).
2. **Postcuántico y doble salto**; el aviso en vivo de lo que sale fuera del túnel.
3. **Anticensura** (AmneziaWG; el túnel dentro de los relés de llamadas que ya existen), defensa contra el análisis de
   tráfico (relleno y paquetes señuelo, bajo demanda) y servidores sin disco.
4. **Compilación reproducible, auditoría externa** y, si algún día se decide, móviles (hoy GUARDIANA es solo para PC).

## Lo que ya está (rama `antivirus-vpn`, 1 nov 2026)

`crates/tunel`: las llaves de WireGuard del equipo, la llave precompartida postcuántica (ML-KEM-768 + HKDF-SHA256, nueva
en cada alta) y el alta anónima con la comprobación de todo lo que dice el servidor (la dirección y el DNS del túnel
tienen que ser privados y el servidor, un nombre de guardianagroup.com). Probado en Linux, Windows y macOS.

## El adaptador de Windows: WireGuardNT (licencias comprobadas el 1 nov 2026)

- El código de WireGuardNT (el controlador y su biblioteca) es GPLv2
  (<https://git.zx2c4.com/wireguard-nt/about/>).
- Los binarios firmados que publica WireGuard LLC (`wireguard.dll`, en el .zip de
  <https://download.wireguard.com/wireguard-nt/>) tienen su propia licencia
  (<https://git.zx2c4.com/wireguard-nt/tree/prebuilt-binaries-license.txt>): se pueden distribuir **junto a un programa
  que los use solo a través de su interfaz pública** (`wireguard.h`), sin modificarlos, sin quitar sus avisos de
  propiedad y sin usar el nombre de WireGuard para respaldar el producto sin su permiso por escrito.
- Así lo hace la propia aplicación de WireGuard para Windows. Para GUARDIANA: el `wireguard.dll` firmado, tal cual se
  descarga, al lado del programa, llamado solo por su interfaz pública; el código de GUARDIANA sigue abierto. En la
  web, «usa WireGuard» como dato, nunca como respaldo.

## El interruptor de corte (WFP)

`crates/tunel/src/corte.rs` es el plan, como datos y con pruebas: las mismas reglas que aplica la aplicación de WireGuard
para Windows (`tunnel/firewall/rules.go`, MIT, leído en su fuente el 1 nov 2026), y lo que pidió la ronda 87 (cortar con
el Filtering Platform, no con rutas). Permitido: la interfaz del túnel, el propio servicio de GUARDIANA (de él salen los
paquetes encriptados hacia el servidor), el propio equipo (bucle local, donde vive el DNS de la casa), DHCP y el
descubrimiento de vecinos de IPv6, y las máquinas virtuales de Hyper-V entre sí. Bloqueado: el DNS a cualquiera que no
sea el resolutor del túnel, y todo lo demás. En una sesión dinámica: si el servicio se para, Windows quita los filtros
(y el adaptador del túnel desaparece con él).

Hecho y probado en un Windows real (la integración continua de GitHub, como administrador; `.github/workflows/vpn.yml`):

- `crates/apps/src/wireguard_nt.rs` carga `wireguard.dll` (WireGuardNT 1.1, firma de WireGuard LLC válida, SHA-256
  `B1B85E072C45D81358BE29D94C599DC76652F912BE8C0F0A41E2D5D89A6461D3`, comprobados en cada prueba) y crea un adaptador,
  le da la configuración de `crates/tunel/src/nt.rs`, lo sube, lo baja y lo cierra (1 nov 2026: correcto).
- `crates/apps/src/corte_vpn.rs` convierte el plan en filtros de una sesión dinámica; la prueba comprueba que Windows
  acepta cada filtro que permite (los que bloquean se dejan fuera para no cortar la red del equipo de pruebas).

- `crates/apps/src/red_vpn.rs`: la dirección del túnel, sus rutas (métrica 0), los ajustes del adaptador (sin
  descubrimiento de routers, sin detección de duplicados, sin DHCPv6, MTU 1420) y su resolutor (con `netsh`), como la
  aplicación de WireGuard para Windows (`tunnel/addressconfig.go`).
- `crates/apps/src/tunel_win.rs`: todo junto, en orden y deshaciendo si algo falla: adaptador → configuración → red →
  interruptor de corte → arriba. La prueba real lo hace entero sin la ruta por defecto ni el corte (para no tocar la red
  del equipo de pruebas).

- `Adaptador::lee` y `nt::estado_del_par`: lo que el controlador dice del servidor (bytes y último saludo). La prueba
  real comprueba que devuelve la llave dada: la disposición de las estructuras es la buena.

- `crates/tunel/src/conexion.rs` (todos los sistemas, con pruebas): una conexión entera. Llaves nuevas en cada
  conexión (nada une dos sesiones), el alta por HTTPS a un nombre de guardianagroup.com (sin seguir redirecciones), la
  respuesta comprobada, el nombre del servidor resuelto antes de abrir el túnel (con el corte puesto ya no se resuelve
  nada fuera), el túnel abierto por la mitad del sistema y el primer saludo del servidor esperado 8 segundos: si no
  llega, el túnel se cierra (y el corte con él) y se dice «el servidor no contesta», para no dejar el equipo sin
  internet. Las pruebas hacen el alta por la red de verdad contra un servidor local y comprueban que los dos lados
  llegan a la misma llave postcuántica.
- `crates/apps/src/tunel_win.rs::conecta`: lo mismo con el túnel de Windows (todo dentro y el corte puesto).
- `crates/panel/src/vpn.rs`: el estado y los botones en el panel del servicio. `GET /api/vpn` (en qué está, a qué
  servidor, desde cuándo, la dirección del túnel, los bytes y el último saludo, o por qué no está disponible: fuera de
  Windows, o sin `wireguard.dll` junto al programa), `POST /api/vpn/conecta` con `{"servidor": "…guardianagroup.com"}`
  (empieza; la página pregunta cómo va) y `POST /api/vpn/desconecta`. Nada se guarda: al parar el servicio, el túnel
  se cierra con él.
- La prueba de extremo a extremo en Linux (`.github/workflows/vpn.yml`, «extremo»): el servidor de GUARDIANA con el
  WireGuard del núcleo en un espacio de red, el lado del equipo (`crates/tunel/examples/alta_prueba.rs`) en otro, el
  alta postcuántica por la red y tráfico de verdad por el túnel (solo pasa si los dos lados sacaron la misma llave).

Pendiente, en este orden:

1. La pantalla de la VPN en la web del programa (Órbita), con la lista de servidores cuando existan.
2. ~~El `wireguard.dll` firmado dentro del instalador~~ Hecho (1 nov 2026, noche): `candidato.yml` baja el .zip de
   WireGuard LLC, `build/msi.ps1` comprueba la firma Authenticode y el SHA-256 fijado en
   `crates/apps/src/wireguard_nt.rs` (`HUELLA_DLL`) y mete `wireguard.dll` y su licencia (`LICENCIA-WIREGUARD-NT.txt`)
   junto a `guardiana.exe`; el servicio vuelve a comprobar el SHA-256 antes de cargarla (otra compilación, o un archivo
   puesto en su lugar, no se carga y se dice). La prueba de cliente (`build/cliente/instalacion.py`) lo comprueba
   instalado.
3. Los servidores, dentro del sello (con la llave del dueño), y la primera conexión de verdad.

Decisiones tomadas en la ronda 88 (1 nov 2026): incluida en la licencia de GUARDIANA, sin cuenta aparte; servidores de la fase 1, los de Hetzner (Helsinki, Núremberg); contra el abuso sin guardar nada (puerto 25 cerrado, topes de altas y de conexiones nuevas, pares sin uso se quitan).

## Los servidores (propuesta para la fase 1)

- WireGuard del núcleo de Linux (`wg0`, UDP 51820) en los servidores actuales, cada uno con su red privada
  (`10.66.0.0/16`; el servidor y su resolutor en `10.66.0.1`).
- El alta: `POST https://<servidor>/vpn/alta` (Caddy delante) a un servicio pequeño, aparte del motor, que solo sabe
  añadir y quitar pares de WireGuard y hacer su mitad de ML-KEM (en Rust, con `crates/tunel`). Recibe la llave pública
  del equipo y su llave ML-KEM; devuelve la dirección del túnel, la llave pública del servidor, el cifrado ML-KEM y el
  resolutor.
- Sin registros: los pares viven solo en la memoria del núcleo y se quitan tras 24 horas sin tráfico. Nada en disco ni
  en el diario.
- Contra el abuso, sin guardar nada: puerto 25 cerrado; un tope de altas por minuto; un tope de conexiones nuevas por
  segundo por par.
- El resolutor del túnel (Unbound, con DNSSEC) en `10.66.0.1`; el DNS de la casa del programa le manda lo que no corta.
- El servicio y su configuración van dentro del sello: solo se instalan con la firma del dueño.

## Reglas

- **Sin registros**: los servidores no guardan qué llave se conecta ni a dónde va. Lo que sí se hace contra el abuso, sin
  guardar nada: cerrar el puerto 25 de salida y frenar los barridos de puertos.
- **Sin promesas absolutas**: lo que la VPN no protege se dice en su pantalla.

## Decisiones (ronda 88, 1 nov 2026, con el encargo del dueño)

> «Te dejo a ti y a Gemini que toméis la decisión más coherente y os encarguéis vosotros del proyecto.» «Una sola cuota
> mensual y el cliente tenga todos los servicios en 1 solo programa.» (el dueño, 1 nov 2026, 22:34 y 22:40)

- **Incluida en la licencia de GUARDIANA** (y en los 7 días de prueba), sin cuenta aparte. En la fase 2, el servidor
  comprobará la licencia sin saber de quién es (fichas con firma ciega).
- **Servidores de la fase 1**: Hetzner (Helsinki y Núremberg), que incluye 20 TB de tráfico al mes por servidor; Miami
  (Vultr) después. Se mide el uso real y con eso se recalcula la cuota.
- **Contra el abuso, sin registros**: puerto 25 cerrado, tope de altas por minuto, tope de conexiones nuevas por segundo
  por par y pares fuera tras 24 horas sin uso.
- Detalle y fuentes: `herramientas/agencia-rondas/ronda88/decisiones.md` en el repositorio de la web.
