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
// 1.0.55 («más rápido»): how long pages took to load in the real window, for the run's annotations (a measure, never a
// failure): the engine's own Navigation Timing of the page, in ms.
const cargas = [];
async function midecarga(que, pagina) {
  try {
    const n = await pagina.evaluate(() => { const e = performance.getEntriesByType('navigation')[0]; return e ? { respuesta: e.responseStart - e.startTime, dom: e.domContentLoadedEventEnd - e.startTime, carga: e.loadEventEnd - e.startTime } : null; });
    if (n) cargas.push(`${que}: primera respuesta ${n.respuesta.toFixed(1)} ms, DOM listo ${n.dom.toFixed(1)} ms, carga completa ${n.carga.toFixed(1)} ms`);
  } catch (_) {}
}
const esc = (s) => String(s).replace(/%/g, '%25').replace(/\r/g, '%0D').replace(/\n/g, '%0A');

// --- the test site: one page that pulls from trackers, a form, and a leak ------------------
const registro = [];
const PNG = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=', 'base64');
function pagina(req, res) {
  const ruta = req.url.split('?')[0];
  if (ruta === '/propio.png') { res.writeHead(200, { 'Content-Type': 'image/png' }); return res.end(PNG); }
  // 1.0.46 (Z4): a long page, to try the mouse wheel for real.
  if (ruta === '/largo') { res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' }); return res.end('<!doctype html><title>Larga</title><body style="font:16px system-ui;margin:0">' + Array.from({ length: 200 }, (_, i) => `<p style="margin:0;padding:12px 24px">Línea ${i + 1}</p>`).join('') + '</body>'); }
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
// A script of another company that reads back a canvas with text on it, the way fingerprinting
// scripts do (decision 206: measured, never changed). It reads twice: the same both times.
const FP = `(function () {
  var c = document.createElement('canvas'); c.width = 240; c.height = 60;
  var x = c.getContext('2d'); x.textBaseline = 'top'; x.font = '14px Arial';
  x.fillText('Cwm fjordbank glyphs vext quiz', 2, 2);
  var a = c.toDataURL(); var b = c.toDataURL();
  window.__zeroHuella = a.length; window.__zeroHuellaIgual = a === b;
  window.__zeroNativo = Function.prototype.toString.call(HTMLCanvasElement.prototype.toDataURL);
  window.__zeroFpUrl = a;
})();`;
// A frame of another company that does the same from its own inline script.
const MARCO = `<!doctype html><meta charset="utf-8"><script>
  var c = document.createElement('canvas'); c.width = 240; c.height = 60;
  var x = c.getContext('2d'); x.textBaseline = 'top'; x.font = '14px Arial';
  x.fillText('Sphinx of black quartz, judge my vow', 2, 2);
  c.toDataURL();
</script><p>marco</p>`;
const servidor = http.createServer((req, res) => {
  let cuerpo = '';
  req.on('data', (c) => { cuerpo += c; });
  req.on('end', () => {
    const host = String(req.headers.host || '').split(':')[0];
    registro.push({ t: Date.now(), host, metodo: req.method, ruta: req.url, gpc: req.headers['sec-gpc'] || '', cuerpo });
    if (host === 'sitio-prueba.test') return pagina(req, res);
    if (host === 'scripts.medidor-prueba.io') { res.writeHead(200, { 'Content-Type': 'application/javascript' }); return res.end(FP); }
    if (host === 'marco-prueba.io') { res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' }); return res.end(MARCO); }
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
// A real turn of the mouse wheel over the page area of ZERO's window (Windows' own input, not
// the engine's debugging port: the owner's report was about where Windows sends the wheel).
function rueda(pid) {
  const ps = path.join(os.tmpdir(), 'zero-rueda.ps1');
  fs.writeFileSync(ps, `Add-Type @"
using System; using System.Runtime.InteropServices;
public class Rueda {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, int d, UIntPtr e);
}
"@
$h = (Get-Process -Id ${pid}).MainWindowHandle
$r = New-Object Rueda+RECT
[Rueda]::GetWindowRect($h, [ref]$r) | Out-Null
[Rueda]::SetCursorPos([int](($r.L + $r.R) / 2) - 220, [int](($r.T + $r.B) / 2) + 120) | Out-Null
Start-Sleep -Milliseconds 300
for ($i = 0; $i -lt 6; $i++) { [Rueda]::mouse_event(0x0800, 0, 0, -120, [UIntPtr]::Zero); Start-Sleep -Milliseconds 120 }
`);
  try { execSync(`powershell -NoProfile -ExecutionPolicy Bypass -File "${ps}"`, { stdio: 'ignore' }); } catch (_) {}
}
// 5 Nov 2026: what Windows says of the VPN's icon next to the clock (`--avisos`'s hidden window, icon number 1): whether
// it has it and where (Shell_NotifyIconGetRect), and, when it is on the screen, the colour of its dot (the bottom right
// of the icon) and a picture of it, enlarged, in `recorte`.
function iconoEnWindows(recorte) {
  const ps = path.join(os.tmpdir(), 'zero-icono-vpn.ps1');
  const destino = recorte ? path.join(salida, recorte).replace(/'/g, "''") : '';
  fs.writeFileSync(ps, `
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class IconoVpn {
  [StructLayout(LayoutKind.Sequential)] public struct ID { public uint cbSize; public IntPtr hWnd; public uint uID; public Guid guidItem; }
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("shell32.dll")] public static extern int Shell_NotifyIconGetRect(ref ID id, out RECT r);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindowW(string c, string t);
}
"@
$h = [IconoVpn]::FindWindowW('GuardianaAvisos', $null)
if ($h -eq [IntPtr]::Zero) { 'sin-ventana'; exit }
$id = New-Object IconoVpn+ID
$id.cbSize = [System.Runtime.InteropServices.Marshal]::SizeOf([type][IconoVpn+ID])
$id.hWnd = $h
$id.uID = 1
$r = New-Object IconoVpn+RECT
$hr = [IconoVpn]::Shell_NotifyIconGetRect([ref]$id, [ref]$r)
$w = $r.R - $r.L; $a = $r.B - $r.T
$color = ''
if ($hr -eq 0 -and $w -gt 4 -and $a -gt 4 -and $w -lt 200 -and $a -lt 200) {
  $b = New-Object System.Drawing.Bitmap $w, $a
  $g = [System.Drawing.Graphics]::FromImage($b)
  $g.CopyFromScreen($r.L, $r.T, 0, 0, $b.Size)
  # the most saturated pixel: the dot when it is green, amber or red; the padlock's pale blue when it is grey
  $mejor = -1
  for ($x = 0; $x -lt $w; $x++) { for ($y = 0; $y -lt $a; $y++) {
    $p = $b.GetPixel($x, $y)
    $s = [Math]::Max($p.R, [Math]::Max($p.G, $p.B)) - [Math]::Min($p.R, [Math]::Min($p.G, $p.B))
    if ($s -gt $mejor) { $mejor = $s; $color = "$($p.R),$($p.G),$($p.B)" }
  } }
  if ('${destino}') {
    $z = New-Object System.Drawing.Bitmap ($w * 8), ($a * 8)
    $gz = [System.Drawing.Graphics]::FromImage($z)
    $gz.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::NearestNeighbor
    $gz.DrawImage($b, 0, 0, $w * 8, $a * 8)
    $z.Save('${destino}')
  }
}
"$hr $($r.L) $($r.T) $w $a $color"
`);
  try { return execSync(`powershell -NoProfile -ExecutionPolicy Bypass -File "${ps}"`, { encoding: 'utf8' }).trim(); } catch (e) { return `error ${(e && e.message) || ''}`.slice(0, 300); }
}
function pantalla(nombre) {
  const ruta = path.join(salida, nombre).replace(/'/g, "''");
  try {
    execSync(`powershell -NoProfile -Command "Add-Type -AssemblyName System.Windows.Forms,System.Drawing; $b=[System.Windows.Forms.Screen]::PrimaryScreen.Bounds; $i=New-Object System.Drawing.Bitmap $b.Width,$b.Height; $g=[System.Drawing.Graphics]::FromImage($i); $g.CopyFromScreen($b.Location,[System.Drawing.Point]::Empty,$b.Size); $i.Save('${ruta}')"`, { stdio: 'ignore' });
  } catch (_) {}
}

(async () => {
  await new Promise((r) => servidor.listen(PUERTO, '127.0.0.1', r));
  // «Mis datos» left the browser (2 Nov 2026, the owner, 16:50); what a person marked before it left still protects.
  // The test starts as someone who had marked their email («Tinta»), kept in the data folder as the section kept it.
  fs.mkdirSync(path.join(base, 'datos'), { recursive: true });
  fs.writeFileSync(path.join(base, 'datos', 'tinta.json'), JSON.stringify([{ tipo: 'correo', valor: 'ana@correo.co' }]));
  const reglas = ['sitio-prueba.test', '*.doubleclick.net', 'www.google-analytics.com', 'collect.otra-empresa.io', 'www.facebook.com', 'connect.facebook.net', 'www.google.com', 'analytics.tiktok.com', 'scripts.medidor-prueba.io', 'static.hotjar.com', 'marco-prueba.io'].map((h) => `MAP ${h} 127.0.0.1`).join(',');
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
  // A bar button opens its view, and the same button closes it again: open only what is not
  // already open (the chivatos check leaves the shield open; clicking it again hid the panel).
  const abreVista = async (id) => {
    if (!(await barra.evaluate((i) => document.getElementById(i).classList.contains('activo'), id))) await barra.click('#' + id);
  };
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
  await midecarga('página de prueba (rastreadores, aviso de cookies, formulario)', web);
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
  // 2 Nov 2026 (the owner, 16:50: «que aparezca ya en la pantalla del navegador»): the three protections beside «Protegido»,
  // as switches with their names, the same settings as «Ajustes».
  for (const nombre of ['Rastreadores y anuncios', 'Avisos de cookies', 'Ventanas emergentes']) {
    const sw = barra.getByRole('switch', { name: nombre, exact: true });
    comprueba((await sw.isVisible()) && (await sw.isChecked()), `en la barra, «${nombre}» a la vista y encendido`);
  }
  await barra.getByRole('switch', { name: 'Avisos de cookies', exact: true }).click();
  comprueba(await hasta(() => panel.evaluate(() => document.getElementById('a-cookies').checked === false)), 'apagar «Avisos de cookies» en la barra lo apaga en «Ajustes»');
  await barra.getByRole('switch', { name: 'Avisos de cookies', exact: true }).click();
  comprueba(await hasta(() => panel.evaluate(() => document.getElementById('a-cookies').checked === true)), 'y encenderlo en la barra lo enciende');
  pantalla('z2-interruptores.png');
  // The VPN (2 Nov 2026, the VPN review): no chip while there is none; given a VPN whose server does not answer (as the
  // program would say it), the chip and its «Desconectar», which reaches the program. Here there is no GUARDIANA service
  // behind, so the program answers that there is no VPN and the chip goes.
  comprueba(await barra.evaluate(() => document.getElementById('vpn').classList.contains('oculto')), 'sin VPN, la barra no enseña su chip');
  await barra.evaluate(() => Z.recibe({ tipo: 'vpn', fase: 'sin_respuesta', desconectando: false, aviso: null }));
  const chip = barra.getByRole('group', { name: 'VPN de GUARDIANA' });
  comprueba(await hasta(async () => (await chip.isVisible()) && (await chip.textContent()).includes('VPN sin respuesta')), 'con la VPN sin respuesta, su chip lo dice junto a «Protegido»');
  await espera(300);
  pantalla('z4-vpn.png');
  await chip.getByRole('button', { name: 'Desconectar', exact: true }).click();
  comprueba(await hasta(() => barra.evaluate(() => document.getElementById('vpn').classList.contains('oculto'))), '«Desconectar» llega al programa, que contesta (sin servicio de GUARDIANA aquí: sin VPN, el chip se va)');
  // The star keeps the page in the favourites.
  await barra.click('#estrella');
  comprueba(await hasta(() => barra.evaluate(() => document.querySelector('#estrella').getAttribute('aria-pressed') === 'true')), 'la estrella guarda la web en favoritos');
  // 1.0.46 (Z1-b): the bar's shield counts the attempts cut on this page (the pixel, the script
  // and the advertising frame), and says so beside the figure.
  const n = Number((await barra.textContent('#b-escudo-n')).replace(/\D/g, ''));
  comprueba(n >= 3, `el escudo cuenta los intentos cortados en esta página (${n})`);
  comprueba((await barra.textContent('#b-escudo-que')) === 'en esta página', 'y dice «en esta página»');
  // 2 Nov 2026 (the owner, 08:01: no doors between sections): the bar has no products' selector, only «Inicio» (back to
  // the main cover, Órbita's tab), and nothing else that leads to another section.
  comprueba(await barra.evaluate(() => { const b = [...document.querySelectorAll('#productos [data-producto]')]; return b.length === 1 && b[0].dataset.producto === 'orbita'; }), 'la barra solo tiene «Inicio», que vuelve a la portada');
  // Beside a web, the shield open (384 px) since 5 Nov 2026, or folded into its rail of 152 if the person folded it, or the
  // view that was open. A click on the web no longer folds it.
  comprueba(await hasta(() => panel.evaluate(() => document.body.classList.contains('en-escudo') || document.body.classList.contains('plegado') || document.querySelector('#v-licencia').classList.contains('vista-activa'))), 'junto a la web, el escudo abierto (o plegado en su carril, o la vista que estaba abierta)');
  comprueba(await hasta(async () => (await barra.textContent('#aviso-corte')).includes('Google')), 'la barra dice a quién cortó');
  await abreVista('b-escudo');
  comprueba(await hasta(async () => (await panel.locator('#e-terceros li.cortado').count()) >= 1), 'el panel lista lo cortado');
  comprueba(await hasta(async () => Number((await panel.textContent('#z1-n')).replace(/\D/g, '')) === n), 'el resumen de la web dice la misma cifra que la barra');
  comprueba(await panel.evaluate(() => !!document.getElementById('zero-subasta')), 'y deja su sitio a «Tu visita, a subasta»');
  await espera(600);
  pantalla('z1-b-resumen.png');
  comprueba((await panel.textContent('#titulo')) === 'Conexiones con otras empresas', 'el escudo se titula «Conexiones con otras empresas»');
  const filas = await panel.locator('#e-terceros .quien').allTextContents();
  comprueba(filas.some((f) => f.includes('Google')), `el panel nombra a la empresa (${filas.join(', ')})`);
  // The rows never move (the owner, 10 Oct 2026): one list, in the order each company first
  // showed up. A button press changes the row's colour, chip and words, not its place, not the
  // focus, and not any figure of what already happened.
  const fila = (sitio) => panel.locator('#e-terceros li', { hasText: sitio });
  const posicion = (sitio) => panel.evaluate((x) => [...document.querySelectorAll('#e-terceros li')].findIndex((l) => l.dataset.sitio === x), sitio);
  const caja = (sitio) => panel.evaluate((x) => { const l = [...document.querySelectorAll('#e-terceros li')].find((y) => y.dataset.sitio === x); const r = l.getBoundingClientRect(); const b = l.querySelector('.botones').getBoundingClientRect(); return [Math.round(r.height), Math.round(b.left), Math.round(b.width)]; }, sitio);
  const cifras = async () => JSON.stringify({
    barra: await barra.textContent('#b-escudo-n'),
    resumen: await panel.textContent('#e-resumen'),
    filas: await panel.evaluate(() => [...document.querySelectorAll('#e-terceros li .linea')].map((l) => l.textContent)),
    // «Hoy, en todo el navegador», at the top of the shield: the same figures as the new tab.
    dia: await panel.evaluate(() => ['e-dia-frase', 'e-dia-cortadas', 'e-dia-datos', 'e-dia-parametros', 'e-dia-corredores'].map((k) => document.getElementById(k).textContent)),
  });
  const enfocado = () => panel.evaluate(() => { const b = document.activeElement; const l = b && b.closest && b.closest('#e-terceros li'); return l ? [l.dataset.sitio, b.getAttribute('aria-checked') || b.textContent] : null; });
  // A switch press (18 Oct 2026: the cut switch replaced «Bloquear» / «Desbloquear»), and what
  // must hold after it: the same place, height and button column, the focus on that row's
  // switch, the figures untouched; only the switch and its word change.
  // aria-checked="true" is «Permitido» and «false» is «Cortado» (the switch's common standard).
  const interruptor = (sitio) => fila(sitio).locator('.botones button.interruptor');
  const pulsa = async (sitio, cortadoAntes, que) => {
    const pos = await posicion(sitio);
    const medidas = await caja(sitio);
    await espera(1500);
    const antes = await cifras();
    const sw = interruptor(sitio);
    comprueba((await sw.count()) === 1 && (await sw.getAttribute('aria-checked')) === String(!cortadoAntes) && !(await sw.isDisabled()), `${que}: la fila tiene su interruptor ${cortadoAntes ? 'en rojo (cortado)' : 'en verde (permitido)'}`);
    await sw.click();
    comprueba(await hasta(async () => (await sw.getAttribute('aria-checked')) === String(cortadoAntes) && (await sw.getAttribute('aria-busy')) === null), `${que}: el interruptor pasa a ${cortadoAntes ? 'verde (permitido)' : 'rojo (cortado)'} en su sitio`);
    const palabra = await fila(sitio).locator('.botones .interruptor-estado').textContent();
    comprueba(!!palabra && palabra.trim().length > 0, `${que}: el interruptor dice su estado en palabras («${palabra}»)`);
    comprueba((await posicion(sitio)) === pos, `${que}: la fila sigue en la posición ${pos}`);
    comprueba(JSON.stringify(await caja(sitio)) === JSON.stringify(medidas), `${que}: misma altura y misma columna de botones (${JSON.stringify(medidas)})`);
    const foco = await enfocado();
    comprueba(!!foco && foco[0] === sitio, `${que}: el foco sigue en un botón de esa fila (${foco})`);
    await espera(1500);
    const despues = await cifras();
    comprueba(despues === antes, `${que}: las cifras del escudo, del día y de «Hoy» no cambian al pulsar${despues === antes ? '' : ` (${antes} → ${despues})`}`);
  };
  await pulsa('doubleclick.net', true, 'desbloquear');
  comprueba(await fila('doubleclick.net').evaluate((l) => !l.classList.contains('cortado') && l.textContent.includes('permitido por ti')), 'desbloqueado, la fila dice que fue cosa tuya');
  await pulsa('doubleclick.net', false, 'volver a bloquear');
  comprueba(await fila('doubleclick.net').evaluate((l) => l.classList.contains('cortado')), 'y vuelve a quedar cortado, como estaba');
  // Maximum protection: a beacon to another company, which passed, no longer leaves.
  const baliza = async (ruta) => { await web.evaluate(([p, r]) => navigator.sendBeacon(`http://collect.otra-empresa.io:${p}${r}`, 'x'), [PUERTO, ruta]); await espera(800); };
  await baliza('/baliza-1');
  comprueba(vio('collect.otra-empresa.io', '/baliza-1'), 'sin protección máxima, el aviso a otra empresa sale');
  await panel.click('#e-maxima-activar');
  comprueba(await hasta(() => panel.evaluate(() => !document.querySelector('#e-maxima-activa').classList.contains('oculto'))), '«Activar protección máxima» se enciende');
  await baliza('/baliza-2');
  comprueba(!vio('collect.otra-empresa.io', '/baliza-2'), 'con protección máxima, ese aviso ya no sale');
  // And it has its way back, like any other cut (review of 10 Oct 2026).
  comprueba(await hasta(async () => (await fila('otra-empresa.io').count()) === 1 && (await fila('otra-empresa.io').evaluate((l) => l.classList.contains('cortado') || l.classList.contains('parcial')))), 'lo que corta la protección máxima sale como cortado, con «Desbloquear»');
  await pulsa('otra-empresa.io', true, 'protección máxima, desbloquear');
  await baliza('/baliza-3');
  comprueba(vio('collect.otra-empresa.io', '/baliza-3'), 'desbloqueado, ese aviso vuelve a salir');
  await pulsa('otra-empresa.io', false, 'protección máxima, volver a bloquear');
  await panel.click('#e-maxima-quitar');
  comprueba(await hasta(() => panel.evaluate(() => document.querySelector('#e-maxima-activa').classList.contains('oculto'))), 'y se puede desactivar');
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

  // 2 Nov 2026 (the owner, 16:50): «Tarea para tu IA», «Preguntar a una IA sin dar tus datos» and «Mis datos» left
  // the browser: no button, view or page of theirs is left in the bar or the panel.
  comprueba(await barra.evaluate(() => !document.querySelector('#b-mandato, #b-tachon, #b-datos, #b-parar')), 'la barra ya no lleva la tarea para tu IA, la pregunta a una IA ni «Mis datos»');
  comprueba(await panel.evaluate(() => !document.querySelector('#v-mandato, #v-tachon, #v-datos')), 'ni el panel sus vistas');
  // Marked data (the email marked before, see tinta.json above): a page tries to send it to another company.
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
  await abreVista('b-escudo');
  await hasta(() => panel.evaluate(() => document.querySelector('#v-escudo').classList.contains('vista-activa')));
  const textoChivatos = async () => panel.evaluate(() => document.querySelector('#e-chivatos').textContent);
  comprueba(await hasta(async () => { const x = await textoChivatos(); return x.includes('A Meta') && x.includes('compra') && x.includes('89.900 COP'); }), `el escudo dice lo que la página intentó contarle a Meta: la compra y el importe (${await textoChivatos()})`);
  comprueba((await textoChivatos()).includes('con tu correo convertido en hash'), 'y que llevaba tu correo cifrado');
  comprueba(await panel.evaluate(() => [...document.querySelectorAll('#e-chivatos li')].every((l) => l.classList.contains('cortado'))), 'cada línea dice «cortado»');
  comprueba((await panel.textContent('#e-chivatos-titulo')) === 'Lo que esta página intentó contar de ti', 'la sección se titula «Lo que esta página intentó contar de ti»');
  const tarjeta = await panel.textContent('#e-tarjeta-frase');
  comprueba(tarjeta.includes('intentó contarles a 2 empresas que compré, con mi correo convertido en hash'), `la tarjeta para compartir lo dice en una frase (${tarjeta})`);
  comprueba(!/89|COP|ana@/.test(tarjeta), 'y no lleva importes ni el correo');
  comprueba(await panel.locator('#e-terceros li.cortado', { hasText: 'facebook.com' }).count() === 1, 'Meta sale cortada');
  await espera(500);
  pantalla('03b-chivatos.png');

  // Decision 206: measured, never cut. A script of another company reads a canvas with text on
  // it, and the page loads a session recording service (Hotjar, by its name).
  await web.evaluate((p) => {
    for (const src of [`http://scripts.medidor-prueba.io:${p}/fp.js`, `http://static.hotjar.com:${p}/c/hotjar-1.js`]) {
      const s = document.createElement('script');
      s.src = src;
      document.head.appendChild(s);
    }
  }, PUERTO);
  comprueba(await hasta(() => web.evaluate(() => typeof window.__zeroHuella === 'number')), 'el guion de otra empresa se ejecuta');
  comprueba(await web.evaluate(() => window.__zeroHuella > 100 && window.__zeroHuellaIgual === true), 'y lee el lienzo como siempre: la medida no cambia lo que devuelve');
  comprueba(await web.evaluate(() => window.__zeroNativo === 'function toDataURL() { [native code] }'), `y la función sigue pareciendo la original (${await web.evaluate(() => window.__zeroNativo)})`);
  const textoMedidas = async () => panel.evaluate(() => document.querySelector('#e-medidas').textContent);
  comprueba(await hasta(async () => { const x = await textoMedidas(); return x.includes('intentó leer la huella de tu equipo') && x.includes('medidor-prueba.io (canvas'); }), `el escudo dice qué guion intentó leer la huella (${await textoMedidas()})`);
  comprueba((await textoMedidas()).includes('servicio de grabación de sesiones (Hotjar)'), 'y que la página carga (o intentó cargar) un servicio de grabación de sesiones');
  comprueba(await hasta(async () => (await panel.textContent('#e-sesion')).includes('huella de tu equipo')), `el resumen «Desde que abriste ZERO» lo cuenta (${await panel.textContent('#e-sesion')})`);
  await espera(500);
  pantalla('03c-medidas.png');

  // Decision 206, approved: with maximum protection, another company's script gets a slightly
  // different canvas (the same on every read), the page's own reads are untouched, and frames
  // are measured too. Picked up by a page opened anew.
  await panel.click('#e-maxima-activar');
  await hasta(() => panel.evaluate(() => !document.querySelector('#e-maxima-activa').classList.contains('oculto')));
  await barra.fill('#campo-dir', `${SITIO}/prueba?id=8`);
  await barra.press('#campo-dir', 'Enter');
  await hasta(() => web.evaluate(() => location.search === '?id=8'));
  await web.waitForLoadState('load').catch(() => {});
  await espera(800);
  await web.evaluate((p) => {
    const s = document.createElement('script');
    s.src = `http://scripts.medidor-prueba.io:${p}/fp.js`;
    document.head.appendChild(s);
    const f = document.createElement('iframe');
    f.src = `http://marco-prueba.io:${p}/marco.html`;
    document.body.appendChild(f);
  }, PUERTO);
  comprueba(await hasta(() => web.evaluate(() => typeof window.__zeroFpUrl === 'string')), 'con protección máxima, el guion de otra empresa se ejecuta igual');
  const propio = await web.evaluate(() => {
    const c = document.createElement('canvas'); c.width = 240; c.height = 60;
    const x = c.getContext('2d'); x.textBaseline = 'top'; x.font = '14px Arial';
    x.fillText('Cwm fjordbank glyphs vext quiz', 2, 2);
    return c.toDataURL();
  });
  const ajeno = await web.evaluate(() => window.__zeroFpUrl);
  comprueba(ajeno !== propio && await web.evaluate(() => window.__zeroHuellaIgual === true), 'a otra empresa le da un lienzo distinto, y el mismo en cada lectura; la página lee el suyo');
  comprueba(await hasta(async () => (await textoMedidas()).includes('le dimos una distinta')), `el escudo dice «le dimos una distinta» (${await textoMedidas()})`);
  comprueba(await hasta(async () => (await textoMedidas()).includes('marco-prueba.io (canvas')), `y mide dentro de un marco de otra empresa (${await textoMedidas()})`);
  await espera(500);
  pantalla('03d-maxima-huella.png');
  await panel.click('#e-maxima-quitar');
  await hasta(() => panel.evaluate(() => document.querySelector('#e-maxima-activa').classList.contains('oculto')));

  // A new tab and a new window.
  await barra.click('#nueva');
  comprueba(await hasta(async () => (await barra.locator('.pestana').count()) === 2), 'el + abre una pestaña');
  await espera(500);
  pantalla('04-pestanas.png');

  // The new tab: our search box with its engine inside, and the day's cuts one click away.
  const nueva = await paginaQue(nav, (u) => u.endsWith('/inicio.html'));
  comprueba(await hasta(async () => (await nueva.getAttribute('#q', 'placeholder')) === 'Busca en la web o escribe una dirección'), 'la caja de búsqueda es nuestra');
  comprueba(await hasta(() => nueva.evaluate(() => (document.querySelector('#motor').selectedOptions[0] || {}).textContent === 'Google')), 'y lleva dentro el buscador, a la vista (Google por defecto desde el 2 nov 2026)');
  const primeros = await nueva.evaluate(() => [...document.querySelectorAll('#motor option')].slice(0, 3).map((o) => o.textContent).join(', '));
  comprueba(primeros === 'Google, Brave Search, DuckDuckGo', `los buscadores, en el orden del dueño: primero Google, después Brave, después DuckDuckGo (${primeros})`);
  comprueba(await nueva.evaluate(() => !document.querySelector('.acciones, [data-vista="mandato"], [data-vista="tachon"], [data-vista="datos"]')), 'la pestaña nueva ya no lleva la tarea para tu IA, la pregunta a una IA ni «Mis datos»');
  comprueba(await hasta(async () => (await nueva.locator('#favs-rejilla .fav').count()) === 1), 'la web guardada con la estrella sale en la pestaña nueva');
  // The summary is on the first screen (12 Oct 2026): the companies cut today, with no click.
  comprueba(await hasta(async () => (await nueva.locator('#r-empresas li').count()) >= 1), 'el resumen sale directamente en la pestaña nueva, con las empresas');
  comprueba((await nueva.textContent('#ver-lista')) === 'Ver la lista completa, una por una', 'y la lista completa queda a un clic');
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
  const cortadasEscudo = Number((await panel.textContent('#e-dia-cortadas')).replace(/\D/g, ''));
  comprueba(cortadasEscudo === cortadasHoy, `el día del escudo y la pestaña nueva dicen lo mismo (${cortadasEscudo} y ${cortadasHoy})`);
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
  // From the bar's shield, too: on the browser's own pages it is the day's and leads to the list.
  // Already on the list: it stays, no second copy.
  comprueba(await hasta(async () => (await barra.textContent('#b-escudo-que')) === 'hoy'), 'en las páginas propias el escudo de la barra es el del día');
  await barra.click('#b-escudo');
  await espera(1000);
  comprueba((await barra.locator('.pestana').count()) === 2, 'desde la lista, el escudo no abre otra copia');
  comprueba(await hasta(async () => /empresas? de fuera/.test(await panel.textContent('#e-dia-frase'))), `el escudo enseña también el día entero (${await panel.textContent('#e-dia-frase')})`);
  // From the web page: a tab of its own, and «Volver» brings the web back.
  await barra.click('.pestana[aria-selected="false"]');
  await espera(500);
  const tituloWeb = await barra.textContent('.pestana[aria-selected="true"] .titulo');
  await abreVista('b-escudo');
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

  // Z4 (1.0.46): the wheel scrolls the page in front right after the bar had the focus.
  await barra.click('#b-menu');
  // «Ir» (2 Nov 2026, the owner, 16:50): next to the address, for whoever does not press Enter; it does what Enter does.
  comprueba(await barra.getByRole('button', { name: 'Ir', exact: true }).isVisible(), 'junto a la dirección está el botón «Ir», a la vista y con ese nombre');
  await barra.fill('#campo-dir', `${SITIO}/largo`);
  await barra.click('#ir');
  const larga = await paginaQue(nav, (u) => u.startsWith(SITIO + '/largo'));
  await larga.waitForLoadState('load').catch(() => {});
  await espera(1000);
  await midecarga('página larga, sin terceros', larga);
  rueda(proceso.pid);
  comprueba(await hasta(async () => (await larga.evaluate(() => window.scrollY)) > 0, 6000), `la rueda del ratón desplaza la página en cuanto se carga (Z4, ${await larga.evaluate(() => window.scrollY).catch(() => -1)} px)`);

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
  // The browser keeps protecting the same, for free (2 Nov 2026, the owner, 17:01: «El navegador, sí, también quiero que
  // lo dejes gratis»); the bar says the plan, quietly.
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
  comprueba(await hasta(async () => (await barra2.textContent('#b-licencia')) === 'GUARDIANA Gratis'), `la barra dice el plan: GUARDIANA Gratis (${await barra2.textContent('#b-licencia')})`);
  comprueba(await barra2.evaluate(() => !document.querySelector('#b-escudo').classList.contains('oculto') && !/sin protecci/i.test(document.body.innerText)), 'y el escudo sigue en su sitio: nada dice «sin protección»');
  comprueba(await barra2.evaluate(() => document.querySelector('#estado-z span').textContent === 'Protegido'), 'arriba sigue diciendo «Protegido»');
  const rechazosAntes = registro.filter((r) => r.ruta === '/cookies-rechazadas').length;
  await barra2.fill('#campo-dir', `${SITIO}/prueba?utm_source=boletin&id=7`);
  await barra2.press('#campo-dir', 'Enter');
  const web2 = await paginaQue(nav2, (u) => u.startsWith(SITIO));
  await web2.waitForLoadState('load').catch(() => {});
  await espera(1500);
  await midecarga('página de prueba, segundo arranque', web2);
  comprueba(registro.some((r) => r.host === 'sitio-prueba.test' && r.ruta.startsWith('/prueba')), 'las páginas se siguen abriendo');
  // The same protection as during the trial: the ad frame is cut, the address is cleaned, the cookie notice refused.
  comprueba(!registro.some((r) => r.host === 'googleads.g.doubleclick.net') && !registro.some((r) => r.host === 'stats.g.doubleclick.net'), 'y GUARDIANA ZERO sigue cortando: ni el marco de anuncios ni el píxel salen');
  comprueba(!registro.some((r) => r.ruta.includes('utm_source=boletin')) && registro.some((r) => r.host === 'sitio-prueba.test' && r.ruta === '/prueba?id=7'), 'y sigue limpiando las direcciones');
  comprueba(await hasta(async () => registro.filter((r) => r.ruta === '/cookies-rechazadas').length > rechazosAntes), 'y sigue rechazando los avisos de cookies');
  await barra2.click('#b-licencia');
  comprueba(await hasta(() => panel2.evaluate(() => document.querySelector('#v-licencia').classList.contains('vista-activa'))), 'el plan de la barra lleva a «Suscripción»');
  comprueba(await hasta(async () => { const d = await panel2.textContent('#l-detalle'); return d.includes('La prueba terminó') && d.includes('GUARDIANA Gratis') && d.includes('sigue protegiendo igual'); }), `que dice el plan y que ZERO sigue protegiendo igual (${await panel2.textContent('#l-detalle')})`);
  await espera(500);
  pantalla('06-prueba-terminada.png');
  await panel2.screenshot({ path: path.join(salida, 'panel-suscripcion.png') }).catch(() => {});
  // A key the gateway does not know: one connection, made away from the window, and a reason.
  await panel2.fill('#l-clave', 'CLAVE-DE-PRUEBA-QUE-NO-EXISTE');
  await panel2.click('#l-activar');
  comprueba(await hasta(async () => (await panel2.textContent('#l-error')).length > 10, 60000), `una clave que no vale dice por qué (${await panel2.textContent('#l-error')})`);
  comprueba(await hasta(async () => (await panel2.textContent('#l-conexiones')).includes('dodopayments.com')), 'y la conexión con la pasarela queda anotada');
  comprueba((await barra2.textContent('#b-licencia')) === 'GUARDIANA Gratis', 'sin clave válida sigue en GUARDIANA Gratis, protegiendo');
  try { execSync('taskkill /IM guardiana-zero.exe', { stdio: 'ignore' }); } catch (_) {}
  comprueba(await hasta(async () => salio !== null, 20000), 'cierra otra vez sin forzar');

  // 5 Nov 2026 (the owner, 3 Nov: «Hazlo claro»): the VPN's icon next to Windows' clock, drawn by `--avisos`, the
  // program's process without a window that starts with the session (the installer's Run entry), so it is there with
  // ZERO closed. A panel of the test's own on the panel's port answers `GET /api/vpn` as the service does
  // (crates/panel/src/vpn.rs), with the panel's key where the service keeps it; the process writes what it shows to
  // `bandeja.txt` (only in the tests: «<colour> <1 if Windows took it>», then its lines), and Windows itself is asked
  // whether it has the icon and of what colour it is on the screen.
  {
    const LLAVE = crypto.randomBytes(32).toString('hex');
    const carpetaLlave = path.join(process.env.ProgramData || 'C:\\ProgramData', 'Guardiana');
    const rutaLlave = path.join(carpetaLlave, 'panel.token');
    let llaveAntes = null;
    try { llaveAntes = fs.readFileSync(rutaLlave); } catch (_) {}
    const habiaCarpeta = fs.existsSync(carpetaLlave);
    fs.mkdirSync(carpetaLlave, { recursive: true });
    fs.writeFileSync(rutaLlave, LLAVE);
    let fase = 'apagada';
    const lecturas = [];
    const panelVpn = http.createServer((req, res) => {
      const conLlave = req.headers['x-guardiana-token'] === LLAVE;
      lecturas.push({ metodo: req.method, ruta: req.url, conLlave });
      if (!conLlave) { res.writeHead(401); return res.end(); }
      if (req.method === 'GET' && req.url === '/api/vpn') {
        const on = fase !== 'apagada';
        res.writeHead(200, { 'Content-Type': 'application/json' });
        return res.end(JSON.stringify({ disponible: true, fase, estricto: true, desconectar: on, salida: on ? { id: 'hel', pais: 'FI', ciudad: 'Helsinki' } : null }));
      }
      res.writeHead(404);
      return res.end();
    });
    const conPanel = await new Promise((r) => { panelVpn.once('error', () => r(false)); panelVpn.listen(7443, '127.0.0.1', () => r(true)); });
    comprueba(conPanel, 'el icono de la VPN: un panel de prueba en el puerto del panel (7443)');
    let explorador = false;
    try { explorador = /explorer\.exe/i.test(execSync('tasklist /FI "IMAGENAME eq explorer.exe" /NH', { encoding: 'utf8' })); } catch (_) {}
    const bandeja = path.join(base, 'datos', 'bandeja.txt');
    try { fs.unlinkSync(bandeja); } catch (_) {}
    const icono = () => { try { return fs.readFileSync(bandeja, 'utf8'); } catch (_) { return ''; } };
    const avisos = spawn(exe, ['--avisos'], { env: { ...process.env, GUARDIANA_ZERO_DATOS: base }, stdio: 'ignore' });
    let avisosSalio = null;
    avisos.on('exit', (c) => { avisosSalio = c; });
    // Windows takes the icon only with a taskbar (Explorer): without one on this runner, only what it decided is checked
    const puesto = explorador ? ' 1' : ' ';
    if (!explorador) console.log('::warning title=icono de la VPN::este Windows no tiene el Explorador en marcha: no hay barra de tareas donde poner el icono, y solo se comprueba lo que decide');
    const en = (color, que) => hasta(async () => icono().startsWith(color + puesto) && icono().includes(que), 20000);
    const enPantalla = (color, recorte) => {
      const w = iconoEnWindows(recorte);
      const [hr, , , , , rgb] = w.split(' ');
      const [r, g, b] = (rgb || '').split(',').map(Number);
      const visto = !rgb ? '' : Math.max(r, g, b) - Math.min(r, g, b) < 100 ? 'gris' : g > r && g > b ? 'verde' : r > 200 && g > 140 ? 'ambar' : r > g && r > b ? 'rojo' : '?';
      console.log(`::notice title=icono de la VPN (${color})::Shell_NotifyIconGetRect y el color en la pantalla: ${w}${visto ? ` (se ve ${visto})` : ''}`);
      return { tiene: hr === '0', visto };
    };
    comprueba(await en('gris', 'VPN desconectada'), `el icono de la VPN aparece junto al reloj, gris, con «GUARDIANA · VPN desconectada» (${JSON.stringify(icono())})`);
    // where Windows put it (on the taskbar, or among the hidden icons) and how it looks: said, never a failure
    enPantalla('gris', 'vpn-icono-gris.png');
    fase = 'conectando';
    comprueba(await en('ambar', 'VPN: conectando'), `mientras conecta, ámbar con «VPN: conectando…» (${JSON.stringify(icono())})`);
    fase = 'conectada';
    comprueba(await en('verde', 'VPN conectada\nFinlandia · Helsinki'), `al conectar se pone verde y dice dónde sale (${JSON.stringify(icono())})`);
    await espera(1500);
    pantalla('vpn-icono-verde.png');
    const visto = enPantalla('verde', 'vpn-icono-verde-recorte.png').visto;
    if (visto && visto !== 'verde') console.log(`::warning title=icono de la VPN::conectada, en la pantalla se ve ${visto}`);
    fase = 'sin_respuesta';
    comprueba(await en('rojo', 'VPN sin respuesta'), `si su servidor no contesta, rojo con «VPN sin respuesta» (${JSON.stringify(icono())})`);
    fase = 'apagada';
    comprueba(await en('gris', 'VPN desconectada'), `al desconectar vuelve a gris (${JSON.stringify(icono())})`);
    comprueba(lecturas.length >= 4 && lecturas.every((l) => l.conLlave && l.metodo === 'GET' && l.ruta === '/api/vpn'), `pregunta solo «GET /api/vpn» y con la llave del panel (${lecturas.length} lecturas)`);
    // the installer closes it as it closes ZERO (its hidden window takes WM_CLOSE), and the icon goes with it
    try { execSync(`taskkill /PID ${avisos.pid}`, { stdio: 'ignore' }); } catch (_) {}
    comprueba(await hasta(async () => avisosSalio !== null, 15000), 'y se cierra sin forzar, como lo cierra el instalador');
    panelVpn.close();
    try { if (llaveAntes) fs.writeFileSync(rutaLlave, llaveAntes); else fs.unlinkSync(rutaLlave); } catch (_) {}
    if (!habiaCarpeta) try { fs.rmdirSync(carpetaLlave); } catch (_) {}
  }
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
  // 1.0.55: the window's own figures (only with GUARDIANA_ZERO_DEPURA, one line per run) and the pages' loads.
  let tiempos = '';
  try { tiempos = fs.readFileSync(path.join(base, 'datos', 'tiempos.txt'), 'utf8'); } catch (_) {}
  for (const l of tiempos.split(/\r?\n/).filter(Boolean)) console.log(`::notice title=tiempos::${esc(l)}`);
  for (const c of cargas) console.log(`::notice title=carga::${esc(c)}`);
  for (const f of fallos) console.log(`::error title=prueba de punta a punta::${esc(f)}`);
  console.log(`::notice title=prueba de punta a punta::${bien.length} bien, ${fallos.length} fallos`);
  servidor.close();
  process.exit(fallos.length ? 1 : 0);
});
