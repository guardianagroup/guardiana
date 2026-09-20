// GUARDIANA · el servidor del cebo (decisión 185).
//
// Para qué existe: GUARDIANA solo ve las preguntas que salen del equipo de una persona. Si quien
// sigue el enlace de un archivo trampa es el **servidor** de un servicio de otro país, esa visita
// ocurre allí y desde aquí no se ve. Este servicio, que no forma parte del programa y con el que
// el programa nunca habla, existe para una sola cosa: nuestras mediciones publicadas.
//
// Lo que hace: responde a cualquier dirección que empiece por /c/<cebo> y apunta la visita.
// Lo que guarda, y nada más: la hora, qué cebo, qué ruta pidió, con qué nombre se presentó el
// programa (user-agent) y de qué dirección vino. Se borra a los 90 días.
// Lo que enseña en su página: todo eso menos la dirección completa, que se resume a su red.
//
// Corre en Deno Deploy (gratis, sin tarjeta). Arranque local:  deno run -A servidor-cebo/main.ts

const kv = await Deno.openKv();
const DIAS = 90;

/** La red de una dirección, sin la dirección entera: `200.14.x.x`, `2801:1e::`.
 *
 * Esto se aplica **antes de guardar**, no al enseñar: la dirección completa de quien llama no se
 * escribe en ningún sitio. La primera visita de prueba enseñó por qué hacía falta afinarlo: llegó
 * como `::ffff:181.63.26.36`, que es una dirección de las de siempre vestida de IPv6, y el
 * recorte la dejaba casi entera. */
function red(ip: string): string {
  let limpia = ip.trim().toLowerCase();
  if (limpia.startsWith("[") && limpia.endsWith("]")) limpia = limpia.slice(1, -1);
  // `::ffff:1.2.3.4` es una IPv4 dentro de una IPv6: se trata como lo que es.
  const mapeada = limpia.match(/^::ffff:(\d+\.\d+\.\d+\.\d+)$/);
  if (mapeada) limpia = mapeada[1];
  if (limpia === "" || limpia === "::1" || limpia.startsWith("127.")) return "desconocida";
  if (limpia.includes(":")) {
    const grupos = limpia.split(":").filter((g) => g !== "").slice(0, 2);
    return grupos.length ? grupos.join(":") + "::" : "desconocida";
  }
  const p = limpia.split(".");
  return p.length === 4 ? `${p[0]}.${p[1]}.x.x` : "desconocida";
}

function escapar(s: string): string {
  return s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]!));
}

async function anotar(cebo: string, ruta: string, ip: string, agente: string) {
  const ahora = Date.now();
  // Se guarda la red, no la dirección: lo que no se escribe no se puede filtrar después.
  await kv.set(["visitas", ahora, crypto.randomUUID()], { ahora, cebo, ruta, red: red(ip), agente }, {
    expireIn: DIAS * 24 * 60 * 60 * 1000,
  });
}

async function visitas(): Promise<Array<Record<string, unknown>>> {
  const salida: Array<Record<string, unknown>> = [];
  for await (const e of kv.list({ prefix: ["visitas"] }, { reverse: true, limit: 500 })) {
    salida.push(e.value as Record<string, unknown>);
  }
  return salida;
}

const PAGINA = (filas: string) => `<!doctype html>
<html lang="es"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Cebo · GUARDIANA</title>
<style>
 body{margin:0;background:#F4F6FA;color:#0B1020;font:16px/1.6 system-ui,-apple-system,"Segoe UI",Roboto,sans-serif}
 main{max-width:900px;margin:0 auto;padding:32px 20px 64px}
 h1{font-size:26px;margin:0 0 6px} p{color:#5B6275;max-width:60em}
 table{border-collapse:collapse;width:100%;margin-top:22px;background:#fff;font-size:14px}
 th,td{border:1px solid #D9DEE8;padding:8px 10px;text-align:left;vertical-align:top}
 th{font-size:12px;letter-spacing:.06em;text-transform:uppercase;color:#5B6275}
 code{font-family:ui-monospace,Menlo,Consolas,monospace;font-size:13px}
 .vacio{color:#5B6275;font-style:italic}
</style></head><body><main>
<h1>El cebo de GUARDIANA</h1>
<p>Esta página apunta quién visita las direcciones que GUARDIANA deja dentro de sus archivos
trampa <strong>en nuestras propias mediciones</strong>. No forma parte del programa: el programa
que se instala en un computador no habla nunca con este servidor, y una trampa puesta por una
persona se detecta dentro de su equipo, sin que nada salga de ahí.</p>
<p>De cada visita se guarda la hora, qué cebo, qué dirección pidió, con qué nombre se presentó el
programa y <strong>la red</strong> de la que vino. La dirección completa no se guarda en ningún
momento: se recorta antes de escribirla. Todo se borra solo a los ${DIAS} días.</p>
${filas}
</main></body></html>`;

Deno.serve(async (req: Request, info: Deno.ServeHandlerInfo) => {
  const url = new URL(req.url);
  const ip = (req.headers.get("x-forwarded-for") ?? "").split(",")[0].trim() ||
    (info.remoteAddr as Deno.NetAddr).hostname;
  const agente = req.headers.get("user-agent") ?? "";

  if (url.pathname.startsWith("/c/")) {
    const cebo = url.pathname.slice(3).split("/")[0];
    // Un cebo es un identificador corto que ponemos nosotros. Cualquier otra cosa se contesta
    // igual pero no se apunta: si no, basta con que alguien le tire piedras a esta dirección
    // para llenar la lista de basura y gastar el plan gratuito.
    if (!/^[a-z0-9-]{4,40}$/.test(cebo)) {
      return new Response("ok\n", { headers: { "content-type": "text/plain; charset=utf-8" } });
    }
    await anotar(cebo, url.pathname + url.search, ip, agente);
    // Respuesta sosa a propósito: quien muerde no tiene que enterarse de nada.
    return new Response("ok\n", { headers: { "content-type": "text/plain; charset=utf-8" } });
  }

  if (url.pathname === "/visitas.json") {
    return Response.json(await visitas());
  }

  const v = await visitas();
  const filas = v.length === 0
    ? '<p class="vacio">Todavía no ha venido nadie.</p>'
    : `<table><tr><th>Cuándo (UTC)</th><th>Cebo</th><th>Pidió</th><th>Red</th><th>Se presentó como</th></tr>` +
      v.map((x) =>
        `<tr><td><code>${escapar(new Date(Number(x.ahora)).toISOString())}</code></td>` +
        `<td><code>${escapar(String(x.cebo))}</code></td>` +
        `<td><code>${escapar(String(x.ruta))}</code></td>` +
        `<td><code>${escapar(String(x.red ?? "desconocida"))}</code></td>` +
        `<td>${escapar(String(x.agente).slice(0, 120))}</td></tr>`
      ).join("") + "</table>";
  return new Response(PAGINA(filas), { headers: { "content-type": "text/html; charset=utf-8" } });
});
