# Guardiana 1.0 — Brief de construcción para Claude Code

Sustituye al brief de arquitectura anterior. Plazo: 28 días. 8 de septiembre de 2026.

Pega este documento como primer mensaje en el repositorio vacío. Todo lo que dice se cumple; lo que no dice se pregunta antes de inventarlo.

**Actualizado el 15 de septiembre de 2026** (decisión 62): el producto que se construye no cambia, cambia dónde se pone el foco. La capa de agentes pasa a ser lo primero que se cuenta y lo primero que se ve en el panel; el guardián de la casa sigue siendo la base gratuita y el motor de todo. Lo que se añade está en §0, §1, §3, §5, §8 y §14; lo que sigue fuera, también. El registro de cambios de este documento es la sección 15.

---

## 0 · Qué construimos

Un programa para Windows y Linux que convierte el computador en el **guardián DNS de la casa**: primero de sí mismo, después de los teléfonos, el televisor y todo lo que use la Wi‑Fi, sin instalar nada en ellos. Ve a qué servicios intenta hablar cada dispositivo, lo clasifica, lo explica en una frase, lo anota en un extracto encadenado por hash, y corta lo que el usuario decida.

Sobre ese motor va **la capa de agentes**, que es lo primero que se cuenta: cuando en el equipo trabaja un agente de inteligencia artificial, el usuario le dio permiso para una tarea, y nadie le enseña con cuántos servicios habla mientras la hace. Guardiana se pone en medio y se lo enseña: el usuario declara a qué servicios necesita hablar ese aparato y Guardiana lista todo lo que se salió de ahí. **Comprueba a dónde habla, no qué lee.** Esa frase se dice en la portada, en la página del panel y en cualquier sitio donde alguien pudiera entender otra cosa.

El marco es de permiso, no de miedo: Guardiana existe para poder soltarle el trabajo al agente sin quedarse mirando, no para decir que la inteligencia artificial sea peligrosa.

**Principios que no se negocian:**

1. **Todo ocurre en la casa.** Sin cuenta, sin servidor nuestro, cero telemetría. El programa no contacta ningún servidor nuestro nunca. Las únicas conexiones salientes son las que el usuario provoca (activar licencia, comprobar versión, actualizar listas), cada una anotada en su propio extracto.
2. **Puertos:** el panel escucha en `127.0.0.1` siempre. En Modo Hogar, y solo entonces, el DNS (`53/udp`, `53/tcp`) y el panel (`7443/tcp`) escuchan además en la interfaz de red local. Nunca en internet. Apagar Modo Hogar cierra esos puertos.
3. **Nada se bloquea sin decisión del usuario**, nunca antes de 24 horas observando, siempre con deshacer visible.
4. **Cada señal se explica en una frase** que una persona sin conocimientos técnicos entiende. La etiqueta "esperado" existe y se usa: las actualizaciones del sistema no son una fuga.
5. **Código abierto desde el primer commit**, `cargo build --locked` reproducible, cada binario con hash y firma, registro público antes que la descarga.
6. **No vigila personas.** En Modo Hogar, el panel de la casa ve totales por dispositivo; el detalle de un dispositivo solo se ve desde ese dispositivo, salvo que su dueño lo comparta.
7. **Lo que no puede hacer se dice donde el usuario lo esperaría**, con esas palabras.

## 1 · Alcance

**Entra en el lanzamiento (día 28):**
- Resolutor DNS local con listas abiertas, categorías, señales y extracto por dispositivo.
- El computador se apunta a sí mismo con consentimiento y con deshacer.
- Servicio Windows (SCM) y Linux (systemd).
- Panel local en el navegador: radiografía de 60 segundos, extracto, dispositivos, Modo Hogar (QR, comprobador, guías), "lo que Guardiana sabe de ti", informe semanal, compartir, licencia, verify.
- Bloqueo por DNS con reglas inviolables, por dispositivo y por casa.
- **Capa de agentes:** lista propia de servicios de inteligencia artificial, etiqueta por consulta, página `/ia` del panel y **alcance declarado por dispositivo** (el usuario escribe los servicios permitidos; Guardiana informa de lo que se salió en las últimas 24 horas y no corta nada por su cuenta). Se comprueban destinos, nunca procesos ni archivos.
- Prueba de 7 días de Modo Hogar; plan Hogar con activación de una sola conexión y alternativa por archivo firmado.
- Firma minisign, hashes, `ledger.jsonl`, Rekor, `guardiana verify`, script de compilación reproducible publicado.
- Instaladores: Windows (MSI) y Linux (.deb y tarball).

**Entra solo si está probado en una máquina ajena el día 24 (sección 12):** reproducibilidad verificada byte a byte; aviso de actualización con verificación de firma; "qué app" en Windows, solo lectura.

**Fuera:** sensores por proceso con bloqueo por aplicación (2.0), interfaz egui, macOS, actualización automática silenciosa, y el **cortafuegos de IA** entendido como lo que sigue fuera: cortar por aplicación o por proceso, vigilar el sistema de archivos (qué archivos abre o lee un agente, los «archivos trampa»), leer el contenido de las conexiones y cualquier juicio sobre la intención de un agente. La capa de agentes del punto anterior no es eso: mira nombres de destino, igual que el resto del programa.

## 2 · Stack y estructura

- **Rust estable**, edición 2021, `Cargo.lock` en el repositorio, `cargo build --locked` siempre.
- Dependencias mínimas y auditadas: `hickory-server`/`hickory-resolver` (DNS), `rusqlite` (con `bundled`), `tokio`, `axum` (panel HTTP), `serde`/`serde_json`, `sha2`, `minisign-verify`, `qrcode`, `windows-service` (Windows), `pnet`/lectura de tablas de vecinos (LAN), `ferrisetw` solo si entra el extra de Windows. `cargo deny` y `cargo audit` en CI; `unsafe` prohibido salvo justificación comentada.
- Workspace:

```
guardiana/
  crates/
    core/       eventos, extracto SQLite encadenado, exportación CSV/JSON, reglas
    dns/        resolutor: escucha, reenvío, caché, respuestas de bloqueo, canario, comprobador
    lists/      descarga y actualización de listas abiertas, categorías, atribución y licencias
    classify/   categoría por nombre + señales (baliza, destino nuevo, fuera de horas, evasión de DNS, volumen)
    devices/    identificación de dispositivos en la LAN (IP, MAC, nombre), consentimiento
    panel/      servidor HTTP local, API JSON, páginas estáticas del panel (reutilizan el CSS de la web)
    service/    daemon: Windows SCM y systemd; cambio y restauración del DNS del sistema; watchdog
    license/    prueba de 7 días, activación con una conexión, archivo de licencia firmado
    verify/     guardiana verify: hashes, firma, puertos, servicio, DNS del sistema, listas
    cli/        guardiana observe | ledger | export | hogar | verify | update
  build/        Dockerfile fijado por digest, repro.sh, release.sh (hashes, minisign, firma de código, ledger, rekor)
  docs/         THREAT_MODEL.md, WHAT_IT_DOES_NOT_DO.md, VERIFY.md, LISTS.md, HOGAR.md, guías por router y por teléfono
  ledger.jsonl  el registro público
```

## 3 · Modelo de datos (SQLite, una base por instalación)

- `events(id, ts, device_id, client_ip, qname, qtype, category, list_source, signals_json, verdict, decided_by, rule_id, prev_hash, row_hash)`. `verdict ∈ {observado, respondido, cortado}`. `decided_by ∈ {usuario, regla_usuario, nadie}`. `row_hash = sha256(prev_hash || campos)`; `prev_hash` del primer evento es el hash de la clave pública del instalador.
- `devices(id, mac, last_ip, name, first_seen, last_seen, share_detail_with_home BOOL default 0, hours_profile_json)`. El computador es el dispositivo `self`.
- `rules(id, scope ∈ {device, home}, device_id, match_kind ∈ {domain, suffix, category}, pattern, action ∈ {cortar, permitir}, created_at, created_by, expires_at, undone_at)`.
- `outbound(id, ts, purpose ∈ {licencia, version, listas}, host, bytes, initiated_by_user BOOL)`: todo lo que el programa mismo envía. Se muestra en "Lo que Guardiana sabe de ti" y es lo que se exporta con `guardiana ledger --self`.
- `settings(key, value)`: retención, modo hogar, upstream, idioma, token del panel y el **alcance declarado** de cada dispositivo (`alcance:<device_id>`, un servicio permitido por línea).
- `changes(id, ts, kind ∈ {hogar_on, hogar_off, dns_on, dns_off}, who, detail)`: los cambios que Guardiana hace en el equipo porque alguien se los pidió, con quién los pidió (panel, terminal, desinstalador). Se ven en `/extracto` y en `/estado`. No se anota nada que no haya pedido alguien: el programa no cambia nada solo.
- Retención: plan gratis, detalle 24 h y totales diarios 7 días; plan Hogar, configurable, sin límite por defecto. Un trabajo de limpieza cada hora. Borrado total desde el panel y con `guardiana ledger --wipe`, con confirmación.
- Verificación de cadena: `guardiana ledger --check` recorre y valida los hashes; el panel muestra el resultado.

## 4 · El resolutor DNS

- Escucha: `127.0.0.1:53` siempre; en Modo Hogar, además la IP de la interfaz LAN. Nunca `0.0.0.0` hacia interfaces que no sean privadas; comprobación de rango privado en cada arranque.
- Reenvío: al resolutor que tenía el sistema antes de que Guardiana lo cambiara (se guarda en `settings`), o al que el usuario elija en el panel; se muestra cuál es. Caché respetando TTL. Sin DNSSEC en 1.0 (se documenta).
- **Seguridad del cambio de DNS en el computador:** primario `127.0.0.1`, secundario el resolutor original. Si el servicio cae, el sistema sigue resolviendo por el secundario y el evento "Guardiana no estaba vigilando entre X e Y" se anota al volver. Windows puede consultar ambos en paralelo en algunos casos: se documenta como límite conocido, no se esconde. Al desinstalar, se restaura exactamente la configuración anterior.
  - Windows: `SetInterfaceDnsSettings`/`netsh` por interfaz activa; reaplicar si cambia la red.
  - Linux: detectar `systemd-resolved` (usar `resolvectl` sobre la interfaz), NetworkManager (perfil) o `/etc/resolv.conf` plano; nunca sobrescribir sin copia de seguridad.

> **Nota del 15 de septiembre de 2026 (decisión 101):** lo de arriba —primario Guardiana,
> secundario el resolutor original— **se cumple en Windows y no se puede cumplir en Linux con
> `systemd-resolved`**, y se descubrió midiéndolo, no leyéndolo. Para `systemd-resolved` la lista de
> servidores de un enlace es un **conjunto entre el que elige**, no un orden que obedece: con
> `127.0.0.1` el primero y el router el segundo, elegía el router, y las consultas del equipo **no
> pasaban por el guardián** aunque el panel dijera que sí. Como el brief §14 prohíbe una frase de
> interfaz que el programa no cumple, ahí el guardián pasa a ser el **único** resolutor: sin reserva.
> Lo que se pone en su lugar para no dejar a nadie sin nombres: la unidad de systemd arranca
> `Restart=always`, el vigilante comprueba cada minuto que siga siéndolo, `guardiana dns --restore`
> devuelve la configuración exacta —incluido si `/etc/resolv.conf` era un enlace y a dónde— y el
> cambio **se niega a aplicarse** si el equipo tiene DNS cifrado estricto. Afecta también a la prueba
> de «servicio caído» de §12: en Linux con `systemd-resolved` lo que se comprueba es que el servicio
> **vuelve**, no que haya un secundario. En NetworkManager y en `/etc/resolv.conf` plano el secundario
> sigue como dice el brief. Medido en Ubuntu 24.04; detalle en `docs/PRUEBAS.md`.
- Respuestas de bloqueo: `NXDOMAIN` por defecto (opción `0.0.0.0` en ajustes). Cada bloqueo se anota con la regla que lo causó.
- Canario de Firefox: `use-application-dns.net` → `NXDOMAIN`, como regla visible en el extracto, activable por el usuario (activada por defecto en Modo Hogar).
- Comprobador: `comprobar.guardiana.hogar` → resuelve a la IP LAN del computador; el panel sirve en ese nombre una página "Este dispositivo ya pasa por Guardiana".
- Evasión de DNS: lista de nombres de resolutores DoH/DoT conocidos; se anota como señal, no se corta salvo regla del usuario.
- Nunca se anotan ni se muestran respuestas, solo consultas: nombre, tipo, dispositivo, hora, categoría, veredicto.
- Rendimiento objetivo: < 5 ms añadidos por consulta en caché, < 1 % CPU en reposo.

## 5 · Listas y clasificación

- Listas abiertas con licencia que permita el uso, descargadas solo cuando el usuario pulsa "actualizar listas" o activa la actualización semanal (anotada en `outbound`): EasyPrivacy, Disconnect, Peter Lowe, DuckDuckGo Tracker Radar. `docs/LISTS.md` con licencia y atribución de cada una. Las listas se distribuyen dentro del instalador con su fecha.
- Categorías: `rastreador`, `publicidad`, `telemetria`, `esperado`, `desconocido`. `esperado` = lista mantenida por nosotros de actualizaciones de sistema, resolutores, hora, mensajería y videollamada, sincronización activada por el usuario; abierta y documentada.
- Señales, cada una con su frase:
  - `baliza`: mismo nombre a intervalos regulares (tolerancia 20 %) durante más de 30 minutos. "Contacta el mismo destino cada N minutos, como un latido."
  - `destino_nuevo`: primera vez para este dispositivo. "Este dispositivo nunca había hablado con este servicio."
  - `fuera_de_horas`: consulta fuera del perfil horario del dispositivo (se aprende tras 3 días). "Habla a horas en las que este dispositivo no suele hacerlo."
  - `evasion_dns`: nombre de resolutor cifrado conocido. "Intenta resolver nombres por su cuenta, fuera de la vista de Guardiana."
  - `volumen`: más de 5 veces la mediana de consultas de ese dispositivo en la última hora. "Está preguntando mucho más de lo habitual."
- Listas propias añadidas después del brief original. Ninguna es un veredicto y ninguna cambia la categoría de nada:
  - `empresas.txt`: quién es el dueño del nombre (`graph.facebook.com` → Meta), para que se entienda con quién se está hablando.
  - `ia.txt`: qué nombres pertenecen a un servicio de inteligencia artificial. Es lo que alimenta la página `/ia` y el alcance declarado.
- Ninguna señal es un veredicto. La interfaz nunca dice "malicioso": dice qué observó.

## 6 · Bloqueo: reglas inviolables

- **Cortar un nombre concreto que elige la persona no espera**: está desde el primer minuto, y mientras el
  dispositivo lleve menos de 24 horas observado pide confirmación diciendo cuántas lleva y que todavía no
  se puede avisar de qué deja de funcionar. **Lo ancho sí espera las 24 horas** —una regla por categoría,
  una regla para toda la casa y el Modo Vigilante—; en su lugar, "Guardiana está observando: X horas de 24".
  (Cambio del 20 sep 2026, decidido por el responsable sobre la regla original, que no dejaba cortar nada
  antes de 24 horas; el porqué está en la decisión 187.)
- Nunca se cortan, aunque el usuario lo pida en una regla por categoría: resolutores del sistema, dominios de actualización del sistema y del propio Guardiana, hora (NTP), mensajería y videollamada reconocidas. Una regla explícita por dominio sobre uno de estos pide confirmación con el texto: "Esto puede dejar sin actualizaciones / sin mensajes a este dispositivo."
- Deshacer siempre visible: cada regla tiene "deshacer" y "deshacer todo lo de hoy". Deshacer no borra el evento: anota que se deshizo.
- Reglas por casa solo desde el panel del computador (con token). Reglas por dispositivo también desde la página del propio dispositivo.

## 7 · Dispositivos y Modo Hogar

- Identificación: IP de la consulta → MAC por la tabla de vecinos (ARP/NDP) → nombre asignado por el usuario. Si cambia la IP y coincide la MAC, es el mismo dispositivo. Un dispositivo nuevo aparece como "Dispositivo nuevo en tu Wi‑Fi" hasta que se nombra, desde el panel o desde el propio dispositivo.
- Consentimiento: `share_detail_with_home` apagado por defecto. Al abrir el panel desde un dispositivo por primera vez, se muestra: "Esta red usa Guardiana. Esto es lo que se anota de este dispositivo: nombres de servicios, hora, categoría. No apps, no contenido."
- Activar Modo Hogar (plan Hogar o prueba de 7 días): comprueba IP fija (avisa si es dinámica y explica la reserva DHCP), abre los puertos en la LAN, añade la regla de cortafuegos solo para perfil privado (Windows: `netsh advfirewall` con nombre "Guardiana Modo Hogar"; Linux: documentar `ufw`/`firewalld`), muestra la IP y el QR con `http://<ip>:7443/hogar`. Desactivar cierra puertos y quita la regla.
- `guardiana hogar status` imprime puertos abiertos, dispositivos, reglas activas.

## 8 · El panel (única interfaz)

- Servido por el servicio con `axum`; páginas estáticas + API JSON; sin JavaScript de terceros, sin fuentes externas, sin cookies. Reutiliza el CSS y los componentes de la web Guardiana 2050 (ya diseñados).
- Seguridad: en `127.0.0.1`, token aleatorio por instalación guardado en un archivo legible solo por el usuario; el lanzador abre `http://127.0.0.1:7443/?t=<token>` y la sesión vive en memoria. Comprobación estricta de `Host` (contra rebinding). En LAN, cada dispositivo solo puede leer su propio detalle (por IP) y cambiar sus propios ajustes; el panel de la casa requiere el token. Sin token no hay escritura.
- Páginas:
  - `/` radiografía de 60 segundos: contadores en vivo (servicios, rastreadores, destinos nuevos, esperados), lista viva con etiqueta y frase, botón "Compartir".
  - `/extracto`: filtros por dispositivo, categoría, señal, veredicto; exportar CSV/JSON; comprobación de cadena.
  - `/dispositivos`: totales por dispositivo; detalle solo si es `self` o si compartió.
  - `/hogar`: activar/desactivar, IP, QR, comprobador, guías por router y por teléfono, estado de la prueba de 7 días.
  - `/mi-dispositivo` (desde un teléfono): su radiografía, sus reglas, interruptor de compartir detalle, compartir.
  - `/sabe-de-ti`: todo lo que Guardiana guarda, la tabla `outbound`, botón de borrar todo.
  - `/informe`: informe semanal de la casa (totales por dispositivo) y texto listo para WhatsApp.
  - `/licencia`: estado, activar con clave, activar con archivo firmado, qué conexión hace y cuándo la hizo.
  - `/verify`: salida de `guardiana verify` en pantalla.
  - `/ia`: la capa de agentes, primera página del menú. Qué servicios de inteligencia artificial habló cada dispositivo en los últimos 7 días y el alcance declarado de cada uno, con la lista de lo que se salió en 24 horas. El límite («comprueba a dónde habla, no qué lee») va arriba de esa página, no en letra pequeña.
- Compartir: tarjeta PNG generada en el navegador (Canvas) a partir de los totales, sin nombres de servicios por defecto; texto para WhatsApp por enlace `wa.me` que abre la app del usuario (acción del usuario, nada se envía por Guardiana). Las cifras de la tarjeta son las reales del extracto; ninguna cifra se redondea "hacia arriba".
- Textos en `panel/i18n/es.json` desde el primer día; el inglés después.

## 9 · Licencia

> **Nota del 14 de septiembre de 2026 (decisión 52, del responsable):** este apartado se escribió para el
> plan Hogar de pago único con prueba de 7 días. Desde el 14 sep, Modo Hogar es gratis y lo que se vende es
> **Plus por suscripción** (mensual o anual, 7 días de prueba local sin tarjeta, nunca antes de 24 h
> observando). Una suscripción exige comprobar la clave **una vez por periodo de pago**, no «nunca más»;
> cada comprobación queda en `outbound` y en `/sabe-de-ti`, con periodo de gracia de 7 días si la
> pasarela no responde. El archivo firmado se emite por periodo. Lo demás de este apartado sigue vigente.
> Implementado en `crates/license` (decisión 53 para los momentos en que se ofrece Plus).

- Prueba de 7 días de Modo Hogar: contador local en `settings`, con el texto en pantalla "Este contador vive en tu equipo. Reinstalar lo reinicia. Confiamos en ti."
- Activación con clave (pasarela de pago): una única llamada `POST` a la API de licencias de la pasarela, iniciada por el usuario desde `/licencia`, anotada en `outbound` con host y bytes; el resultado se guarda firmado localmente. Después, nunca más se conecta; sin comprobaciones periódicas.
- Activación con archivo: JSON de licencia firmado con nuestra clave minisign (clave pública incrustada en el binario y publicada en la web); para quien no quiera ni esa conexión.
- Sin licencia y sin prueba: todo lo del plan gratis sigue funcionando; Modo Hogar se apaga con aviso, nunca se rompe el DNS del computador.

## 10 · Confianza y publicación

- `build/Dockerfile` fijado por digest, toolchain fijado, `SOURCE_DATE_EPOCH` del commit, `--locked`, `strip`, sin rutas absolutas; `build/repro.sh` produce los binarios y sus hashes en cualquier máquina.
- `build/release.sh`: compila en el contenedor → SHA‑256 de cada binario → firma minisign (clave privada solo en el equipo de release, nunca en CI) → firma de código Windows con el certificado IV de SSL.com vía eSigner (CodeSignTool o su API CSC desde Linux; paso opcional que falla en claro si no hay certificado; siempre con sello de tiempo) → publica los hashes del binario sin firma y del firmado → añade la línea a `ledger.jsonl` → sube la línea a Rekor → solo entonces publica la release.
- `ledger.jsonl`: una línea por versión: `{version, commit, date, files:[{name, sha256_unsigned, sha256_signed, minisign}], rekor_uuid}`. Se publica antes que la descarga, nunca después.
- `guardiana verify`: hash de los binarios instalados y comprobación de la firma minisign adjunta contra la clave incrustada; comparación con `ledger.jsonl` local si existe (o descargado a petición, anotado); estado del servicio; DNS del sistema (qué apunta a dónde); puertos abiertos (con Modo Hogar apagado debe listar ninguno); versión de listas. Salida legible y `--json`.
- `docs/THREAT_MODEL.md`, `docs/WHAT_IT_DOES_NOT_DO.md` y `docs/VERIFY.md` se escriben en la semana 1 y se enlazan desde la web.
- Claves: la clave privada minisign del programa y el keystore de firma de la futura app Android se generan en la semana 1 en el equipo de release, fuera de línea, con copia de seguridad en dos sitios; nunca en CI. Perder cualquiera de las dos es perder la identidad del programa o de la app.

## 11 · Instaladores y pruebas

- Windows: MSI con `cargo-wix` (WiX es libre). Instala en `Program Files`, registra el servicio, crea el lanzador del panel, cambia el DNS con consentimiento en el primer arranque del panel (no en la instalación). Desinstalar restaura el DNS y quita la regla de cortafuegos si existe. La regla de cortafuegos solo se crea al activar Modo Hogar.
- Linux: `.deb` con `cargo-deb` (unidad systemd, `postinst` que habilita el servicio, `prerm` que restaura el DNS) y tarball con script de instalación legible. Flathub después del lanzamiento.
- Calidad: `cargo test`, `cargo clippy -D warnings`, `cargo fmt --check`, `cargo audit`, `cargo deny`; pruebas de integración del resolutor (resolver, cachear, bloquear, canario, comprobador) y de la cadena del extracto; prueba de "servicio caído" (el sistema sigue resolviendo por el secundario).
- Cada hito se prueba en una máquina que no sea la de desarrollo, instalando desde cero.

## 12 · Extras, en orden, solo si están probados el día 24

1. **Reproducibilidad verificada:** una persona ajena reproduce los hashes de la release en su máquina con `build/repro.sh`; se publica quién y cuándo en `docs/VERIFY.md`.
2. **Aviso de actualización:** `guardiana update --check` descarga (a petición del usuario o con el ajuste semanal activado, anotado) `ledger.jsonl` de la web, compara versiones y avisa en el panel; `guardiana update` descarga el instalador, verifica hash y firma contra el registro y solo entonces lo lanza. Nunca silencioso, nunca automático.
3. **"Qué app" en Windows, solo lectura:** consumir con `ferrisetw` el proveedor ETW `Microsoft-Windows-DNS-Client` (eventos de consulta con `QueryName` y PID; comprobar los identificadores de evento en la documentación antes de implementar), resolver PID → ruta del ejecutable, SHA‑256 y estado de firma (WinTrust), y guardar `process_json` en `events` para el dispositivo `self`. La interfaz añade la columna "app" solo en el computador; los teléfonos siguen por nombre. Nada de bloqueo por aplicación.
4. **Expedientes 2 y 3:** grabación y extractos exportados; no es código.
5. **Qué pidió el agente, no solo a dónde** (después de la 1.0, junto con el punto 3): con «qué app» en Windows funcionando, la página `/ia` podría decir qué programa hizo cada consulta y no solo qué aparato. Los «archivos trampa» del documento de mensaje siguen fuera mientras exijan vigilar el sistema de archivos: eso es un producto distinto y una promesa que hoy no se puede cumplir.

## 13 · Orden de trabajo, 28 días

- **Días 1–7:** `core`, `dns`, `lists`, `classify`, `cli observe|ledger|export`, cambio de DNS con secundario de seguridad, `docs/THREAT_MODEL.md` y `docs/WHAT_IT_DOES_NOT_DO.md`. Hito: en una máquina ajena, `guardiana observe` muestra nombre → categoría → dispositivo en vivo.
- **Días 8–14:** `service` (Windows y Linux), `panel` (radiografía, extracto, dispositivos, sabe-de-ti, compartir), `devices`, Modo Hogar con QR y comprobador, consentimiento. Hito: beta cerrada con 10 fundadores (5 Linux, 5 Windows).
- **Días 15–21:** bloqueo y reglas inviolables, señales completas, prueba de 7 días, informe semanal, `license`, `verify`, `build/repro.sh` y `build/release.sh`, primera línea de `ledger.jsonl` en pruebas, firma con certificado si llegó. Hito: beta Hogar en 3 casas ajenas.
- **Días 22–27:** instaladores, guías por router y por teléfono, correcciones, pruebas desde cero, extras de la sección 12 que hayan llegado. Día 27: `ledger.jsonl` y Rekor publicados.
- **Día 28:** descarga abierta.

Regla de corte: si un día 24 algo de la sección 1 no está probado en una máquina ajena, se recorta lo que esté por debajo en esta lista, nunca la confianza (sección 10) ni las reglas inviolables (sección 6). La fecha no se mueve.

## 14 · Reglas de la interfaz

- Nunca "100 % seguro", "invisible", "protegido" sin objeto. Nunca "malicioso" como veredicto.
- Cada señal, una frase. Cada bloqueo, quién lo decidió y cuándo.
- La etiqueta "esperado" se muestra con el mismo peso que "rastreador".
- Sin trucos: sin contadores inventados, sin cifras redondeadas hacia arriba, sin urgencia falsa en la prueba de 7 días.
- Todo lo que sale del equipo por acción del programa está en `/sabe-de-ti`, con host, fecha y bytes.
- Lo que no puede hacer en cada pantalla se dice en esa pantalla: "En Modo Hogar, Guardiana ve nombres de servicios, no qué app los pidió."
- **Sobre la capa de agentes:** nunca se promete ver archivos, procesos ni contenido. La frase exacta, y la única, es «comprueba a dónde habla, no qué lee». Que un servicio de IA aparezca en la lista no es una acusación: significa que ese aparato habló con él.
- **Nunca sonar anti‑IA.** Quien compra Guardiana usa agentes todos los días y quiere seguir usándolos. El marco es de permiso («suéltale el trabajo sin quedarte mirando»), nunca de apocalipsis. Ni en el programa ni en la publicidad."

## 15 · Registro de cambios de este documento

- **15 sep 2026 · la capa de agentes pasa delante (decisión 62).** Origen: `guardiana_el_gancho.pdf`, documento interno de mensaje del responsable. Liderar con la vigilancia de datos nos pone en la fila de Pi‑hole, Portmaster, GlassWire y Little Snitch, que regalan lo que nosotros cobraríamos; la capa de agentes nos deja solos. Qué cambió: §0 (qué construimos, con el límite y el marco de permiso), §1 (la capa de agentes entra en el lanzamiento y se aclara qué sigue siendo «cortafuegos de IA» y sigue fuera), §3 (`alcance:` en settings y la tabla `changes`), §5 (listas propias `empresas.txt` e `ia.txt`), §8 (página `/ia`), §14 (dos reglas nuevas de interfaz). Lo que **no** cambió: los siete principios de §0, las reglas inviolables de §6, la confianza de §10 y la regla de corte de §13.
- Del documento de mensaje se corrigió una frase antes de usarla: el anuncio decía «¿sabes cuántos archivos leyó?» y Guardiana no ve archivos. La versión que se publica es «¿sabes con cuántos servicios habló?». Los «archivos trampa» quedan fuera por la misma razón, y quedan anotados en §12 como extra solo si algún día hay una forma de verlos que no obligue a instalar un vigilante del sistema de archivos.
- **15 sep 2026 · en Linux con `systemd-resolved` no hay secundario (decisión 101).** Origen: una medición en Ubuntu 24.04, no una idea. El §4 pedía primario Guardiana y secundario el resolutor original; ahí eso no funciona, porque `systemd-resolved` elige entre los servidores del enlace en vez de preguntarlos por orden, y con el router en la lista las consultas del equipo no llegaban al guardián. Qué cambió: nota dentro del §4 y su reflejo en la prueba de «servicio caído» del §12. Lo que **no** cambió: nada más; en Windows, NetworkManager y `/etc/resolv.conf` plano el secundario sigue igual, y la regla del §14 —ninguna frase de interfaz que el programa no cumpla— es precisamente la que obligó al cambio.
