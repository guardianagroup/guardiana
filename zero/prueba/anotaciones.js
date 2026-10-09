// Turns cargo's JSON messages into GitHub annotations: errors can then be read from the run
// page and the public API, without the log. Usage: node anotaciones.js cargo.json [prefix]
'use strict';
const fs = require('fs');
const [archivo, prefijo = ''] = process.argv.slice(2);
const esc = (s) => String(s).replace(/%/g, '%25').replace(/\r/g, '%0D').replace(/\n/g, '%0A');
let lineas = [];
try { lineas = fs.readFileSync(archivo, 'utf8').split(/\r?\n/); } catch (_) {}
const mensajes = [];
for (const l of lineas) {
  if (!l.startsWith('{')) continue;
  let m;
  try { m = JSON.parse(l); } catch (_) { continue; }
  if (m.reason !== 'compiler-message' || !m.message) continue;
  const lvl = m.message.level;
  if (lvl !== 'error' && lvl !== 'warning') continue;
  if (/aborting due to|could not compile/.test(m.message.message)) continue;
  const sp = (m.message.spans || []).find((s) => s.is_primary) || (m.message.spans || [])[0];
  const texto = m.message.rendered || m.message.message;
  if (mensajes.some((x) => x.texto === texto)) continue;
  mensajes.push({ lvl, sp, texto });
}
for (const { texto } of mensajes) process.stdout.write(texto + '\n');
mensajes.slice(0, 9).forEach(({ lvl, sp, texto }) => {
  const donde = sp ? `file=${prefijo}${sp.file_name.replace(/\\/g, '/')},line=${sp.line_start},col=${sp.column_start},` : '';
  console.log(`::error ${donde}title=${lvl}::${esc(texto.slice(0, 4000))}`);
});
if (mensajes.length > 9) {
  // Everything else in one annotation, so nothing is lost behind the limit of ten.
  const resto = mensajes.slice(9).map((x) => x.texto).join('\n').slice(0, 60000);
  console.log(`::error title=y ${mensajes.length - 9} más::${esc(resto)}`);
}
