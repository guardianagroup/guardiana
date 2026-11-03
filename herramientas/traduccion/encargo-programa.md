# Encargo de traducción: el programa GUARDIANA (panel, Antivirus, VPN, ZERO), un idioma

Carpeta de trabajo: `/tmp/claude-0/-home-claude/d4be7cd4-a664-5b0e-9e6c-cc1d5a1846c1/scratchpad/traduccion/`.

## Entrada
- `trozos/01.es.json` … `trozos/21.es.json`: JSON plano `{"clave": "texto en español"}`. El español es el original.
- `trozos/NN.en.json`: las mismas claves en inglés. Úsalo solo como referencia cuando el español sea ambiguo.
- Glosario de la web para tu idioma (consistencia con lo ya publicado): `/home/claude/guardianagroup.com/orbita/static/i18n/<código>.json`,
  campo `"t"`: `{"texto español": "traducción"}`. Úsalo para los nombres de sección y los términos fijos (Órbita, Comunicaciones,
  Navegador, Antivirus, Ajustes, Volver, Tu PC…). No lo copies entero; búscale solo los términos que necesites (con un script).

## Salida
Para cada trozo `NN`, escribe `<código>/NN.json` con exactamente las mismas claves que `trozos/NN.es.json` y su traducción como
valor. Escríbelo SIEMPRE con un script de Python (`json.dump(..., ensure_ascii=False, indent=1)`), nunca a mano, para no romper el
JSON. Después de cada trozo, ejecuta `python3 comprueba.py <código> NN`. Tiene que decir `bien`; si no, corrige ese trozo.

## Reglas (del dueño, obligatorias)
- Traducción natural, precisa y profesional, como un buen producto de seguridad. Usa el tratamiento cercano (tú) donde el idioma
  lo admita sin sonar raro; si no, el registro estándar de software en ese idioma.
- «encriptado/encriptar» → el término común de «encrypted» en ese idioma, nunca uno vago tipo «codificado».
- No traduzcas nombres propios ni marcas: GUARDIANA, ZERO, SECURE, NÚCLEO, Windows, WireGuard, WireGuardNT, Microsoft Defender,
  Prebid, ML-KEM-768, Filtering Platform, DHCP, DNS, VPN, USB, PDF, IPv4, IPv6, hosts. «IA» → el término local de «AI».
- Deja intactos los marcadores `{algo}`, las etiquetas HTML, las URL y las rutas, el orden de las variables, los números y
  unidades, las flechas (→), los puntos medios (·), las comillas tal como estén y los espacios del principio y del final.
- No añadas ni quites información, no inventes cifras ni promesas. Nada de «auditada», «certificada», «anónima».
- Excepción: cuando un texto cita el nombre exacto de un menú o ajuste de otra empresa (Windows, Apple, Google…), usa el
  nombre real que esa empresa le da en tu idioma, aunque lleve una palabra que no usaríamos para lo nuestro: si no, la
  persona no lo encuentra.
- Árabe, persa, hebreo y urdu: texto normal (el programa pone la dirección); números en cifras occidentales.
- Variantes: chino simplificado (zh), chino tradicional de Taiwán (zh-tw), portugués de Portugal (pt-pt, no brasileño),
  noruego bokmål (nb), filipino (fil).
- Textos muy cortos (botones, etiquetas): breves y claros, al estilo de las interfaces en ese idioma.

## Forma de trabajar (para no agotar tu memoria)
- Un trozo cada vez, en orden: lee `trozos/NN.es.json` (y, si hace falta, el `.en.json`), escribe el script que crea
  `<código>/NN.json`, ejecuta `comprueba.py`, pasa al siguiente. No vuelvas a leer trozos terminados ni imprimas traducciones
  enteras en la consola.
- Si ya existen trozos en `<código>/` que pasan `comprueba.py`, no los repitas: sigue por el primero que falte.
- Al terminar los 21, ejecuta `python3 comprueba.py <código>` (todos) y responde solo: el idioma, «21/21 bien», y dudas reales (máximo
  5 líneas). No toques ningún repositorio git.
