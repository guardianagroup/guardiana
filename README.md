# Guardiana

*English version: [README.en.md](README.en.md).*

> **En 30 segundos.** Le diste red a una IA: GUARDIANA te dice con qué servicios habló —nunca el
> contenido— y te deja cortar. Las primeras 24 horas solo mira. Gratis, código abierto (GPL‑3.0),
> Windows y Linux. **Versión 1.0: 5 de octubre de 2026, 15:00 UTC.** Hasta ese día aquí no hay
> descarga: hay el código con el que se compila, y la huella de cada versión se publica antes que
> el archivo.
>
> Probarlo desde el código, sin tocar el DNS del equipo ni pedir permisos de administrador:
>
> ```bash
> cargo build --locked
> GUARDIANA_DATA=/tmp/guardiana ./target/debug/guardiana observe --listen 127.0.0.1:5335 --upstream 1.1.1.1
> dig -p 5335 @127.0.0.1 example.com     # en otra terminal
> GUARDIANA_DATA=/tmp/guardiana ./target/debug/guardiana ledger
> ```
>
> Eso levanta el guardián en un puerto sin privilegios, resuelve una consulta y enseña lo que anotó.


Programa para Windows y Linux que convierte el PC en el **guardián DNS de la casa**:
primero de sí mismo, después de los teléfonos, el televisor y todo lo que use la Wi‑Fi, sin
instalar nada en ellos. Ve a qué servicios intenta hablar cada dispositivo, lo clasifica, lo
explica en una frase, lo anota en un extracto encadenado por hash y corta solo lo que el usuario
decida.

**Todo ocurre en la casa.** Sin cuenta, sin servidor nuestro, cero telemetría. Las únicas
conexiones salientes son las que el usuario provoca (activar licencia, comprobar versión,
actualizar listas) y cada una queda anotada en el propio extracto.

- Lo que no hace, dicho con esas palabras: [docs/WHAT_IT_DOES_NOT_DO.md](docs/WHAT_IT_DOES_NOT_DO.md)
- Modelo de amenazas: [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md)
- Cómo comprobar que lo instalado es lo publicado: [docs/VERIFY.md](docs/VERIFY.md)
- Modo Hogar (teléfonos sin app): [docs/HOGAR.md](docs/HOGAR.md) y las guías de
  [router](docs/guias/router.md), [iPhone](docs/guias/iphone.md) y [Android](docs/guias/android.md)
- Listas que se usan y con qué licencia: [docs/LISTS.md](docs/LISTS.md)
- Guía de la beta cerrada: [docs/BETA.md](docs/BETA.md)
- Decisiones tomadas y registro de pruebas: se llevan en el repositorio de trabajo; lo que afecta a lo
  que promete el programa está en este repositorio (arriba) y en https://guardianagroup.com

## Instalar

| Sistema | Paquete | Cómo |
|---|---|---|
| Windows 10/11 (64 bits) | `guardiana-<versión>-windows-x64.msi` | Doble clic. Instala en Archivos de programa y registra el servicio. |
| Debian, Ubuntu y derivados | `guardiana_<versión>_amd64.deb` | `sudo apt install ./guardiana_<versión>_amd64.deb` |
| Otro Linux con systemd | `guardiana-<versión>-linux-x86_64.tar.gz` | Descomprimir y `sudo ./instalar.sh` |

Instalar **no cambia el DNS del sistema**: eso se hace desde el panel, con consentimiento, y se
deshace en el mismo sitio. Desinstalar apaga Modo Hogar, quita la regla del cortafuegos y
restaura el DNS exactamente como estaba.

Antes de instalar, compara la huella SHA‑256 del archivo con la de `SHA256SUMS` y con la línea
correspondiente de [`ledger.jsonl`](ledger.jsonl), el registro público que se publica antes que la
descarga. Después de instalar, `guardiana verify` lo comprueba en tu equipo.

## Usar

```
guardiana panel          abre el panel en el navegador (la única interfaz)
guardiana verify         huella, firma, servicio, DNS del sistema, puertos, listas, cadena
guardiana dns --status   a dónde apunta el DNS del sistema
guardiana hogar status   estado del Modo Hogar
guardiana ledger --check comprueba la cadena del extracto
guardiana export         exporta el extracto en CSV o JSON
```

Nada se bloquea sin decisión del usuario, nunca antes de 24 horas observando un dispositivo,
siempre con "deshacer" visible. Ninguna señal es un veredicto; la interfaz nunca dice "malicioso".

## Compilar

Rust estable, edición 2021, `Cargo.lock` en el repositorio.

```
cargo build --workspace --locked
cargo test --workspace --locked
```

Compilación reproducible en un contenedor fijado por digest: `build/repro.sh` (véase
[docs/VERIFY.md](docs/VERIFY.md)). Paquetes: `build/package.sh` (Linux) y `build/msi.ps1`
(Windows). Estructura del código y reglas de trabajo: [CLAUDE.md](CLAUDE.md) y
[docs/BRIEF.md](docs/BRIEF.md).

## Licencia

GPL‑3.0‑or‑later. Las listas de terceros conservan su licencia (EasyPrivacy: GPL‑3.0 / CC BY‑SA
3.0; lista de Peter Lowe: uso libre con atribución). Detalle en `docs/LISTS.md`.
