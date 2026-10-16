# Imágenes de la Tierra de «GUARDIANA por dentro»

Las sirve el propio panel, para que el globo funcione sin internet (brief §8).

| Archivo | Qué es | De dónde sale | Licencia |
|---|---|---|---|
| `dia.jpg` | La Tierra de día, 4096 × 2048, proyección equirrectangular, con relieve y fondo marino | NASA, *Blue Marble* (Visible Earth / Earth Observatory) | Dominio público (imagen de la NASA) |
| `luces.jpg` | Las luces de las ciudades de noche, solo las luces (en gris) | Sacada de la imagen nocturna de la NASA (*Earth at Night* / *Black Marble*): se quita el fondo azul y se dejan las luces | Dominio público (imagen de la NASA) |
| `agua.png` | Dónde hay agua (blanco) y dónde tierra (negro), 2048 × 1024 | Calculada a partir de `dia.jpg` (los píxeles azules oscuros del océano), sin otra fuente | Derivada de dominio público |

Las copias de partida vinieron del repositorio abierto `vasturiano/three-globe` (carpeta
`example/img`). Ese repositorio también trae un mapa de relieve y una máscara de agua cuyo origen no
dice; por eso no se usan aquí.

Los países del mapa (la rejilla de 1,5° de `por-dentro.js`) salen de *Natural Earth* 110 m (dominio
público), vía el paquete `world-atlas` 2.0.2.

La luz del globo es la del sol de cada momento: el punto subsolar se calcula en el navegador con la
declinación y la ecuación del tiempo (aproximación de pocos minutos de error).

## De cerca: las teselas (`t/`, 15 Oct 2026)

Al acercar el globo, la página carga solo las teselas de 45° que hay bajo la vista (8 columnas ×
4 filas, 2048 × 2048 cada una, con 8 píxeles de sangre de las vecinas para que no se vean las
juntas). Las corta `build/tierra.py` en CI (rama `tierra-*`) de las imágenes de la NASA a su
tamaño completo: *Blue Marble Next Generation* de julio con relieve y fondo marino (21 600 ×
10 800) y *Black Marble 2016* (13 500 × 6 750), ambas de dominio público. `t/tiles.json` dice de
dónde salió cada una y su huella.

## El globo entero, del mismo mes (16 Oct 2026)

`dia.jpg` y `luces.jpg` (4096 × 2048) se rehicieron a partir de las propias teselas (la parte
interior de cada una, sin la sangre, juntas y reducidas con Lanczos). Antes venían de otra copia
de la *Blue Marble* con nieve en Siberia, y al mover el globo se veía un recuadro nevado donde una
tesela de julio todavía no se había cargado. Ahora el globo lejano y el de cerca son la misma
imagen de julio.
