# Visión profunda (1.0.45)

Diagnóstico → «Visión profunda» (`/vision`, API `/api/vision`). Solo Windows; en Linux y Mac la
página dice «No disponible todavía».

El responsable, 18 oct 2026: «precisión máxima de todos los datos hacia dónde van, vienen desde
cualquier país del mundo»; «la experiencia del cliente debe ser sentir el monitoreo constante…
para saber que todo está bien y nada está vigilando o extrayendo sus datos». Consenso del comité
(Claude, ChatGPT, Grok, Kimi).

## Qué mide y de dónde sale

| Qué | Fuente en Windows | Qué se guarda |
|---|---|---|
| Bytes subidos y bajados por programa y destino | `Microsoft-Windows-Kernel-Network`, sucesos 10/11 (TCP IPv4), 26/27 (TCP IPv6), 42/43 y 58/59 (UDP, también QUIC) — la misma sesión de la 1.0.21 | Por minuto en `trafico` (retención del detalle); por hora en `vision_resumen` (90 días, encadenado) |
| Reenvíos TCP | Mismo canal, sucesos 14/30 | Columna `reenviados`, **aparte**: nunca se suma a `enviados` |
| Conexiones | Mismo canal, sucesos 12/28 (abrir) y 15/31 (aceptar) | Columna `conexiones` |
| Conexiones a direcciones sin nombre visto (IP fija, DoH propio, QUIC a una dirección aprendida fuera) | Una medida cuyo destino no coincide con ninguna dirección que el resolutor de GUARDIANA dio en las últimas 6 h. Desde la 1.0.45 se recuerdan **todas** las direcciones de cada respuesta (hasta 17), no solo la primera, para no contar como «sin nombre» la segunda dirección de un nombre que sí se pidió | Bytes `sin_dns` por hora, programa y empresa |
| Empresa | Servicio de IA si el nombre es uno (`ia.txt`), si no grupo o empresa del nombre (`empresas.txt`), si no la empresa dueña de la red (`AS…`), si no el nombre de la red (DB-IP) | Solo la etiqueta, el país y la red; **nunca la dirección** (decisión 203) |
| Archivos abiertos (opcional, apagado de fábrica) | `Microsoft-Windows-Kernel-File`, sucesos 12 (crear/abrir), 15 (leer) y 14 (cerrar), filtrados por número en el núcleo | Solo el **nombre** del archivo (sin carpeta) de Documentos, Escritorio, Descargas, Imágenes (también bajo OneDrive), y solo dentro de un aviso; 24 h; fuera de toda exportación; se borran al apagar |
| Ajustes de Defender sobre su nube | `reg.exe query` de `SubmitSamplesConsent` y `SpynetReporting` (primero la directiva, después el ajuste propio) | Nada: se lee al abrir la página, como mucho cada 10 min |

## «Posible transferencia»

`guardiana_core::vision::Detector`. Para cada programa y empresa: desde un archivo abierto por ese
programa, lo que envió a esa empresa en los **2 minutos** siguientes (más 10 s de holgura, lo que
tarda una toma) debe llegar a **4 MiB** y ser una subida (enviado ≥ 2 × recibido). Se avisa una vez
por (programa, empresa); lo usado no se vuelve a usar. La frase dice siempre «Posible
transferencia», «leyó» solo si Windows contó lecturas y «abrió» si solo vio la apertura. Nunca
«robó» ni «subió tus archivos»: es una coincidencia en el tiempo, no una prueba de lo que viajó.

Lo que puede dar un aviso sin que haya nada raro: una copia de seguridad o una sincronización
(OneDrive, Dropbox) — que es exactamente lo que se quiere ver; un antivirus que analiza un archivo
y a la vez sube telemetría grande; una videollamada que comparte un documento. Lo que no lo da:
una descarga, poco volumen, el reparto del envío entre varias empresas.

## El registro encadenado

`vision_resumen`: una fila por hora, programa y empresa (máximo 60 por hora; el resto se suma en
una fila `@otras`, salvo las que tienen un aviso). `row_hash = sha256(prev_hash || campos)`, cada
campo como longitud de 4 bytes big-endian y su texto UTF-8, en este orden: hora, programa,
empresa, país, enviados, recibidos, reenviados, conexiones, sin_dns, transferencias. La primera
fila encadena del génesis del extracto; al podar (90 días), el ancla pasa a la última fila
quitada. La página enseña si la cadena cuadra. Ni direcciones ni nombres de archivo entran nunca.

## Coste

- La sesión de red ya existía (1.0.21): sus sucesos ya llegaban; los de reenvío y conexión se
  descartaban y ahora se cuentan. Coste añadido: una comparación y una suma por suceso.
- La sesión de archivos solo existe mientras la vigilancia está encendida. El filtro por número
  de suceso lo aplica el núcleo, así que al programa solo llegan abrir, leer y cerrar. Una lectura
  de un archivo que no se sigue se descarta antes de interpretar ningún campo. Estimación, **por
  medir en un PC real**: por debajo del 1 % de un núcleo con uso normal; con mucha lectura de
  disco (un análisis completo del antivirus, una compilación) pueden llegar decenas de miles de
  lecturas por segundo y el coste subir a unos pocos puntos de un núcleo mientras dure.
- Todo se suma en memoria y se escribe cada 10 s (por minuto) y cada hora (resumen). Memoria
  acotada: 4.096 archivos abiertos seguidos, 1.024 aperturas por programa, 512 programas.

## Fuera, a propósito

- **Nada de descifrar, ni proxy intermedio, ni certificado raíz.** Fuera de alcance por decisión.
- **Sin captura de paquetes**, así que sin SNI ni JA4. Y sin leer el certificado del servidor: en
  TLS 1.3 viaja cifrado.
- **Sin controlador WFP propio** (modo núcleo): todo es ETW en modo usuario.
- **Sin la tabla de conexiones de IP Helper** (`GetExtendedTcpTable`): los sucesos de conexión del
  canal de red dan lo mismo con su programa y su momento; la tabla solo añadiría las conexiones
  abiertas antes de arrancar el servicio, a cambio de llamar con `unsafe` cada pocos segundos.

## Lo que hay que comprobar en un PC con Windows

1. `guardiana apps --red` durante una descarga con pérdidas (Wi-Fi lejana): que salen sucesos
   14/30 con `size`, y que su tamaño **no** está también en los 10/26 (si lo estuviera, habría que
   restarlo de `enviados`).
2. Que los sucesos 12/28/15/31 traen `daddr`/`dport` del otro extremo y `PID` del programa.
3. Encender «Posibles transferencias», abrir un PDF de Documentos en un programa y subirlo (por
   ejemplo, adjuntarlo en el navegador a un correo): que llega el aviso con el nombre y la empresa.
4. Que los sucesos de `Microsoft-Windows-Kernel-File` traen `FileObject`, `FileName`,
   `CreateOptions` e `IOSize` con esos nombres, y que el `ProcessId` de la cabecera es el programa
   que abrió (no `System`).
5. Coste real de la sesión de archivos durante un análisis completo de Defender (Administrador de
   tareas, proceso `guardiana`).
6. Que `reg query` lee `SubmitSamplesConsent`/`SpynetReporting` como servicio (cuenta SYSTEM) con la
   protección contra alteraciones encendida.
7. Los nombres de proceso de las apps de IA (`ChatGPT.exe`, `claude.exe`, `Copilot.exe`,
   `M365Copilot.exe`…): Copilot en particular puede llamarse distinto según la versión.
8. Un programa con DNS propio (Firefox con DoH, por ejemplo) aparece con bytes «sin nombre visto».
