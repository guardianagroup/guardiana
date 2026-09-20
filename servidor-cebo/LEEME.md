# El servidor del cebo

**Qué es:** un servicio diminuto que apunta quién visita las direcciones que GUARDIANA deja dentro
de sus archivos trampa **en nuestras mediciones publicadas**.

**Qué no es, y esto es lo importante:** no forma parte del programa. El GUARDIANA que se instala en
el computador de una persona **nunca habla con este servidor**; una trampa puesta por alguien en su
casa se detecta dentro de su propio equipo, por DNS, y no sale nada de ahí. Este servicio existe
solo porque nosotros queremos medir una cosa que desde el equipo no se puede ver: si el **servidor**
de una IA o de una web extranjera sigue el enlace que encuentra en un archivo.

## Qué guarda

Por cada visita: la hora, qué cebo, qué dirección pidió, con qué nombre se presentó el programa
(user-agent) y de dónde vino. **Se borra solo a los 90 días.** En la página pública se enseña la
red de origen (`200.14.x.x`), no la dirección entera.

## Cómo se pone en marcha (gratis, sin tarjeta)

Corre en [Deno Deploy](https://deno.com/deploy), que tiene plan gratis y entra con la cuenta de
GitHub. Un solo paso, y es del responsable porque hay que crear una cuenta:

1. Entrar en `deno.com/deploy` con la cuenta de GitHub de `guardianagroup`.
2. **New Project → Deploy from GitHub**, elegir el repositorio `guardianagroup/guardiana`, rama
   `main`, y como entrypoint: `servidor-cebo/main.ts`.
3. Queda una dirección del estilo `https://<nombre>.deno.dev`. Con eso ya está: cada `git push`
   la actualiza.

Después, si se quiere una dirección propia (`cebo.guardianagroup.com`), se añade en Porkbun un
CNAME hacia esa dirección y se declara el dominio en Deno Deploy. No hace falta para empezar.

## Cómo se usa en una medición

El cebo lleva dentro `https://<nombre>.deno.dev/c/<id>`. Quien siga el enlace queda en la lista.
El protocolo de cada medición se escribe **antes** de mirar los números, como en el resto de
radiografías, y se publica entero.

## Correrlo en local para probarlo

```
deno run -A servidor-cebo/main.ts
# y en otra terminal:
curl -s localhost:8000/c/prueba123 ; curl -s localhost:8000/visitas.json
```
