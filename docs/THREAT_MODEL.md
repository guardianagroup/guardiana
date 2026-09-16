# Guardiana — Modelo de amenazas

Versión 1.0 · 8 de septiembre de 2026 · derivado de `docs/BRIEF.md` y `docs/HOGAR.md`.
Este documento dice qué protege Guardiana, de quién, con qué medios, y qué queda fuera.
Lo que queda fuera está también en `docs/WHAT_IT_DOES_NOT_DO.md` con las palabras que usa la interfaz.

## 1 · Qué protege (activos)

| Activo | Dónde vive | Qué se anota de él |
|---|---|---|
| Las consultas DNS del computador | Resolutor local en `127.0.0.1:53` | Nombre, tipo, hora, dispositivo `self`, categoría, señales, veredicto. Nunca la respuesta. |
| Las consultas DNS de los dispositivos de la Wi‑Fi (Modo Hogar) | Resolutor local en la IP LAN | Lo mismo, por dispositivo (IP + MAC + nombre asignado). |
| El extracto (SQLite encadenado por hash) | Base de datos local, una por instalación | Cadena `row_hash = sha256(prev_hash ‖ campos)`; se verifica con `guardiana ledger --check`. |
| La configuración de DNS original del sistema | Tabla `settings` | Copia exacta antes de cambiar nada, para restaurarla al desinstalar. |
| Las reglas de corte y su historial | Tabla `rules` | Quién, cuándo, y si se deshizo. |
| La identidad del programa | Clave minisign, `ledger.jsonl`, Rekor | Cada binario con hash y firma publicados antes que la descarga. |
| La privacidad de cada persona de la casa | Tabla `devices`, `share_detail_with_home` | Detalle solo desde el propio dispositivo salvo que su dueño lo comparta. |

## 2 · De quién (adversarios) y qué puede cada uno

### A. Rastreadores, telemetría y publicidad dentro de apps y webs
Lo que Guardiana existe para ver. Capacidad: emitir consultas DNS a nombres conocidos, a intervalos
regulares, fuera de horas, en volumen. Mitigación: listas abiertas, categorías, las cinco señales
(brief §5), corte por DNS tras 24 h de observación y solo por decisión del usuario (brief §6).
Límite: si la app usa IP fija, DNS cifrado propio o VPN, Guardiana no la ve; lo anota como
"tráfico fuera de vista" cuando lo detecta (señal `evasion_dns`) y no lo esconde.

### B. Alguien dentro de la Wi‑Fi de casa
Capacidad: enviar consultas al puerto 53 abierto en LAN, abrir el panel en `http://<ip>:7443`,
falsificar su IP o su MAC. Mitigación:
- El panel de la casa (escritura, reglas por casa, detalle de otros) exige el token que solo
  existe en el archivo del usuario del computador. Sin token no hay escritura.
- Cada dispositivo solo lee su propio detalle, identificado por su IP de origen. Se asume que en
  una red doméstica la suplantación de IP es improbable; se documenta como límite, no se promete
  aislamiento fuerte entre dispositivos.
- Los puertos LAN solo existen con Modo Hogar encendido; apagarlo los cierra y `guardiana verify`
  lo demuestra. Comprobación de rango privado de la interfaz en cada arranque: nunca `0.0.0.0`
  hacia interfaces públicas.
- Regla de cortafuegos solo para el perfil de red privada.

### C. Una web maliciosa abierta en el navegador del computador (DNS rebinding, CSRF)
Capacidad: hacer que el navegador hable con `127.0.0.1:7443` en nombre del usuario. Mitigación:
comprobación estricta de la cabecera `Host` (solo `127.0.0.1`, `localhost`, la IP LAN configurada
y `comprobar.guardiana.hogar`), token de sesión en memoria que no se transmite por cookie, sin
JavaScript de terceros, sin fuentes externas, sin cookies.

### D. Quien intente entregar un binario manipulado (cadena de suministro, espejo falso)
Capacidad: sustituir el instalador o el binario. Mitigación (brief §10): compilación reproducible en
contenedor fijado por digest, `cargo build --locked`, SHA‑256 y firma minisign de cada binario, línea
en `ledger.jsonl` subida a Rekor antes de publicar la descarga, `guardiana verify` compara los
binarios instalados con la clave incrustada y con el registro. Dependencias auditadas con
`cargo audit` y `cargo deny` en cada commit. Clave privada solo en el equipo de release, nunca en CI.

### E. Nosotros mismos (el fabricante)
Capacidad que renunciamos: recibir datos. Mitigación: no hay servidor nuestro al que el programa
hable. Las únicas conexiones salientes son las que el usuario provoca (activar licencia, comprobar
versión, actualizar listas), cada una anotada en la tabla `outbound` con host, fecha y bytes, y
visible en `/sabe-de-ti`. Código abierto desde el primer commit para que cualquiera lo compruebe.

### F. Un miembro de la casa que quiera vigilar a otro
Capacidad: abrir el panel de la casa desde el computador. Mitigación: agregados por dispositivo;
el detalle solo se ve desde ese dispositivo o si su dueño activó "compartir mi detalle". Aviso al
abrir el panel desde un dispositivo nuevo: "Esta red usa Guardiana. Esto es lo que se anota de este
dispositivo." Guardiana no es control parental y no ve contenido ni apps.

### G. Software con privilegios en el propio computador (malware, otro usuario administrador)
Fuera de alcance. Quien tiene administrador puede cambiar el DNS, leer o alterar la base de datos y
detener el servicio. La cadena de hashes detecta alteraciones a posteriori si se conserva una
exportación anterior; no impide que ocurran.

### H. El resolutor de arriba (upstream) y el proveedor de internet
Ven todas las consultas que Guardiana reenvía. Guardiana no cifra el reenvío en 1.0 (sin DoH/DoT
propio, sin DNSSEC), y lo dice. El usuario elige el resolutor en el panel y ve cuál es.

## 3 · Supuestos

- La Wi‑Fi de casa es una red privada (rango RFC 1918 / ULA) detrás de un router.
- El usuario del computador tiene permisos de administrador para cambiar el DNS y abrir el puerto 53.
- El computador guardián tiene IP fija en la LAN cuando Modo Hogar está activo; si no, se avisa.
- Las listas abiertas son correctas en su mayoría; una categoría es una observación, no un veredicto.

## 4 · Fallos seguros

- Si el servicio cae, el sistema sigue resolviendo por el DNS secundario (el original). Al volver, se
  anota "Guardiana no estaba vigilando entre X e Y". Windows puede consultar ambos en paralelo: límite
  conocido, documentado.
- Sin licencia y sin prueba: Modo Hogar se apaga con aviso; el DNS del computador nunca se rompe.
- Desinstalar restaura exactamente la configuración de DNS anterior y quita la regla de cortafuegos.
- Nunca se corta nada antes de 24 h de observación, ni sin decisión del usuario, ni sin deshacer.

## 5 · Lo que este modelo no cubre

Ver `docs/WHAT_IT_DOES_NOT_DO.md`. En resumen: apps que esquivan el DNS, tráfico fuera de la Wi‑Fi
de casa, qué app pidió cada nombre en teléfonos, bytes y contenido, y cualquier atacante con
privilegios en el computador.

Revisión de seguridad con hallazgos y estado: `docs/SEGURIDAD.md`.
