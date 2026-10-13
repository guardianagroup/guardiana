# GUARDIANA vigila a tu antivirus (desde 1.0.20)

> «Alguien de código abierto debe vigilar al vigilante.» (el dueño, 13 oct 2026)

Un antivirus es el programa en el que más se confía: lee cada archivo y, en muchos productos,
cada página. La pantalla **Tu antivirus** del panel lo vigila con lo mismo que GUARDIANA usa con
cualquier otro programa, y nada más.

## Qué enseña

- **A dónde habló.** Para cada antivirus cuyos procesos pidieron nombres en este ordenador en
  los últimos siete días: cada nombre, cuántas veces, cuántas de noche (00:00–06:00 de la
  persona), cuántas cortadas y cuándo fue la última.
- **Qué dice su fabricante de cada nombre.** Al lado va el uso que el propio fabricante publica
  en su documentación (actualizaciones, análisis en la nube, envío de muestras, licencia,
  telemetría, mensajes dentro del programa), con el enlace. Lo que el fabricante no documenta
  sale primero y dice «el fabricante no lo documenta» y, si las listas lo saben, de quién es el
  nombre. Es una pregunta, no una acusación.
- **Quién firma cada proceso**, según Windows. Un proceso con nombre de antivirus firmado por
  otro, o sin firma, no tiene por qué ser ese antivirus.
- **Lo que han decidido las autoridades** sobre cada producto, esté instalado o no: multas,
  órdenes con pago, sentencias, prohibiciones, advertencias oficiales y compromisos aceptados,
  cada uno con la página de la autoridad. Una decisión recurrida o anulada en parte se dice así:
  la multa checa a Avast (351 millones de coronas, 2024) aparece junto a la sentencia de 2025
  que anuló su importe.

## De dónde sale

`crates/lists/data/antivirus.txt`, una lista abierta que mantenemos nosotros, con su formato en
la cabecera. Reglas:

1. Solo procesos y dominios que publica el fabricante (o su grupo), con el enlace encima de cada
   producto. Ningún dominio inferido por su nombre ni copiado de foros.
2. Cuando la documentación es de la edición de empresa (GravityZone, Apex One, WatchGuard,
   Sophos Central, Kaspersky Endpoint Security), se dice. Los dominios que el fabricante da sin
   función (`*.avast.com`, `*.sophosupd.com`) van como «otros».
3. Un proceso pertenece a un solo producto. `afwServ.exe`, común a Avast y AVG, se deja fuera, y
   también nombres demasiado cortos para atribuirlos sin dudas (`ns.exe`, `nis.exe`).
4. Solo decisiones de autoridades con su nota o resolución oficial publicada. No entran
   demandas privadas, acuerdos sin resolución ni noticias. Si no tenemos el enlace oficial, la
   decisión espera: la multa rumana a Bitdefender de abril de 2025 (10 000 EUR, por un servicio
   de análisis de correo, no por el antivirus) está pendiente de ese enlace.

La investigación se hizo el 13 de octubre de 2026 con ChatGPT y se contrastó con las páginas
oficiales (FTC, ÚOOÚ, Tribunal Municipal de Praga, BIS, CMA, BSI, DHS, Gobierno de Canadá y
la documentación de cada fabricante).

## Lo que no puede ver

- Lo que el antivirus **envía**: GUARDIANA ve nombres, nunca contenido.
- Lo que manda **sin preguntar un nombre al sistema** (con su propio resolutor o a una dirección
  fija).
- Lo que hace **a través de otros programas de Windows** (Windows Update descarga firmas de
  Defender bajo su propio proceso).
- Una consulta DNS no prueba qué datos viajaron. Por eso la pantalla dice lo que documenta el
  fabricante y lo que no, y nunca «te espía».

## Analizar y medir en tiempo real (1.0.21)

Dos cosas más, en este ordenador:

- **Qué antivirus hay**: la página pregunta a Windows (Centro de seguridad,
  `root/SecurityCenter2`) qué antivirus hay instalado y si está activo y al día, al abrirse y cada
  dos minutos; en 1.0.22 ya no hay botón. Solo lectura, sin elevación. En Windows Server o fuera
  de Windows no hay Centro de seguridad y se dice así.
- **Qué envía y recibe, por país**: GUARDIANA cuenta, por cada proceso del antivirus, los bytes
  que salen y entran por cada destino, y los agrupa por país del servidor y por destino. La
  medida viene del canal de red de Windows (`Microsoft-Windows-Kernel-Network`, GUID
  `7dd42a49-5329-4832-8dfd-43d979153a88`), usando el PID que lleva el propio suceso. El destino es
  el nombre que el programa pidió para esa dirección (cuando GUARDIANA vio la pregunta); la
  dirección en sí nunca se guarda (decisión 203). Las medidas siguen la retención del detalle (24
  h en el plan gratis) y no están en la cadena de hashes: son medidas, no sucesos.

Lo que «enviado» significa es lo que la pila dio por enviado, no una prueba de que el servidor lo
recibiera ni de qué contenía. Eso último no se puede saber sin descifrar, y GUARDIANA no descifra.

GUARDIANA nunca pide apagar el antivirus. Vigilarlo no es quitarlo.

## Todo dentro (1.0.22)

- **Resumen de las últimas 24 horas**, siempre arriba y en directo: bytes enviados y recibidos por
  todos los antivirus, países, destinos distintos, lo enviado de madrugada y quiénes han hablado.
- **En directo**: lo último de los últimos 20 minutos, minuto a minuto, lo nuevo arriba.
- **Exportar** todo el tráfico a CSV (hora local, producto, programa, destino, país, red, bytes).
- **El negocio del antivirus**: cifras globales con su fuente; donde no hay cifra fiable, se dice.
- **Vigilar, sí o no** (`/api/antivirus/vigilar`): apagado, GUARDIANA deja de medir y de guardar lo
  que envían los antivirus. No deshace ningún corte y las consultas de nombres siguen anotándose.
- **Cortar** (`/api/antivirus/cortar`) un antivirus, o todos con **Cortar todo lo que envía el
  antivirus** (`/api/antivirus/maxima`; hasta el 13 oct 2026 se llamaba «Protección máxima del
  antivirus», un nombre engañoso: con el corte, el antivirus protege menos). Va plegado en
  «Controles avanzados»; puesto, una línea fuera del pliegue lo recuerda, y en «Por dentro» lo dice
  la barra de arriba. Filtros de verdad en el cortafuegos de Windows (WFP, modo usuario, sin
  controlador propio) sobre los ejecutables vistos esa semana. La sesión es dinámica: si GUARDIANA
  se detiene o se cae, Windows quita todo solo, y GUARDIANA lo vuelve a poner al arrancar hasta que
  la persona lo quite. No se finge nada: el corte es un bloqueo, no un «éxito» falso. Un clic para
  ponerlo (con aviso previo de que sin actualizaciones ni análisis en la nube protege menos) y uno
  para quitarlo.

Descartado a propósito: engañar al antivirus con un éxito falso (necesitaría un controlador del
núcleo y no es posible del todo) y descifrar lo que envía (exigiría un certificado en medio de
todas tus conexiones, y GUARDIANA no hace eso).

## GUARDIANA por dentro (1.0.22)

Una página aparte (`/por-dentro`), con un botón bien visible en la portada del panel y en «Tu
antivirus»: lo que el antivirus, el navegador y los programas que más se mueven envían y reciben, en
directo, sobre la Tierra.

- **Dos vistas** (desde la 1.0.23; la 1.0.22 tenía cinco): **Destinos**, dónde está cada servidor
  sobre la Tierra (el mapa por defecto, el globo como opción), y **Programas**, cuánto envía cada
  programa y adónde, por la puerta de GUARDIANA. La nebulosa y el radar se quitaron: pedían
  explicación y ayudaban poco a decidir. Un interruptor **Todo · Datos · Antivirus** separa en el
  dibujo lo que envía el antivirus de lo que envían el navegador y los demás programas. Globo y mapa
  están hechos en WebGL2 escrito aquí, con imágenes de la NASA servidas por el propio panel y la luz
  del sol de ese momento; un equipo sin WebGL2 ve el mismo mundo plano, dibujado en 2D. Sin internet
  y sin código de terceros.
- **Tres datos distintos**: la empresa del nombre (de la lista abierta de GUARDIANA), la empresa de la
  red donde está el servidor y su país. Que un servidor esté en la red de Amazon no quiere decir que
  Amazon use lo que se envía, y la página lo dice. Los intentos parados se cuentan, no se miden en
  bytes: se dibujan como líneas discontinuas, nunca en la escala de los datos.
- **Datos** (`/api/por-dentro`, cada 5 segundos): por programa y destino, los bytes, paquetes,
  protocolo y puerto que contó Windows; el país y la red del servidor (con su AS); si el fabricante
  documenta el nombre (solo antivirus); cuántas veces lo pidió y cuántas lo paró GUARDIANA. Nunca la
  dirección del servidor (decisión 203). Un nombre parado antes de salir no tiene servidor: se pone en
  la sede de su empresa y se dice así.
- **Programas**: los antivirus de la lista, los navegadores conocidos (Chrome, Edge, Firefox, Brave,
  Opera, Vivaldi, GUARDIANA ZERO…) y los ocho programas que más se mueven. Solo un antivirus se puede
  cortar entero (un navegador dejaría de funcionar); los nombres se cortan uno a uno con una regla de
  este ordenador, que se deshace con otro clic.
- **Tu país** sale de la zona horaria y del idioma del equipo; no se pregunta nada fuera.
- **Horas abierta**: 30 imágenes por segundo, 20 tras dos minutos sin tocar nada, y se para cuando la
  pestaña no se ve.
- **Solo medidas, nunca datos de ejemplo.** Sin medidas, lo dice con palabras: sin sesión;
  «Midiendo» mientras llegan las primeras (y, si pasado medio minuto no llega nada, que aún no ha
  llegado ninguna); la vigilancia del antivirus apagada, con el enlace a «Tu antivirus» para
  encenderla; o, fuera de Windows (`mide_bytes: false`), que ese sistema no mide los datos de cada
  programa y qué sí ve.

Las fuentes de las imágenes y del mapa están en `crates/panel/static/tierra/LEEME.md`.
