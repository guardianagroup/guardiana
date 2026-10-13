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
