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
   Servidores: los actuales (Núremberg, Helsinki y Miami) con WireGuard, y un alta de la llave pública del equipo sin
   cuenta y sin guardar a quién pertenece.
2. **Postcuántico y doble salto**; el aviso en vivo de lo que sale fuera del túnel.
3. **Anticensura** (AmneziaWG; el túnel dentro de los relés de llamadas que ya existen), defensa contra el análisis de
   tráfico (relleno y paquetes señuelo, bajo demanda) y servidores sin disco.
4. **Compilación reproducible, auditoría externa** y, si algún día se decide, móviles (hoy GUARDIANA es solo para PC).

## Lo que ya está (rama `antivirus-vpn`, 1 nov 2026)

`crates/tunel`: las llaves de WireGuard del equipo, la llave precompartida postcuántica (ML-KEM-768 + HKDF-SHA256, nueva
en cada alta) y el alta, sin cuenta, con la comprobación de todo lo que dice el servidor (la dirección y el DNS del túnel
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
quien sea, y todo lo demás. Sin el modo estricto, en una sesión dinámica: si el servicio se para, Windows quita los
filtros (y el adaptador del túnel desaparece con él). Con el modo estricto (más abajo, encendido salvo que la persona lo
apague), el mismo plan vive en Windows aparte del servicio y sigue cuando el servicio no está.

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

**Lo que Windows reenvía** (1.0.56, revisión del 5 nov 2026). Lo que la pila IP de Windows pasa de una de sus interfaces
a otra —la conexión compartida (ICS), la zona con cobertura móvil y el NAT del propio Windows (WinNAT)— no pasa por las
capas de autorización de conexiones, donde estaba todo el plan, sino por las de reenvío (`FWPM_LAYER_IPFORWARD_V4` y
`_V6`; Microsoft, «WFP layer requirements and restrictions»). Hasta la 1.0.56 lo reenviado salía por la tarjeta con la dirección real siempre que una ruta lo mandara allí: con el túnel caído (el
modo estricto reconectando, el equipo arrancando, el servicio muerto o parado) y, con él arriba, hacia la red de casa,
por una ruta de un DHCP ajeno (TunnelVision) o en IPv6. Ahora el plan bloquea todo lo reenviado con el peso más bajo
(«lo reenviado fuera del túnel»), en el corte de siempre y en el modo estricto, persistente y de arranque, con las mismas
claves que lo demás (así que se quita igual, lo pusiera la versión que lo pusiera); y el permiso del túnel deja pasar lo
que se reenvía hacia la interfaz del túnel y lo que sale de ella hacia las máquinas, así que con el túnel arriba siguen
saliendo por él. La condición de la interfaz de salida es `FWPM_CONDITION_IP_FORWARD_INTERFACE`, el LUID de «la interfaz
por la que se va a enviar el paquete reenviado» (Microsoft, «Filtering conditions available at each filtering layer»),
y no `FWPM_CONDITION_DESTINATION_INTERFACE_INDEX`, la misma interfaz por su índice: el LUID es lo que nombra el túnel en
las demás capas y lo que da el adaptador, sin nada que traducir. `FWPM_CONDITION_IP_LOCAL_INTERFACE`, la condición del
túnel en las capas de conexiones, en las de reenvío es la interfaz por la que llegó el paquete: se usa ahí para las
respuestas que salen del túnel. Lo mismo hace el interruptor de corte de VpnHood para Windows; las reglas de WireGuard
no filtran el reenvío; y a los usuarios de simplewall un bloqueo ahí les cortaba la zona con cobertura, ICS y los
conmutadores virtuales con NAT, así que ese tráfico pasa por ahí. **Lo que no cubre, y no se ha probado en un equipo
real:** WSL2 (con NAT o en modo «mirrored»), ni Hyper-V, Windows Sandbox o Docker Desktop cuando el NAT lo hace el propio
conmutador virtual de Hyper-V sin que la pila IP de Windows lo reenvíe (Mullvad vio así el tráfico de WSL, solo como
tramas Ethernet); ni las máquinas en un conmutador externo, que son de la red y no de este equipo. Su tráfico puede salir
fuera del túnel, también con el modo estricto, y la pantalla de la VPN lo dice. Siguiente paso: el cortafuegos de Hyper-V
(Windows 11 22H2 y posteriores), como hace Mullvad. Lo que la integración continua comprueba es que Windows acepta estos
filtros y los quita; no tiene WSL, ni Hyper-V, ni ICS para probar tráfico reenviado de verdad.

Las pruebas del plan, en todos los sistemas, le preguntan qué decide para cada conexión como lo decide Windows dentro de
una subcapa (el peso más alto entre los filtros que casan; dos condiciones del mismo campo valen cualquiera de las dos):
los paquetes al servidor pasan, de ida y de vuelta; nada más del servicio fuera del túnel, ni hacia la red de casa ni el
DNS por una ruta ajena; la casa llega a lo del Modo Hogar y a nada más; nada de IPv6 fuera del túnel salvo lo del enlace;
de lo reenviado, solo lo que va hacia el túnel o sale de él, y sin túnel nada; y cada condición está en una capa que la
tiene, según la tabla de Microsoft.

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
  llega, el túnel se cierra y se dice «el servidor no contesta». Sin el modo estricto, el corte se va con él (lo que la
  ronda 91 llamó «se suelta a los 8 s»); con el modo estricto, el bloqueo sigue y se vuelve a intentar (más abajo). Las
  pruebas hacen el alta por la red de verdad contra un servidor local y comprueban que los dos lados
  llegan a la misma llave postcuántica.
- `crates/apps/src/tunel_win.rs::conecta`: lo mismo con el túnel de Windows (todo dentro y el corte puesto).
- `crates/panel/src/vpn.rs`: el estado y los botones en el panel del servicio. `GET /api/vpn` (en qué está, a qué
  servidor, desde cuándo, la dirección del túnel, los bytes y el último saludo, o por qué no está disponible: fuera de
  Windows, o sin `wireguard.dll` junto al programa), `POST /api/vpn/conecta` con `{"servidor": "…guardianagroup.com"}`
  (empieza; la página pregunta cómo va) y `POST /api/vpn/desconecta`. Sin el modo estricto nada se guarda: al parar el
  servicio, el túnel se cierra con él (el modo estricto, más abajo). La VPN es la del proceso, no la de cada vuelta del motor (1.0.54): el motor rehace el panel
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
  preguntado desde la tarjeta, fuera del túnel, se cortan antes de salir, y que Windows lo dice: al programa (WSAEACCES)
  en una conexión TCP, y en su propio registro (sucesos 5157 y 5152) en un datagrama UDP, que tira sin avisar a quien lo
  manda; y, ya desconectado, que la red vuelve. Desconecta siempre: en cada camino, y a los 150 s un vigilante cierra el
  túnel y sale; si el proceso muere, Windows quita los filtros y el adaptador con él. Para el corte, el programa de la
  prueba es «el servicio» (su archivo es el que tiene el permiso hacia el servidor): lo que se le corta fuera del túnel
  es lo que la 1.0.55 quitó al servicio.
  - La primera vez (3 nov 2026, 00:19 UTC): conectada en 3,6 s, el servidor contestó con el corte puesto (el permiso
    estrecho casa con los paquetes de verdad de WireGuardNT); el resolutor del túnel contesta; la dirección pública que
    ve el eco es la del servidor (`62.238.17.121`, la de `vpn.guardianagroup.com`) y no la del equipo; la puerta de
    enlace, cortada por el filtro (WSAEACCES); y al desconectar, la red vuelve entera. El DNS a 1.1.1.1 y 8.8.8.8
    desde la tarjeta no tuvo respuesta, pero sin aviso de Windows al programa: no prueba por sí solo que lo cortara el
    filtro, y por eso la prueba lee ahora el registro de Windows. El enlace del equipo de pruebas con GitHub no se
    cortó (unos 11 s con el túnel puesto).
  - La segunda (3 nov 2026, 00:40 UTC, con el registro de Windows): las 13 comprobaciones, bien. Conectada en 2,2 s; la
    dirección pública, la del servidor; la puerta de enlace, cortada con WSAEACCES y anotada por Windows (suceso 5157,
    de salida, TCP, capa de conexión); el DNS a 1.1.1.1 y 8.8.8.8 desde la tarjeta, sin respuesta y anotado por
    Windows como cortado por el filtro (suceso 5157, de salida, UDP, capa de conexión, y otro filtro que el del TCP,
    como corresponde al plan, que corta el DNS fuera del túnel con un filtro propio); y al desconectar, la red vuelve. El
    enlace con GitHub tampoco se cortó (unos 14 s con el túnel puesto).
- La prueba de extremo a extremo en Linux (`.github/workflows/vpn.yml`, «extremo»): el servidor de GUARDIANA con el
  WireGuard del núcleo en un espacio de red, el lado del equipo (`crates/tunel/examples/alta_prueba.rs`) en otro, el
  alta postcuántica por la red y tráfico de verdad por el túnel (solo pasa si los dos lados sacaron la misma llave).

## El modo estricto (ronda 91, 3 nov 2026)

> «Que la página web no lo pueda detectar… No se puede caer entre medias… Ahí tenemos que ser impecables.» (decisión de la
> agencia en la ronda 91, la prioridad número uno del dueño; `herramientas/agencia-rondas/ronda91/decisiones.md`, §1 y
> §6.1, en el repositorio de la web)

Encendido salvo que la persona lo apague (un interruptor, «Modo estricto», en la página de la VPN de Órbita). Mientras la
VPN debe estar encendida, nada sale del equipo fuera del túnel, también cuando GUARDIANA no está:

- **El bloqueo vive en Windows, aparte del servicio.** `crates/tunel/src/corte.rs` (`estricto`) parte el plan de siempre
  por lo que debe durar cada filtro, y `crates/apps/src/corte_vpn.rs` (`pon_estricto`) lo pone desde una sesión que no es
  dinámica: **filtros persistentes** (`FWPM_FILTER_FLAG_PERSISTENT`: siguen si el servicio se cae o lo matan, durante una
  actualización, al reiniciar el servicio de filtrado de Windows y el equipo) bajo un proveedor y una subcapa
  persistentes; y los bloqueos, con lo que mantiene viva la red de debajo, otra vez como **filtros de arranque**
  (`FWPM_FILTER_FLAG_BOOTTIME`: desde que arranca el equipo hasta que el servicio de filtrado carga los persistentes;
  Microsoft no deja que un filtro sea las dos cosas). Windows los acepta en la subcapa propia (prueba real en `vpn.yml`,
  5 nov 2026: 26 persistentes y 17 de arranque con los permisos y el bloqueo de lo reenviado; 38 y 29 con el bloqueo
  entero, dos de cada conjunto en las capas de reenvío); si no los aceptara allí, irían a su subcapa universal. Cada
  conjunto se cambia en una transacción, que quita el de antes y pone el nuevo: nunca hay un momento sin bloqueo.
- **Lo que dura lo que el túnel**: el permiso de la interfaz del túnel (con lo que se reenvía hacia el túnel y desde él)
  va a la misma subcapa desde una sesión dinámica del servicio (`PermisoTunel`), así que se va con el adaptador, que se
  va con el proceso: sin túnel, nada reenviado sale. Con el túnel abierto, el modo
  estricto decide lo mismo que el corte de siempre, más un permiso: el alta nueva del servicio con su servidor (TCP a su
  HTTPS, solo de salida, no firme), porque con el túnel caído no hay otro camino hasta él (la prueba
  `el_modo_estricto_conectado_decide_como_el_corte_de_siempre`).
- **Claves fijas**, para que el servicio, `guardiana vpn apaga` y el desinstalador encuentren y quiten siempre lo suyo, lo
  pusiera la versión que lo pusiera (`crates/tunel/src/estricto.rs`, `claves`): el proveedor
  `36694406-4925-4a21-9e16-0031b0e144b5` («GUARDIANA — VPN, modo estricto»), la subcapa
  `50573eae-5843-4fc2-aefc-12aeda4a7908` (peso 0xFFFF), los filtros persistentes `329dccb1-2e23-4244-bf84-61d4a4b7xxxx`
  y los de arranque `53fb6f77-c067-462b-9aca-6a6337d8xxxx`, donde `xxxx` es su sitio en el plan (hasta 64; quitar prueba
  las 64). El proveedor nombra el servicio `guardiana`: al arrancar, Windows solo carga sus filtros persistentes mientras
  ese servicio exista y arranque con el equipo, así que una desinstalación que no pudiera quitarlos no deja nada en vigor
  tras el siguiente reinicio.
- **Lo que la persona quiere, guardado**: `sistema/vpn.json` en la carpeta de datos (solo el sistema y los
  administradores escriben ahí): el modo estricto, si la VPN está encendida, su servidor y las direcciones del servidor
  (con el bloqueo puesto no se resuelve ningún nombre fuera del túnel, así que el alta nueva va a la dirección que ya se
  sabe; su certificado se sigue comprobando con su nombre). Mientras la VPN está encendida, también la última dirección
  conocida de cada servidor de GUARDIANA (`conocidas`, como mucho 16, solo nombres de guardianagroup.com): los servidores
  a los que fue y lo medido de las salidas con la VPN apagada, para poder cambiar de salida, o volver a la de antes, con
  el túnel caído (revisión de la 1.0.56). Ni llaves ni a dónde se va; al apagar la VPN, nada de esto se queda. Sin el
  modo estricto no se guarda la VPN encendida: acaba con el servicio, como siempre. Si al conectar en el modo estricto no
  se puede guardar, la VPN no cambia y el estado lo dice (sin quedar guardado el modo estricto no se sostiene; hasta la
  revisión de la 1.0.56 se apagaba sin decir nada).
- **Al arrancar, el servicio reconcilia** (`estricto::al_arrancar`, lo primero de `engine::run`, y solo en el servicio:
  `guardiana observe` no toca la VPN de nadie): si la VPN debe seguir encendida en el modo estricto, el bloqueo vuelve a
  ponerse en el acto (con el archivo del programa como esté, también tras una actualización) y el túnel se levanta otra
  vez; si ya no se quiere, lo que quedó en Windows se quita. Así pasan el fallo y el reinicio del servicio, la
  actualización (que para el servicio, lo cambia y lo arranca) y el reinicio del equipo, hasta que el servicio levanta
  el túnel. Al parar, el servicio cierra el túnel y deja el bloqueo. Desde la revisión de la 1.0.56: el servicio depende
  del servicio de filtrado de Windows (BFE), así que arranca después de él (el instalador lo registra así, y el propio
  servicio lo deja así al arrancar, con `sc.exe config guardiana depend= BFE`, si el servicio ya existía sin ello); si
  la reconciliación no sale bien (Windows no deja quitar lo que sobra), se repite cada minuto hasta que salga, como la
  mirada a los cortes del antivirus (`guardiana_panel::comprueba_la_vpn`); y si falta `wireguard.dll`, la VPN no puede
  volver: no se pone otra vez el bloqueo (el que Windows ya tenga sigue, y «Desconectar» lo quita), el estado lo dice
  (`"fase": "error"` con el motivo) y se vuelve a mirar cada minuto. Si Windows no acepta los filtros de arranque, el
  bloqueo vale desde que arranca su servicio de filtrado: se anota y `GET /api/vpn` lo dice (`"aviso_arranque"`).
- **Sin vuelta a la conexión normal.** Si el servidor no contesta al primer saludo (los 8 s de siempre), el túnel se
  cierra, el bloqueo sigue y se vuelve a intentar (alta nueva, túnel nuevo; entre un intento fallido y el siguiente, 5 s,
  luego 15 s dos veces y después 30 s cada vez: `estricto::espera_tras`, contando el fallo antes de esperar); si deja de
  contestar con el túnel abierto (200 s sin saludo), igual. Si el primer alta no contesta, el bloqueo se pone ya con lo
  que se sabe del servidor. La página dice «reconectando» y el aviso: «La VPN no conecta: no sale nada de tus programas
  fuera del túnel hasta que vuelva o la apagues tú». Tras un reposo del equipo, el servidor tiene su margen entero otra
  vez antes de darlo por caído. El vigía de la conexión lee lo guardado en cada vistazo: si `guardiana vpn apaga` apagó
  la VPN sin el servicio, el túnel se cierra (un archivo que en ese momento no se puede leer no cuenta como apagada).
  Cambiar de servidor (`POST /api/vpn/conecta` con otro) mantiene el bloqueo todo el rato; con el túnel caído ningún
  nombre resuelve, así que va a la dirección que ya se sabe de esa salida (lo guardado o lo medido con la VPN apagada) y,
  si no se sabe ninguna, la respuesta lo dice (`{"ok": false, "motivo": …}`) y nada cambia. Hasta la revisión de la
  1.0.56 contestaba que sí y seguía en silencio con el servidor de antes, aunque estuviera caído.
- **El DNS y el IPv6**, como siempre: cada salida con su resolutor (Unbound en `10.66.0.1`, por el túnel), ningún DNS
  fuera del túnel, nada de IPv6 fuera de él salvo lo del enlace.
- **La licencia**: conectar sigue siendo de Plus; la VPN que el servicio levanta al arrancar es la que la persona ya tenía
  encendida, y sigue hasta que la apague (como seguía hasta ahora una conexión hecha en la prueba).

Las salidas, siempre (tan importantes como el bloqueo):

1. **«Desconectar»** (el botón de Órbita, el chip de la barra de ZERO, `POST /api/vpn/desconecta`), en cualquier fase,
   también conectando o reconectando: la VPN apagada en lo guardado (primero, para que el próximo arranque quite lo que
   quede si algo falla), el túnel cerrado y todo lo del modo estricto fuera de Windows. Con el candado de los cambios y la
   generación: una conexión a medias nunca vuelve a poner el bloqueo después. Si lo guardado no se puede escribir
   (revisión de la 1.0.56), se quita el archivo (sin él, la VPN está apagada); si tampoco, la respuesta lo dice y se
   vuelve a intentar cada minuto mientras el servicio siga. Es lo más seguro: un arranque que encuentra la VPN encendida
   en lo guardado no puede distinguir un «Desconectar» que no se pudo guardar de un bloqueo que otro programa quitó de
   Windows o de un cierre a mitad de la primera conexión, y en esos casos dejarla apagada sería que los programas
   salieran fuera del túnel sin que la persona lo pidiera; así que la vuelve a levantar con su bloqueo, y es al apagar
   donde se hace todo para que quede escrito. Solo si ni se escribe ni se quita el archivo y el servicio se reinicia antes
   de conseguirlo, la VPN vuelve con su bloqueo, y «Desconectar» ya lo había dicho. Lo mismo al apagar el modo estricto.
2. **`guardiana vpn apaga`**, también con el servicio parado: le pide al servicio que desconecte (con la llave del panel) y,
   siempre, lo quita directamente por sus claves fijas y deja la VPN apagada en lo guardado. La llave solo va al servicio
   de GUARDIANA: antes de mandarla se pregunta al gestor de servicios de Windows si el servicio «guardiana» está en marcha,
   y si no, se va directamente por el otro camino (revisión de la 1.0.56: con el servicio parado, cualquier programa de
   cualquier cuenta podría escuchar en 127.0.0.1:7443 y llevarse la llave que manda un administrador). Necesita
   administrador; **la entrada «Apagar la VPN de GUARDIANA» del menú Inicio** lo ejecuta y, sin permisos, se los pide a
   Windows ella misma. `guardiana vpn estado [--json]` dice lo guardado y lo que Windows tiene.
3. **El arranque del servicio**, cuando el modo estricto ya no se quiere (lo apagaron, o la VPN quedó apagada): quita lo
   que quedó.
4. **El interruptor**: apagar el modo estricto con el servidor sin contestar es volver a la conexión normal, como haría la
   VPN sin él; con el túnel abierto, el cambio no deja ni un momento sin corte (primero el corte nuevo, luego se va el
   viejo). Si en ese momento Windows no deja quitar el bloqueo, el estado lo dice (sin decir que la conexión del equipo
   sigue como estaba) y `"desconectar": true` deja a la página enseñar «Desconectar», que lo vuelve a intentar.
5. **La desinstalación**: el instalador ejecuta `guardiana vpn apaga --desinstalando` después de parar el servicio (no al
   actualizar ni al reparar), y `guardiana service uninstall` lo hace también. Si algo fallara, el proveedor nombra el
   servicio: sin él, Windows no carga los filtros persistentes en el siguiente arranque.

**El Modo Hogar con el modo estricto.** Sin excepción de red local de salida (TunnelVision, CVE-2024-3661, y TunnelCrack):
de lo que sale fuera del túnel, solo el servicio hacia su servidor, la petición de DHCP a la difusión, DHCPv6 a sus
direcciones de multidifusión y el descubrimiento de vecinos; la casa, y una «red local» que un DHCP ajeno anuncie sobre
direcciones públicas, se cortan como cualquier sitio (la prueba `sin_excepcion_de_la_red_local`). Los permisos de entrada
del Modo Hogar (53 UDP y TCP, 7443 y 80 TCP, solo al servicio, solo de entrada) **se quedan**: contestan a los
dispositivos de la casa en la conexión que cada uno abre (Windows autoriza la conexión al aceptarla y sus respuestas
viajan en ella; el servicio no puede abrir ninguna hacia la casa) y no llevan fuera nada del tráfico de este equipo; lo
que esos dispositivos preguntan al DNS de GUARDIANA sale por el túnel, al resolutor del túnel, como lo de este equipo, y
con el túnel caído no sale. La regla del Modo Hogar en el cortafuegos de Windows (solo la red local, solo en una red
privada) sigue decidiendo después, porque estos permisos no son firmes. Quitarlos dejaría a la casa sin DNS mientras la
VPN está puesta, sin ganar nada contra una fuga.

Pruebas: las del plan y de lo guardado, en los tres sistemas (`corte.rs`, `estricto.rs`); las reales en Windows, como
administrador (`.github/workflows/vpn.yml`): que Windows acepta cada filtro persistente y de arranque con sus claves, que
volver a ponerlos no los duplica, que el permiso del túnel va en la misma subcapa, que el bloqueo de lo reenviado está en
las dos capas de reenvío (Windows lo dice al preguntarle la capa de cada filtro) y que quitar lo quita todo
(`corte_vpn`); con un servidor que no contesta (192.0.2.1, una dirección de documentación), que nada sale ni con el túnel
puesto ni al cerrarse, y que al quitar el bloqueo la red vuelve (`tunel_win`); y el trabajo «estricto», con el servicio de
verdad: el bloqueo al arrancar, nada por la tarjeta (WSAEACCES), «reconectando» con el aviso en `GET /api/vpn`, el bloqueo
que sigue con el servicio muerto y con el servicio parado, «Desconectar» que lo quita todo y devuelve la red, el arranque
que quita lo que quedó con el modo estricto apagado, y `guardiana vpn apaga` con el servicio parado; en cada paso, el
bloqueo de lo reenviado en la propia lista de filtros de Windows (`netsh wfp show state`); y, desde la revisión de la
1.0.56, que el servicio depende de BFE (también uno registrado antes sin ello, que lo recibe al arrancar),
`"desconectar"` y `"comprobado"` en la API, y sin `wireguard.dll` el arranque que no vuelve a poner el bloqueo, lo dice y
deja que «Desconectar» quite el que quedó. Lo que la integración continua no puede probar: el efecto de los filtros de
arranque en un reinicio de verdad, el instalador entero, tráfico reenviado de verdad (sus máquinas no tienen WSL ni
Hyper-V), ni la reconciliación que se repite cada minuto (haría falta que el servicio de filtrado de Windows fallara); de
esta, lo pendiente y su generación se prueban en `crates/panel/src/vpn.rs`.

La API (para la web): `GET /api/vpn` añade `"estricto"` (bool, el interruptor), `"bloqueado"` (bool, el bloqueo está en
Windows), `"aviso"` (el aviso en el idioma de la persona, o vacío), `"estricto_titulo"` y `"estricto_linea"` (el nombre
del interruptor y su línea, en su idioma), y la fase `"reconectando"`. Desde la revisión de la 1.0.56, también
`"desconectar"` (bool: hace falta «Desconectar», porque la VPN está encendida, el bloqueo sigue en Windows, también en
`"error"`, o la VPN sigue encendida en lo guardado; la página enseña el botón con él) y `"aviso_arranque"` (en su idioma, o vacío: Windows no aceptó los filtros
de arranque del bloqueo, y tras un reinicio el bloqueo vale desde que arranca su servicio de filtrado). `POST /api/vpn/estricto` con
`{"activo": true|false}` (o sin cuerpo: lo contrario de lo que está) contesta `{"ok": true, "estricto": bool}` o
`{"ok": false, "motivo": "…"}`; no es de Plus, y por ZERO solo tras un clic de verdad, como `conecta`.

Las salidas, en la API (5 nov 2026; el dueño, 3 nov, 09:25: «Todas las VPN te dicen a dónde»):

- `GET /api/vpn` añade `"salida"` (la salida en la que está o a la que iba: `id`, `pais`, `ciudad`, `continente`,
  `proveedor`, `nombre`, `zona`, `ip` —la del túnel abierto, la medida o la que se sabe—, `disponible` —`true` solo
  conectada; si no, lo medido con la VPN apagada, o `null` si no se sabe— y `ms`; `null` sin VPN),
  `"desde"`, `"saludo"`, `"subidos"`, `"bajados"` (siguen también `desde_ms`, `ultimo_saludo_ms`, `enviados` y
  `recibidos`), `"postcuantica"` (con el túnel: el alta siempre lleva su mitad de ML-KEM) y `"dns_tunel"`.
- `GET /api/vpn/salidas`: `{"salidas": [cada salida, como arriba], "mas_rapida": id | null, "medido": ms | null}`. Se
  miden todas las salidas a la vez, cada una en su hilo, y de cada una: su nombre resuelto, tres conexiones TCP a su
  HTTPS una detrás de otra (la mediana) y su alta. Solo con la VPN apagada y sin el bloqueo —encendida, la medida iría
  por el túnel—, y lo medido vale un minuto; una medida durante la que se encendió la VPN no se guarda ni vale como
  medida. Con la VPN encendida y nada medido, de cada salida solo su dirección (su nombre resuelto por el túnel, o la que
  se sabe), con `"disponible": null` y sin `ms`, ni `"mas_rapida"` ni `"medido"` (revisión de la 1.0.56: antes salía
  disponible solo porque su nombre resolvía). La salida en la que está, disponible si está conectada.
- `POST /api/vpn/conecta` acepta `{"salida": "<id>" | "rapida"}` (la más rápida de lo medido; sin medida, Helsinki);
  con el bloqueo del modo estricto puesto y sin túnel, solo a una salida cuya dirección se sabe (si no,
  `{"ok": false, "motivo": …}`). `POST /api/vpn/estricto`, también `{"estricto": bool}`.
- `GET /api/vpn/comprueba`: `{"ip_vista", "pais_ip", "dns": "tunel" | "fuera" | "desconocido", "ipv6": "bloqueado" |
  "sin_ipv6" | "fuga" | null, "comprobado": {"ip": bool, "dns": false, "ipv6": false}, "zona": {"equipo": null,
  "desfase_equipo", "salida", "desfase_salida", "coincide"}, "idioma": null, "ubicacion_windows": "permitida" |
  "denegada" | "desconocida"}`. `"comprobado"` (revisión de la 1.0.56) dice qué se midió de verdad: la dirección, cuando
  un eco contestó (`"ip": true`); el DNS y el IPv6 nunca, porque salen de lo que el túnel tiene puesto, así que la página
  los da «según la configuración». Lo que se pregunta fuera (el eco y el permiso de ubicación, con `reg.exe`) vale 15 s
  para el mismo estado de la VPN. La dirección es la que ve llegar el
  eco (`GET /vpn/alta`) de un servidor de GUARDIANA, nunca un tercero: con el túnel abierto, el de otra salida (la propia
  no contesta a lo que llega por su túnel: sus reglas solo dejan el resolutor), así que es la dirección con la que sale el
  tráfico del túnel; sin VPN, la de este equipo. Sin otro servidor que conteste, no hay dirección y la página no la da
  por comprobada. El país, de la base de direcciones del programa. El DNS y el IPv6, de lo que el túnel tiene puesto
  (el resolutor solo hacia el del túnel; el corte). La hora del equipo llega del navegador (`X-Guardiana-Tz`); su zona y
  su idioma los pone la página, que los lee ella misma.
- Por ZERO (`crates/zero/src/sesion.rs`, `PANEL_LECTURAS`) pasan las tres lecturas, y el cuerpo de `conecta` y de
  `estricto` comprobado y reescrito (`cuerpo_para_el_panel`: un id de salida, o el lado del interruptor; sin cuerpo,
  vacío o `{}`, también en `desconecta`, ninguno; cualquier otro cuerpo, 400). El guion del programa (`paginas.rs`) deja
  pasar, campo a campo, la dirección de la salida y la que ve una web (el servidor de la web ya ve esta al cargar la
  página), `"desconectar"`, `"aviso_arranque"` y `"comprobado"`, y nunca la del túnel (`"direccion"`; una prueba lo
  comprueba ejecutando el guion con node cuando está).

Pendiente, en este orden:

1. La pantalla de la VPN en la web del programa (Órbita), con la lista de servidores cuando existan.
2. ~~El `wireguard.dll` firmado dentro del instalador~~ Hecho (1 nov 2026, noche): `candidato.yml` baja el .zip de
   WireGuard LLC, `build/msi.ps1` comprueba la firma Authenticode y el SHA-256 fijado en
   `crates/apps/src/wireguard_nt.rs` (`HUELLA_DLL`) y mete `wireguard.dll` y su licencia (`LICENCIA-WIREGUARD-NT.txt`)
   junto a `guardiana.exe`; el servicio vuelve a comprobar el SHA-256 antes de cargarla (otra compilación, o un archivo
   puesto en su lugar, no se carga y se dice). La prueba de cliente (`build/cliente/instalacion.py`) lo comprueba
   instalado.
3. Los servidores, dentro del sello (con la llave del dueño). La primera conexión de verdad ya está hecha (3 nov 2026,
   «de verdad» en `.github/workflows/vpn.yml`, más arriba).
4. ~~La reconexión automática~~ Hecha en el modo estricto (ronda 91, 3 nov 2026, más arriba): el túnel caído se cierra,
   el bloqueo sigue, y el alta nueva sale por la tarjeta al TCP 443 del servidor, el único permiso que el bloqueo añade
   para el servicio; luego, túnel nuevo. Sin el modo estricto sigue sin reconexión automática. `Tunel::renueva` (alta
   nueva sin cerrar el túnel) sigue sin usarse.

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
- **Con el modo estricto, el bloqueo es de la VPN, no del servicio** (ronda 91): persistente y de arranque, se queda
  cuando el servicio se cae, se para o el equipo se reinicia, y solo lo quita apagar la VPN.
- **Siempre hay una salida** (ronda 91): «Desconectar» en cualquier fase, `guardiana vpn apaga` con el servicio parado (y
  su entrada en el menú Inicio), el arranque del servicio cuando ya no se quiere y la desinstalación.
- **Sin excepción de red local de salida en el modo estricto** (ronda 91): el Modo Hogar contesta a la casa por las
  conexiones que la casa abre, y nada más.

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
