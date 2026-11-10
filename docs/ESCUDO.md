# El Antivirus de GUARDIANA (el «escudo», desde 1.0.52, en preparación)

> «El escáner que tenemos del PC. Quiero que sea el antivirus.» «El mejor antivirus de la Tierra… la capacidad de análisis
> del escáner debe ser única… Y siempre código abierto.» (el dueño, 1 nov 2026, 16:21 y 16:36)

Investigación de la agencia: ronda 87 (Gemini y la revisión de Claude), en el repositorio de la web,
`herramientas/agencia-rondas/ronda87/`. Este documento dice cómo se construye aquí, sobre lo que el programa ya tiene.

## Lo que lo hace distinto

Ningún antivirus doméstico une hoy, en un solo programa y sin nube, estas cuatro cosas que GUARDIANA ya tiene:

1. **El DNS de la casa** (`crates/dns`): sabe con qué nombres ha hablado cada equipo y desde cuándo.
2. **Los programas y sus archivos** (`crates/apps`): quién firma cada programa (`firmas.rs`), qué archivos de la persona
   abre (`archivos.rs`, eventos del núcleo de Windows con ETW) y el cortafuegos de Windows (`cortafuegos.rs`, WFP).
3. **El Vigía** (`crates/apps/src/vigia*`, `crates/panel/src/vigia.rs`): micrófono, cámara, pantalla y control remoto.
4. **La revisión de Windows del Antivirus** (antes «Escáner PC»; `revision.rs` y `revision.ps1`): la seguridad de
   Windows en solo lectura.

El escudo las cruza. Ejemplos de lo que busca: un programa descargado y sin firma que pide un nombre que la casa no había
visto nunca; un proceso en segundo plano que abre las contraseñas o las cookies del navegador; un programa que mira la
pantalla o el micrófono mientras habla con un destino nuevo. Y lo explica en palabras llanas: qué programa, de dónde vino,
qué hizo y a quién habló.

## Fases

**Fase 1 (sin controlador propio; convive con Microsoft Defender).**

- A1 · *Análisis de archivos*: huella SHA-256 de cada archivo contra listas de muestras conocidas cuya licencia lo
  permita, empezando por Descargas y lo que llega por Comunicaciones y por ZERO; cuarentena reversible.
  **Licencias, comprobado el 1 nov 2026:** las condiciones de abuse.ch (en vigor desde el 4 nov 2025) no dan
  MalwareBazaar, URLhaus ni ThreatFox como CC0: el uso con ánimo de lucro puede pedir una suscripción (Spamhaus), y para
  descargar las listas hace falta una llave de su portal. Solo Feodo Tracker, SSL Blacklist y YARAify son CC0
  (<https://abuse.ch/terms-of-use/>, <https://bazaar.abuse.ch/export/>). Usarlas es decisión del dueño.
  Crate `crates/escudo` (listas, análisis y cuarentena), probado en Linux y en Windows.
- A2 · *Reglas*: YARA-X (Rust, BSD-3-Clause) con reglas abiertas de licencia compatible (signature-base y otras con la
  licencia DRL 1.1, que pide citar al autor en cada aviso; LOLBAS, GPL-3.0, para los programas de Windows que se abusan).
- A3 · *Comportamiento*: eventos de Windows (ETW: procesos, DNS por proceso, archivos) en un grafo de procesos, con reglas
  Sigma (DRL 1.1); y el cruce con el DNS de la casa y el Vigía. Contención antes que bloqueo: a lo sospechoso se le corta la
  red con el cortafuegos que ya existe, y se bloquea al ver el daño (nunca solo por una puntuación).
- A4 · *AMSI*: los guiones (PowerShell, JScript, VBScript, macros) se analizan justo antes de ejecutarse.

**Fase 2.** Modelo local pequeño para ejecutables (sin nube), memoria de los procesos, señuelos contra el ransomware y
limpieza completa de lo que deja un programa malicioso (lo que el Antivirus ya encuentra: arranque, tareas, servicios).

**Fases 3 y 4 (necesitan decisión y papeles de la empresa).** Controlador propio (minifiltro): certificado EV y alta en el
Partner Center de Microsoft. Programa MVI de Microsoft: producto con protección en tiempo real, certificado en los
últimos 12 meses por un laboratorio de su lista, acuerdos firmados y Trusted Signing
(<https://learn.microsoft.com/en-us/defender-xdr/virus-initiative-criteria>).

## Lo que ya está (rama `antivirus-vpn`, 1 nov 2026)

- `crates/escudo`: huellas (listas compactas con su propia huella), análisis de archivos y carpetas sin seguir
  enlaces, cuarentena encriptada que devuelve el archivo exactamente igual, y la regla «primera vez».
- El servicio pasa por «primera vez» cada consulta de este equipo con su programa (`crates/cli/src/escudo.rs`); el
  observador dice si Windows ya comprobó la firma (si no, la consulta espera hasta un minuto).
- «Ladrones de datos» (`ladrones.rs`): un programa que no es el de la aplicación abre las contraseñas, cookies o
  llaves guardadas del navegador, una cartera o una sesión de mensajería.
- «Cadenas» (`cadenas.rs`): qué programa abrió cuál. Un documento de Office que abre una consola o un intérprete, un
  navegador que abre una consola, un guion que abre otro intérprete, y un programa descargado (o en una carpeta donde
  cualquiera escribe) que abre las herramientas de copias de seguridad de Windows.
- Las dos miran los eventos del núcleo de Windows en la misma sesión (`crates/apps/src/guardia.rs`): solo nombres y
  rutas, nunca el contenido de los archivos.
- «Arranque» (`arranque.rs`): de lo nuevo que arranca solo con Windows (lo que ya anota la vigilancia continua), el
  programa sin firma que está en una carpeta donde cualquiera escribe o que llegó de internet. Grave si está en
  Descargas, en una carpeta temporal o en el Escritorio, si llegó de internet o si es un servicio. Un programa firmado
  no se juzga aquí; uno cuya firma no se pudo comprobar, solo en esas tres carpetas.
  Aparte, las suscripciones de WMI que ejecutan una orden o un guion (`CommandLineEventConsumer`,
  `ActiveScriptEventConsumer`), que la vigilancia lee como un arranque más: una nueva es grave siempre, porque es una
  forma de arrancar sola que no sale en el Inicio ni en las tareas y la usan intrusiones que no dejan archivos; lo que
  ejecuta suele ser un programa de Windows (PowerShell), así que su firma no dice nada.
  Y los otros sitios desde donde arranca un programa, como los enseña Autoruns: lo que ejecuta Winlogon al iniciar
  sesión (Shell y Userinit; un cambio es grave), un «depurador» en las opciones de ejecución de Windows (Image File
  Execution Options), que abre otro programa cada vez que se abre uno (grave si sustituye a un programa de
  accesibilidad, como `sethc.exe` o `utilman.exe`, que se abren sin contraseña desde la pantalla de inicio de sesión: la
  puerta trasera de las «teclas especiales»; o si ejecuta un intérprete o está donde cualquiera escribe), y las DLL que
  Windows carga en todos los programas (AppInit_DLLs, grave).
- «Secuestro» (`secuestro.rs`): un programa que renombra o borra al menos 100 archivos de la persona (documentos,
  escritorio, imágenes, música, vídeos, descargas, OneDrive) en menos de un minuto, lo que hace un programa de
  secuestro de archivos al encriptarlos. Un programa firmado no se juzga, salvo los intérpretes (PowerShell, la
  consola, Windows Script Host). Lo que sobrescribe los archivos sin renombrarlos ni borrarlos no lo ve esta regla.
- «Protección» (`proteccion.rs`): lo que la vigilancia continua anota de Seguridad de Windows, cuando la protección se
  apaga: la protección en tiempo real o contra alteraciones de Microsoft Defender, o ningún antivirus encendido
  (graves), o un perfil del cortafuegos (aviso). Lo que ya estaba apagado en su primera mirada, aviso sin gravedad.
  Y las exclusiones de Microsoft Defender (lo que no analiza: una carpeta, un programa o un tipo de archivo), que la
  vigilancia lee como servicio del sistema: cada una nueva avisa, y es grave si deja fuera mucho (una unidad entera,
  una carpeta del sistema o la de una persona, Descargas, el Escritorio, AppData o las temporales, un programa que
  ejecuta otros como PowerShell, o los archivos que ejecutan, como `.exe` o `.ps1`), porque añadir exclusiones así es
  de lo que hacen los programas dañinos para esconderse. Las que ya estaban en su primera lectura solo avisan si son
  así de amplias.
  Y cuándo se vació por última vez cada registro de eventos de Windows (lo anota el propio Windows: 1102 en el de
  seguridad, 104 en el del sistema para los demás). Vaciar el de seguridad, donde Windows apunta los inicios de sesión
  y los cambios de permisos, es como un intruso borra su rastro: grave. Otro registro: aviso (también lo hacen algunos
  programas de limpieza). La primera lectura es el punto de partida.
- «Hosts» (`hosts.rs`): el archivo hosts de Windows, cada minuto. Un nombre que ahora va a una dirección de internet
  (no a 0.0.0.0, a este equipo ni a la red de casa) es grave: así se desvía una web a otro servidor sin que ningún DNS
  lo vea. La primera lectura es el punto de partida y se guarda junto al diario (`escudo-hosts.json`).
- «Sin nombre» (`sin_nombre.rs`; ronda 87, segunda meta de Gemini: «toda conexión de un proceso debe venir de una
  consulta DNS»): un programa que se conecta directamente a una dirección de internet a la que no apuntaba ningún nombre
  que este equipo haya preguntado en las últimas horas. Solo mientras GUARDIANA es el DNS del equipo; sin las locales ni
  las anycast. Firmado no se juzga; solo avisa de los que están en carpetas donde cualquiera escribe o llegaron de
  internet (grave sin firma en Descargas, temporales o Escritorio, o descargado). La dirección no se guarda: solo su red y
  su país.
- «Disfraz» (`disfraz.rs`): un programa que se llama como uno de Windows (`svchost.exe`, `lsass.exe`, `csrss.exe`…) y
  arranca desde fuera de la carpeta de Windows, el único sitio donde está el de verdad. Grave. Lo ve la misma vigilancia
  de los inicios de programa que «cadenas».
- «Engaño» (`engano.rs`): un programa que arranca con un nombre hecho para parecer otra cosa: un documento seguido de
  la extensión de un programa (`factura.pdf.exe`, que con las extensiones ocultas se ve `factura.pdf`) o un carácter
  invisible que da la vuelta al texto para esconder el `.exe`. Grave. Lo ve la misma vigilancia de los inicios.
- «Medio» (`medios.rs`): un programa que se abre desde un disco o una imagen montada (un archivo ISO o IMG abierto con
  doble clic: el núcleo lo ve en `\Device\CdRom…`) o desde una unidad extraíble (una memoria USB o una tarjeta). Abrir
  lo que viene dentro de una imagen recibida por correo o descargada es una vía de entrada frecuente. Firmado, nada;
  sin firma, grave; sin poder comprobarla, aviso. La prueba real monta una ISO hecha con IMAPI2 y abre un programa
  desde ella.
- «Credenciales» (`credenciales.rs`): un programa que abre la memoria de LSASS, el proceso de Windows que guarda las
  credenciales de las cuentas con sesión abierta, pidiendo leerla (`PROCESS_VM_READ`): así se roban las contraseñas de
  un equipo. Lo ve la misma sesión con el canal de auditoría del núcleo (`Microsoft-Windows-Kernel-Audit-API-Calls`,
  evento 5: `TargetProcessId`, `DesiredAccess`, `ReturnCode`). Lo de Windows y lo instalado en Program Files
  (antivirus, copias de seguridad) no se juzga; un programa que ejecuta otros (rundll32, PowerShell…) o una herramienta
  que copia la memoria de un proceso, sí, esté donde esté. Grave, también si Windows lo negó (la protección de LSA):
  el intento ya es la señal. Desde la 1.0.58 (10 nov 2026) tampoco se juzga un programa con firma válida que no sea
  una de esas herramientas: muchos programas instalados en la carpeta de la persona recorren la lista de procesos
  pidiendo a cada uno los derechos de siempre, leer la memoria entre ellos, y LSASS es uno más de la lista. OneDrive lo
  hace cada hora desde `AppData\Local\Microsoft\OneDrive`: hasta la 1.0.57 era un aviso grave cada hora, con el
  botón de la cuarentena, en cualquier equipo con OneDrive (en el del dueño, 62 en tres días). Los derechos pedidos no
  distinguen a uno de otro; quién firma el archivo, sí. El aviso espera hasta un minuto a que Windows compruebe la
  firma; sin firma válida, o sin poder comprobarla, se avisa como antes. Cuando Windows da por firmado un programa, los
  avisos de esta regla que una versión anterior dejó sobre ese mismo archivo se quitan de la lista del panel.
- «Segunda opinión» (`defensor.rs`): en cada aviso grave sobre el archivo de un programa (fuera de la carpeta de
  Windows), se pregunta a Microsoft Defender por ese archivo (`MpCmdRun -Scan -ScanType 3 -File … -DisableRemediation`:
  solo dice, no toca nada; un archivo, una vez al día, en un hilo aparte). Si lo reconoce como amenaza, es otro aviso
  grave con el nombre que le da Defender.
- El panel: `GET /api/escudo` (si aún aprende, cuántos sitios, los avisos); la web dentro del programa puede leerlo.
- Los avisos graves llegan a la persona aunque no abra nada (el dueño, 1 nov 2026, 19:29: «Me parece bien la
  propuesta, sí, buena idea»): `GUARDIANA ZERO.exe --avisos` arranca al iniciar sesión (entrada Run del instalador) y
  con cada apertura de ZERO, sin ventana visible y uno por persona; lee `escudo-avisos.jsonl` y saca la notificación de
  Windows de cada aviso grave nuevo (más de tres a la vez: una que los cuenta). Al pulsarla, `guardiana://escaner` abre
  Órbita en el Antivirus, cuya portada enseña los avisos graves. Qué se enseña lo decide `crates/zero/src/avisos.rs`;
  la ventana oculta de `zero/navegador/src/avisos.rs` recibe el «cerrar» del instalador al actualizar o desinstalar.
- Falta: la pantalla del antivirus en la web del programa, el análisis de Descargas desde el panel y las listas de
  muestras (decisión del dueño, abajo).

## La contención, apagada en 1.0.52 (2 nov 2026)

Los señuelos y la congelación al momento (señuelo borrado, renombrado o reescrito; borrado de las copias de seguridad;
ráfagas graves) están hechos y probados en un Windows de verdad (`escudo.yml`, `contencion::tests`), pero la segunda
revisión independiente encontró fallos que impiden darlos a la gente todavía. La 1.0.52 sale con el interruptor
`guardiana_apps::CONGELAR` apagado: el escudo avisa, la cuarentena funciona, no se planta ningún señuelo y no se congela
nada. Lo que hay que arreglar antes de encenderlo:

1. El servicio (SYSTEM) reescribe un señuelo siguiendo enlaces: un usuario podría apuntarlo a cualquier archivo. Escribir
   los señuelos sin seguir enlaces (sin puntos de análisis en la ruta) o no reescribirlos.
2. «Es mío» guarda la ruta del programa para siempre: nunca para las herramientas de Windows (vssadmin, powershell, cmd…).
3. Números de proceso reutilizados: la familia y el autor se comprueban con la hora de creación de cada proceso, y no se
   congela lo que se vio hace más de unos segundos.
4. El propio programa (ZERO y sus WebView2) y los navegadores nunca se congelan como «familia».
5. Nada se da por congelado si no se congeló; las herramientas de Windows dentro de `C:\Windows` también cuentan.
6. Congelar dos veces y reanudar una: se reanuda tantas veces como se congeló; un id por caso.
7. Las decisiones, una sola vez (escritas antes de actuar, con bloqueo).

## Lo que cuesta (medido)

La vigilancia de archivos ve cada archivo que se abre en el equipo, así que tiene que pesar poco. La prueba real
(`cuanto_cuesta_la_guardia` en `crates/apps/src/guardia.rs`, `.github/workflows/escudo.yml`) la compila como se publica,
pone en marcha la sesión del escudo y hace que otro proceso cree y lea 20 000 archivos en la carpeta temporal; dice
cuántas aperturas llegaron y cuánto procesador gastó el proceso del escudo (con la propia medida incluida).

| Fecha (Windows de GitHub) | Aperturas | Procesador | Por apertura | Leyendo los campos |
|---|---|---|---|---|
| 1 nov 2026, antes | 183 338 | 438 ms | 2,4 µs | 1,1 µs |
| 1 nov 2026, búsqueda anclada en `\appdata\` | 183 318 | 281 ms | 1,5 µs | 1,0 µs |

La carpeta temporal está dentro de AppData, el caso más caro de `ladrones::que_es`; por eso la búsqueda mira ahora solo
donde empieza cada `\appdata\` de la ruta. Lo que queda es sobre todo leer los campos de cada suceso con ferrisetw.

## Reglas de la casa que se mantienen

- **Nada sale del equipo.** El análisis es local. Nada de subir muestras a ninguna nube.
- **Ninguna lista es un veredicto por sí sola** (`docs/LISTS.md`): un archivo firmado por un editor de confianza o del
  catálogo de Windows no se pone nunca en cuarentena por una lista.
- **La cuarentena se puede deshacer** y lo dice: qué archivo, de dónde, por qué y con qué fuente.
- **Sin cifras inventadas** en las pantallas: cada aviso cita su fuente y la regla.

## Decisiones (ronda 88, 1 nov 2026, con el encargo del dueño)

- **Las listas viajan con cada versión** del programa, que no las descarga nunca (`docs/LISTS.md`): el camino (a). El (b),
  a diario desde los servidores de GUARDIANA y solo si dos dan la misma lista, cuando los servidores estén dentro del
  sello. El formato de `crates/escudo` vale para los dos.
- **Solo listas de licencia libre**: de abuse.ch, las CC0 (Feodo Tracker, SSL Blacklist, YARAify); MalwareBazaar,
  URLhaus y ThreatFox no, porque su uso con ánimo de lucro puede pedir una suscripción de pago.
- **Cuarentena desde la lista de avisos: sí**, solo para archivos que el escudo señaló, nunca en la carpeta de Windows ni
  en Program Files, y siempre restaurable.
  Hecho (1 nov 2026, noche): el panel da a cada aviso su id y si su archivo puede ir (`GET /api/escudo`), lista lo
  apartado (`GET /api/escudo/cuarentena`), lo pone (`POST /api/escudo/cuarentena/{id}`; también fuera quedan
  `ProgramData\Microsoft`, y un programa abierto no se mueve: lo dice) y lo devuelve
  (`POST /api/escudo/cuarentena/restaura/{id}`). ZERO deja pasar desde la web de GUARDIANA solo la lista y el poner,
  por id; devolver, solo desde el panel («Tu antivirus» → «El escudo»). La portada del Antivirus en la web ya no dice
  «Solo lectura»: dice exactamente qué puede hacer.
- Detalle: `herramientas/agencia-rondas/ronda88/decisiones.md` en el repositorio de la web.
