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
4. **El Escáner PC** (`revision.rs` y `revision.ps1`): la seguridad de Windows en solo lectura.

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
limpieza completa de lo que deja un programa malicioso (lo que el Escáner PC ya encuentra: arranque, tareas, servicios).

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
- El panel: `GET /api/escudo` (si aún aprende, cuántos sitios, los avisos); la web dentro del programa puede leerlo.
- Falta: la pantalla del antivirus en la web del programa, el análisis de Descargas desde el panel y las listas de
  muestras (decisión del dueño, abajo).

## Reglas de la casa que se mantienen

- **Nada sale del equipo.** El análisis es local. Nada de subir muestras a ninguna nube.
- **Ninguna lista es un veredicto por sí sola** (`docs/LISTS.md`): un archivo firmado por un editor de confianza o del
  catálogo de Windows no se pone nunca en cuarentena por una lista.
- **La cuarentena se puede deshacer** y lo dice: qué archivo, de dónde, por qué y con qué fuente.
- **Sin cifras inventadas** en las pantallas: cada aviso cita su fuente y la regla.

## Decisión pendiente del dueño

Las listas viajan hoy dentro de cada versión del programa y el programa no las descarga nunca (`docs/LISTS.md`). Para el
antivirus hay dos caminos: (a) igual, con cada versión; (b) a diario desde los servidores de GUARDIANA, y solo si dos
servidores dan la misma lista. El formato de `crates/escudo` vale para los dos.
