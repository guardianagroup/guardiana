# La bóveda: formato abierto y lector independiente

Documento del formato `guardiana-boveda/1` (decisiones 103 y 126). Está escrito para que cualquiera
pueda leer una bóveda **sin GUARDIANA**: con este texto, Argon2id, XChaCha20-Poly1305 y SHA-256
basta. El lector independiente `guardiana-lector` (`crates/vault-reader`, unas 250 líneas, sin código
del producto) es la prueba de que el documento es suficiente: un test crea una bóveda con el producto
y la abre con el lector.

Dos reglas que no son código sino promesas (decisión 103): **cancelar la suscripción nunca bloquea el
descifrado** (nada de la bóveda mira la licencia) y **el formato y el lector se publican desde el
primer día**, este día.

## Qué hay en el disco

```
<boveda>/
  boveda.json        cabecera: formato, id, fecha, parámetros de Argon2id y la clave maestra
                     envuelta dos veces (con la contraseña y con las 24 palabras)
  objetos/<id>.gdn   un archivo por objeto guardado
  registro.jsonl     el registro de accesos, encadenado
```

Nada del disco contiene la clave maestra en claro, ni el nombre de ningún archivo guardado: los
nombres viajan cifrados dentro de cada objeto. El registro no está cifrado: lleva horas, acciones e
identificadores, nunca nombres ni contenido.

## boveda.json

```json
{
  "formato": "guardiana-boveda/1",
  "id": "6f1c…(16 hex)",
  "creada": "2026-09-17T16:04:05Z",
  "kdf": { "memoria_kib": 65536, "pasadas": 3, "hilos": 1 },
  "llaves": [
    { "tipo": "contrasena", "sal": "<32 bytes hex>", "nonce": "<24 bytes hex>", "envuelta": "<48 bytes hex>" },
    { "tipo": "palabras",   "sal": "<32 bytes hex>", "nonce": "<24 bytes hex>", "envuelta": "<48 bytes hex>" }
  ]
}
```

- La **clave maestra** son 32 bytes aleatorios del sistema. Nunca se escribe en claro.
- Cada entrada de `llaves` es una manera de abrirla. Se deriva una clave de 32 bytes con
  **Argon2id v1.3** (memoria, pasadas e hilos de `kdf`; sal de la entrada) a partir del secreto:
  la contraseña maestra en UTF-8 (`contrasena`) o los 32 bytes de entropía de las 24 palabras
  (`palabras`). Con esa clave se abre `envuelta` con **XChaCha20-Poly1305** (nonce de 24 bytes) y
  datos asociados `guardiana-boveda/1 boveda <id> llave <tipo>`. El resultado son los 32 bytes de la
  clave maestra. Si la etiqueta no cuadra, la contraseña o las palabras están mal: no hay «casi».
- Todo campo binario va en hexadecimal minúsculo, para que el archivo sea JSON llano.

## Las 24 palabras

Son BIP-39 con la lista inglesa de 2048 palabras (`english.txt`, la del repositorio público de
BIP): 256 bits de entropía + 8 bits de suma de comprobación (el primer byte del SHA-256 de la
entropía), 264 bits en 24 grupos de 11, cada grupo el índice de una palabra. Cualquier herramienta
BIP-39 comprueba las palabras; la bóveda solo las usa para envolver su propia clave maestra.

**Trozos (3 de 5).** Los 32 bytes de entropía se parten con Shamir sobre GF(256) (polinomio
x⁸+x⁴+x³+x+1, el de AES): cada byte es el término independiente de un polinomio aleatorio de grado 2
y el trozo `x` guarda su valor en `x` (x = 1…5). Con tres trozos se interpola en 0; con dos no se sabe
nada. Texto de un trozo: `GDN-3DE5-<x>-<32 bytes hex>-<2 bytes hex>`, y los dos últimos bytes son el
principio del SHA-256 de `x ‖ datos`, para detectar una letra mal copiada.

## objetos/<id>.gdn

```
"GDNBOV1\n"          8 bytes fijos
u32 big-endian       longitud L de la cabecera JSON
L bytes              cabecera JSON
por cada trozo i de 0 a trozos−1:
  24 bytes           nonce
  min(trozo, resto)+16 bytes   texto cifrado + etiqueta de ese trozo del contenido
```

Cabecera:

```json
{
  "id": "<16 hex>",
  "llave": { "nonce": "<24 hex>", "envuelta": "<48 hex>" },
  "meta":  { "nonce": "<24 hex>", "cifrado": "<hex>" },
  "trozo": 1048576, "trozos": 3, "tamano": 2500000
}
```

- Cada objeto tiene **su propia clave** de 32 bytes, envuelta con la clave maestra
  (XChaCha20-Poly1305, datos asociados `guardiana-boveda/1 objeto <id> llave`).
- `meta` es un JSON cifrado con la clave del objeto (datos asociados `… objeto <id> meta`):
  `{"nombre": "dni.pdf", "guardado": "2026-09-17T16:05:00Z", "sha256": "<hex del contenido en claro>", "tamano": 2500000}`.
- El contenido va en trozos de `trozo` bytes en claro (1 MiB), cada uno cifrado con la clave del
  objeto y datos asociados `… objeto <id> trozo <i>/<trozos>`: un trozo movido, quitado o traído de
  otro objeto falla en la etiqueta. Un archivo vacío es un trozo vacío.
- Al descifrar se recalcula el SHA-256 del contenido y se compara con `meta.sha256`.

## registro.jsonl

Una línea JSON por acción:

```json
{"n":2,"hora":"2026-09-17T16:05:00Z","accion":"guardar","objeto":"<id>","detalle":"terminal · 2500000 bytes","anterior":"<64 hex>","hash":"<64 hex>"}
```

`hash = sha256(anterior ‖ campos)`, con cada campo (`n` en decimal, `hora`, `accion`, `objeto` o
vacío, `detalle`) precedido de su longitud en 4 bytes big-endian, igual que el extracto DNS. La
primera línea encadena con `sha256("guardiana-boveda/1\n" ‖ id de la bóveda)`. Acciones: `crear`,
`abrir`, `fallo` (contraseña o palabras incorrectas; no dice cuál), `guardar`, `leer`, `contrasena`,
`recuperar`, `trozos`, `capsula`. Borrar o cambiar una línea rompe la cadena y `guardiana boveda
registro` (o el lector) dice en cuál.

## La cápsula

`guardiana boveda capsula CARPETA` copia la bóveda entera, tal cual está cifrada, a
`CARPETA/guardiana-boveda-<id>-<fecha>/`, con su registro, y pone al lado el lector independiente
si está instalado junto al programa. Un disco externo con eso es el segundo nivel de la
recuperación (decisión 103): sin el equipo, con la contraseña o las 24 palabras, se abre.

## Lo que este formato no hace todavía (y se dice)

- No bloquea la memoria (`mlock`/`VirtualLock`) ni vigila el portapapeles: la versión de terminal
  suelta las claves al terminar (se borran de memoria al soltarse); la del panel, cuando llegue, lo
  hará por inactividad y al suspender.
- No hay segundo factor (passkeys) ni permisos a terceros: son fases posteriores.
- No hay puerta trasera, ni de soporte: quien pierda la contraseña **y** las 24 palabras **y** tres
  trozos pierde el contenido. Se dice antes de crear la bóveda.

## Cómo se usa (terminal)

```
guardiana boveda crear                      crea ~/.guardiana/boveda (Windows: %USERPROFILE%\Guardiana\boveda) y enseña las 24 palabras una vez
guardiana boveda guardar ARCHIVO            guarda una copia cifrada
guardiana boveda lista                      qué hay dentro
guardiana boveda sacar ID_O_NOMBRE [--a RUTA]
guardiana boveda registro                   cada apertura, con la cadena comprobada
guardiana boveda contrasena                 cambia la contraseña (las palabras no cambian)
guardiana boveda recuperar                  entra con las 24 palabras y pone contraseña nueva
guardiana boveda trozos                     parte las palabras en 5 trozos
guardiana boveda juntar                     3 trozos → las 24 palabras
guardiana boveda capsula CARPETA            copia a un disco externo, con el lector
guardiana-lector lista|sacar|registro CARPETA [--palabras]    lo mismo sin GUARDIANA
```
