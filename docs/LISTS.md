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

## País y red del servidor (decisión 203)

Desde el 11 de octubre de 2026, cuando GUARDIANA reenvía una consulta, mira la primera dirección de
la respuesta en dos tablas que van dentro del programa y anota en el extracto **solo lo que dicen**:
el país del servidor que respondió, su red (ASN), si es anycast o de la red local, y qué base lo dijo
(`dbip-2026-10`). **La dirección no se guarda**, y **no se consulta ningún servicio de fuera**: eso
le contaría a un tercero por dónde navega la casa. Una fila guarda el país que daba la base del
programa que la escribió; si la base cambia, las filas viejas siguen diciendo qué base usaron.

| Base | Qué da | Licencia | Atribución | Cómo entra |
|---|---|---|---|---|
| DB-IP «IP to Country Lite» | país (código ISO de dos letras) de cada rango de direcciones, IPv4 e IPv6 | [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/) | «IP Geolocation by DB-IP», con enlace a https://db-ip.com en las páginas que enseñan el dato (lo piden ellos) | `build/red-ip.py descargar AAAA-MM` baja `https://download.db-ip.com/free/dbip-country-lite-AAAA-MM.csv.gz` y lo convierte en `ip-pais.bin` |
| DB-IP «IP to ASN Lite» | red (sistema autónomo: número y organización) de cada rango | CC BY 4.0 | la misma | el mismo guion, `dbip-asn-lite-AAAA-MM.csv.gz` → `ip-red.bin` |

`crates/lists/data/red-ip.json` dice de qué mes es la base, la huella SHA-256 de lo que se bajó y de
lo que se generó, y cuántos rangos hay. El formato binario (rangos ordenados que se buscan por
búsqueda binaria; IPv6 con detalle de /64) está escrito en la cabecera de `build/red-ip.py`. CC BY 4.0
es compatible con distribuirla dentro de un programa GPL-3.0; la FSF la cuenta entre las licencias
libres compatibles con la GPLv3. No es una dependencia de Rust, así que `deny.toml` no cambia.

**Mientras `red-ip.json` diga `"muestra": true`**, lo que va en el programa es una muestra de pruebas
escrita a mano (`build/red-ip-muestra/`), no datos de DB-IP: la red de desarrollo del 10 de octubre
de 2026 no dejó bajar la base. El panel lo dice en cada sitio donde enseña un país o una red, y
`build/publicar.py codigo --empujar` y `build/release.sh` se niegan a seguir.

Otra base mirada y descartada: iptoasn.com (dominio público, PDDL, se actualiza cada hora). Su país
va por rango sin decir de dónde sale, y para «dónde está el servidor» hace falta una base de
geolocalización, que es lo que es DB-IP. Usar las dos tablas de DB-IP deja una sola fuente y una sola
fecha que citar.

Lo que el país **no** dice, y el panel no lo dice tampoco: dónde acaban los datos (un servidor de
entrada los reenvía), ni el país de la empresa dueña del nombre (eso es `empresas.txt`).

`anycast.txt` (nuestra) marca las redes y direcciones que su propio operador dice que son anycast
(Cloudflare, Google Public DNS, Quad9), con la fuente al lado. Ahí una base de IP a país no sirve: la
misma dirección contesta desde muchos países. El panel dice «red de reparto: el servidor suele estar
cerca de ti», y no las cuenta en ningún país. Las direcciones
privadas, locales o reservadas no tienen país.

## Reglas de bloqueo de GUARDIANA ZERO (desde ZERO 1.0.15)

ZERO, el navegador, aplica además las mismas reglas de red que usan Brave y uBlock Origin, y oculta
los huecos de anuncio con sus reglas cosméticas (el responsable, 13 oct 2026: «al nivel de Brave o
más»). Van dentro del programa, en `crates/zero/data/filtros/`, copiadas el día de cada versión desde
[uBlockOrigin/uAssets](https://github.com/uBlockOrigin/uAssets) (que trae también EasyList y URLhaus)
sin comentarios ni reglas que no sean de red; cada archivo dice en su cabecera de dónde sale, el
commit y la licencia. ZERO no las descarga nunca. Solo se aplican con «Cortar rastreadores» activado.

| Lista | Motivo en ZERO | Licencia | Atribución |
|---|---|---|---|
| EasyList | `lista_anuncios` | GPL-3.0-or-later / CC BY-SA 3.0 | The EasyList authors, https://easylist.to |
| EasyPrivacy | `lista_rastreo` | GPL-3.0-or-later / CC BY-SA 3.0 | The EasyList authors, https://easylist.to |
| uBlock filters, Privacy, Unbreak | `lista_anuncios` / `lista_rastreo` | GPL-3.0 | uBlock Origin, https://github.com/uBlockOrigin/uAssets |
| uBlock filters – Badware risks | `peligro` | GPL-3.0 | uBlock Origin |
| Online Malicious URL Blocklist (URLhaus) | `peligro` | CC0 / MIT | malware-filter, https://gitlab.com/malware-filter/urlhaus-filter, datos de https://urlhaus.abuse.ch |
| Reglas cosméticas de EasyList y uBlock filters (`cosmeticos.txt`) | ocultan huecos | las de cada lista | las de cada lista |

El motor (`crates/zero/src/filtros.rs`) entiende la sintaxis de Adblock Plus con las extensiones de
uBlock que importan para la red (`||`, `|`, `^`, `*`, `@@`, tipos, `third-party`, `domain=`, `to=`,
`important`, `badfilter`, `popup`). Una regla con una opción que no entiende (o una expresión
regular) se deja fuera en vez de adivinarla. Nunca se bloquea la página que la persona abre.

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
| `empresas.txt` | etiqueta (no categoría) | Quién es dueño de cada nombre, con su país y su ciudad cuando están comprobados (decisión 61), el grupo del que es filial o marca cuando lo es y está publicado (Slack → Salesforce; decisión 203, con la fuente encima de cada uno) y sus redes (`AS32934` en Meta), comprobadas en PeeringDB. |
| `anycast.txt` | etiqueta del servidor | Redes y direcciones anycast según su propio operador, con la fuente (decisión 203). |
| `ia.txt`, `entrega.txt` | etiqueta | El servicio de IA al que pertenece un nombre (decisión 62) y las redes de reparto que son carretera y no destino (decisión 148). |
| `corredores.txt` | etiqueta «compra y venta de datos» | Las empresas inscritas en el registro público de corredores de datos de California (CPPA): su nombre, su país, lo que ellas mismas marcaron (ubicación exacta, menores, a quién vendieron) y la página que declararon para pedir que borren tus datos. Copia fechada del CSV del registro, con su huella en la cabecera, más los nombres de rastreo de cada inscrita comentados uno a uno (decisión 194). No cambia la categoría ni corta nada: dice qué es la dueña del nombre, y la persona decide. |
| `sectores.txt` | etiqueta de sector | A qué se dedica una web conocida (deporte, noticias, banca, compras, red social, gobierno…): unas 560 webs grandes de Colombia, España, América Latina, Brasil, Portugal y Estados Unidos, comprobadas por su propia web. El panel lo dice en lugar de «sin clasificar» para la dirección principal de esa web; un subnombre que dice lo que es (`cdn.`, `login.`) sigue diciendo eso. No cambia la categoría ni corta nada (9 oct 2026). |

Cualquiera puede proponer cambios a estas listas con un cambio en el repositorio.

Y una lista de otros, para leer los nombres: `public_suffix_list.dat`, la Public Suffix List de Mozilla
(MPL-2.0), en `crates/lists/data/` desde el 11 de octubre de 2026 (antes estaba en `crates/zero/data/`;
la fecha, el commit y la huella están en `crates/zero/data/LEEME.md`). Dice dónde termina el sitio en
un nombre (`tracker.co.uk`, no `co.uk`). La usan GUARDIANA ZERO y, desde la decisión 205, el resolutor,
para saber si un alias de la respuesta es de otro sitio (un nombre disfrazado). No da ninguna categoría.

## Prioridad cuando varias listas coinciden

Primero gana la entrada más específica: `www.ejemplo.com` antes que `ejemplo.com`.
Para el mismo nombre en varias listas: `esperado` > `telemetria` > `rastreador` > `publicidad`.
Sin coincidencia: `desconocido`. Las actualizaciones del sistema no son una fuga aunque aparezcan en otra lista.

## Formatos que entiende `crates/lists`

- Adblock Plus, solo reglas `||dominio^` (EasyPrivacy, AdGuard DNS filter).
- hosts: `127.0.0.1 dominio` o `0.0.0.0 dominio` (Peter Lowe, StevenBlack).
- Lista simple de GUARDIANA: un dominio por línea, comentarios con `#`, secciones con `@`.
