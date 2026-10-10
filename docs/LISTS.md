# GUARDIANA — Listas, licencias y atribución

Brief §5. Las listas viven en `crates/lists/data/` y se distribuyen dentro del instalador con su
fecha (`MANIFEST.json`). El programa no las descarga nunca: van dentro del instalador, con su
fecha, y se renuevan con cada versión.
`build/fetch-lists.sh` es el script que una persona ejecuta antes de una versión para refrescarlas.

Ninguna lista es un veredicto. La categoría describe qué es el destino, no si hace daño.
Toda entrada coincide con el dominio y con sus subdominios.

## Listas abiertas de terceros que van en la 1.0

| Lista | Categoría en GUARDIANA | Licencia | Atribución | Qué se incluye |
|---|---|---|---|---|
| EasyPrivacy | `rastreador` | GPL-3.0-or-later, o CC BY-SA 3.0 a elección ([licencia](https://easylist.to/pages/licence.html)) | The EasyList authors, https://easylist.to | Solo las reglas de dominio completo (`\|\|dominio^` sin opciones), que son las únicas que un resolutor DNS puede aplicar. Se descartan reglas de rutas, cosméticas y con opciones. |
| Peter Lowe's Ad and tracking server list | `publicidad` | Sin licencia formal; el autor autoriza públicamente combinar y redistribuir la lista («Feel free to combine this list with yours … and put it up on the web») | Peter Lowe, https://pgl.yoyo.org/adservers/ | La lista en formato hosts, completa. |

GUARDIANA se publica bajo GPL-3.0-or-later, compatible con redistribuir EasyPrivacy.

## Listas del brief que NO van en la 1.0, y por qué

| Lista | Licencia | Motivo |
|---|---|---|
| Disconnect (`services.json`) | CC BY-NC-SA 4.0 | Prohíbe el uso comercial. GUARDIANA se vende por suscripción. Entra solo con permiso escrito de Disconnect, Inc. |
| DuckDuckGo Tracker Radar | CC BY-NC-SA 4.0 | Mismo motivo. Entra solo con permiso escrito de DuckDuckGo. |

Candidatas con licencia compatible, pendientes de decisión del responsable (no están en el brief):
AdGuard DNS filter (GPL-3.0, ~176 000 dominios, mezcla publicidad y rastreo) y StevenBlack/hosts
(MIT, ~80 000 dominios). Ambas se pueden añadir con el mismo analizador sin cambiar código.

## Listas mantenidas por GUARDIANA (abiertas, en este repositorio)

| Archivo | Categoría o señal | Qué contiene |
|---|---|---|
| `esperado.txt` | `esperado` | Actualizaciones de sistema (Windows, Apple, Debian, Ubuntu, Fedora, Arch, Flatpak, navegadores), certificados, comprobaciones de seguridad y de conexión del propio sistema, hora (NTP), mensajería y videollamada reconocidas, y el dominio propio de GUARDIANA (guardianagroup.com) con la pasarela de su licencia. Las secciones `@actualizaciones`, `@certificados`, `@seguridad`, `@hora`, `@mensajeria`, `@videollamada`, `@guardiana` deciden la frase de confirmación de la sección 6 del brief. |
| `telemetria.txt` | `telemetria` | Destinos que los propios fabricantes documentan como telemetría o diagnóstico (Microsoft, Apple, Mozilla, Google, Canonical, NVIDIA, Dell). |
| `evasion_dns.txt` | señal `evasion_dns` | Nombres de resolutores DoH/DoT conocidos. Consultarlos indica que un programa resuelve por su cuenta, fuera de la vista de GUARDIANA. Se anota; no se corta salvo regla del usuario. |
| `empresas.txt` | etiqueta (no categoría) | Quién es dueño de cada nombre, con su país y su ciudad cuando están comprobados (decisión 61). |
| `ia.txt`, `entrega.txt` | etiqueta | El servicio de IA al que pertenece un nombre (decisión 62) y las redes de reparto que son carretera y no destino (decisión 148). |
| `corredores.txt` | etiqueta «compra y venta de datos» | Las empresas inscritas en el registro público de corredores de datos de California (CPPA): su nombre, su país, lo que ellas mismas marcaron (ubicación exacta, menores, a quién vendieron) y la página que declararon para pedir que borren tus datos. Copia fechada del CSV del registro, con su huella en la cabecera, más los nombres de rastreo de cada inscrita comentados uno a uno (decisión 194). No cambia la categoría ni corta nada: dice qué es la dueña del nombre, y la persona decide. |
| `sectores.txt` | etiqueta de sector | A qué se dedica una web conocida (deporte, noticias, banca, compras, red social, gobierno…): unas 560 webs grandes de Colombia, España, América Latina, Brasil, Portugal y Estados Unidos, comprobadas por su propia web. El panel lo dice en lugar de «sin clasificar» para la dirección principal de esa web; un subnombre que dice lo que es (`cdn.`, `login.`) sigue diciendo eso. No cambia la categoría ni corta nada (9 oct 2026). |

Cualquiera puede proponer cambios a estas listas con un cambio en el repositorio.

## Prioridad cuando varias listas coinciden

Primero gana la entrada más específica: `www.ejemplo.com` antes que `ejemplo.com`.
Para el mismo nombre en varias listas: `esperado` > `telemetria` > `rastreador` > `publicidad`.
Sin coincidencia: `desconocido`. Las actualizaciones del sistema no son una fuga aunque aparezcan en otra lista.

## Formatos que entiende `crates/lists`

- Adblock Plus, solo reglas `||dominio^` (EasyPrivacy, AdGuard DNS filter).
- hosts: `127.0.0.1 dominio` o `0.0.0.0 dominio` (Peter Lowe, StevenBlack).
- Lista simple de GUARDIANA: un dominio por línea, comentarios con `#`, secciones con `@`.
