// GUARDIANA ZERO, end to end on a clean Windows: start the real program, drive its own pages
// and a test site through the engine's debugging port, and check on the server side what left
// and what did not. Usage: node prueba.js <guardiana-zero.exe> <output folder>
'use strict';
const { spawn, execSync } = require('child_process');
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
  const reglas = ['sitio-prueba.test', '*.doubleclick.net', 'www.google-analytics.com', 'collect.otra-empresa.io'].map((h) => `MAP ${h} 127.0.0.1`).join(',');
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
  // The language can be chosen; the rest of the test reads Spanish.
  await barra.click('#b-menu');
  await hasta(() => panel.evaluate(() => document.querySelector('#v-ajustes').classList.contains('vista-activa')));
  await panel.selectOption('#a-idioma', 'es');
  comprueba(await hasta(async () => (await barra.textContent('.pestana .titulo')) === 'Nueva pestaña'), 'cambiar el idioma a español cambia la barra al momento');
  comprueba(await hasta(async () => (await inicio.textContent('[data-t="inicio_mes"]')) === 'Tu mes en datos'), 'y la pestaña nueva también');

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
  const n = Number(await barra.textContent('#b-escudo-n'));
  comprueba(n === 1, `el escudo cuenta empresas, no dominios: DoubleClick y Analytics son Google (${n})`);
  comprueba(await hasta(async () => (await barra.textContent('#aviso-corte')).includes('Google')), 'la barra dice a quién cortó');
  await barra.click('#b-escudo');
  comprueba(await hasta(async () => (await panel.locator('#e-cortados li').count()) >= 1), 'el panel lista lo cortado');
  const filas = await panel.locator('#e-cortados .quien').allTextContents();
  comprueba(filas.some((f) => f.includes('Google')), `el panel nombra a la empresa (${filas.join(', ')})`);
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

  // A new tab and a new window.
  await barra.click('#nueva');
  comprueba(await hasta(async () => (await barra.locator('.pestana').count()) === 2), 'el + abre una pestaña');
  await espera(500);
  pantalla('04-pestanas.png');

  // The new tab: our search box, saying where searches go, and the day's cuts one click away.
  const nueva = await paginaQue(nav, (u) => u.endsWith('/inicio.html'));
  comprueba(await hasta(async () => (await nueva.getAttribute('#q', 'placeholder')) === 'Busca en la web o escribe una dirección'), 'la caja de búsqueda es nuestra');
  comprueba(await hasta(async () => (await nueva.textContent('#motor-nota')).includes('DuckDuckGo')), 'y dice a dónde van las búsquedas');
  let cortadasHoy = 0;
  await hasta(async () => { cortadasHoy = Number((await nueva.textContent('#h-cortadas')).replace(/\D/g, '')); return cortadasHoy >= 3; });
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
  // From the shield, too.
  await barra.click('#b-escudo');
  await hasta(() => panel.evaluate(() => document.querySelector('#v-escudo').classList.contains('vista-activa')));
  await panel.click('#e-ver-todo');
  comprueba(await hasta(async () => (await barra.locator('.pestana').count()) === 3), 'el escudo también lleva a la lista');

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
