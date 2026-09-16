# Guardiana — Modo Hogar (Wi‑Fi): teléfonos sin app

Brief de construcción · parte del lanzamiento (decisión del 8 de septiembre de 2026) · complemento del brief de arquitectura v1.0 · para Claude Code

---

## 0 · Qué resuelve

Los teléfonos no pueden ejecutar Guardiana sin instalar algo, y no se paga peaje a Apple ni a Google. La salida: el teléfono no ejecuta nada. El computador de la casa que ya tiene Guardiana se convierte en el guardián de la Wi‑Fi, y cada teléfono lo usa cambiando un ajuste que trae de fábrica: el servidor DNS. Sin app, sin cuenta, sin tienda, sin certificados.

**Qué ve Guardiana de un teléfono así:** cada nombre de dominio que el teléfono pregunta (a qué servicios intenta hablar), cuándo, con qué frecuencia y su categoría (rastreador, telemetría, publicidad, servicio esperado).

**Qué no ve:** qué app lo pidió, cuántos bytes, ni el contenido. Se dice tal cual en la web y en la interfaz.

## 1 · Principios que se mantienen y la única excepción

- **Local.** El "servidor" está dentro de la casa, en el computador del usuario. Nada sale a internet hacia nosotros.
- Cero telemetría, extracto encadenado por hash, listas abiertas, mismas heurísticas y mismo modelo de amenazas del brief v1.0.
- **Excepción explícita al principio "jamás un puerto de red".** En Modo Hogar el servicio abre exactamente dos puertos, solo en la interfaz de red local (nunca en internet), desactivados por defecto, encendidos por el usuario, anotados en el extracto y cerrados al apagar el modo:
  - DNS: 53/udp y 53/tcp.
  - Panel local: HTTP en el puerto 7443 (o el primero libre a partir de ahí; se muestra).
  - El instalador pide la regla del cortafuegos del sistema solo para redes privadas y explica por qué. `guardiana verify` comprueba que con el modo apagado no hay ningún puerto abierto.
- **No es control parental.** Vigila apps, no personas (sección 6).

## 2 · Cómo se conecta un teléfono (del más fácil al más manual)

**Requisito previo:** el computador guardián necesita IP fija en la red local (reserva DHCP en el router o IP estática). Guardiana lo detecta y guía; sin esto, el DNS deja de responder cuando cambia la IP.

1. **Router, una vez, toda la casa.** En el router: DNS del DHCP = IP del computador con Guardiana. Todos los dispositivos de la Wi‑Fi quedan cubiertos sin tocarlos: teléfonos, televisor, consolas. Guardiana muestra la IP exacta y guías con capturas para los routers de los operadores más comunes. Si el router del proveedor no permite cambiarlo, camino 2.
2. **Por teléfono, en los ajustes de la Wi‑Fi.** iPhone: Ajustes → Wi‑Fi → (i) → Configurar DNS → Manual → añadir la IP. Android: red Wi‑Fi → Modificar → Avanzado → IP estática → DNS 1 (muchos Android obligan a fijar también la IP del teléfono; la guía lo explica y sugiere una IP libre).
3. **Código QR.** El computador muestra un QR. El teléfono lo abre y llega al panel local con la guía paso a paso para su sistema y un comprobador: si la página `comprobar.guardiana.hogar` carga, el teléfono ya pasa por Guardiana (ese nombre solo lo resuelve nuestro DNS).

## 3 · Qué hace el servicio en Modo Hogar

- **Resolutor DNS local** (biblioteca `hickory-dns` en Rust). Reenvía las consultas al resolutor que el usuario elija (por defecto el del router; opcionalmente uno cifrado que el usuario active, y se muestra cuál). Caché de respuestas.
- **Anota cada consulta:** instante · dispositivo (IP + MAC vía tabla ARP/vecinos + nombre asignado) · nombre consultado · categoría (mismas listas abiertas) · señales · veredicto (observado / respondido / cortado) · quién decidió.
- **Identifica dispositivos.** "Dispositivo nuevo en tu Wi‑Fi": el usuario le pone nombre desde el panel o desde el propio teléfono. Se identifica por MAC (estable dentro de la misma red aunque el teléfono la aleatorice entre redes).
- **Heurísticas aplicables sin proceso:** baliza (mismo nombre a intervalos regulares), destino nuevo, fuera de horas (por dispositivo), evasión de DNS (consultas a resolutores DoH conocidos: se anota; no se corta salvo decisión del usuario), volumen anómalo de consultas.
- **Bloqueo por DNS.** Responde NXDOMAIN a las categorías o dominios que el usuario corte, por dispositivo o para toda la casa. Mismas reglas inviolables del brief v1.0: nunca actualizaciones del sistema, ni el resolutor del sistema, ni mensajería o videollamada reconocidas sin confirmación; "cortar" solo tras 24 horas observando; botón de deshacer siempre visible.
- **Señal a navegadores.** Responde NXDOMAIN al dominio canario `use-application-dns.net`, que Firefox consulta para respetar el DNS de la red. Es una regla visible en el extracto, no un truco escondido.

## 4 · El panel local: lo que ve cada uno

Servidor HTTP mínimo, solo LAN, páginas estáticas + JSON del extracto. Sin JavaScript de terceros, sin cookies, sin fuentes externas.

- **Desde un teléfono: "Radiografía de este teléfono".** Su propio detalle, identificado por su IP: dominios, categorías, rastreadores, qué se cortó, con la explicación de una frase por señal. Botón de compartir (tarjeta y texto para WhatsApp), igual que en el computador. Ajuste "compartir mi detalle con el panel del hogar": apagado por defecto.
- **Desde el computador: panel del hogar.** Por dispositivo, solo agregados: número de destinos, rastreadores, cortes, última actividad. El detalle de un dispositivo aparece únicamente si su dueño lo compartió desde ese dispositivo.
- **Informe semanal del hogar.** Agregados por dispositivo y texto listo para WhatsApp: *"Esta semana, en casa: 5 dispositivos, 312 rastreadores detectados, 187 cortados."*
- **Prueba de 7 días.** (Superado el 14 sep 2026, decisión 52: Modo Hogar es gratis sin límite; la prueba
  de 7 días es ahora la de Plus.) Texto original: Modo Hogar funciona gratis 7 días; el contador vive en el propio equipo, sin cuenta ni servidor. La interfaz lo dice tal cual: es local, y reinstalar lo reinicia; confiamos en la honradez de quien prueba. Después, plan Hogar.

## 5 · Límites que la web y la interfaz dicen en ese lugar

- Solo en la Wi‑Fi de casa. Con datos móviles o fuera de casa, el teléfono no pasa por Guardiana.
- El computador guardián debe estar encendido. Alternativa barata: un portátil viejo o un miniPC con Linux (donde Guardiana no tiene ningún aviso de instalación), siempre encendido.
- Ve nombres, no apps: no sabe qué app pidió cada dominio.
- Lo esquivan: apps con IPs fijas, apps con DNS cifrado propio, VPN en el teléfono, iCloud Private Relay (Safari). Cuando se detecta, se anota como "tráfico fuera de vista"; nada se esconde.
- No es control parental: no ve contenido, no ve qué app, no muestra detalle sin permiso del dueño del dispositivo.
- iPhone: sin app y sin pagar a Apple, esto es todo lo que se puede hacer, y es así para cualquiera en la industria.
- Android: en 2.x podrá existir un APK sin tienda (F‑Droid o descarga directa; sin peaje a Google) que sí vea app por app mediante un VpnService local, sin servidor. No antes de que Modo Hogar funcione en casas ajenas.

## 6 · Reglas de integridad añadidas

- No vigila personas: agregados por defecto; el detalle solo desde el propio dispositivo.
- Todo dispositivo nuevo se anuncia en el panel y en el propio dispositivo al abrir el panel local: *"Esta red usa Guardiana. Esto es lo que se anota de tu teléfono."*
- Nunca "protección total del teléfono". El texto dice: *"en la Wi‑Fi de casa, por nombre de dominio"*.
- Modo Hogar apagado = puertos cerrados, y `guardiana verify` lo demuestra.
- Nada sale de la casa. Ni siquiera el informe semanal, salvo que el usuario pulse compartir.

## 7 · Plan de construcción

**Sustituido por el plan de 4 semanas (`GUARDIANA_Plan_4_Semanas.md`): Modo Hogar es ahora el núcleo de la 1.0 y se construye en las semanas 1 a 3, extendido al propio computador.** Lo que sigue se conserva solo como referencia.

### Plan anterior (semanas 9–11), solo referencia

Modo Hogar entra en el lanzamiento. Para que quepa: el lanzamiento pasa de la semana 12 a la 14 y macOS observador sale del lanzamiento (primera actualización). Si en la semana 11 no funciona en casas ajenas, se lanza sin él y se añade en cuanto funcione.

- **Semana 9:** resolutor + extracto por dispositivo + identificación de dispositivos, reutilizando el clasificador y las listas de la semana 3. CLI `guardiana hogar`.
- **Semana 10:** heurísticas, bloqueo por DNS con las reglas de seguridad, dominio comprobador, panel local (móvil primero), QR, prueba de 7 días.
- **Semana 11:** guías por sistema y por router, consentimiento y agregados, informe semanal. Beta Hogar en tres casas ajenas con routers de los tres operadores más comunes.
- **Semanas 12–13:** entra en la compilación reproducible, la firma y `guardiana verify` junto con el resto del programa.

El cortafuegos de IA sigue siendo 2.0 y empieza después.

## 8 · Cómo entra en el crecimiento

- **La entrada del embudo ya es móvil.** TikTok y Reels se ven en el teléfono. La web ofrece "Radiografía web de tu teléfono" (huella del navegador, calculada en el propio teléfono, sin enviar nada) y un botón **"Envíamelo al computador"** (compartir el enlace por WhatsApp a uno mismo o por correo) para instalar en el computador de la casa. Sin servidor.
- **Modo Hogar es la razón de comprar el plan Hogar:** *"Una licencia. Toda tu casa. Los teléfonos también, sin instalar nada en ellos."*
- **Expediente 6:** *"Puse mi teléfono a pasar por Guardiana una semana: X servicios hablaron con Y rastreadores mientras dormía."* Reproducible por cualquiera con el extracto exportado.

## 9 · Coste

Cero. Un ajuste en el router o en el teléfono, y el computador que ya está en casa.
