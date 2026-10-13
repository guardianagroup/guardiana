# Listas de reglas de ZERO (codificadas)

Estos archivos son EasyList, EasyPrivacy, las listas de uBlock Origin, URLhaus y las reglas
cosméticas, tal como dice `docs/LISTS.md`, **codificados**: cada byte se cambia por otro con una permutación fija (Fisher-Yates con un
xorshift64 de semilla `0x9E3779B97F4A7C15`, ver `crates/zero/src/filtros.rs`), que quita las
cadenas legibles y deja que el instalador las comprima igual.

No es para esconderlos: el código que los descodifica está aquí al lado y es público. Es porque
las listas de malware llevan escritas direcciones de malware real, y un antivirus que ve esas
cadenas dentro de un programa lo confunde con lo que bloquea (le pasó a Microsoft Defender con
ZERO 1.0.15 el 13 de octubre de 2026). Los antivirus guardan sus propias bases igual.

Para leerlos en claro: `python3 build/filtros.py --leer crates/zero/data/filtros/easylist.bin`.
Para renovarlos antes de una versión: `python3 build/filtros.py --renovar <copia de uAssets>`.
