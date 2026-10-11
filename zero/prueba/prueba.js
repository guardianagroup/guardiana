// GUARDIANA ZERO, end to end on a clean Windows: start the real program, drive its own pages
// and a test site through the engine's debugging port, and check on the server side what left
// and what did not. Usage: node prueba.js <guardiana-zero.exe> <output folder>
'use strict';
const { spawn, execSync } = require('child_process');
const crypto = require('crypto');
const fs = require('fs');
const path = require('path');
const http = require('http');
const os = require('os');
const { chromium } = require('playwright-core');

const [exe, salida] = process.argv.slice(2);
fs.mkdirSync(salida, { recursive: true });
const base = path.join(salida, 'zero');
const PUERTO = 8080;
const SITIO = `http://sitio-prueba.test:${PUERTO}`;
const espera = (ms) => new Promise((r) => setTimeout(r, ms));
const fallos = [];
const bien = [];
const comprueba = (ok, que) => { (ok ? bien : fallos).push(que); console.log(`${ok ? 'OK  ' : 'FALLO'} ${que}`); };
const esc = (s) => String(s).replace(/%/g, '%25').replace(/\r/g, '%0D').replace(/\n/g, '%0A');

// --- the test site: one page that pulls from trackers, a form, and a leak ------------------
const registro = [];
const PNG = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=', 'base64');
function pagina(req, res) {
  const ruta = req.url.split('?')[0];
  if (ruta === '/propio.png') { res.writeHead(200, { 'Content-Type': 'image/png' }); return res.end(PNG); }
  if (ruta === '/enviar') { res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' }); return res.end('<!doctype html><title>Recibido</title><p id="r">recibido</p>'); }
  res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
  res.end(`<!doctype html><html lang="es"><head><meta charset="utf-8"><title>Sitio de prueba</title></head>
<body style="font:16px system-ui;padding:24px">
<h1>Sitio de prueba</h1>
<p>Una página corriente que, como tantas, llama a empresas de fuera.</p>
<img src="/propio.png" alt="propia">
<img src="http://stats.g.doubleclick.net:${PUERTO}/collect?v=1" alt="">
<script src="http://www.google-analytics.com:${PUERTO}/analytics.js"></script>
<iframe src="http://googleads.g.doubleclick.net:${PUERTO}/pagead/ads" width="300" height="60"></iframe>
<form id="f" action="/enviar" method="post"><label>Correo <input name="email" value="ana@correo.co"></label> <button id="b">Suscribirme</button></form>
<p><a id="nueva" href="/otra" target="_blank">Abrir en otra ventana</a></p>
<div id="onetrust-banner-sdk" style="position:fixed;bottom:0;left:0;right:0;padding:16px;background:#eee"><p>Usamos cookies</p><button id="onetrust-accept-btn-handler" onclick="fetch('/cookies-aceptadas')">Aceptar</button> <button id="onetrust-reject-all-handler" onclick="fetch('/cookies-rechazadas');this.parentNode.remove()">Rechazar todo</button></div>
</body></html>`);
}
const servidor = http.createServer((req, res) => {
  let cuerpo = '';
  req.on('data', (c) => { cuerpo += c; });
  req.on('end', () => {
    const host = String(req.headers.host || '').split(':')[0];
    registro.push({ t: Date.now(), host, metodo: req.method, ruta: req.url, gpc: req.headers['sec-gpc'] || '', cuerpo });
    if (host === 'sitio-prueba.test') return pagina(req, res);
    res.writeHead(200, { 'Content-Type': 'text/plain', 'Access-Control-Allow-Origin': '*' });
    res.end('ok');
  });
});

async function paginaQue(nav, cond, ms = 30000) {
  const fin = Date.now() + ms;
  while (Date.now() < fin) {
    for (const c of nav.contexts()) for (const p of c.pages()) { if (cond(p.url())) return p; }
    await espera(250);
  }
  throw new Error('no apareció la página esperada');
}
async function hasta(f, ms = 15000) {
  const fin = Date.now() + ms;
  while (Date.now() < fin) { try { if (await f()) return true; } catch (_) {} await espera(250); }
  return false;
}
function pantalla(nombre) {
  const ruta = path.join(salida, nombre).replace(/'/g, "''");
  try {
    execSync(`powershell -NoProfile -Command "Add-Type -AssemblyName System.Windows.Forms,System.Drawing; $b=[System.Windows.Forms.Screen]::PrimaryScreen.Bounds; $i=New-Object System.Drawing.Bitmap $b.Width,$b.Height; $g=[System.Drawing.Graphics]::FromImage($i); $g.CopyFromScreen($b.Location,[System.Drawing.Point]::Empty,$b.Size); $i.Save('${ruta}')"`, { stdio: 'ignore' });
  } catch (_) {}
}

(async () => {
  await new Promise((r) => servidor.listen(PUERTO, '127.0.0.1', r));
  const reglas = ['sitio-prueba.test', '*.doubleclick.net', 'www.google-analytics.com', 'collect.otra-empresa.io', 'www.facebook.com', 'connect.facebook.net', 'www.google.com', 'analytics.tiktok.com'].map((h) => `MAP ${h} 127.0.0.1`).join(',');
  const proceso = spawn(exe, [], {
    env: { ...process.env, GUARDIANA_ZERO_DATOS: base, GUARDIANA_ZERO_DEPURA: '1', GUARDIANA_ZERO_ARGS: `--remote-debugging-port=9222 --host-resolver-rules="${reglas}"` },
    stdio: 'ignore',
  });
  let salio = null;
  proceso.on('exit', (c) => { salio = c; });
  const arriba = await hasta(async () => (await fetch('http://127.0.0.1:9222/json/version')).ok, 90000);
  comprueba(arriba, 'el programa arranca y el motor responde');
  if (!arriba) throw new Error('sin motor');
  const nav = await chromium.connectOverCDP('http://127.0.0.1:9222');
  const barra = await paginaQue(nav, (u) => u.endsWith('/barra.html'));
  const panel = await paginaQue(nav, (u) => u.endsWith('/panel.html'));
  const inicio = await paginaQue(nav, (u) => u.endsWith('/inicio.html'));
  comprueba(true, 'barra, panel y pestaña nueva cargados desde zero.guardiana');
  comprueba(await hasta(async () => ['Nueva pestaña', 'New tab', 'Nova aba'].includes(await barra.textContent('.pestana .titulo'))), 'la barra habla el idioma de Windows y nombra la pestaña');
  comprueba(await hasta(async () => (await inicio.textContent('#frase')).length > 10), 'la pestaña nueva muestra la frase del día');
  comprueba(await hasta(() => panel.evaluate(() => document.querySelector('#v-bienvenida').classList.contains('vista-activa'))), 'la primera vez pregunta si cortar');
  await espera(800);
  pantalla('01-bienvenida.png');
  await panel.click('#b-si');
  comprueba(await hasta(() => barra.evaluate(() => !document.querySelector('#b-escudo').classList.contains('mirando'))), 'tras «Sí, cortarlos» el escudo corta');
  comprueba(await hasta(() => panel.evaluate(() => document.querySelector('#v-escudo').classList.contains('vista-activa'))), 'y el escudo se queda abierto al lado, en directo');
  // The language can be chosen; the rest of the test reads Spanish.
  await barra.click('#b-menu');
  await hasta(() => panel.evaluate(() => document.querySelector('#v-ajustes').classList.contains('vista-activa')));
  await panel.selectOption('#a-idioma', 'es');
  comprueba(await hasta(async () => (await barra.textContent('.pestana .titulo')) === 'Nueva pestaña'), 'cambiar el idioma a español cambia la barra al momento');
  comprueba(await hasta(async () => (await inicio.textContent('[data-t="inicio_mes"]')) === 'Tu mes en datos'), 'y la pestaña nueva también');

  // The subscription: the browser's own seven days start with the first run, kept twice, and
  // nothing has left for it yet.
  await panel.click('#a-licencia-ver');
  comprueba(await hasta(() => panel.evaluate(() => document.querySelector('#v-licencia').classList.contains('vista-activa'))), 'Ajustes lleva a «Suscripción»');
  comprueba(await hasta(async () => (await panel.textContent('#l-estado')).includes('quedan 7 días')), `la prueba propia empieza con 7 días (${await panel.textContent('#l-estado')})`);
  comprueba(fs.existsSync(path.join(base, 'marca', 'prueba-empezada')) && fs.existsSync(path.join(base, 'datos', 'licencia.db')), 'la fecha se guarda en dos sitios: los datos y una marca aparte');
  comprueba(await panel.evaluate(() => !document.querySelector('#l-conexiones-vacio').classList.contains('oculto')), 'la suscripción no se ha conectado con nadie');
  comprueba(await barra.evaluate(() => document.querySelector('#b-licencia').classList.contains('oculto')), 'con 7 días, la barra no dice nada de la prueba');
  await espera(500);
  pantalla('01b-suscripcion.png');

  // A page with trackers.
  await barra.fill('#campo-dir', `${SITIO}/prueba?utm_source=boletin&gclid=abc&id=7`);
  await barra.press('#campo-dir', 'Enter');
  const web = await paginaQue(nav, (u) => u.startsWith(SITIO));
  await web.waitForLoadState('load').catch(() => {});
  await espera(1500);
  const vio = (host, ruta) => registro.some((r) => r.host === host && (!ruta || r.ruta.startsWith(ruta)));
  const doc = registro.find((r) => r.host === 'sitio-prueba.test' && r.ruta.startsWith('/prueba'));
  comprueba(doc && doc.ruta === '/prueba?id=7', `las etiquetas de rastreo se quitan antes de salir (${doc && doc.ruta})`);
  comprueba(vio('sitio-prueba.test', '/propio.png'), 'lo propio de la página carga');
  comprueba(!vio('stats.g.doubleclick.net'), 'el píxel de DoubleClick no sale');
  comprueba(!vio('www.google-analytics.com'), 'Google Analytics no sale');
  comprueba(!vio('googleads.g.doubleclick.net'), 'el marco de anuncios no sale');
  comprueba(registro.filter((r) => r.host === 'sitio-prueba.test').every((r) => r.gpc === '1'), 'cada petición lleva Sec-GPC: 1');
  comprueba(await web.evaluate(() => navigator.globalPrivacyControl === true), 'navigator.globalPrivacyControl es true');
  comprueba(await web.evaluate(() => !(window.chrome && window.chrome.webview)), 'la página no ve el canal con el navegador');
  comprueba(await hasta(async () => vio('sitio-prueba.test', '/cookies-rechazadas')), 'el aviso de cookies se rechaza solo («Rechazar todo»)');
  comprueba(!vio('sitio-prueba.test', '/cookies-aceptadas'), 'y no se acepta nada');
  // The star keeps the page in the favourites.
  await barra.click('#estrella');
  comprueba(await hasta(() => barra.evaluate(() => document.querySelector('#estrella').getAttribute('aria-pressed') === 'true')), 'la estrella guarda la web en favoritos');
  const n = Number(await barra.textContent('#b-escudo-n'));
  comprueba(n === 1, `el escudo cuenta empresas, no dominios: DoubleClick y Analytics son Google (${n})`);
  comprueba(await hasta(async () => (await barra.textContent('#aviso-corte')).includes('Google')), 'la barra dice a quién cortó');
  await barra.click('#b-escudo');
  comprueba(await hasta(async () => (await panel.locator('#e-cortados li').count()) >= 1), 'el panel lista lo cortado');
  comprueba((await panel.textContent('#titulo')) === 'Conexiones con otras empresas', 'el escudo se titula «Conexiones con otras empresas»');
  const filas = await panel.locator('#e-cortados .quien').allTextContents();
  comprueba(filas.some((f) => f.includes('Google')), `el panel nombra a la empresa (${filas.join(', ')})`);
  // Every button has its way back: «Desbloquear», then «Volver a bloquear».
  const enCortados = () => panel.locator('#e-cortados li', { hasText: 'doubleclick.net' });
  const enVistos = () => panel.locator('#e-vistos li', { hasText: 'doubleclick.net' });
  comprueba((await enCortados().first().locator('button').textContent()) === 'Desbloquear', 'lo cortado ofrece «Desbloquear»');
  await enCortados().first().locator('button').click();
  comprueba(await hasta(async () => (await enVistos().count()) === 1 && (await enVistos().first().locator('button').textContent()) === 'Volver a bloquear'), 'desbloqueado, ofrece «Volver a bloquear» y dice que fue cosa tuya');
  await enVistos().first().locator('button').click();
  comprueba(await hasta(async () => (await enCortados().count()) === 1 && (await enVistos().count()) === 0), 'y vuelve a quedar cortado, como estaba');
  // Maximum protection: a beacon to another company, which passed, no longer leaves.
  const baliza = async (ruta) => { await web.evaluate(([p, r]) => navigator.sendBeacon(`http://collect.otra-empresa.io:${p}${r}`, 'x'), [PUERTO, ruta]); await espera(800); };
  await baliza('/baliza-1');
  comprueba(vio('collect.otra-empresa.io', '/baliza-1'), 'sin protección máxima, el aviso a otra empresa sale');
  await panel.click('#e-maxima-activar');
  comprueba(await hasta(() => panel.evaluate(() => !document.querySelector('#e-maxima-activa').classList.contains('oculto'))), '«Activar protección máxima» se enciende');
  await baliza('/baliza-2');
  comprueba(!vio('collect.otra-empresa.io', '/baliza-2'), 'con protección máxima, ese aviso ya no sale');
  // And it has its way back, like any other cut (review of 10 Oct 2026).
  const filaBaliza = () => panel.locator('#e-cortados li', { hasText: 'otra-empresa.io' });
  comprueba(await hasta(async () => (await filaBaliza().count()) === 1 && (await filaBaliza().first().locator('button').textContent()) === 'Desbloquear'), 'lo que corta la protección máxima sale como cortado, con «Desbloquear»');
  await filaBaliza().first().locator('button').click();
  await espera(300);
  await baliza('/baliza-3');
  comprueba(vio('collect.otra-empresa.io', '/baliza-3'), 'desbloqueado, ese aviso vuelve a salir');
  const balizaVista = panel.locator('#e-vistos li', { hasText: 'otra-empresa.io' });
  comprueba(await hasta(async () => (await balizaVista.count()) === 1 && (await balizaVista.first().locator('button').textContent()) === 'Volver a bloquear'), 'con protección máxima, lo desbloqueado ofrece «Volver a bloquear» (informe del 10 oct 2026)');
  comprueba(await panel.evaluate(() => { const b = document.activeElement; return !!b && !!b.closest('#e-vistos li') && b.textContent === 'Volver a bloquear'; }), 'y la fila se sigue: el foco queda en su botón «Volver a bloquear»');
  await balizaVista.first().locator('button').click();
  comprueba(await hasta(async () => (await filaBaliza().count()) === 1), 'y «Volver a bloquear» lo deja cortado otra vez');
  await panel.click('#e-maxima-quitar');
  comprueba(await hasta(() => panel.evaluate(() => document.querySelector('#e-maxima-activa').classList.contains('oculto'))), 'y se puede volver a la normal');
  await espera(600);
  pantalla('02-escudo.png');
  await web.screenshot({ path: path.join(salida, 'web.png') }).catch(() => {});
  await barra.screenshot({ path: path.join(salida, 'barra.png') }).catch(() => {});
  await panel.screenshot({ path: path.join(salida, 'panel.png') }).catch(() => {});

  // The form guard: the email leaves only after a yes.
  await web.evaluate(() => document.querySelector('#f').requestSubmit());
  comprueba(await hasta(() => panel.evaluate(() => document.querySelector('#v-formulario').classList.contains('vista-activa'))), 'antes de enviar un correo, pregunta');
  comprueba(!registro.some((r) => r.ruta === '/enviar'), 'mientras pregunta, el formulario no sale');
  await espera(500);
  pantalla('03-formulario.png');
  await panel.click('#f-enviar');
  comprueba(await hasta(() => registro.some((r) => r.ruta === '/enviar' && r.cuerpo.includes('ana%40correo.co'))), 'con el «Enviar», el formulario llega');

  // Marked data: a page tries to send the person's email to another company.
  await barra.click('#b-datos');
  await hasta(() => panel.evaluate(() => document.querySelector('#v-datos').classList.contains('vista-activa')));
  await panel.selectOption('#d-tipo', 'correo');
  await panel.fill('#d-valor', 'ana@correo.co');
  await panel.click('#d-nuevo button');
  comprueba(await hasta(async () => (await panel.textContent('#d-marcados')).includes('an•••@correo.co')), 'el dato marcado aparece enmascarado');
  const antes = registro.length;
  await web.evaluate(async (p) => {
    try { await fetch(`http://collect.otra-empresa.io:${p}/e`, { method: 'POST', body: JSON.stringify({ email: 'ana@correo.co' }) }); } catch (_) {}
    try { await fetch(`/guardar`, { method: 'POST', body: 'email=ana%40correo.co' }); } catch (_) {}
  }, PUERTO);
  await espera(800);
  const nuevas = registro.slice(antes);
  comprueba(!nuevas.some((r) => r.host === 'collect.otra-empresa.io'), 'el correo marcado no llega a otra empresa');
  comprueba(nuevas.some((r) => r.host === 'sitio-prueba.test' && r.ruta === '/guardar'), 'a la propia web sí llega');

  // «Los chivatos»: the shop's pixels try to tell Meta (by its path, www.facebook.com/tr) and
  // TikTok (a POSTed JSON) about a purchase, with the marked email hashed the way they hash it.
  // Nothing reaches them, and the shield says what they tried to tell.
  const hash = crypto.createHash('sha256').update('ana@correo.co').digest('hex');
  const antesChivatos = registro.length;
  await web.evaluate(async ([p, h]) => {
    const img = new Image();
    img.src = `http://www.facebook.com:${p}/tr?id=1&ev=Purchase&cd[value]=89900&cd[currency]=COP&ud[em]=${h}&eid=pedido-7`;
    // Without any data of the person: only its path says it is Meta's pixel.
    const vista = new Image();
    vista.src = `http://www.facebook.com:${p}/tr?id=1&ev=PageView&noscript=1`;
    // Meta's pixel script, and a Google Ads conversion on www.google.com: both told by their
    // path, on names that also serve other things.
    const guion = document.createElement('script');
    guion.src = `http://connect.facebook.net:${p}/en_US/fbevents.js`;
    document.head.appendChild(guion);
    const conversion = new Image();
    conversion.src = `http://www.google.com:${p}/pagead/1p-conversion/123/?value=89900&currency_code=COP`;
    try {
      await fetch(`http://analytics.tiktok.com:${p}/api/v2/pixel`, {
        method: 'POST',
        body: JSON.stringify({ event: 'CompletePayment', event_id: 'pedido-7', properties: { value: 89900, currency: 'COP' }, context: { user: { email: h } } }),
      });
    } catch (_) {}
  }, [PUERTO, hash]);
  await espera(1200);
  const chivatos = registro.slice(antesChivatos);
  comprueba(!chivatos.some((r) => r.host === 'www.facebook.com'), 'el píxel de Meta (www.facebook.com/tr) no llega');
  comprueba(!chivatos.some((r) => r.host === 'analytics.tiktok.com'), 'el píxel de TikTok no llega');
  comprueba(!chivatos.some((r) => r.host === 'connect.facebook.net'), 'el guion del píxel de Meta (connect.facebook.net/…/fbevents.js) no llega');
  comprueba(!chivatos.some((r) => r.host === 'www.google.com' && r.ruta.startsWith('/pagead/')), 'la conversión de Google Ads (www.google.com/pagead/1p-conversion) no llega');
  await barra.click('#b-escudo');
  await hasta(() => panel.evaluate(() => document.querySelector('#v-escudo').classList.contains('vista-activa')));
  const textoChivatos = async () => panel.evaluate(() => document.querySelector('#e-chivatos').textContent);
  comprueba(await hasta(async () => { const x = await textoChivatos(); return x.includes('A Meta') && x.includes('compra') && x.includes('89.900 COP'); }), `el escudo dice lo que la página intentó contarle a Meta: la compra y el importe (${await textoChivatos()})`);
  comprueba((await textoChivatos()).includes('con tu correo cifrado'), 'y que llevaba tu correo cifrado');
  comprueba(await panel.evaluate(() => [...document.querySelectorAll('#e-chivatos li')].every((l) => l.classList.contains('cortado'))), 'cada línea dice «cortado»');
  comprueba((await panel.textContent('#e-chivatos-titulo')) === 'Lo que esta página intentó contar de ti', 'la sección se titula «Lo que esta página intentó contar de ti»');
  const tarjeta = await panel.textContent('#e-tarjeta-frase');
  comprueba(tarjeta.includes('intentó contarles a 2 empresas que compré, con mi correo cifrado'), `la tarjeta para compartir lo dice en una frase (${tarjeta})`);
  comprueba(!/89|COP|ana@/.test(tarjeta), 'y no lleva importes ni el correo');
  comprueba((await panel.locator('#e-cortados li', { hasText: 'facebook.com' }).count()) === 1, 'Meta sale entre lo cortado');
  await espera(500);
  pantalla('03b-chivatos.png');

  // A new tab and a new window.
  await barra.click('#nueva');
  comprueba(await hasta(async () => (await barra.locator('.pestana').count()) === 2), 'el + abre una pestaña');
  await espera(500);
  pantalla('04-pestanas.png');

  // The new tab: our search box with its engine inside, and the day's cuts one click away.
  const nueva = await paginaQue(nav, (u) => u.endsWith('/inicio.html'));
  comprueba(await hasta(async () => (await nueva.getAttribute('#q', 'placeholder')) === 'Busca en la web o escribe una dirección'), 'la caja de búsqueda es nuestra');
  comprueba(await hasta(() => nueva.evaluate(() => (document.querySelector('#motor').selectedOptions[0] || {}).textContent === 'DuckDuckGo')), 'y lleva dentro el buscador, a la vista');
  comprueba(await hasta(async () => (await nueva.locator('#favs-rejilla .fav').count()) === 1), 'la web guardada con la estrella sale en la pestaña nueva');
  comprueba((await nueva.textContent('.hoy-cab .enlace')) === 'Ver resumen', 'el enlace del día dice «Ver resumen»');
  // The figure counts up when it first appears: read it once it has stopped moving.
  let cortadasHoy = 0;
  let anterior = -1;
  await hasta(async () => {
    cortadasHoy = Number((await nueva.textContent('#h-cortadas')).replace(/\D/g, ''));
    const quieto = cortadasHoy === anterior && cortadasHoy >= 3;
    anterior = cortadasHoy;
    if (!quieto) await espera(400);
    return quieto;
  });
  comprueba(cortadasHoy >= 3, `la pestaña nueva cuenta las peticiones cortadas (${cortadasHoy})`);
  await nueva.click('a.hecho.rojo');
  const cortes = await paginaQue(nav, (u) => u.endsWith('/cortes.html'));
  comprueba(await hasta(async () => (await cortes.locator('#filas tr.fila').count()) >= 3), '«peticiones cortadas» lleva a la lista, una a una');
  const total = Number((await cortes.textContent('#c-total')).replace(/\D/g, ''));
  comprueba(total === cortadasHoy, `la lista cuenta lo mismo que la pestaña nueva (${total} y ${cortadasHoy})`);
  const tabla = await cortes.textContent('#filas');
  comprueba(tabla.includes('Google') && tabla.includes('otra-empresa.io'), 'la lista nombra a cada empresa y su destino');
  comprueba(!tabla.includes('v=1'), 'sin los parámetros de la dirección');
  await cortes.click('#filas tr.fila');
  comprueba(await hasta(async () => (await cortes.locator('#filas tr.detalle').count()) === 1), 'cada corte se abre con su detalle');
  comprueba((await cortes.locator('#f-motivo option[value="@dato"]').count()) === 1, 'se puede ver solo lo que llevaba un dato tuyo');
  await espera(600);
  pantalla('05-cortes.png');
  await cortes.screenshot({ path: path.join(salida, 'cortes.png'), fullPage: true }).catch(() => {});
  // Saved as PDF and as CSV, in Downloads.
  const descargas = path.join(os.homedir(), 'Downloads');
  fs.mkdirSync(descargas, { recursive: true });
  const guardado = async (boton, ext) => {
    await cortes.evaluate(() => { document.getElementById('aviso').textContent = ''; });
    await cortes.click(boton);
    await hasta(async () => (await cortes.textContent('#aviso')).includes('.' + ext), 30000);
    const f = fs.readdirSync(descargas).filter((x) => x.startsWith('guardiana-zero-cortes') && x.endsWith('.' + ext));
    if (!f.length) return null;
    const ruta = path.join(descargas, f[0]);
    fs.copyFileSync(ruta, path.join(salida, 'cortes.' + ext));
    return fs.readFileSync(ruta);
  };
  const pdf = await guardado('#pdf', 'pdf');
  comprueba(!!pdf && pdf.subarray(0, 5).toString() === '%PDF-' && pdf.length > 5000, `«Guardar PDF» deja el informe en Descargas (${pdf ? pdf.length : 0} bytes)`);
  const csv = await guardado('#csv', 'csv');
  comprueba(!!csv && csv.toString('utf8').includes('Google') && csv.toString('utf8').split('\n').filter(Boolean).length === total + 1, '«Exportar CSV» guarda cada corte en una fila');
  // The summary: its own PDF, without the list; the list is back on screen afterwards.
  await cortes.evaluate(() => { document.getElementById('aviso').textContent = ''; });
  await cortes.click('#pdf-resumen');
  await hasta(async () => (await cortes.textContent('#aviso')).includes('.pdf'), 30000);
  const resumenes = fs.readdirSync(descargas).filter((x) => x.startsWith('guardiana-zero-resumen') && x.endsWith('.pdf'));
  const resumen = resumenes.length ? fs.readFileSync(path.join(descargas, resumenes[0])) : null;
  if (resumen) fs.copyFileSync(path.join(descargas, resumenes[0]), path.join(salida, 'resumen.pdf'));
  comprueba(!!resumen && resumen.subarray(0, 5).toString() === '%PDF-' && resumen.length > 3000, `«Resumen en PDF» deja su propio informe en Descargas (${resumen ? resumen.length : 0} bytes)`);
  comprueba(await cortes.evaluate(() => !document.body.classList.contains('solo-resumen')), 'y la lista vuelve a verse');
  // «Todo» is the whole log the browser keeps.
  await cortes.click('.periodos button[data-periodo="31"]');
  comprueba(await hasta(async () => (await cortes.textContent('.periodos button[data-periodo="31"]')) === 'Todo'), 'el periodo largo se llama «Todo»');
  // From the shield, too. Already on the list: it stays, no second copy.
  await barra.click('#b-escudo');
  await hasta(() => panel.evaluate(() => document.querySelector('#v-escudo').classList.contains('vista-activa')));
  comprueba(await hasta(async () => /empresas? de fuera/.test(await panel.textContent('#e-dia-frase'))), `el escudo enseña también el día entero (${await panel.textContent('#e-dia-frase')})`);
  await panel.click('#e-ver-todo');
  await espera(1000);
  comprueba((await barra.locator('.pestana').count()) === 2, 'desde la lista, «ver todo» no abre otra copia');
  // From the web page: a tab of its own, and «Volver» brings the web back.
  await barra.click('.pestana[aria-selected="false"]');
  await espera(500);
  const tituloWeb = await barra.textContent('.pestana[aria-selected="true"] .titulo');
  await barra.click('#b-escudo');
  await hasta(() => panel.evaluate(() => document.querySelector('#v-escudo').classList.contains('vista-activa')));
  await panel.click('#e-ver-todo');
  comprueba(await hasta(async () => (await barra.locator('.pestana').count()) === 3), 'el escudo también lleva a la lista');
  const otra = await paginaQue(nav, (u) => u.endsWith('/cortes.html') && nav.contexts().some((c) => c.pages().filter((p) => p.url().endsWith('/cortes.html')).length === 2));
  const lista2 = nav.contexts().flatMap((c) => c.pages()).filter((p) => p.url().endsWith('/cortes.html')).find((p) => p !== cortes) || otra;
  await lista2.click('#volver');
  comprueba(await hasta(async () => (await barra.locator('.pestana').count()) === 2), '«Volver» cierra la lista que se abrió aparte');
  comprueba(await hasta(async () => (await barra.textContent('.pestana[aria-selected="true"] .titulo')) === tituloWeb), `y vuelve a la web en la que estabas (${tituloWeb})`);

  // «Buscar actualización»: one read of the public ledger, when asked, and written down.
  await barra.click('#b-menu');
  await hasta(() => panel.evaluate(() => document.querySelector('#v-ajustes').classList.contains('vista-activa')));
  await panel.click('#a-buscar-version');
  comprueba(await hasta(async () => /última versión|versión nueva/.test(await panel.textContent('#a-version-res')), 30000), `«Buscar actualización» lee el registro público (${await panel.textContent('#a-version-res')})`);
  await panel.click('#a-licencia-ver');
  comprueba(await hasta(async () => (await panel.textContent('#l-conexiones')).includes('raw.githubusercontent.com')), 'y esa conexión queda anotada en «Suscripción»');

  // Closing writes everything down.
  try { execSync('taskkill /IM guardiana-zero.exe', { stdio: 'ignore' }); } catch (_) {}
  comprueba(await hasta(async () => salio !== null, 20000), 'cierra al pedírselo (sin forzar)');
  const datos = path.join(base, 'datos');
  const leer = (f) => { try { return JSON.parse(fs.readFileSync(path.join(datos, f), 'utf8')); } catch (_) { return null; } };
  const pref = leer('preferencias.json');
  comprueba(pref && pref.bienvenida === true && pref.reglas.cortar_seguimiento === true, 'guarda la decisión de cortar');
  const diario = leer('diario.json');
  comprueba(!!diario, 'guarda las cifras del día');
  const dias = Object.values((diario || {}).dias || {});
  comprueba(dias.length > 0 && dias.every((d) => !('sitio-prueba.test' in (d.terceros || {}))), 'la web que abres nunca cuenta como empresa de fuera');
  comprueba(dias.some((d) => d.parametros_quitados === 2), 'cuenta las 2 etiquetas de rastreo quitadas');
  comprueba((leer('libro.json') || { entradas: {} }).entradas['sitio-prueba.test'] !== undefined, 'el libro anota quién recibió el correo');

  // Eight days later, without a subscription: the trial mark says it started eight days ago.
  // The browser opens pages and does nothing of its own (decided 9 Oct 2026).
  fs.writeFileSync(path.join(base, 'marca', 'prueba-empezada'), String(Date.now() - 8 * 86400000));
  registro.length = 0;
  salio = null;
  const proceso2 = spawn(exe, [], {
    env: { ...process.env, GUARDIANA_ZERO_DATOS: base, GUARDIANA_ZERO_DEPURA: '1', GUARDIANA_ZERO_ARGS: `--remote-debugging-port=9222 --host-resolver-rules="${reglas}"` },
    stdio: 'ignore',
  });
  proceso2.on('exit', (c) => { salio = c; });
  comprueba(await hasta(async () => (await fetch('http://127.0.0.1:9222/json/version')).ok, 90000), 'vuelve a arrancar con la prueba terminada');
  const nav2 = await chromium.connectOverCDP('http://127.0.0.1:9222');
  const barra2 = await paginaQue(nav2, (u) => u.endsWith('/barra.html'));
  const panel2 = await paginaQue(nav2, (u) => u.endsWith('/panel.html'));
  comprueba(await hasta(async () => (await barra2.textContent('#b-licencia')) === 'Prueba terminada: sin protección'), `la barra dice que ya no protege (${await barra2.textContent('#b-licencia')})`);
  comprueba(await barra2.evaluate(() => document.querySelector('#b-escudo').classList.contains('oculto')), 'y el escudo, que no contaría nada, deja su sitio');
  const rechazosAntes = registro.filter((r) => r.ruta === '/cookies-rechazadas').length;
  await barra2.fill('#campo-dir', `${SITIO}/prueba?utm_source=boletin&id=7`);
  await barra2.press('#campo-dir', 'Enter');
  const web2 = await paginaQue(nav2, (u) => u.startsWith(SITIO));
  await web2.waitForLoadState('load').catch(() => {});
  await espera(1500);
  comprueba(registro.some((r) => r.host === 'sitio-prueba.test' && r.ruta.startsWith('/prueba')), 'las páginas se siguen abriendo');
  // Only the engine's own tracking prevention remains (balanced, Microsoft Edge's default): it
  // still stops some known trackers by itself. What GUARDIANA ZERO cut and the engine does not,
  // the ad frame, now arrives.
  comprueba(registro.some((r) => r.host === 'googleads.g.doubleclick.net'), 'pero GUARDIANA ZERO ya no corta: el marco de anuncios llega');
  comprueba(registro.some((r) => r.ruta.includes('utm_source=boletin')), 'ni se limpian las direcciones');
  comprueba(registro.filter((r) => r.ruta === '/cookies-rechazadas').length === rechazosAntes, 'ni se tocan los avisos de cookies');
  await barra2.click('#b-mandato');
  comprueba(await hasta(() => panel2.evaluate(() => document.querySelector('#v-licencia').classList.contains('vista-activa'))), 'los mandatos llevan a «Suscripción»');
  comprueba(await hasta(async () => (await panel2.textContent('#l-detalle')).includes('La prueba terminó')), 'que explica qué dejó de hacer');
  await espera(500);
  pantalla('06-prueba-terminada.png');
  await panel2.screenshot({ path: path.join(salida, 'panel-suscripcion.png') }).catch(() => {});
  // A key the gateway does not know: one connection, made away from the window, and a reason.
  await panel2.fill('#l-clave', 'CLAVE-DE-PRUEBA-QUE-NO-EXISTE');
  await panel2.click('#l-activar');
  comprueba(await hasta(async () => (await panel2.textContent('#l-error')).length > 10, 60000), `una clave que no vale dice por qué (${await panel2.textContent('#l-error')})`);
  comprueba(await hasta(async () => (await panel2.textContent('#l-conexiones')).includes('dodopayments.com')), 'y la conexión con la pasarela queda anotada');
  comprueba(await barra2.evaluate(() => !document.querySelector('#b-licencia').classList.contains('oculto')), 'sin clave válida sigue sin proteger');
  try { execSync('taskkill /IM guardiana-zero.exe', { stdio: 'ignore' }); } catch (_) {}
  comprueba(await hasta(async () => salio !== null, 20000), 'cierra otra vez sin forzar');
})().catch((e) => { comprueba(false, `la prueba se interrumpió: ${e && e.message}`); }).finally(() => {
  try { execSync('taskkill /F /IM guardiana-zero.exe', { stdio: 'ignore' }); } catch (_) {}
  fs.writeFileSync(path.join(salida, 'servidor.json'), JSON.stringify(registro, null, 1));
  let reg = '';
  try { reg = fs.readFileSync(path.join(base, 'datos', 'registro.txt'), 'utf8'); } catch (_) {}
  // The browser's own log with every decision: what was a third party and why.
  if (reg) {
    const terceros = reg.split(/\r?\n/).filter((l) => /tercero=true|navegación|crear|preparar|motor|guion/.test(l)).join('\n');
    console.log(`::warning title=registro.txt::${esc(terceros.slice(-12000))}`);
  }
  for (const f of fallos) console.log(`::error title=prueba de punta a punta::${esc(f)}`);
  console.log(`::notice title=prueba de punta a punta::${bien.length} bien, ${fallos.length} fallos`);
  servidor.close();
  process.exit(fallos.length ? 1 : 0);
});
