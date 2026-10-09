# Guardiana — Lo que no hace

Versión 1.0 · 8 de septiembre de 2026 · derivado de `docs/BRIEF.md` y `docs/HOGAR.md`.
Cada punto se dice también en la pantalla donde el usuario lo esperaría, con estas palabras.
La regla: nunca "100 % seguro", nunca "invisible", nunca "protegido" sin objeto, nunca "malicioso".

## Lo que no ve

- **No ve qué app pidió cada nombre, salvo en este computador y solo en Windows.** Desde el 20 de
  septiembre de 2026 (decisión 183), en Windows y solo para el equipo donde está instalada,
  Guardiana sí puede decir qué programa pidió cada nombre: se lo cuenta el propio Windows por su
  canal de sucesos, y lo que guarda es el nombre del programa, su ruta y su huella. En Modo Hogar,
  para teléfonos y televisores, sigue sin poder saberse: "En Modo Hogar, Guardiana ve nombres de
  servicios, no qué app los pidió." Y en ningún caso corta por programa: eso es 2.0.
- **No mira archivos, ni siquiera los suyos.** Los «archivos trampa» (decisión 184) no son
  vigilancia del sistema de archivos: el cebo es un nombre único escrito dentro del archivo, y lo
  único que Guardiana ve —como siempre— es si alguien pregunta por ese nombre. Nunca abre el
  archivo, y un programa que lo lea y no siga el enlace no aparece en ningún sitio.
- **No ve contenido.** Ni páginas, ni mensajes, ni qué se hizo en cada servicio.
- **No ve cuántos bytes.** Solo que se preguntó por un nombre, cuándo y cuántas veces.
- **No anota respuestas.** Solo consultas: nombre, tipo, dispositivo, hora, categoría, veredicto.
- **No ve lo que esquiva el DNS.** Apps con IP fija, apps con su propio DNS cifrado (DoH/DoT), una
  VPN en el teléfono, iCloud Private Relay en Safari. Cuando lo detecta, lo anota como "tráfico
  fuera de vista"; no lo esconde ni lo cuenta como protegido.
- **No ve nada fuera de la Wi‑Fi de casa.** Con datos móviles o en otra red, el teléfono no pasa por
  Guardiana. Frase: "Solo en la Wi‑Fi de casa, por nombre de dominio."
- **No ve lo que hace el router por su cuenta.** El router es la puerta de la casa y Guardiana vive
  detrás de ella. Si el router copia tráfico o lo envía fuera en silencio, desde dentro no se ve.
  Si el router usa Guardiana como DNS, sus propias consultas se anotan como las de cualquier otro
  dispositivo. Frase: "Guardiana ve lo que pasa dentro de la Wi‑Fi, no lo que hace el router."
- **No ve nada si el computador guardián está apagado o dormido.** Un portátil que se suspende
  deja de responder; Modo Hogar avisa si el equipo tiene la suspensión activada. Los dispositivos siguen resolviendo por
  el DNS secundario del router; nada se rompe y nada se anota.

## Lo que no decide

- **No dice "malicioso".** Dice qué observó: la categoría de la lista y la señal, cada una con una
  frase. Ninguna señal es un veredicto.
- **No corta nada por su cuenta.** Ningún bloqueo ancho (categoría, toda la casa, Modo Vigilante)
  antes de 24 horas observando un dispositivo; un nombre concreto en un aparato concreto se puede
  bloquear desde el primer minuto, con confirmación mientras lleve menos de un día mirando. Ninguno
  sin decisión del usuario, ninguno sin "desbloquear" visible en la misma fila.
- **No corta actualizaciones, hora, resolutores, mensajería ni videollamada reconocidas** aunque
  una regla por categoría lo pida. Una regla explícita por dominio sobre uno de estos pide
  confirmación: "Esto puede dejar sin actualizaciones / sin mensajes a este dispositivo."
- **No es control parental.** No ve contenido, no ve apps, no muestra el detalle de un dispositivo
  sin permiso de su dueño.

## Lo que no envía

- **No contacta ningún servidor nuestro.** Nunca. No hay telemetría, no hay cuenta, no hay
  "comprobación periódica".
- **Solo sale del equipo lo que el usuario provoca**: activar la licencia (una llamada) y la
  comprobación periódica de la suscripción. No hay más: las listas van dentro del programa, con su
  fecha, y las versiones nuevas las descarga e instala el usuario desde la web. Cada conexión aparece
  en `/sabe-de-ti` con host, fecha y bytes.
- **No envía el informe semanal ni la tarjeta.** El botón "Compartir" abre la app del usuario
  (WhatsApp por enlace); Guardiana no envía nada.

## Lo que no promete

- **No protege "el teléfono".** Protege "en la Wi‑Fi de casa, por nombre de dominio".
- **En Windows y en el Mac, mientras el servicio está parado, no mira.** Desde el 28 sep 2026
  Guardiana es el único DNS de Windows, por IPv4 y por IPv6, y desde la 1.0.2 también del Mac:
  con el antiguo de secundario, el sistema y los navegadores se iban por él (Edge y Chrome,
  además, cifrado hacia Cloudflare o Google) y no se veía nada. Para que nadie se quede
  sin internet, cuando el servicio se para el equipo recupera su DNS de antes hasta que vuelve a
  arrancar; esas consultas no se anotan. `guardiana verify` hace una consulta de verdad y dice si
  llega a Guardiana.
- **No ve lo que un programa resuelve por su cuenta.** Un navegador con DNS cifrado elegido a mano,
  una VPN o un antivirus con su propio DNS no preguntan al sistema. Se anota como señal
  («evasión de DNS») cuando se detecta, pero lo que viaja por ahí no se ve.
- **No garantiza aislamiento fuerte entre dispositivos de la misma Wi‑Fi.** Cada dispositivo ve su
  detalle por su IP; en una red doméstica se asume que nadie suplanta la IP de otro.
- **En un Windows con varias cuentas, el extracto no es privado entre ellas.** La carpeta de datos
  (`C:\ProgramData\Guardiana`) se cierra al arrancar: se quita la herencia y solo quedan SYSTEM,
  Administradores y **la cuenta que esté en la consola en ese momento**, con permiso de modificar,
  para que el panel y `guardiana verify` funcionen sin pedir elevación. En un equipo de una sola
  persona eso es exactamente «solo el usuario». En un equipo familiar con varias cuentas, cada
  cuenta que haya usado el computador acaba con ese permiso, así que **una puede leer el extracto
  y la llave del panel de las demás**. Es el precio de un solo servicio para todo el equipo, y se
  dice en vez de esconderse. Quien necesite separación de verdad: una cuenta por persona en
  equipos distintos, o esperar a que la 1.1 guarde un extracto por cuenta.
- **No impide manipulación por quien tenga administrador en el computador.** La cadena de hashes
  detecta alteraciones a posteriori; no las evita.
- **No cifra el reenvío al resolutor de arriba.** Sin DoH/DoT propio ni DNSSEC en 1.0. El resolutor
  de arriba y el proveedor de internet ven las consultas reenviadas. Se muestra cuál es el resolutor.
- **No cifra el panel dentro de la Wi‑Fi de casa.** En el propio PC el panel va por `http://127.0.0.1`, que no sale del equipo y el navegador trata como seguro. En Modo Hogar, el teléfono abre `http://<IP del PC>:7443` sin cifrar: quien pueda leer tu Wi‑Fi (tiene la contraseña de la red) podría ver nombres de servicios y la llave de sesión del panel. No hay certificados válidos para direcciones de casa y uno autofirmado haría saltar avisos rojos en cada teléfono; es el mismo trato que hacen routers e impresoras. La Wi‑Fi de casa es el perímetro. Un certificado propio instalado al conectar el teléfono por QR queda para después de la 1.0.

- **El Modo Vigilante vale para todo el aparato, no solo para el agente.** GUARDIANA ve nombres, no
  programas: no puede saber si una consulta la pidió el agente o el navegador. En un equipo donde
  además trabajas, encenderlo corta también tus webs. Medido en el equipo de pruebas: con un alcance
  de dos servicios declarados, en 24 horas se habrían cortado 476 nombres distintos, entre ellos
  `claude.ai`, `www.google.com` y `chatgpt.com`. Por eso el panel avisa antes de encenderlo, con esa
  cuenta hecha sobre las últimas 24 horas del propio equipo y algunos de los nombres que caerían.
- **El Modo Vigilante corta nombres, no acciones.** Cuando está encendido, lo que no está en el
  alcance que declaraste no se resuelve, así que la conexión no llega a empezar. Eso es todo lo que
  hace, y no es poco, pero conviene saber lo que **no** es: no impide que un agente borre, escriba o
  lea archivos de tu disco (para eso no pregunta ningún nombre); no deshace lo que ya se envió antes
  de encenderlo; y no alcanza a un agente que ya tenga la dirección guardada, que use su propio DNS
  cifrado o que salga por una VPN. Tampoco corta nada en un aparato con menos de 24 horas
  observadas, ni las actualizaciones del sistema, la hora, los resolutores y la mensajería
  reconocida, que nunca cuentan como «fuera».
- **Un pase temporal deja pasar todo mientras dura.** Está para quien manda un trabajo largo y se
  va; mientras corre, nada se corta, aunque todo se sigue anotando. Caduca solo, con un máximo de
  24 horas, y entonces el corte vuelve.

- **La prueba de 7 días vive en tu equipo** y se cuenta en dos sitios: tu extracto y una marca
  aparte que solo un administrador puede quitar —el registro en Windows, `/etc/guardiana` en Linux
  y en Mac—. Esa marca lleva la fecha en que empezaron los siete días, la última hora del reloj
  que vio (para que atrasar la fecha no alargue la prueba) y una copia de la licencia activada;
  la pantalla de licencia dice exactamente qué hay y dónde está. Así desinstalar, reinstalar o borrar el extracto no devuelve siete
  días nuevos. Sin urgencia falsa y sin marcas escondidas (decisión 187 bis).

## Lo que no incluye la 1.0

- Modo Hogar en macOS: el guardián del propio Mac sí entra en la 1.0, pero los teléfonos y la tele no pueden pasar por un Mac, porque eso necesita tocar el cortafuegos de macOS y no se publica sin probarlo en una máquina que no sea la de desarrollo.
- La aplicación de Mac no está notarizada por Apple: macOS avisa la primera vez y hay que abrirla con clic derecho. Está explicado en la página de instalar.
- Bloqueo por aplicación y sensores por proceso (2.0).
- Actualización automática: nunca, ni silenciosa ni con aviso. No existe la orden `guardiana update`
  ni ninguna comprobación de versión: el programa no sabe si hay una versión nueva. Se descarga de la
  web, se comprueba su huella contra el registro público y se instala encima.
- Cortafuegos de IA (2.0).
- Interfaz de escritorio nativa (egui): el panel en el navegador es la única interfaz.
- IPv6: el resolutor escucha en IPv4 (`127.0.0.1` y la IP LAN IPv4). Un dispositivo que use un
  servidor DNS IPv6 no pasa por Guardiana. Se dice en `/hogar`.
- Verificación de reproducibilidad por terceros, aviso de actualización y "qué app" en Windows:
  extras de la sección 12, solo si están probados en una máquina ajena el día 24.
