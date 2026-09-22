# ¿Qué es el DNS y por qué mi casa necesita un guardián?

Cada vez que un aparato de tu casa quiere hablar con un servicio (abrir una web, cargar un anuncio,
mandar un dato al fabricante), primero pregunta por su nombre: «¿dónde está `ejemplo.com`?». Esa
pregunta es el DNS. La contesta un servidor, normalmente el de tu operador, y el aparato se conecta
a la dirección que le devuelven.

## Por qué importa

Ese paso lo dan casi todos los aparatos, no solo el PC: el teléfono, la tele, la consola, el altavoz,
la cámara. Y lo dan cientos de veces al día sin que lo veas. Quien contesta las preguntas del DNS ve
la lista completa de con quién intenta hablar tu casa: nombres, no contenido, pero nombres que dicen
mucho (`ads.`, `metrics.`, `tracking.`).

Hoy esa lista la ve tu operador, o Google o Cloudflare si cambiaste el DNS a ellos. Tú, no.

## Qué hace un guardián DNS en casa

GUARDIANA convierte tu PC en quien contesta esas preguntas dentro de tu Wi‑Fi. Cada consulta pasa por
él antes de salir a internet, así que:

- **Lo ves**: qué servicios pide cada aparato, con su categoría (publicidad, rastreador, telemetría,
  esperado) y una frase por señal.
- **Lo cortas si quieres**: una regla tuya hace que ese nombre «no exista» para toda la casa, o para
  un aparato. Un nombre suelto, desde el primer minuto; los cortes que afectan a toda una categoría o
  a toda la casa esperan a que Guardiana lleve un día mirando. Siempre con deshacer.
- **No sale nada de casa**: el guardián está en tu PC; no hay servidor nuestro ni cuenta.

Para que los teléfonos y la tele pasen por él, se les dice que usen el PC como DNS: en el router una
sola vez para toda la casa, o en cada aparato. Son las otras guías de esta sección.

## Lo que un guardián DNS no ve

Es importante decirlo: ve nombres, no lo que va dentro. No ve qué app pidió cada nombre en un
teléfono. Y hay tráfico que lo esquiva: apps que traen su propio DNS cifrado, una VPN, o la
retransmisión privada de iCloud en Safari. GUARDIANA lo señala cuando lo detecta y lo deja escrito en
sus límites. Un guardián DNS no es un antivirus ni un control parental: es la vista de quién habla
con quién en tu casa, y el poder de decir que no.
