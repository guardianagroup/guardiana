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
el Filtering Platform, no con rutas), con el permiso del servicio más estrecho que el de WireGuard (1.0.55). Permitido:
la interfaz del túnel; del propio servicio de GUARDIANA, solo los paquetes encriptados hacia el servidor (UDP, a su
dirección y a su puerto, en las capas de su familia: los conectores de WireGuardNT son del proceso que sube el
adaptador, que es el servicio, `PsGetCurrentProcess` en el `Up` de su controlador, leído en su fuente el 2 nov 2026); lo
que el Modo Hogar sirve a la casa (el DNS en el 53, UDP y TCP; el panel en el 7443 y la página de comprobación en el 80),
solo de entrada y solo al servicio; el propio equipo (bucle local, donde vive el DNS de la casa), DHCP y el
descubrimiento de vecinos de IPv6, y las máquinas virtuales de Hyper-V entre sí. Bloqueado: el DNS fuera del túnel, a
quien sea, y todo lo demás. En una sesión dinámica: si el servicio se para, Windows quita los filtros (y el adaptador del
túnel desaparece con él).

Lo que cambió en la 1.0.55 (tercera revisión, 2 nov 2026), y por qué:

- **El servicio, solo hacia el servidor.** Hasta la 1.0.55 el corte dejaba al servicio ir a cualquier sitio fuera del
  túnel, con el peso más alto: salía lo que las rutas mandaran fuera, las de la red de casa y también una que meta un
  DHCP ajeno (TunnelVision, CVE-2024-3661). Ahora solo el UDP a la dirección y al puerto del servidor. Es el único
  permiso que una subcapa de debajo no puede deshacer (`FWPM_FILTER_FLAG_CLEAR_ACTION_RIGHT`), para que otro cortafuegos
  no corte el túnel.
- **Sin excepción de DNS fuera del túnel.** «DNS del túnel» dejaba el puerto 53 hacia `10.66.0.1` sin mirar por qué
  interfaz: una ruta a `10.66.0.0/17` de un DHCP ajeno se habría llevado el DNS de todos los programas a quien la puso.
  El resolutor del túnel se alcanza por la interfaz del túnel, que ya está permitida con más peso; el DNS fuera del túnel
  se corta siempre («DNS fuera del túnel», que repite a propósito lo que ya hace «todo lo demás»: se ve por su nombre en
  la lista de filtros de Windows y sigue cortando si algún día se añade un permiso por debajo).
- **El Modo Hogar sigue sirviendo a la casa.** Con el corte puesto, la casa no llegaba al DNS, al panel ni a la página de
  comprobación del Modo Hogar. Ahora entran esas conexiones, solo esas y solo al servicio; la respuesta vuelve por la
  misma conexión y el servicio no abre nada hacia la casa. Son permisos que la regla del Modo Hogar en el cortafuegos de
  Windows (solo la red local, solo en una red privada) aún puede negar. Las pruebas del panel comprueban que la lista del
  corte (`corte::HOGAR`) es la de `crates/service/src/home.rs`.
- **El Modo Hogar nunca escucha en la dirección del túnel.** Con la ruta por defecto en el túnel, la dirección de la red
  que medía GUARDIANA (`guardiana_devices::local_lan_ipv4`, una conexión UDP hacia `192.0.2.1` que no envía nada) era la
  del túnel, y el Modo Hogar se mudaba allí: los teléfonos de casa se quedaban sin DNS y el servicio contestaba dentro
  del túnel. El túnel mide la dirección de la red antes de tomar la ruta y la guarda junto a su resolutor
  (`guardiana_core::tunel_dns::lan`); mientras vive el túnel, esa es la dirección de la red, si el equipo aún la tiene
  (con otra concesión del DHCP bajo el túnel no hay ninguna hasta que se cierre: el Modo Hogar espera, y el panel no deja
  de arrancar por escuchar en una dirección que ya no es suya). Por eso el motor ya no se salta la comparación de la
  dirección mientras dura el túnel (lo que añadió la 1.0.54): antes, durante y después del túnel es la misma, y un
  cambio de red de verdad se nota al cerrarse. Y el panel en la red de casa es opcional, como su DNS (segunda revisión
  de la 1.0.55): si no se puede abrir, el equipo sigue con su panel en 127.0.0.1 y la comprobación de cada 15 segundos lo
  vuelve a intentar. Hasta entonces era obligatorio, y cuando fallaba el panel de 127.0.0.1 ya estaba sirviendo, nadie lo
  paraba y las vueltas siguientes no podían abrir el panel; ahora `guardiana_panel::start` abre todas las direcciones
  antes de servir ninguna, y si una obligatoria falla no deja nada escuchando.
- **El resolutor del túnel nunca se guarda como DNS de la red.** Windows lo lista como DNS del adaptador del túnel, y una
  vuelta del motor hecha con la VPN puesta podía tomarlo y guardarlo como bueno: al cerrarse la VPN, los nombres irían a
  él fuera del túnel. Ahora se quita de todas las listas mientras la VPN está puesta (solo esa dirección: una red de casa
  puede usar `10.66.0.0/16`). Si no queda ningún otro, esa vuelta le pregunta a él, no lo guarda y se rehace cuando la VPN
  se cierra: fallar ahí habría parado el servicio, y la VPN con él.
- **Una conexión que termina después de un cierre se cierra sola.** Conectar tarda unos segundos sin el candado puesto, y
  una conexión que acababa después de cerrar la VPN (al parar el servicio) se guardaba y seguía abierta, con su corte. Ahora
  se guarda solo si su generación sigue siendo la vigente; si no, se cierra en el acto (`crates/panel/src/vpn.rs`,
  `guarda_si_sigue`).
- **IPv6.** El túnel no tiene dirección IPv6, y la ruta por defecto de IPv6 en su adaptador puede no entrar (no se mira si
  entra). Ninguna de las dos cosas deja salir IPv6: fuera del túnel y del bucle local, el plan solo permite de IPv6 el
  DHCPv6 entre direcciones del enlace y el descubrimiento de vecinos (ICMPv6 133 a 137), y nada del servicio salvo que el
  servidor sea de IPv6 (y entonces solo el UDP a él). Todo lo demás de IPv6 cae en «todo lo demás».

Las pruebas del plan, en todos los sistemas, le preguntan qué decide para cada conexión como lo decide Windows dentro de
una subcapa (el peso más alto entre los filtros que casan; dos condiciones del mismo campo valen cualquiera de las dos):
los paquetes al servidor pasan, de ida y de vuelta; nada más del servicio fuera del túnel, ni hacia la red de casa ni el
DNS por una ruta ajena; la casa llega a lo del Modo Hogar y a nada más; nada de IPv6 fuera del túnel salvo lo del enlace.

Hecho y probado en un Windows real (la integración continua de GitHub, como administrador; `.github/workflows/vpn.yml`):

- `crates/apps/src/wireguard_nt.rs` carga `wireguard.dll` (WireGuardNT 1.1, firma de WireGuard LLC válida, SHA-256
  `B1B85E072C45D81358BE29D94C599DC76652F912BE8C0F0A41E2D5D89A6461D3`, comprobados en cada prueba) y crea un adaptador,
  le da la configuración de `crates/tunel/src/nt.rs`, lo sube, lo baja y lo cierra (1 nov 2026: correcto).
- `crates/apps/src/corte_vpn.rs` convierte el plan en filtros de una sesión dinámica; la prueba comprueba que Windows
  acepta cada filtro que permite (los que bloquean se dejan fuera para no cortar la red del equipo de pruebas), desde la
  1.0.55 para un servidor de IPv4 y para uno de IPv6.

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
  se cierra con él. La VPN es la del proceso, no la de cada vuelta del motor (1.0.54): el motor rehace el panel
  cuando cambia la red, y hasta la 1.0.54 eso dejaba el túnel huérfano y la página en «apagada».
- El DNS con la VPN puesta (1.0.54, segunda revisión del 2 nov 2026): el servicio era el único programa que el corte
  dejaba salir (hasta la 1.0.55, a cualquier sitio), y el DNS de GUARDIANA (127.0.0.1) reenviaba al router, fuera del
  túnel. Ahora, mientras el túnel lleva todo, `guardiana_core::tunel_dns` manda cada nombre solo al resolutor del túnel
  (`10.66.0.1`), con su propia caché; si no contesta, el nombre falla y nunca sale fuera. Las sondas del motor preguntan
  también solo a ese resolutor.
- La primera conexión de verdad (`.github/workflows/vpn.yml`, «de verdad», solo cuando se lanza a mano;
  `crates/apps/examples/vpn_de_verdad.rs`): un Windows de la integración continua, como administrador, se conecta a
  `vpn.guardianagroup.com` con `tunel_win::conecta` (todo dentro y el corte puesto) y comprueba, con un plazo en cada
  paso: el saludo del servidor; que el resolutor del túnel contesta; que la dirección pública que ve un servicio de eco
  por HTTPS es la del servidor (la que da su nombre antes de conectar); que la puerta de enlace y un DNS público
  preguntado desde la tarjeta, fuera del túnel, se cortan antes de salir (Windows lo dice: WSAEACCES); y, ya
  desconectado, que la red vuelve. Desconecta siempre: en cada camino, y a los 150 s un vigilante cierra el túnel y
  sale; si el proceso muere, Windows quita los filtros y el adaptador con él.
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
4. La reconexión automática (quitada en la 1.0.54): el permiso del servicio ya es solo hacia el servidor (1.0.55); falta
   que el alta nueva salga por una ruta /32 al servidor (o un conector atado a la tarjeta), y entonces el corte tendrá
   que dejar al servicio también el TCP 443 a esa dirección, que hoy no deja (solo el UDP del túnel).
   `Tunel::renueva` rechaza otra dirección o puerto del servidor mientras el corte está puesto.

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
- Si el servicio del alta vuelve a arrancar (una actualización, un fallo), primero toma los pares que WireGuard ya
  tiene (`Estado::adopta`): los equipos conectados siguen con su túnel y su dirección no se da a otro. Lo que no tenga
  exactamente una dirección del túnel no lo puso el alta, y se quita.
- Contra el abuso, sin guardar nada: puerto 25 cerrado; un tope de altas por minuto para todo el servidor (600) y otro
  por origen (10: una dirección IPv4, o un /64 de IPv6). El origen es la dirección que vio Caddy, guardada solo como un
  resumen con una llave que se sortea en cada arranque, en la memoria del servicio y durante ese minuto. Y un tope de
  conexiones nuevas por segundo por par.
- Dónde está (2 nov 2026; dueño, 10:40: «Utilizamos el de Helsinki. Conéctalo ya»): en el servidor de Helsinki, con
  `zero-motor/vpn.sh` del repositorio de la web, que compila este servicio desde un commit fijado y monta WireGuard,
  sus reglas de red y Unbound. Entra por la misma actualización que el resto del servidor; pasará al sello cuando el
  sello esté puesto.
- El resolutor del túnel (Unbound, con DNSSEC) en `10.66.0.1`; el DNS de la casa del programa le manda lo que no corta.
- El servicio y su configuración van dentro del sello: solo se instalan con la firma del dueño.

## Reglas

- **Sin registros**: los servidores no guardan qué llave se conecta ni a dónde va. Lo que sí se hace contra el abuso, sin
  guardar nada: cerrar el puerto 25 de salida y frenar los barridos de puertos.
- **Sin promesas absolutas**: lo que la VPN no protege se dice en su pantalla.
- **Del servicio, fuera del túnel, solo lo del servidor** (1.0.55): el corte le deja el UDP a la dirección y al puerto del
  servidor, y nada más; ni hacia la red de casa ni por una ruta que meta un DHCP ajeno. Lo que entra para el Modo Hogar
  se contesta por la misma conexión.
- **El DNS, solo por el túnel o al propio equipo** (1.0.55): el corte no tiene excepción de DNS fuera del túnel, ni para la
  dirección del resolutor del túnel; y ese resolutor no se guarda nunca como DNS de la red.
- **De IPv6, fuera del túnel, solo lo del enlace** (1.0.55): el corte deja el DHCPv6 y el descubrimiento de vecinos, y nada
  más (el túnel no tiene dirección IPv6).
- **La VPN no mueve el Modo Hogar** (1.0.55): sigue en la dirección de la red de casa, la de antes del túnel, nunca en la
  del túnel.
- **Una conexión no sobrevive a un cierre** (1.0.55): la que termina de abrirse después se cierra en el acto.

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
