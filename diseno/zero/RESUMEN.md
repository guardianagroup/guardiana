# GUARDIANA · Resumen para continuar el proyecto

Pega este texto en el chat nuevo y adjunta `zero-metamorfosis.html`: es el último diseño funcionando.

## Qué es el proyecto
GUARDIANA es una suite de privacidad: web guardianagroup.com y el programa «Órbita».
- **Órbita** tiene un planeta con 5 puertas. Todas están marcadas «En construcción», menos ZERO, que es en la que estamos.
- **ZERO** es el navegador de Órbita. Bloquea rastreadores, publicidad y huellas digitales, al nivel de DuckDuckGo y Brave. Cada bloqueo se llama «corte».
- El código está en GitHub: `guardianagroup/guardiana`, rama `diseno/orbita`. En el panel del programa están `crates/panel/static/orbita.js` y `orbita.css`.
- Hay tres commits locales sin subir. El último es «Órbita: five doors, ZERO engine fan and layouts, clean ZERO with no planet behind». No se suben hasta que yo lo diga.

## Reglas fijas (obligatorias)
- No mostrar nunca el nombre del fundador.
- No cambiar el programa publicado hasta que el nuevo esté terminado entero.
- No pegar código en el chat: entregar archivos o páginas.
- No inventar cifras: ni latencias, ni FPS, ni contadores falsos. Los datos de demostración van rotulados como demostración.
- No prometer nunca una protección absoluta.
- No usar marcas reales de terceros en las maquetas. Los buscadores reales sí pueden aparecer en el abanico.
- No subir nada a GitHub hasta que yo lo diga.
- No publicar la web sin preguntar «¿publico?» y recibir un «sí».
- Colores:
  - Negro `#06080C`
  - Acero `#9CC3E4`
  - Blanco frío `#F4F6F8`
  - Rojo `#E07A6C`, solo para los cortes
- Forma de trabajar: siempre en modo agencia. Cualquier cambio de diseño se consulta con ChatGPT, Gemini y Grok, y se combinan sus mejores ideas («en equipo somos más fuertes»).
- Ambición: hay muchos navegadores; ZERO tiene que ser la referencia de todos.
- Nivel de exigencia: «máximo nivel de agencia», como si fuera un encargo de Apple o Samsung por el navegador del año. Calidad ultrarrealista. Tiene que sentirse vivo.

## Diseño de ZERO decidido
- **Al abrir:** pantalla grande. La barra de búsqueda de cristal («LENTE») está en el centro exacto de la pantalla. No hay planeta ni nada detrás.
- **Sin espejo:** nada de reflejos ni suelo espejo, ni arriba ni abajo. Decisión del cliente.
- **Al buscar (Enter):** la web ocupa la pantalla. A la derecha queda un panel de 300 px con los cortes: empresa, país y motivo. La barra sube arriba.
- **Modo máximo:** la web al 100 %. La barra aparece arriba al acercar el ratón y hay una píldora con el número de cortes.
- **Búsqueda por voz:** micrófono que analiza la voz en el propio equipo; no la envía.
- **Abanico de buscadores reales**, agrupados en Privados, Generales, Por país e IA.

## La pieza viva: NÁCAR (elegida por el cliente)
Es un organismo de luz inspirado en los ctenóforos marinos, diseñado por ChatGPT en el torneo de agencias:
- Huevo perlado de 36.000 partículas, con 24 costillas y 12 hileras de cilios.
- Iris interior que sigue al cursor, filamentos de luz y brillo (bloom). Está hecho en WebGL 1, sin librerías.

Mejoras ya añadidas por Claude:
- **Ondas de luz nacarada** que recorren los cilios (tonos fríos, nunca rojo).
- **Rayos de luz** que salen del organismo.
- **Un núcleo que late** dos veces cada 5 segundos.
- **Desenfoque de profundidad**, idea de Gemini.
- **Parpadeo del iris** cada 17 segundos, idea de Grok.
- **Arreglo de un fallo** del código original: los filamentos no seguían al organismo cuando pasaba al panel.

## En qué estamos: la ENTRADA de cine (lo pendiente de decidir)
Idea del cliente: la bola o red neuronal llega, piensa y **se transforma en la barra de búsqueda**. Después, la barra se queda **brillante en los lados**. Tiene que entenderse que «un cerebro o red neuronal se ha convertido en buscador».

Lo que falta decidir es la puesta en escena de cómo llega. Ya hay tres versiones construidas y funcionando, en el archivo adjunto:
1. **Metamorfosis · punto:** un punto late, de él sale la red y crece. Piensa (impulsos de luz y dos latidos fuertes) y se aplana en el contorno de la barra. El cristal se condensa y los lados quedan brillando.
2. **Metamorfosis · desde abajo:** aparece una línea de horizonte, la red sube desde debajo y luego se convierte en la barra.
3. **Metamorfosis · conexión:** partículas sueltas por toda la pantalla, como neuronas, se conectan y forman la red, que después se convierte en la barra.

Decisión del cliente: **trabajamos con Metamorfosis**. Las demás entradas (Eclosión, Iris, Horizonte, División y Espejo) quedan descartadas.

Última versión de Metamorfosis («punto», unos 7,5 s), pulida en modo agencia en la ronda 5 con las ideas de ChatGPT y Gemini:
- **0–0,9 s:** negro puro. Aparece un punto que late dos veces. Ya no hay ondas circulares ni atmósfera azul: lo quitaron ChatGPT y Gemini.
- **0,9–2,6 s:** el esqueleto de la red se dibuja primero y después se llena de partículas.
- **2,2–3,3 s, «El primer corte» (idea de Gemini):** mientras la red piensa, un filamento intenta escaparse por la izquierda y se tiñe de rojo #E07A6C. En el segundo latido la red lo corta en seco y se apaga. Aporta el contraste de color y cuenta que el organismo existe para cortar amenazas.
- **3,35–4,8 s, la transformación (idea de ChatGPT):** sin destello que la tape, se ve cómo el cerebro se estira hasta el contorno de la barra, y frena al final para transmitir peso. Hay una onda horizontal suave.
- **4,3–5,6 s, el cristal:** se condensa desde el desenfoque. Es casi opaco, con un bisel blanco fino arriba, una sombra inferior y dos reflejos finos que se deslizan y se detienen. Los lados quedan brillando.
- **5,6–5,85 s:** un instante de quietud total.
- **5,85–6,9 s:** una corriente nacarada recorre el interior del cristal, de ella surge «ZERO» y suena el «ting» de cristal. Después la luz baja y aparece el campo de búsqueda con el cursor.
- **En reposo:** parte de la red sigue fluyendo dentro de la barra, tenue, y despierta con cada tecla. Los lados laten.

Ideas de la agencia aún por hacer:
- **Gemini:** que las letras de «ZERO» las formen las propias partículas, y que el iris quede vivo en el extremo derecho de la barra mirando al cursor.
- **ChatGPT:** reflejos tipo Fresnel en el cristal.
- **Grok:** en la ronda 5 devolvió una respuesta vacía.

**Sonido sintetizado en la propia página**, sin archivos y reducido a lo esencial: un latido doble, el chasquido del primer corte, un golpe grave en la transformación y un único «ting» de cristal. Los navegadores solo dejan sonar audio después de un clic; en el producto suena porque ZERO se abre con un clic desde Órbita. Hay un botón para silenciarlo.

## HECHO EN LA RONDA 7 (incluido en `zero-metamorfosis.html`)
**Selector de buscadores** (dirección de ChatGPT, coincidente con la de Claude):
- El botón muestra Google por defecto, con su monograma «G».
- Al pulsarlo se abre un panel de cristal tipo Raycast:
  - «Elige dónde buscar», con un buscador interno que filtra por nombre, país o enfoque.
  - Una fila de favoritos y recientes.
  - Cuatro grupos: Privacidad (DuckDuckGo, Brave Search, Startpage, Qwant, Mojeek, Swisscows, MetaGer, Ecosia, Kagi), Generales (Google, Bing, Yahoo), Por país (Yandex, Baidu, Naver, Daum, Seznam, Yahoo Japón, Sogou) e Inteligencia artificial (Perplexity, You.com, ChatGPT).
- Cada fila lleva monograma (sin logotipos), nombre, una nota honesta (enfoque y origen de los resultados), país de sede y una estrella para fijarlo. Hay un aviso: «Agrupados por enfoque: no es un ranking de privacidad».
- Teclado: ↑↓, Enter y Esc. Al elegir, una corriente de luz recorre el cristal, el foco vuelve a la barra y la elección se recuerda.
- Cada buscador tiene ya su dirección de búsqueda real en los datos, lista para el programa.

**Modo ULTRA** (navegación reforzada mediante Tor; el cliente lo llama «Ultra»; ChatGPT proponía «Circuito»):
- Un botón con dos anillos en la barra, junto al selector. Al activarlo, el cristal gana un doble filete luminoso, los bordes de la pantalla se oscurecen y se pasa a un buscador del grupo Privacidad. Al salir, vuelve el buscador anterior.
- Bajo la barra aparece una línea sutil: «◎ ULTRA · Ruta Tor · Sesión separada · Nivel: … · ¿Qué te protege?».
- Un panel explica «Lo que hace» (ruta Tor, DNS por Tor, sesión separada que se borra, sin fugas WebRTC ni conexiones directas, huella uniforme, fuera rastreadores, buscador privado) y «Lo que no puede hacer» (si inicias sesión te identifican; las descargas abiertas fuera de ZERO; el proveedor ve que usas Tor; ninguna herramienta garantiza el anonimato total).
- Tres niveles, como en el navegador Tor: Estándar, Más seguro y Máxima seguridad.
- Geometría (pedido del cliente): el panel tiene el mismo ancho que la barra, está alineado a sus bordes, es compacto y nunca se sale de la pantalla. El botón del buscador lleva un chevron dibujado que gira al abrir.
- El panel aclara que es un diseño en prueba, sin conexión Tor real en la demostración.
- **Aviso técnico clave de ChatGPT:** ZERO usa WebView2 en Windows, que no trae las defensas de Tor contra la huella digital y puede dejar datos en disco. ULTRA no debe anunciarse como modo anónimo hasta pasar pruebas independientes de fugas (IP, DNS, WebRTC, almacenamiento y huella). Si WebView2 no lo permite, habrá que usar un motor endurecido o abrir el navegador Tor. La parte de red se haría con Arti (Tor en Rust) y bloqueo total si Tor falla.

## (Encargo original de la ronda 7, ya hecho arriba): el selector de buscadores
Es el botón de la derecha de la barra; antes ponía «PRIVADOS ⌄». Lo que pide el cliente:
- **Por defecto: Google**, que es el más conocido. Al pulsar el botón se abre la lista con todos los principales, de más privados a menos.
- **Buscadores a incluir:**
  - Google, Bing, Yahoo.
  - DuckDuckGo, Brave Search, Startpage, Qwant, Ecosia, Mojeek, Swisscows, MetaGer, Kagi.
  - Yandex, Baidu, Naver, Seznam, Yahoo Japón.
  - Perplexity, You.com.
  - Opera no se incluye porque es un navegador, no un buscador; se le explicó al cliente.
- **Son muchos y tiene que verse muy bonito.** Se trabaja en modo agencia: se preguntó a ChatGPT y a Gemini cómo agruparlos, qué dato honesto mostrar de cada uno (sin inventar cifras ni puntuaciones; por ejemplo, índice propio, país de sede, enfoque), cómo representarlos sin sus logotipos (son marcas de terceros) y qué pasa al elegir uno.
- **Propuesta de Claude:**
  - Un panel tipo lanzador, con un buscador interno para filtrar, una fila de recientes y grupos («Más usados», «Centrados en privacidad», «Regionales», «Con IA»).
  - Cada buscador aparece con un monograma (su inicial) en lugar del logotipo, su nombre, una nota corta y el país de sede.
  - Se puede manejar con el teclado, y la elección se recuerda.

## (Encargo original de la ronda 7b, ya hecho arriba): botón de navegación ultraprivada
El cliente quiere en ZERO un botón para un modo de navegación ultraprivada, con un nombre por decidir («ultra privacidad» o similar). Tiene que ser la navegación más anónima que la agencia pueda diseñar.
- **Regla:** nunca prometer anonimato absoluto. Se explica qué protege y qué no; por ejemplo, si el usuario inicia sesión en una cuenta, esa web sabe quién es.
- **Capas realistas que propone Claude** (la referencia mundial es el navegador Tor):
  1. **Sesión efímera:** todo en memoria (sin historial, cookies, caché ni descargas recordadas) y borrado total al cerrar.
  2. **Red:** salida por la red Tor (en Rust existe Arti, la implementación oficial de Tor), lo que oculta la IP. Además, DNS cifrado sin fugas y bloqueo de fugas de IP por WebRTC.
  3. **Huella digital uniforme, como Tor:** mismo tamaño de ventana por bloques, zona horaria UTC, idioma genérico, agente de usuario común, lienzo, WebGL y audio bloqueados o neutralizados, y fuentes limitadas.
  4. **Bloqueo máximo:** rastreadores, terceros, cookies de terceros, almacenamiento particionado, parámetros de rastreo en las URL y referer recortado. Solo HTTPS.
  5. **Nivel «más seguro» opcional:** sin JavaScript en webs que no sean de confianza.
  6. **Buscador privado forzado** en este modo (DuckDuckGo, Brave Search, Startpage o Mojeek), nunca Google.
- **Diseño:** el botón en la barra, una transición visual al entrar (la interfaz cambia de tono para que se note que estás en ese modo) y un panel honesto de «qué te protege ahora».
- Se pide propuesta a ChatGPT y Gemini (nombre, diseño, capas técnicas).

## DECISIÓN FINAL DEL CLIENTE (la que vale)
Se descartan la Luna y la Tierra. **Nos quedamos con la red neuronal: aparece y se transforma en la barra («Metamorfosis · punto»)**, con las mejoras de la agencia: la transformación visible, el cristal real, la quietud, la corriente que escribe «ZERO» y la red viva dentro de la barra. Es la entrada principal. **El cliente pidió quitar el filamento rojo del «primer corte»: ya no está.** Las variantes del planeta quedan solo como historial.

## Historial: la entrada con el PLANETA (descartada) (idea del cliente, ronda 6)
El cliente propuso abrir con la Tierra a todo color, viva, que se convierte en polvo o arena cósmica y de ahí nace el navegador. Mensaje: «toda la información del mundo está ahí».
- Se usa la textura real de la Tierra que ya tiene Órbita (`crates/panel/static/tierra/dia.jpg`). Va en `zero-paquete/tierra/dia.jpg` junto al HTML; el archivo HTML necesita esa carpeta al lado para ver la Tierra.

**ÚLTIMO CAMBIO DEL CLIENTE: la Tierra se sustituye por la LUNA**, para no repetir siempre la Tierra.
- Se usa la textura real de la NASA (Lunar Reconnaissance Orbiter, LROC, dominio público), que va en `zero-paquete/luna/luna.jpg`.
- La Luna aparece plateada, sin atmósfera, mostrando su cara visible.
- Se desmorona de abajo arriba en arena plateada que cae y forma la barra.
- Las tres entradas del planeta se llaman ahora «Luna · …».

**DECISIÓN DEL CLIENTE (la más reciente): «Planeta · cae y forma la barra», sin bola intermedia.** Dura unos 8,5 s:
- La Tierra aparece a todo color y gira.
- Se va desmoronando poco a poco, de abajo arriba, en granos con los colores del planeta.
- Los granos caen con gravedad lenta y, al llegar, se posan en el contorno de la barra, que se va dibujando sola.
- Al posarse, el color pasa a acero.
- Cuando el planeta ha caído entero, el cristal se condensa, los lados brillan, hay un instante de quietud, la corriente de luz escribe «ZERO» y aparece el cursor.
- Es la primera entrada del selector. Las demás quedan como alternativas.

Tres versiones construidas:
1. **Planeta · caída (agencia)**, recomendada por ChatGPT, unos 10 s:
   - La Tierra gira a todo color.
   - Se deshace en granos de luz que caen como arena cósmica.
   - Una parte se detiene en el aire y el resto sube, desafiando la gravedad, y se organiza en la red NÁCAR.
   - Los colores de la Tierra pasan a acero.
   - Primer corte rojo, transformación en barra, cristal y «ZERO».
2. **Planeta · polvo:** la Tierra se convierte en una nube de polvo cósmico de colores que llena la pantalla y se ordena en la red.
3. **Planeta · arena**, la idea literal del cliente: la Tierra se vierte como un reloj de arena y, al posarse, forma directamente la barra (sin red intermedia).

Debate abierto en la agencia:
- **ChatGPT** avisa de que el filamento rojo del «primer corte» solo debería salir si hay un corte real; en demostración, rotulado como simulado.
- **Gemini** lo defiende como recurso narrativo.
- Lo decide el cliente.
- La respuesta de Gemini a la ronda 6 estaba pendiente.

Opción de final: «solo barra viva», con la red convertida del todo en el borde luminoso de la barra, o «barra + red encima», con parte de la red volviendo a formar el organismo sobre la barra.

Detalles comunes de la entrada:
- Dura unos 6 segundos y lleva bandas negras de cine.
- Se salta con cualquier tecla o clic.
- En el producto final se verá solo la primera vez y nunca con movimiento reducido.

## La agencia (otros asistentes que se usan como equipo de diseño)
- **ChatGPT (GPT-6, razonamiento «Alta»):** el mejor en interfaz y guiones. Diseñó NÁCAR y la entrada «Espejo», que se descartó al quitar el espejo.
- **Gemini (4 Argon, razonamiento «Alto»):** ideas muy ambiciosas. Propuso el registro forense del panel (agrupar los cortes por empresa) y unos filamentos que caen del organismo y rozan la barra; esto último aún no está hecho. Antes inventaba cifras; hay que vigilarlo.
- **Grok (Expert):** flojo en código, a veces aporta alguna idea suelta.
- **Kling (vídeo):** disponible, pero aún no usado. Gemini preparó prompts para un vídeo de presentación.

## Pendientes
1. Elegir la llegada de la metamorfosis (punto, desde abajo o conexión; la favorita por ahora es «punto») y el final (solo barra o barra + red). Probarla en un equipo real con sonido y pulir los tiempos.
2. Decidir si añadir los filamentos de Gemini.
2b. **NUEVO, pedido por el cliente:** la zona de la derecha del navegador, donde se abren los otros «subnavegadores» o servicios (por ejemplo Drive), se ve desordenada y con poco diseño. Hay que rediseñarla en modo agencia (ChatGPT, Gemini, Grok y Claude) para que quede ordenada y a la altura del resto. Pedir al cliente una captura de esa zona para empezar.
3. Web: ya está quitado el aviso «¿Ver esta página en español? Sí / No, gracias». Ahora el idioma se elige solo según el del navegador. Falta publicarlo; preguntar «¿publico?».
4. Llevar el diseño final de ZERO al programa real, en `orbita.js` y `orbita.css`, sin publicarlo hasta que esté todo terminado.
5. Más adelante:
   - Las otras puertas de Órbita, una por una.
   - La ubicación nativa.
   - Las capas de bloqueo de ZERO al nivel de DuckDuckGo y Brave: filtrado cosmético, scriptlets, rechazo automático de cookies, bloqueo de ventanas emergentes, cookies de terceros, particionado y anti-huella.
