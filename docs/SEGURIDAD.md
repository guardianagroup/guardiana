# Guardiana — Revisión de seguridad

Primera revisión: 10 de septiembre de 2026, sobre la versión 0.1.3, hecha por Claude Code a
petición del responsable. Se revisó el código con las herramientas automáticas (`cargo audit`,
`cargo deny`, `clippy` con avisos como errores, `unsafe` prohibido) y a mano en las zonas que
importan: el panel, el resolutor, el cambio de DNS, la licencia, el extracto y los permisos de
archivos. Cada hallazgo lleva su gravedad y su estado. La lista viva de límites conocidos está en
`docs/WHAT_IT_DOES_NOT_DO.md` y el modelo de amenazas en `docs/THREAT_MODEL.md`.

**Sobre "grado militar":** no usamos esa frase ni ninguna parecida. No existe un programa
"100 % seguro" y prometerlo sería mentir. Lo que sí hacemos: reducir la superficie de ataque,
revisar, corregir, publicar lo que no cubrimos y dejarnos comprobar. Para una beta de casas y un
lanzamiento público, lo honesto es esto y, más adelante, una auditoría externa independiente
con su informe publicado.

## Lo que se comprobó y está bien

| Zona | Qué se miró | Resultado |
|---|---|---|
| Panel: sesión | Token de 32 bytes aleatorios por instalación; comparación en tiempo constante; comprobación estricta de `Host` contra el rebinding de DNS; sin cookies. | Correcto. |
| Panel: quién puede escribir | Todas las rutas que cambian algo exigen el token, salvo las de "mi dispositivo", que solo actúan sobre el propio dispositivo identificado por su IP (límite documentado). | Correcto. |
| Panel: inyección en pantalla (XSS) | Todo lo que viene de la red (nombres de servicios consultados, nombres de dispositivos que ponen los teléfonos) pasa por el escape HTML; nombres limitados a 60 caracteres; política CSP estricta sin scripts en línea; `frame-ancestors 'none'`; `nosniff`. | Correcto. |
| Extracto (SQLite) | Todas las consultas van con parámetros; el único SQL compuesto solo añade condiciones fijas con `?`. | Correcto. |
| Órdenes al sistema (`netsh`, `powershell`, `resolvectl`, `nmcli`, `ip`) | Se lanzan con argumentos separados, nunca por una shell; lo que se interpola son índices numéricos y direcciones IP ya analizadas. | Correcto. |
| Resolutor | Solo atiende consultas estándar (`QUERY`); reenvío con la biblioteca hickory (puertos de origen aleatorios, identificadores aleatorios, EDNS); caché con tope; respuestas de bloqueo con TTL corto. | Correcto. Sin DNSSEC en 1.0 (documentado). |
| Puertos hacia la red | Solo se abren con Modo Hogar, solo en una dirección privada de la LAN, nunca en `0.0.0.0`; la regla del cortafuegos de Windows es solo para perfil privado; apagar Modo Hogar los cierra y `guardiana verify` lo demuestra. | Correcto. |
| Licencia | La respuesta de activación se analiza como JSON estricto y se guarda con una marca local; el archivo de licencia se verifica con minisign contra la clave incrustada; ninguna comprobación periódica. | Correcto. |
| Dependencias | `cargo audit` sin avisos; `cargo deny` limpio (licencias, orígenes, duplicados). | Correcto el 10 sep 2026. |

## Hallazgos y correcciones

| # | Hallazgo | Gravedad | Estado |
|---|---|---|---|
| 1 | **Windows:** la carpeta de datos (`C:\ProgramData\Guardiana`) heredaba permisos de lectura para todas las cuentas locales: cualquier usuario del PC podía leer el token del panel y el extracto. | Media (alta en un PC con varias cuentas) | **Corregido en 0.1.4:** el servicio quita la herencia, deja SYSTEM y Administradores y da permisos solo al usuario de la consola; se repite cada 10 minutos por si el usuario entra después del arranque. En Linux, modo 0700. |
| 2 | Los enlaces de exportar CSV/JSON llevaban el token de sesión en la dirección: quedaba en el historial y en la lista de descargas del navegador. | Media | **Corregido en 0.1.4:** la exportación va con el token en una cabecera y se guarda como archivo desde el navegador. |
| 3 | **Linux:** la unidad de systemd corría como root sin ningún cerco. | Media | **Corregido en 0.1.4:** `NoNewPrivileges`, `ProtectSystem`, `ProtectHome`, `PrivateTmp` y protecciones del núcleo. Probado en Ubuntu 24.04. **Revisado el 15 sep 2026 (decisión 101):** era `ProtectSystem=full`, que deja `/etc` de solo lectura; desde que el guardián pasa a ser el único resolutor el servicio tiene que **sustituir** `/etc/resolv.conf` —en Ubuntu es un enlace, y cambiarlo es borrar y crear dentro de `/etc`— y crear `/etc/systemd/resolved.conf.d/guardiana.conf`. Probado que `ReadWritePaths=/etc` **no** lo permite («Read-only file system»), porque `/etc` es el propio punto que `full` remonta, así que queda `ProtectSystem=yes`: `/usr`, `/boot` y `/efi` siguen de solo lectura y `/etc` deja de estarlo. Pendiente de compensarlo con un `CapabilityBoundingSet`, que hoy no existe y recortaría mucho más que esto. |
| 4 | **Windows:** el servicio corre como `LocalSystem`. Lo necesita para volver a aplicar el DNS si la red lo pierde. Un fallo de memoria en el resolutor tendría todos los privilegios. | Baja (código Rust sin `unsafe`, biblioteca DNS madura) | Aceptado en 1.0 y documentado. Pendiente: estudiar una cuenta de servicio con menos derechos. |
| 5 | En Modo Hogar, cada dispositivo ve su detalle por su IP. Quien suplante una IP en la Wi‑Fi de casa vería el detalle de otro. | Baja (red doméstica) | Aceptado y dicho en `WHAT_IT_DOES_NOT_DO.md`. |
| 6 | Sin límite de ritmo en el resolutor: un aparato de la casa podría inundar a Guardiana con consultas. | Baja (solo desde la LAN) | Pendiente: límite por origen en 1.x. |
| 7 | El comprobador `comprobar.guardiana.hogar` va por HTTP sin cifrar en la LAN. Solo dice "este dispositivo ya pasa por Guardiana". | Informativa | Aceptado: no lleva datos. |
| 8 | Sin DNSSEC ni cifrado hacia el resolutor de arriba. | Informativa | Documentado; el resolutor de arriba se muestra. |

## Qué queda para llamarlo "revisado de verdad"

1. Auditoría externa independiente antes o justo después del lanzamiento, con informe público.
2. Compilación reproducible verificada por una persona ajena (sección 12 del brief).
3. Firma de código y registro público con claves reales (hasta entonces, versiones de desarrollo).
4. Pruebas de robustez del resolutor con consultas malformadas (fuzzing) en 1.x.
