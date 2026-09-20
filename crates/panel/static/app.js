// Guardiana panel. No third-party code, no external fonts, no cookies (brief §8).
// The session token arrives once in the URL (?t=...), is kept in this tab's
// memory (sessionStorage) and sent as a header on every API call.
(() => {
  'use strict';
  const params = new URLSearchParams(location.search);
  if (params.has('t')) {
    try { sessionStorage.setItem('guardiana_token', params.get('t')); } catch (_) {}
    params.delete('t');
    const rest = params.toString();
    history.replaceState(null, '', location.pathname + (rest ? '?' + rest : ''));
  }
  let token = '';
  try { token = sessionStorage.getItem('guardiana_token') || ''; } catch (_) {}
  // Language (decision 45): the person's choice, kept in this browser; otherwise the browser's own.
  let LANG = '';
  try { LANG = localStorage.getItem('guardiana_lang') || ''; } catch (_) {}
  if (LANG !== 'es' && LANG !== 'en' && LANG !== 'pt') {
    const dos = (navigator.language || 'es').slice(0, 2);
    LANG = (dos === 'en' || dos === 'pt') ? dos : 'es';
  }
  try { document.documentElement.lang = LANG; } catch (_) {}
  // Night mode (owner's request, 20 Sep 2026). The panel ships with the website's light design;
  // whoever prefers a dark screen chooses it here, and the choice stays in this browser: it is
  // never sent anywhere, because there is nowhere to send it to.
  let TEMA = '';
  try { TEMA = localStorage.getItem('guardiana_tema') || ''; } catch (_) {}
  if (TEMA !== 'noche' && TEMA !== 'dia') TEMA = 'dia';
  try { if (TEMA === 'noche') document.documentElement.dataset.tema = 'noche'; } catch (_) {}

  let T = { panel: {}, categorias: {}, veredictos: {}, senales: {}, decidido_por: {} };
  const t = (k) => (T.panel && T.panel[k]) || k;

  // ----- "Guardar en PDF" ----------------------------------------------------
  // The PDF is written here, in the page, with no third-party code (brief §8): A4 pages,
  // Helvetica (a font every PDF reader carries, so nothing is embedded) and WinAnsi text,
  // which covers Spanish. Tables break across pages; every page gets a footer. Saving goes
  // through the browser's "save as" dialog where it exists (Chrome and Edge on a PC) and
  // through a normal download elsewhere (Safari, Firefox, phones).
  const PDF_W = 595.28, PDF_H = 841.89, PDF_M = 40;
  // Helvetica advance widths, thousandths of the size, WinAnsi code points 32-255.
  const HELV = '278,278,355,556,556,889,667,191,333,333,389,584,278,333,278,278,556,556,556,556,556,556,556,556,556,556,278,278,584,584,584,556,1015,667,667,722,722,667,611,778,722,278,500,667,556,833,722,778,667,778,722,667,611,722,667,944,667,667,611,278,278,278,469,556,333,556,556,500,556,556,278,556,556,222,222,500,222,833,556,556,556,556,333,500,278,556,500,722,500,500,500,334,260,334,584,376,744,556,222,556,333,1000,556,556,333,1000,667,333,1000,556,611,556,556,222,222,333,333,350,556,1000,333,1000,500,333,944,556,500,667,278,333,556,556,556,556,260,556,333,737,370,556,584,0,737,333,400,549,333,333,333,576,537,278,333,333,365,556,834,834,834,611,667,667,667,667,667,667,1000,722,667,667,667,667,278,278,278,278,722,722,778,778,778,778,778,584,778,722,722,722,722,667,667,611,556,556,556,556,556,556,889,500,556,556,556,556,278,278,278,278,556,556,556,556,556,556,556,549,611,556,556,556,556,500,556,500'.split(',').map(Number);
  // Unicode code points that WinAnsi keeps in the 0x80-0x9F slots, plus spaces and hyphens
  // that toLocaleString() produces in some locales.
  const WINANSI = { 0x20AC: 0x80, 0x201A: 0x82, 0x0192: 0x83, 0x201E: 0x84, 0x2026: 0x85, 0x2020: 0x86, 0x2021: 0x87, 0x02C6: 0x88, 0x2030: 0x89, 0x0160: 0x8A, 0x2039: 0x8B, 0x0152: 0x8C, 0x017D: 0x8E, 0x2018: 0x91, 0x2019: 0x92, 0x201C: 0x93, 0x201D: 0x94, 0x2022: 0x95, 0x2013: 0x96, 0x2014: 0x97, 0x02DC: 0x98, 0x2122: 0x99, 0x0161: 0x9A, 0x203A: 0x9B, 0x0153: 0x9C, 0x017E: 0x9E, 0x0178: 0x9F, 0xA0: 0x20, 0x2009: 0x20, 0x202F: 0x20, 0x2010: 0x2D, 0x2011: 0x2D, 0x2212: 0x2D };
  // Text as a "binary string": one character per byte, ready for a PDF string.
  function winAnsi(s) {
    let out = '';
    for (const ch of String(s ?? '')) {
      const cp = ch.codePointAt(0);
      let b = cp < 128 || (cp >= 160 && cp < 256) ? cp : (WINANSI[cp] || 63);
      if (b < 32 && b !== 10) b = 32;
      out += String.fromCharCode(b);
    }
    return out;
  }
  const textWidth = (bin, size) => { let w = 0; for (let i = 0; i < bin.length; i++) w += HELV[bin.charCodeAt(i) - 32] || 556; return w * size / 1000; };
  const pdfEscape = (bin) => bin.replace(/[\\()]/g, (c) => '\\' + c);
  // Greedy wrap by words; a word wider than the column (a long domain name) is cut by characters.
  function wrapText(bin, size, maxW) {
    const lines = [];
    for (const para of bin.split('\n')) {
      let line = '';
      for (let word of para.split(' ')) {
        while (textWidth(word, size) > maxW) {
          let cut = word.length;
          while (cut > 1 && textWidth(word.slice(0, cut), size) > maxW) cut--;
          if (line) { lines.push(line); line = ''; }
          lines.push(word.slice(0, cut)); word = word.slice(cut);
        }
        const candidate = line ? line + ' ' + word : word;
        if (textWidth(candidate, size) <= maxW) line = candidate; else { if (line) lines.push(line); line = word; }
      }
      lines.push(line);
    }
    return lines;
  }
  function pdfDocument(title) {
    const pages = []; let ops = []; let y = 0;
    const usable = PDF_W - 2 * PDF_M;
    const n = (v) => v.toFixed(2);
    const newPage = () => { ops = []; pages.push(ops); y = PDF_H - PDF_M; };
    const text = (x, yy, bin, size, bold, gray) => { ops.push(n(gray || 0) + ' g BT /' + (bold ? 'F2' : 'F1') + ' ' + n(size) + ' Tf ' + n(x) + ' ' + n(yy) + ' Td (' + pdfEscape(bin) + ') Tj ET'); };
    const need = (h) => { if (y - h < PDF_M + 18) newPage(); };
    newPage();
    const doc = {
      line(s, size = 10, bold = false, gray = 0) {
        const lh = size * 1.25;
        for (const l of wrapText(winAnsi(s), size, usable)) { need(lh); y -= lh; text(PDF_M, y + size * 0.3, l, size, bold, gray); }
        return doc;
      },
      gap(h = 8) { y -= h; return doc; },
      // cols: [{ title, w (fraction of the width) }]; rows: arrays of strings, or { cells, bold }.
      table(cols, rows, size = 9) {
        const lh = size * 1.25, pad = 3;
        const widths = cols.map((c) => c.w * usable);
        const layout = (cells, bold) => {
          const cellLines = cells.map((c, i) => wrapText(winAnsi(c), size, widths[i] - 2 * pad));
          return { cellLines, bold, h: Math.max(1, ...cellLines.map((l) => l.length)) * lh + 2 * pad };
        };
        const draw = (row, shade) => {
          if (shade) ops.push('0.92 g ' + n(PDF_M) + ' ' + n(y - row.h) + ' ' + n(usable) + ' ' + n(row.h) + ' re f');
          let x = PDF_M;
          row.cellLines.forEach((lines, i) => { lines.forEach((l, k) => text(x + pad, y - pad - lh * (k + 1) + size * 0.3, l, size, row.bold, 0)); x += widths[i]; });
          y -= row.h;
          ops.push('0.75 G 0.4 w ' + n(PDF_M) + ' ' + n(y) + ' m ' + n(PDF_M + usable) + ' ' + n(y) + ' l S');
        };
        const header = layout(cols.map((c) => c.title), true);
        need(header.h + lh); draw(header, true);
        rows.forEach((r) => {
          const row = layout(r.cells || r, !!r.bold);
          if (y - row.h < PDF_M + 18) { newPage(); draw(header, true); }
          draw(row, false);
        });
        return doc;
      },
      bytes(footer) {
        const total = pages.length;
        pages.forEach((p, i) => {
          const left = winAnsi(footer), right = winAnsi(t('pdf_pagina').replace('{n}', i + 1).replace('{m}', total));
          p.push('0.45 g BT /F1 8 Tf ' + n(PDF_M) + ' ' + n(PDF_M - 14) + ' Td (' + pdfEscape(left) + ') Tj ET');
          p.push('0.45 g BT /F1 8 Tf ' + n(PDF_W - PDF_M - textWidth(right, 8)) + ' ' + n(PDF_M - 14) + ' Td (' + pdfEscape(right) + ') Tj ET');
        });
        const objs = [];
        const add = (o) => objs.push(o);
        add('<< /Type /Catalog /Pages 2 0 R >>');
        add('');
        add('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>');
        add('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>');
        const stamp = new Date().toISOString().replace(/[-:]/g, '').slice(0, 15);
        const info = add('<< /Title (' + pdfEscape(winAnsi(title)) + ') /Producer (GUARDIANA) /CreationDate (D:' + stamp + 'Z) >>');
        const kids = pages.map((p) => {
          const stream = p.join('\n');
          const c = add('<< /Length ' + stream.length + ' >>\nstream\n' + stream + '\nendstream');
          return add('<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ' + n(PDF_W) + ' ' + n(PDF_H) + '] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents ' + c + ' 0 R >>');
        });
        objs[1] = '<< /Type /Pages /Kids [' + kids.map((k) => k + ' 0 R').join(' ') + '] /Count ' + kids.length + ' >>';
        let out = '%PDF-1.4\n%\xE2\xE3\xCF\xD3\n';
        const offsets = objs.map((o, i) => { const at = out.length; out += (i + 1) + ' 0 obj\n' + o + '\nendobj\n'; return at; });
        const xref = out.length;
        out += 'xref\n0 ' + (objs.length + 1) + '\n0000000000 65535 f \n' + offsets.map((o) => String(o).padStart(10, '0') + ' 00000 n \n').join('');
        out += 'trailer\n<< /Size ' + (objs.length + 1) + ' /Root 1 0 R /Info ' + info + ' 0 R >>\nstartxref\n' + xref + '\n%%EOF\n';
        const bytes = new Uint8Array(out.length);
        for (let i = 0; i < out.length; i++) bytes[i] = out.charCodeAt(i) & 255;
        return bytes;
      },
    };
    return doc;
  }
  async function savePdfFile(name, bytes) {
    const blob = new Blob([bytes], { type: 'application/pdf' });
    if (typeof window.showSaveFilePicker === 'function') {
      try {
        const handle = await window.showSaveFilePicker({ suggestedName: name, types: [{ description: 'PDF', accept: { 'application/pdf': ['.pdf'] } }] });
        const w = await handle.createWritable(); await w.write(blob); await w.close();
        return 'guardado';
      } catch (e) { if (e && e.name === 'AbortError') return 'cancelado'; }
    }
    const a = document.createElement('a'); a.href = URL.createObjectURL(blob); a.download = name;
    document.body.appendChild(a); a.click(); a.remove(); setTimeout(() => URL.revokeObjectURL(a.href), 10000);
    return 'descargado';
  }
  // Wires a "Guardar en PDF" button: build() returns { name, doc } from what the page holds.
  function savePdf(id, build) {
    const b = $(id);
    if (!b) return;
    const msg = $(id + '-msg');
    b.addEventListener('click', async () => {
      try {
        const { name, doc } = build();
        const outcome = await savePdfFile(name, doc.bytes(t('pdf_pie')));
        if (msg) { msg.textContent = outcome === 'cancelado' ? '' : t('pdf_' + outcome).replace('{nombre}', name); setTimeout(() => { msg.textContent = ''; }, 8000); }
      } catch (e) { if (msg) msg.textContent = String(e.message || e); }
    });
  }
  const pdfName = (what) => 'guardiana-' + what + '-' + new Date().toISOString().slice(0, 10) + '.pdf';
  const pdfHead = (doc, heading) => doc.line('GUARDIANA · ' + heading, 16, true).line(t('pdf_generado').replace('{fecha}', new Date().toLocaleString(LOC, DIA_HORA)), 9, false, 0.4).gap(6);
  const catName = (c) => T.categorias[c] || c;
  const verdictName = (v) => T.veredictos[v] || v;
  const deviceName = (ev) => ev.device_name || (ev.device_id === 'self' ? t('este_computador') : ev.device_id);
  const nameCell = (ev) => [ev.qname + (ev.ia ? ' · IA: ' + ev.ia : '') + (ev.empresa && ev.empresa !== ev.ia ? ' · ' + ev.empresa : '')].concat((ev.frases || []).map((f) => '· ' + f)).join('\n');
  const eventCols = (withDevice) => withDevice
    ? [{ title: t('col_hora'), w: 0.12 }, { title: t('col_nombre'), w: 0.42 }, { title: t('col_categoria'), w: 0.14 }, { title: t('col_dispositivo'), w: 0.18 }, { title: t('col_veredicto'), w: 0.14 }]
    : [{ title: t('col_hora'), w: 0.14 }, { title: t('col_nombre'), w: 0.50 }, { title: t('col_categoria'), w: 0.18 }, { title: t('col_veredicto'), w: 0.18 }];
  const eventCells = (ev, withDevice) => withDevice
    ? [clock(ev.ts), nameCell(ev), catName(ev.category), deviceName(ev), verdictName(ev.verdict)]
    : [clock(ev.ts), nameCell(ev), catName(ev.category), verdictName(ev.verdict)];

  async function api(path, opts = {}) {
    const headers = Object.assign({ 'X-Guardiana-Token': token, 'X-Guardiana-Lang': LANG }, opts.headers || {});
    if (opts.body && typeof opts.body !== 'string') { opts.body = JSON.stringify(opts.body); headers['Content-Type'] = 'application/json'; }
    const r = await fetch(path, Object.assign({}, opts, { headers }));
    if (r.status === 401) { showNoSession(); throw new Error('sin sesión'); }
    if (!r.ok) throw new Error(await r.text());
    return r.json();
  }
  function showNoSession() {
    const el = document.getElementById('no-session');
    if (el) el.classList.remove('hidden');
  }
  const $ = (id) => document.getElementById(id);
  const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
  // Dates follow the panel's language, not the computer's: someone reading the panel in English
  // on a Spanish machine was getting «18/9/2026», and in English that reads as the 9th of a month
  // that does not exist here. English gets a named month so the two can never be swapped.
  const LOC = LANG === 'en' ? 'en-GB' : (LANG === 'pt' ? 'pt-BR' : 'es');
  const DIA = LANG === 'en' ? { day: 'numeric', month: 'short', year: 'numeric' } : undefined;
  const DIA_HORA = LANG === 'en' ? { day: 'numeric', month: 'short', year: 'numeric', hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false } : undefined;
  const clock = (ms) => new Date(ms).toLocaleTimeString(LOC);
  const when = (ms) => new Date(ms).toLocaleString(LOC, DIA_HORA);
  // «Unknown» was 88% of every table: a word that told the person nothing while the program
  // already knew who owned 86% of those names. The stored category does not change -- the ledger
  // keeps what was decided at the time -- but what is SHOWN now says what is actually known:
  // the network that delivers somebody else's content, the company the name belongs to, or,
  // when nothing is known, that no open list knows it either. Never a verdict (brief §6).
  const catTexto = (ev) => {
    if (ev.category !== 'desconocido') return T.categorias[ev.category] || ev.category;
    if (ev.local) return t('cat_red_local');
    if (ev.entrega) return t('cat_entrega');
    if (ev.empresa || ev.ia) return t('cat_de_empresa').replace('{empresa}', ev.empresa || ev.ia);
    return T.categorias.desconocido;
  };
  const catClase = (ev) => (ev.category !== 'desconocido' ? '' : (ev.local || ev.entrega) ? ' entrega' : (ev.empresa || ev.ia) ? ' propio' : ' sinlista');
  const cat = (ev) => `<span class="tag ${esc(ev.category)}${catClase(ev)}">${esc(catTexto(ev))}</span>`;
  const verdict = (v) => `<span class="verdict ${esc(v)}">${esc(T.veredictos[v] || v)}</span>`;
  // The trade goes first: reading «AppsFlyer» tells a person nothing, and reading what
  // AppsFlyer sells tells them everything they need to decide. It is a fact about the
  // company, not about this query, and it is only printed where one is written.
  const phrases = (ev) => (ev.oficio ? `<span class="phrase oficio">· ${esc(ev.oficio)}</span>` : '')
    + (ev.frases || []).map((f) => `<span class="phrase">· ${esc(f)}</span>`).join('');
  const company = (ev) => (ev.ia && !(ev.category === 'desconocido' && !ev.entrega && !ev.empresa) ? ` <span class="tag ia">${esc(ev.ia)}</span>` : '') + (ev.empresa && ev.empresa !== ev.ia && !(ev.category === 'desconocido' && !ev.entrega) ? ` <span class="tag empresa">${esc(ev.empresa)}</span>` : '');
  const hints = (l) => [l && l.callado_min != null ? t('lectura_callado').replace('{min}', l.callado_min) : '', l && l.relay ? t('lectura_relay') : '', l && l.evasiones ? (l.evasiones === 1 ? t('lectura_evasion_una') : t('lectura_evasiones').replace('{n}', l.evasiones)) : ''].filter(Boolean);
  const device = (ev) => esc(ev.device_name || (ev.device_id === 'self' ? t('este_computador') : ev.device_id)) + programa(ev);
  // Qué programa pidió el nombre, cuando el sistema lo dijo (Windows, este equipo). Va pegado al
  // aparato, que es donde se lee «este PC · claude.exe», en vez de una columna que estaría vacía
  // en todas las filas de los teléfonos. Sin dato no se enseña nada: un hueco, nunca una
  // suposición. La ruta entera y la huella van en el título, para quien quiera comprobarlas.
  // Un nombre de trampa en el extracto significa una cosa concreta: ese archivo se leyó. Se
  // enseña en rojo y con su frase, porque es de las pocas cosas de este panel que piden mirar.
  const trampa = (ev) => (ev.trampa ? ` <span class="tag trampa" title="${esc(t('trampa_frase'))}">${esc(t('trampa_etiqueta'))}</span>` : '');
  const programa = (ev) => {
    const p = ev.programa;
    if (!p || !p.nombre) return '';
    const detalle = [p.ruta, p.sha256 ? p.sha256.slice(0, 16) + '…' : ''].filter(Boolean).join(' · ');
    return ` <span class="tag app" title="${esc(detalle)}">${esc(p.nombre)}</span>`;
  };

  // The changes Guardiana made on someone's order (Home Mode, system DNS), with who asked.
  async function paintChanges() {
    const el = $('cambios');
    if (!el) return;
    try {
      const v = await api('/api/cambios');
      el.innerHTML = v.length ? v.map((c) => `<li>${esc(when(c.ts))} · ${esc(t('cambio_' + c.que))} ${esc(t('quien_' + c.quien))}${c.detalle ? ' · <span class="mono">' + esc(c.detalle) + '</span>' : ''}</li>`).join('') : `<li>${esc(t('cambios_ninguno'))}</li>`;
    } catch (_) {}
  }

  // Devices and their 24-hour gate (brief §6), loaded by the pages that cut.
  let gate = {};
  async function loadGate() {
    try {
      const devs = await api('/api/dispositivos');
      gate = {};
      devs.forEach((d) => { gate[d.id] = d; });
    } catch (_) {}
  }
  // Names this person has already cut, so the state survives a redraw. The snapshot table
  // repaints every two seconds: without this, a button turned red by a cut went back to grey
  // as if nothing had happened, and the person would cut the same name twice.
  const CORTADOS = new Set();
  const claveCorte = (dev, nombre) => dev + '|' + nombre;
  async function cargarCortados() {
    try {
      const r = await api('/api/reglas');
      (r.reglas || []).forEach((x) => {
        if (x.activa && x.action === 'cortar' && x.match_kind === 'domain') {
          CORTADOS.add(claveCorte(x.device_id || 'home', x.pattern));
        }
      });
    } catch (_) {}
  }
  function yaCortado(ev) {
    return CORTADOS.has(claveCorte(ev.device_id, ev.qname)) || CORTADOS.has(claveCorte('home', ev.qname));
  }
  function cutCell(ev) {
    if (ev.verdict === 'cortado') return '';
    const d = gate[ev.device_id];
    if (!d) return '';
    if (!d.puede_cortar) return `<span class="muted">${esc(t('observando').replace('{h}', d.horas_observadas))}</span>`;
    if (yaCortado(ev)) return `<button class="cut cortado" disabled>${esc(t('cortado_boton'))}</button>`;
    return `<button class="secondary cut" data-device="${esc(ev.device_id)}" data-name="${esc(ev.qname)}">${esc(t('cortar'))}</button>`;
  }
  async function createRule(body, path = '/api/reglas') {
    let r = await api(path, { method: 'POST', body });
    if (r.necesita === 'confirmar') {
      if (!confirm(r.mensaje)) return null;
      r = await api(path, { method: 'POST', body: Object.assign({}, body, { confirmed: true }) });
    }
    if (r.mensaje && !r.creada) { alert(r.mensaje); return null; }
    return r.creada;
  }
  // A burst is several different names asked by one device within a couple of seconds: one
  // act (you opened an app), not six unrelated rows. The names inside are the same events the
  // table already shows, with the company and, where one is written, its trade.
  async function pintarRafagas() {
    const caja = $('rafagas');
    if (!caja) return;
    const rs = await api('/api/rafagas?horas=24');
    if (!rs.length) { caja.innerHTML = `<p class="muted">${esc(t('rafagas_ninguna'))}</p>`; return; }
    caja.innerHTML = rs.map((r) => {
      const resumen = t('rafagas_resumen').replace('{n}', r.nombres.length).replace('{seg}', (r.duracion_ms / 1000).toFixed(1));
      const quien = esc(r.device_name || (r.device_id === 'self' ? t('este_computador') : r.device_id));
      const oficio = r.con_oficio ? ` · ${esc(t('rafagas_oficio').replace('{n}', r.con_oficio))}` : '';
      const filas = r.nombres.map((e) => `<tr><td><span class="mono">${esc(e.qname)}</span>${company(e)}${phrases(e)}</td><td>${cat(e)}</td></tr>`).join('');
      return `<details class="card"><summary><span class="mono">${clock(r.ts)}</span> · ${quien} · ${esc(resumen)}${oficio}`
        + (r.empresas.length ? `<br><span class="muted">${esc(r.empresas.join(' · '))}</span>` : '')
        + `</summary><div class="wrap"><table>${filas}</table></div></details>`;
    }).join('');
  }

  // The receipt: the few lines a person can say out loud, per device. The table above is the
  // detail; this is what convinces. Every figure comes from the same ledger, and the window is
  // printed with it because the free plan only keeps a day of detail.
  async function pintarRecibos() {
    const caja = $('recibos');
    if (!caja) return;
    const rs = await api('/api/recibo?dias=7');
    if (!rs.length) { caja.innerHTML = `<p class="muted">${esc(t('recibo_ninguno'))}</p>`; return; }
    caja.innerHTML = rs.map((r) => {
      const quien = esc(r.device_name || (r.device_id === 'self' ? t('este_computador') : r.device_id));
      const lineas = [];
      lineas.push(esc(t('recibo_nombres').replace('{n}', r.nombres)));
      const emp = r.empresas_con_oficio
        ? t('recibo_empresas').replace('{n}', r.empresas).replace('{m}', r.empresas_con_oficio)
        : t('recibo_empresas_sin').replace('{n}', r.empresas);
      lineas.push(esc(emp) + (r.oficios.length ? `<br><span class="muted">${esc(r.oficios.join(' · '))}</span>` : ''));
      if (r.latidos.length) {
        const lista = r.latidos.slice(0, 3).map(([n, m]) => t('recibo_latido').replace('{nombre}', n).replace('{min}', m)).join(' · ');
        lineas.push(esc(t('recibo_latidos').replace('{lista}', lista)));
      }
      if (r.evasiones) lineas.push(esc(t('recibo_evasiones').replace('{n}', r.evasiones)));
      if (r.cortadas) lineas.push(esc(t('recibo_cortadas').replace('{n}', r.cortadas)));
      if (r.rafaga) {
        lineas.push(esc(t('recibo_rafaga').replace('{n}', r.rafaga.nombres.length).replace('{seg}', (r.rafaga.duracion_ms / 1000).toFixed(1)))
          + (r.rafaga.empresas.length ? `<br><span class="muted">${esc(r.rafaga.empresas.slice(0, 6).join(' · '))}</span>` : ''));
      }
      return `<div class="card"><h3>${quien}</h3>`
        + `<p class="muted">${esc(t('recibo_desde').replace('{fecha}', when(r.desde)))}</p>`
        + `<ul class="recibo">${lineas.map((l) => `<li>${l}</li>`).join('')}</ul></div>`;
    }).join('');
  }

  function bindCutButtons(container, path) {
    container.addEventListener('click', async (e) => {
      const b = e.target.closest('button.cut');
      if (!b) return;
      const name = b.dataset.name, dev = b.dataset.device;
      if (!confirm(t('cortar_confirmar_dispositivo').replace('{nombre}', name))) return;
      const created = await createRule({ scope: 'device', device_id: dev, match_kind: 'domain', pattern: name, action: 'cortar' }, path);
      // A grey tick did not say what had happened. The button turns into the state: red and
      // with the word, in the language of the panel.
      if (created) {
        CORTADOS.add(claveCorte(dev, name));
        b.textContent = t('cortado_boton'); b.disabled = true; b.classList.remove('secondary'); b.classList.add('cortado');
      }
    });
  }

  // One connection asks the same name several ways at once: the IPv4 address (A), the IPv6 one
  // (AAAA) and the HTTPS record. Printed as one row each, the table looked like it was repeating
  // itself, and a person reading it counted the same service two or three times. They are folded
  // into a single row, with how many times and, on hover, the ways it was asked. Nothing is
  // hidden: the export and the JSON keep every query as it happened.
  function foldEvents(events) {
    const out = [];
    for (const ev of events) {
      const last = out[out.length - 1];
      if (last && last.qname === ev.qname && last.device_id === ev.device_id
          && last.category === ev.category && last.verdict === ev.verdict
          && Math.abs(last.ts - ev.ts) <= 2000) {
        last._n += 1;
        if (ev.qtype && !last._types.includes(ev.qtype)) last._types.push(ev.qtype);
        last.ts = Math.max(last.ts, ev.ts);
        continue;
      }
      out.push(Object.assign({}, ev, { _n: 1, _types: ev.qtype ? [ev.qtype] : [] }));
    }
    return out;
  }
  function eventRows(events) {
    return foldEvents(events).map((ev) => {
      const veces = ev._n > 1 ? ` <span class="muted">×${ev._n}</span>` : '';
      const como = ev._types.length ? ` title="${esc(ev._types.join(', '))}"` : '';
      return `<tr><td class="mono">${clock(ev.ts)}</td><td><span class="mono"${como}>${esc(ev.qname)}</span>${veces}${trampa(ev)}${company(ev)}${phrases(ev)}</td><td>${cat(ev)}</td><td>${device(ev)}</td><td>${verdict(ev.verdict)}</td><td>${cutCell(ev)}</td></tr>`;
    }).join('');
  }

  const PAGE_TITLE = { radiografia: 'nav_radiografia', ia: 'nav_ia', extracto: 'nav_extracto', dispositivos: 'nav_dispositivos', sabeDeTi: 'nav_sabe', estado: 'nav_estado', hogar: 'nav_hogar', miDispositivo: 'nav_mi', informe: 'nav_informe', reglas: 'nav_reglas', verify: 'nav_verify', licencia: 'nav_licencia', comprobador: 'comprobador_titulo' };
  function applyTexts() {
    document.querySelectorAll('[data-t]').forEach((el) => { el.textContent = t(el.getAttribute('data-t')); });
    document.querySelectorAll('[data-t-html]').forEach((el) => { el.innerHTML = t(el.getAttribute('data-t-html')); });
    // Filter options and fixed headers carry stored values (Spanish keys); their labels follow the language.
    // Las etiquetas cortas vivían aquí, en los dos idiomas, hasta el 19 sep 2026. Estaban bien
    // traducidas, pero texto de interfaz dentro del código es lo que la regla del proyecto prohíbe:
    // la siguiente señal que se añada es la que sale sin traducir. Ahora vienen de i18n.
    const maps = { category: T.categorias, signal: T.senales_corto, verdict: T.veredictos };
    Object.entries(maps).forEach(([name, map]) => {
      document.querySelectorAll('select[name=' + name + '] option[value]:not([value=""])').forEach((o) => {
        o.textContent = (map && map[o.value]) || o.value;
      });
    });
    document.querySelectorAll('th[data-cat]').forEach((th) => { th.textContent = (T.categorias && T.categorias[th.getAttribute('data-cat')]) || th.getAttribute('data-cat'); });
    const page = document.body.getAttribute('data-page');
    if (PAGE_TITLE[page]) document.title = 'Guardiana · ' + t(PAGE_TITLE[page]);
    // The language switch lives in the header of every page; the choice is kept in this browser only.
    const header = document.querySelector('header.top');
    if (header && !$('lang-toggle')) {
      // Los tres idiomas a la vista, como en la web: un botón que va rotando escondía el tercero
      // justo a quien lo estaba buscando. El de esta pantalla queda marcado y no se puede pulsar.
      const b = document.createElement('div');
      b.id = 'lang-toggle'; b.className = 'idiomas';
      const CORTO = { es: 'ES', en: 'EN', pt: 'PT' };
      const CAMBIAR = { es: 'Cambiar a español', en: 'Switch to English', pt: 'Mudar para português' };
      ['es', 'en', 'pt'].forEach((l) => {
        const a = document.createElement('button');
        a.type = 'button';
        a.className = 'secondary lang' + (l === LANG ? ' actual' : '');
        a.textContent = CORTO[l];
        a.setAttribute('lang', l);
        if (l === LANG) {
          a.setAttribute('aria-current', 'true');
          a.disabled = true;
        } else {
          a.setAttribute('aria-label', CAMBIAR[l]);
          a.addEventListener('click', () => {
            try { localStorage.setItem('guardiana_lang', l); } catch (_) {}
            location.reload();
          });
        }
        b.appendChild(a);
      });
      const n = document.createElement('button');
      n.id = 'tema-toggle'; n.type = 'button'; n.className = 'secondary lang tema';
      const OTRO = TEMA === 'noche' ? 'dia' : 'noche';
      const NOMBRE_TEMA = {
        es: { noche: 'Modo noche', dia: 'Modo dia' },
        en: { noche: 'Night mode', dia: 'Day mode' },
        pt: { noche: 'Modo noite', dia: 'Modo dia' },
      };
      n.textContent = (NOMBRE_TEMA[LANG] || NOMBRE_TEMA.es)[OTRO];
      n.setAttribute('aria-label', n.textContent);
      n.addEventListener('click', () => {
        try { localStorage.setItem('guardiana_tema', OTRO); } catch (_) {}
        location.reload();
      });
      // El botón del tema viaja con los idiomas: sueltos en la cabecera, en inglés se caían a una
      // segunda línea porque el menú es más largo que en español.
      b.appendChild(n);
      header.appendChild(b);
    }
  }

  // ----- share card (brief §8): drawn in the browser, real figures, no names ----
  function drawCard(canvas, r) {
    const ctx = canvas.getContext('2d');
    const W = canvas.width, H = canvas.height;
    const dark = document.documentElement.dataset.tema === 'noche';
    ctx.fillStyle = dark ? '#0B1020' : '#F4F6FA';
    ctx.fillRect(0, 0, W, H);
    ctx.fillStyle = dark ? '#E8ECF6' : '#0B1020';
    ctx.font = '700 64px -apple-system, "Segoe UI", Roboto, sans-serif';
    ctx.fillText('Guardiana', 80, 140);
    ctx.font = '400 40px -apple-system, "Segoe UI", Roboto, sans-serif';
    ctx.fillStyle = dark ? '#98A1B8' : '#5B6275';
    ctx.fillText(t('tarjeta_titulo'), 80, 210);
    const rows = [
      [r.servicios, t('c_servicios'), null],
      [r.rastreadores, t('c_rastreadores'), '#b23a3a'],
      [r.destinos_nuevos, t('c_nuevos'), null],
      [r.esperados, t('c_esperados'), '#1f6f4a'],
      [r.cortados, t('c_cortados'), '#b23a3a'],
    ];
    let y = 340;
    for (const [n, label, color] of rows) {
      ctx.fillStyle = color || (dark ? '#E8ECF6' : '#0B1020');
      ctx.font = '700 96px -apple-system, "Segoe UI", Roboto, sans-serif';
      ctx.fillText(String(n), 80, y);
      ctx.fillStyle = dark ? '#98A1B8' : '#5B6275';
      ctx.font = '400 40px -apple-system, "Segoe UI", Roboto, sans-serif';
      ctx.fillText(label, 360, y - 8);
      y += 130;
    }
    ctx.fillStyle = dark ? '#98A1B8' : '#5B6275';
    ctx.font = '400 32px -apple-system, "Segoe UI", Roboto, sans-serif';
    ctx.fillText(t('tarjeta_pie'), 80, H - 70);
    ctx.fillText(new Date(r.hasta).toLocaleString(LOC, DIA_HORA), 80, H - 120);
  }
  const waLink = (text) => 'https://wa.me/?text=' + encodeURIComponent(text);

  // ----- pages -------------------------------------------------------------
  const pages = {
    async informe() {
      const r = await api('/api/informe');
      $('i-periodo').textContent = t('informe_periodo').replace('{desde}', new Date(r.desde).toLocaleDateString(LOC, DIA)).replace('{hasta}', new Date(r.hasta).toLocaleDateString(LOC, DIA));
      // Free plan: the table is a marked example and the Plus card shows, with "Ahora no" (decision 53).
      $('i-ejemplo').classList.toggle('hidden', !r.ejemplo);
      if (r.ejemplo) document.querySelector('[data-t=informe_lead]').textContent = t('informe_lead_ejemplo');
      $('i-whatsapp').classList.toggle('hidden', !r.plus);
      let dismissed = false;
      try { dismissed = localStorage.getItem('guardiana_plus_no_informe') === '1'; } catch (_) {}
      $('i-plus').classList.toggle('hidden', r.plus || dismissed);
      $('i-plus-no').addEventListener('click', () => { try { localStorage.setItem('guardiana_plus_no_informe', '1'); } catch (_) {} $('i-plus').classList.add('hidden'); });
      const row = (label, f) => `<td>${label}</td><td>${f.consultas}</td><td>${f.rastreadores}</td><td>${f.publicidad}</td><td>${f.telemetria}</td><td>${f.esperados}</td><td>${f.desconocidos}</td><td>${f.cortados}</td>`;
      $('i-rows').innerHTML = r.dispositivos.map((d) => `<tr>${row(esc(d.name || (d.id === 'self' ? t('este_computador') : d.id)), d.fila)}</tr>`).join('') || `<tr><td colspan="8" class="muted">${esc(t('informe_sin_datos'))}</td></tr>`;
      $('i-total').innerHTML = row('<strong>Total</strong>', r.total);
      $('i-texto').textContent = r.texto_whatsapp;
      $('i-wa').href = waLink(r.texto_whatsapp);
      savePdf('i-pdf', () => {
        const doc = pdfDocument('GUARDIANA · ' + t('nav_informe'));
        pdfHead(doc, t('nav_informe') + (r.ejemplo ? ' · EJEMPLO' : ''));
        doc.line($('i-periodo').textContent, 10);
        if (r.ejemplo) doc.line(t('informe_ejemplo_aviso'), 9, true);
        doc.gap(6);
        const cols = [{ title: t('col_dispositivo'), w: 0.16 }, { title: t('col_consultas'), w: 0.12 }, { title: t('c_rastreadores'), w: 0.12 }, { title: t('col_publicidad'), w: 0.12 }, { title: t('col_telemetria'), w: 0.12 }, { title: t('col_esperados'), w: 0.12 }, { title: t('col_desconocidos'), w: 0.12 }, { title: t('col_cortados'), w: 0.12 }];
        const cells = (label, f) => [label, f.consultas, f.rastreadores, f.publicidad, f.telemetria, f.esperados, f.desconocidos, f.cortados].map(String);
        const rows = r.dispositivos.map((d) => cells(d.name || (d.id === 'self' ? t('este_computador') : d.id), d.fila));
        rows.push({ cells: cells('Total', r.total), bold: true });
        doc.table(cols, rows, 8);
        if (r.plus) doc.gap(10).line(t('informe_whatsapp_titulo'), 12, true).line(r.texto_whatsapp, 10);
        return { name: pdfName('informe-semanal'), doc };
      });
      $('i-copiar').addEventListener('click', async () => {
        try { await navigator.clipboard.writeText(r.texto_whatsapp); $('i-copiado').textContent = t('informe_copiado'); } catch (_) {}
      });
    },
    async reglas() {
      const devs = await api('/api/dispositivos');
      const sel = $('n-device');
      devs.forEach((d) => { const o = document.createElement('option'); o.value = d.id; o.textContent = (d.name || (d.id === 'self' ? t('este_computador') : d.id)) + (d.puede_cortar ? '' : ' · ' + t('observando').replace('{h}', d.horas_observadas)); sel.appendChild(o); });
      $('n-scope').addEventListener('change', () => $('n-device-wrap').classList.toggle('hidden', $('n-scope').value !== 'device'));
      $('n-kind').addEventListener('change', () => { $('n-pattern').placeholder = $('n-kind').value === 'category' ? 'rastreador / publicidad / telemetria / desconocido' : 'ejemplo.com'; });
      const load = async () => {
        const r = await api('/api/reglas');
        document.querySelectorAll('input[name=modo]').forEach((i) => { i.checked = i.value === r.modo_bloqueo; });
        $('r-rows').innerHTML = r.reglas.map((x) => `<tr><td>${x.id}</td><td>${x.activa ? esc(t('regla_activa')) : esc(t('regla_deshecha'))}</td><td>${x.scope === 'home' ? esc(t('alcance_casa')) : esc(x.device_name || x.device_id || '')}</td><td>${esc(t('tipo_' + x.match_kind))}</td><td class="mono">${esc(x.pattern)}</td><td class="verdict ${x.action === 'cortar' ? 'cortado' : ''}">${esc(t('accion_' + x.action))}</td><td>${esc(x.created_by)}</td><td class="muted">${when(x.created_at)}</td><td>${x.activa ? `<button class="secondary undo" data-id="${x.id}">${esc(t('deshacer'))}</button>` : ''}</td></tr>`).join('') || `<tr><td colspan="9" class="muted">${esc(t('reglas_ninguna'))}</td></tr>`;
      };
      $('nueva').addEventListener('submit', async (e) => {
        e.preventDefault();
        const f = new FormData($('nueva'));
        const body = { scope: f.get('scope'), device_id: f.get('device_id') || null, match_kind: f.get('match_kind'), pattern: f.get('pattern'), action: f.get('action') };
        try {
          const created = await createRule(body);
          $('n-msg').textContent = created ? t('regla_creada') : '';
          if (created) { $('n-pattern').value = ''; await load(); }
        } catch (err) { $('n-msg').textContent = String(err.message || err); }
      });
      $('r-rows').addEventListener('click', async (e) => {
        const b = e.target.closest('button.undo');
        if (!b) return;
        await api('/api/reglas/' + b.dataset.id + '/deshacer', { method: 'POST' });
        await load();
      });
      $('undo-today').addEventListener('click', async () => {
        const n = await api('/api/reglas/deshacer-hoy', { method: 'POST' });
        $('undo-msg').textContent = t('deshechas').replace('{n}', n);
        await load();
      });
      $('modo-guardar').addEventListener('click', async () => {
        const modo = (document.querySelector('input[name=modo]:checked') || {}).value || 'nxdomain';
        await api('/api/ajustes/bloqueo', { method: 'POST', body: { modo } });
        $('modo-msg').textContent = t('guardado');
      });
      await load();
    },
    async radiografia() {
      let last = null;
      savePdf('r-pdf', () => {
        const doc = pdfDocument('GUARDIANA · ' + t('radiografia_titulo'));
        pdfHead(doc, t('radiografia_titulo'));
        const r = last || { servicios: 0, rastreadores: 0, destinos_nuevos: 0, esperados: 0, cortados: 0, eventos: [] };
        doc.line([['c_servicios', r.servicios], ['c_rastreadores', r.rastreadores], ['c_nuevos', r.destinos_nuevos], ['c_esperados', r.esperados], ['c_cortados', r.cortados]].map(([k, v]) => t(k) + ': ' + v).join(' · '), 10);
        if (r.hueco) doc.line(t('hueco').replace('{desde}', when(r.hueco.desde)).replace('{hasta}', when(r.hueco.hasta)), 9, false, 0.4);
        doc.gap(6).table(eventCols(true), r.eventos.length ? r.eventos.map((ev) => eventCells(ev, true)) : [[ '', t('sin_consultas_aun'), '', '', '' ]]);
        return { name: pdfName('radiografia'), doc };
      });
      await loadGate();
      await cargarCortados();
      // Until the system DNS points at Guardiana, this PC's own queries never arrive: say so and offer the change (brief §4).
      try {
        const est = await api('/api/estado');
        $('dns-card').classList.toggle('hidden', est.dns_aplicado);
        $('dns-apply').addEventListener('click', async () => {
          $('dns-msg').textContent = t('dns_aplicando'); $('dns-apply').disabled = true;
          try { const r = await api('/api/dns/aplicar', { method: 'POST' }); $('dns-msg').textContent = r.mensaje; setTimeout(() => $('dns-card').classList.add('hidden'), 6000); }
          catch (err) { $('dns-msg').textContent = String(err.message || err); $('dns-apply').disabled = false; }
        });
      } catch (_) {}
      // The first Plus moment (decision 53): after 24 h, once, dismissable, never daily.
      try {
        const dismissed = localStorage.getItem('guardiana_plus_no_inicio') === '1';
        if (!dismissed) {
          const lic = await api('/api/licencia');
          if (lic.invitar) $('plus-card').classList.remove('hidden');
        }
        $('plus-no').addEventListener('click', () => { try { localStorage.setItem('guardiana_plus_no_inicio', '1'); } catch (_) {} $('plus-card').classList.add('hidden'); });
      } catch (_) {}
      bindCutButtons($('live'), '/api/reglas');
      $('share-open').addEventListener('click', () => {
        if (!last) return;
        drawCard($('share-canvas'), last);
        $('share-download').href = $('share-canvas').toDataURL('image/png');
        const text = t('compartir_texto').replace('{servicios}', last.servicios).replace('{rastreadores}', last.rastreadores).replace('{nuevos}', last.destinos_nuevos).replace('{esperados}', last.esperados).replace('{cortados}', last.cortados);
        $('share-wa').href = waLink(text);
        $('share').classList.remove('hidden');
      });
      $('share-close').addEventListener('click', () => $('share').classList.add('hidden'));
      const refresh = async () => {
        const r = await api('/api/radiografia?segundos=60');
        last = r;
        $('c-servicios').textContent = r.servicios;
        $('c-rastreadores').textContent = r.rastreadores;
        $('c-nuevos').textContent = r.destinos_nuevos;
        $('c-esperados').textContent = r.esperados;
        $('c-cortados').textContent = r.cortados;
        $('live').innerHTML = eventRows(r.eventos) || `<tr><td colspan="5" class="muted">${esc(t('sin_consultas_aun'))}</td></tr>`;
        $('hueco').textContent = r.hueco ? t('hueco').replace('{desde}', when(r.hueco.desde)).replace('{hasta}', when(r.hueco.hasta)) : '';
        $('hueco').classList.toggle('hidden', !r.hueco);
      };
      await refresh();
      setInterval(() => refresh().catch(() => {}), 2000);
      // Bursts change slowly and cost a query over the whole day: painted once, not every
      // two seconds like the live table.
      pintarRafagas().catch(() => {});
    },
    async extracto() {
      const form = $('filters');
      paintChanges().catch(() => {});
      await loadGate();
      await cargarCortados();
      let lastR = null, lastFilters = '';
      savePdf('export-pdf', () => {
        const doc = pdfDocument('GUARDIANA · ' + t('nav_extracto'));
        pdfHead(doc, t('nav_extracto'));
        const r = lastR || { eventos: [], total: 0, huecos: [] };
        doc.line(t('mostrando').replace('{n}', r.eventos.length).replace('{total}', r.total), 10);
        if (lastFilters) doc.line(lastFilters, 9, false, 0.4);
        (r.huecos || []).forEach((h) => doc.line(t('hueco').replace('{desde}', when(h.desde)).replace('{hasta}', when(h.hasta)), 9, false, 0.4));
        doc.gap(6).table(eventCols(true), r.eventos.length ? r.eventos.map((ev) => eventCells(ev, true)) : [[ '', t('sin_resultados'), '', '', '' ]]);
        return { name: pdfName('extracto'), doc };
      });
      bindCutButtons($('rows'), '/api/reglas');
      const load = async () => {
        const q = new URLSearchParams(new FormData(form));
        for (const [k, v] of [...q]) if (!v) q.delete(k);
        const r = await api('/api/extracto?' + q.toString());
        lastR = r;
        lastFilters = [...q].filter(([k]) => k !== 'limit').map(([k, v]) => t('f_' + ({ device_id: 'dispositivo', category: 'categoria', signal: 'senal', verdict: 'veredicto' }[k] || k)) + ': ' + (k === 'device_id' ? ($('f-device').selectedOptions[0] || {}).textContent || v : v)).join(' · ');
        $('rows').innerHTML = eventRows(r.eventos) || `<tr><td colspan="5" class="muted">${esc(t('sin_resultados'))}</td></tr>`;
        $('total').textContent = t('mostrando').replace('{n}', r.eventos.length).replace('{total}', r.total);
        $('huecos').innerHTML = (r.huecos || []).map((h) => `<li>${esc(t('hueco').replace('{desde}', when(h.desde)).replace('{hasta}', when(h.hasta)))}</li>`).join('');
        $('huecos-card').classList.toggle('hidden', !(r.huecos && r.huecos.length));
        // Exports go through fetch with the token in a header: the token never
        // ends up in a link, the browser history or the downloads list.
        const exportQuery = q.toString();
        $('export-csv').dataset.query = exportQuery;
        $('export-json').dataset.query = exportQuery;
      };
      const download = async (formato, ext) => {
        const el = $('export-' + formato);
        const query = el.dataset.query || '';
        const resp = await fetch('/api/extracto/exportar?' + query + (query ? '&' : '') + 'formato=' + formato, { headers: { 'X-Guardiana-Token': token, 'X-Guardiana-Lang': LANG } });
        if (!resp.ok) { throw new Error('HTTP ' + resp.status); }
        const blob = await resp.blob();
        const a = document.createElement('a');
        a.href = URL.createObjectURL(blob);
        a.download = 'guardiana-extracto.' + ext;
        document.body.appendChild(a); a.click(); a.remove();
        setTimeout(() => URL.revokeObjectURL(a.href), 10000);
      };
      $('export-csv').addEventListener('click', (e) => { e.preventDefault(); download('csv', 'csv').catch((err) => { $('check-result').className = 'bad'; $('check-result').textContent = String(err.message || err); }); });
      $('export-json').addEventListener('click', (e) => { e.preventDefault(); download('json', 'json').catch((err) => { $('check-result').className = 'bad'; $('check-result').textContent = String(err.message || err); }); });
      form.addEventListener('submit', (e) => { e.preventDefault(); load().catch(console.error); });
      $('check').addEventListener('click', async () => {
        const r = await api('/api/extracto/comprobar');
        const out = $('check-result');
        if (r.ok) {
          out.className = 'ok';
          out.textContent = (r.anchor_is_genesis ? t('cadena_ok') : t('cadena_ok_recortada')).replace('{n}', r.checked).replace('{p}', r.pruned);
        } else {
          out.className = 'bad';
          out.textContent = t('cadena_rota').replace('{id}', r.fallo.id).replace('{motivo}', r.fallo.motivo);
        }
      });
      const devs = await api('/api/dispositivos');
      const sel = $('f-device');
      devs.forEach((d) => { const o = document.createElement('option'); o.value = d.id; o.textContent = d.name || (d.id === 'self' ? t('este_computador') : d.id); sel.appendChild(o); });
      await load();
    },
    async dispositivos() {
      pintarRecibos().catch(() => {});
      const devs = await api('/api/dispositivos');
      $('devices').innerHTML = devs.map((d) => `<tr>
        <td>${esc(d.name || (d.id === 'self' ? t('este_computador') : t('dispositivo_nuevo')))}<br><span class="mono muted">${esc(d.last_ip || '')}</span>${hints(d.lectura).map((h) => `<br><span class="phrase">· ${esc(h)}</span>`).join('')}</td>
        <td>${d.totales.consultas}</td><td>${d.totales.rastreadores}</td><td>${d.totales.publicidad}</td><td>${d.totales.telemetria}</td><td>${d.totales.esperados}</td><td>${d.totales.cortados}</td>
        <td title="${esc((d.lectura.empresas || []).map((e) => e[0] + ' ' + e[1]).join(', '))}">${d.lectura.empresas_total || 0}<br><span class="muted">${esc((d.lectura.empresas || []).slice(0, 3).map((e) => e[0]).join(', '))}</span></td>
        <td class="muted">${when(d.last_seen)}</td>
        <td>${d.detalle_visible ? esc(t('detalle_si')) : esc(t('detalle_no'))}</td></tr>`).join('') || `<tr><td colspan="10" class="muted">${esc(t('sin_dispositivos'))}</td></tr>`;
    },
    async sabeDeTi() {
      const r = await api('/api/sabe-de-ti');
      $('s-eventos').textContent = r.eventos; $('s-dispositivos').textContent = r.dispositivos;
      $('s-reglas').textContent = r.reglas; $('s-nombres').textContent = r.nombres_vistos;
      $('s-ruta').textContent = r.ruta;
      $('s-retencion').textContent = r.retencion;
      $('outbound').innerHTML = r.outbound.map((o) => `<tr><td>${when(o.ts)}</td><td>${esc(o.purpose)}</td><td class="mono">${esc(o.host)}</td><td>${o.bytes}</td></tr>`).join('') || `<tr><td colspan="4" class="ok">${esc(t('outbound_vacio'))}</td></tr>`;
      $('wipe').addEventListener('click', async () => {
        const word = prompt(t('borrar_confirmar'));
        if (word !== 'BORRAR') return;
        await api('/api/sabe-de-ti/borrar', { method: 'POST', body: { confirmacion: word } });
        location.reload();
      });
    },
    async hogar() {
      // On a Mac, say it plainly: Home Mode works there but is not part of 1.0, and the site says
      // the same. Offering it in silence would make the program and the page disagree.
      api('/api/estado').then((e) => {
        if (e && e.so === 'macos') $('hogar-mac').classList.remove('hidden');
      }).catch(() => {});
      const render = (r, aviso) => {
        $('h-estado').textContent = r.encendido
          ? t('hogar_estado_on').replace('{fecha}', r.desde ? when(r.desde) : '').replace('{ip}', r.ip || '')
          : t('hogar_estado_off');
        const ipKey = r.ip_dinamica === true ? 'hogar_ip_dinamica' : r.ip_dinamica === false ? 'hogar_ip_fija' : 'hogar_ip_desconocida';
        $('h-ip-aviso').textContent = r.ip ? t(ipKey).replace('{ip}', r.ip) : t('hogar_sin_lan');
        const sv = (n) => n === 0 ? t('hogar_suspension_nunca_valor') : t('hogar_suspension_valor').replace('{n}', n);
        const sl = r.suspension;
        $('h-suspension').textContent = !sl ? '' : (sl[0] === 0 && sl[1] === 0) ? t('hogar_suspension_nunca') : t('hogar_suspension').replace('{ac}', sv(sl[0])).replace('{dc}', sv(sl[1]));
        $('h-toggle').textContent = r.encendido ? t('hogar_desactivar') : t('hogar_activar');
        if (r.licencia) { $('h-ip-aviso').textContent += ' ' + r.licencia; }
        $('h-toggle').dataset.on = r.encendido ? '1' : '0';
        $('h-aviso').textContent = aviso || '';
        $('h-conectar').classList.toggle('hidden', !r.encendido);
        if (r.encendido) {
          $('h-qr').innerHTML = r.qr_svg || '';
          $('h-url').textContent = r.url || '';
          $('h-router').textContent = t('hogar_router_texto').replace('{ip}', r.ip || '');
          $('h-iphone').textContent = t('hogar_iphone_pasos').replace('{ip}', r.ip || '');
          $('h-android').textContent = t('hogar_android_pasos').replace('{ip}', r.ip || '');
        }
      };
      render(await api('/api/hogar'));
      $('h-toggle').addEventListener('click', async () => {
        const on = $('h-toggle').dataset.on === '1';
        try {
          const r = await api(on ? '/api/hogar/desactivar' : '/api/hogar/activar', { method: 'POST' });
          render(r.hogar, r.aviso);
        } catch (e) { $('h-aviso').textContent = String(e.message || e); }
      });
    },
    async miDispositivo() {
      let lastR = null, lastRules = [];
      savePdf('mi-pdf', () => {
        const doc = pdfDocument('GUARDIANA · ' + t('nav_mi'));
        pdfHead(doc, t('mi_titulo'));
        const r = lastR || { totales: { consultas: 0, rastreadores: 0, esperados: 0, cortados: 0 }, eventos: [] };
        if (r.dispositivo) doc.line((r.dispositivo.name || (r.dispositivo.id === 'self' ? t('este_computador') : r.dispositivo.id)) + (r.dispositivo.ip ? ' · ' + r.dispositivo.ip : ''), 10, true);
        doc.line([['col_consultas', r.totales.consultas], ['c_rastreadores', r.totales.rastreadores], ['c_esperados', r.totales.esperados], ['c_cortados', r.totales.cortados]].map(([k, v]) => t(k) + ': ' + v).join(' · '), 10);
        doc.gap(6).table(eventCols(false), r.eventos.length ? r.eventos.map((ev) => eventCells(ev, false)) : [[ '', t('sin_consultas_aun'), '', '' ]]);
        if (lastRules.length) {
          doc.gap(10).line(t('mi_reglas_titulo'), 12, true).gap(4);
          doc.table([{ title: t('col_tipo'), w: 0.2 }, { title: t('col_patron'), w: 0.4 }, { title: t('col_accion'), w: 0.2 }, { title: t('col_estado'), w: 0.2 }], lastRules.map((x) => [t('tipo_' + x.match_kind), x.pattern, t('accion_' + x.action), x.activa ? t('regla_activa') : t('regla_deshecha')]));
        }
        return { name: pdfName('mi-dispositivo'), doc };
      });
      let seen = false;
      try { seen = sessionStorage.getItem('guardiana_consent') === '1'; } catch (_) {}
      if (!seen) $('consent').classList.remove('hidden');
      $('consent-ok').addEventListener('click', () => { try { sessionStorage.setItem('guardiana_consent', '1'); } catch (_) {} $('consent').classList.add('hidden'); });
      const load = async () => {
        const r = await api('/api/mi-dispositivo');
        lastR = r;
        if (r.es_este_computador) { $('mi-self').classList.remove('hidden'); $('mi-body').classList.add('hidden'); return; }
        $('mi-pasa').textContent = r.dispositivo ? t('mi_pasa') : t('mi_no_pasa');
        $('m-consultas').textContent = r.totales.consultas; $('m-rastreadores').textContent = r.totales.rastreadores;
        $('m-esperados').textContent = r.totales.esperados; $('m-cortados').textContent = r.totales.cortados;
        // Never overwrite what the person is typing: the page reloads every few seconds.
        const nameInput = document.querySelector('#mi-nombre input');
        if (r.dispositivo && document.activeElement !== nameInput && !nameInput.dataset.dirty) { nameInput.value = r.dispositivo.name || ''; }
        if (r.dispositivo && document.activeElement !== $('mi-compartir')) { $('mi-compartir').checked = !!r.dispositivo.share_detail_with_home; }
        const me = r.dispositivo;
        const can = !!(me && me.puede_cortar);
        $('mi-gate').textContent = me && !can ? t('observando').replace('{h}', me.horas_observadas) : '';
        const lect = r.lectura || {};
        $('mi-empresas').innerHTML = (lect.empresas && lect.empresas.length) ? lect.empresas.map((e) => `<li><b>${esc(e[0])}</b> · ${e[1]}</li>`).join('') + `<li class="muted">${esc(t('lectura_empresas_total').replace('{n}', lect.empresas_total))}</li>` : `<li class="muted">${esc(t('lectura_empresas_ninguna'))}</li>`;
        $('mi-avisos').innerHTML = hints(lect).map((h) => `<p class="limit">${esc(h)}</p>`).join('');
        $('mi-rows').innerHTML = r.eventos.map((ev) => `<tr><td class="mono">${clock(ev.ts)}</td><td><span class="mono">${esc(ev.qname)}</span>${company(ev)}${phrases(ev)}</td><td>${cat(ev)}</td><td>${verdict(ev.verdict)}</td><td>${ev.verdict !== 'cortado' && can ? `<button class="secondary cut" data-device="${esc(ev.device_id)}" data-name="${esc(ev.qname)}">${esc(t('cortar'))}</button>` : ''}</td></tr>`).join('') || `<tr><td colspan="5" class="muted">${esc(t('sin_consultas_aun'))}</td></tr>`;
        const rules = await api('/api/mi-dispositivo/reglas');
        lastRules = rules.reglas;
        $('mi-reglas').innerHTML = rules.reglas.map((x) => `<tr><td>${esc(t('tipo_' + x.match_kind))}</td><td class="mono">${esc(x.pattern)}</td><td>${esc(t('accion_' + x.action))}</td><td>${x.activa ? esc(t('regla_activa')) : esc(t('regla_deshecha'))}</td><td>${x.activa ? `<button class="secondary undo" data-id="${x.id}">${esc(t('deshacer'))}</button>` : ''}</td></tr>`).join('') || `<tr><td colspan="5" class="muted">${esc(t('reglas_ninguna'))}</td></tr>`;
      };
      bindCutButtons($('mi-rows'), '/api/mi-dispositivo/reglas');
      $('mi-reglas').addEventListener('click', async (e) => {
        const b = e.target.closest('button.undo');
        if (!b) return;
        await api('/api/mi-dispositivo/reglas/' + b.dataset.id + '/deshacer', { method: 'POST' });
        await load();
      });
      document.querySelector('#mi-nombre input').addEventListener('input', (e) => { e.target.dataset.dirty = '1'; });
      $('mi-nombre').addEventListener('submit', async (e) => { e.preventDefault(); const inp = document.querySelector('#mi-nombre input'); try { await api('/api/mi-dispositivo/nombre', { method: 'POST', body: { name: inp.value } }); delete inp.dataset.dirty; inp.blur(); $('mi-nombre-msg').textContent = t('mi_nombre_guardado'); setTimeout(() => { $('mi-nombre-msg').textContent = ''; }, 3000); } catch (err) { $('mi-nombre-msg').textContent = String(err.message || err); } await load(); });
      $('mi-compartir').addEventListener('change', async (e) => { await api('/api/mi-dispositivo/compartir', { method: 'POST', body: { compartir: e.target.checked } }); });
      await load();
      setInterval(() => load().catch(() => {}), 5000);
    },
    async ia() {
      const paint = (r) => {
        // Quién habló con ese servicio, si el sistema lo dijo: en esta pantalla es la pregunta
        // que importa —que hablara el agente y no el navegador—, así que va pegado al aparato.
        const quien = (x) => (x.programas && x.programas.length
          ? '<br>' + x.programas.map((p) => `<span class="tag app" title="${esc(t('ia_programa_titulo'))}">${esc(p[0])} ×${p[1]}</span>`).join(' ')
          : '');
        $('ia-servicios').innerHTML = r.servicios.map((x) => `<tr>
          <td><span class="tag ia">${esc(x.servicio)}</span></td>
          <td>${esc(x.device_name || (x.device_id === 'self' ? t('este_computador') : x.device_id))}${quien(x)}</td>
          <td>${x.nombres}</td><td>${x.consultas}</td><td class="muted">${when(x.ultima)}</td></tr>`).join('')
          || `<tr><td colspan="5" class="muted">${esc(t('ia_sin_servicios'))}</td></tr>`;
        $('alcances').innerHTML = r.alcances.map((a) => `<div class="card">
          <h3>${esc(a.device_name || (a.device_id === 'self' ? t('este_computador') : a.device_id))}</h3>
          <p class="mono muted">${a.patrones.map(esc).join(' · ')}</p>
          <p>${esc(t('alcance_dentro').replace('{n}', a.dentro_total))} · <b>${esc(t('alcance_fuera').replace('{n}', a.fuera_total))}</b></p>
          <p class="vigilante"><label><input type="checkbox" class="a-cortar" data-device="${esc(a.device_id)}"${a.cortar ? ' checked' : ''}> <b>${esc(t('alcance_cortar'))}</b></label>
            <br><span class="muted">${esc(t(a.cortar ? 'alcance_cortar_on' : 'alcance_cortar_off'))}</span>
            <br><span class="muted">${esc(t('alcance_cortar_todo'))}</span></p>
          ${a.pase_hasta ? `<p class="ok">${esc(t('alcance_pase_activo').replace('{hora}', when(a.pase_hasta)))}</p>`
            : (a.cortar ? `<p><button class="secondary a-pase" data-device="${esc(a.device_id)}" data-horas="4">${esc(t('alcance_pase').replace('{h}', 4))}</button></p>` : '')}
          ${a.fuera.length ? `<div class="wrap"><table><tbody>${a.fuera.map((f) => `<tr><td class="mono">${esc(f[0])}</td><td>${f[1]}</td>`
            + `<td><button class="secondary a-anadir" data-device="${esc(a.device_id)}" data-name="${esc(f[0])}">${esc(t('alcance_anadir'))}</button></td></tr>`).join('')}</tbody></table></div>`
            : `<p class="ok">${esc(t('alcance_fuera_ninguno'))}</p>`}
          <p class="limit">${esc(t('alcance_esperado_nota'))}</p>
          ${a.cortar ? `<p class="limit">${esc(t('alcance_cortado_aviso'))}</p>` : ''}</div>`).join('');
        const opts = r.alcances.map((a) => [a.device_id, a.device_name, a.patrones.join('\n')])
          .concat(r.sin_alcance.map((d) => [d[0], d[1], '']));
        const sel = $('a-device');
        const keep = sel.value;
        sel.innerHTML = opts.map(([id, name]) => `<option value="${esc(id)}">${esc(name || (id === 'self' ? t('este_computador') : id))}${opts.find((o) => o[0] === id)[2] ? '' : ' · ' + t('alcance_sin_declarar')}</option>`).join('');
        if (keep && opts.some((o) => o[0] === keep)) sel.value = keep;
        const current = opts.find((o) => o[0] === sel.value);
        $('a-patrones').value = current ? current[2] : '';
        sel.onchange = () => { const c = opts.find((o) => o[0] === sel.value); $('a-patrones').value = c ? c[2] : ''; };
      };
      paint(await api('/api/ia'));
      $('alcance-form').addEventListener('submit', async (e) => {
        e.preventDefault();
        const patrones = $('a-patrones').value;
        const r = await api('/api/ia/alcance', { method: 'POST', body: { device_id: $('a-device').value, patrones } });
        $('a-msg').textContent = patrones.trim() ? t('alcance_guardado') : t('alcance_borrado');
        setTimeout(() => { $('a-msg').textContent = ''; }, 5000);
        paint(r);
      });
      // Turning the cut on or off, and answering a name that was cut for falling outside.
      // Both are the person's decision, one name at a time, as the brief requires.
      $('alcances').addEventListener('change', async (e) => {
        const c = e.target.closest('input.a-cortar');
        if (!c) return;
        const dev = c.dataset.device;
        const actual = await api('/api/ia');
        const yo = (actual.alcances || []).find((x) => x.device_id === dev);
        // Turning this on cuts everything outside the list on the WHOLE device, because
        // Guardiana cannot tell which program asked. On a machine someone works on that is
        // most of the internet, so the warning is built from their own last 24 hours: the
        // number and a few of the names that would have been cut.
        if (c.checked && yo) {
          const ejemplos = (yo.fuera || []).slice(0, 4).map((f) => f[0]).join(', ') || '—';
          const aviso = t('alcance_cortar_aviso').replace('{n}', yo.fuera_total).replace('{ejemplos}', ejemplos);
          if (!confirm(aviso)) { c.checked = false; return; }
        }
        const r = await api('/api/ia/alcance', {
          method: 'POST',
          body: { device_id: dev, patrones: (yo ? yo.patrones : []).join('\n'), modo: c.checked ? 'cortar' : 'observar' },
        });
        paint(r);
      });
      $('alcances').addEventListener('click', async (e) => {
        const p = e.target.closest('button.a-pase');
        if (p) {
          p.disabled = true;
          const actual = await api('/api/ia');
          const yo = (actual.alcances || []).find((x) => x.device_id === p.dataset.device);
          paint(await api('/api/ia/alcance', {
            method: 'POST',
            body: { device_id: p.dataset.device, patrones: (yo ? yo.patrones : []).join('\n'), pase_horas: Number(p.dataset.horas) },
          }));
          return;
        }
        const b = e.target.closest('button.a-anadir');
        if (!b) return;
        b.disabled = true;
        const r = await api('/api/ia/alcance/anadir', { method: 'POST', body: { device_id: b.dataset.device, nombre: b.dataset.name } });
        paint(r);
      });
    },
    async comprobador() {},
    async licencia() {
      const render = (r) => {
        $('l-estado').textContent = r.texto;
        $('l-prueba').textContent = r.estado.plan === 'prueba' ? t('licencia_prueba_texto') : '';
        $('l-probar-card').classList.toggle('hidden', !r.invitar);
        $('l-dev').classList.toggle('hidden', !r.clave_dev);
        $('l-clave-lead').textContent = t('licencia_clave_lead').replace('{host}', r.host_activacion);
        $('l-conexiones').innerHTML = r.conexiones.map((o) => `<tr><td>${when(o.ts)}</td><td class="mono">${esc(o.host)}</td><td>${o.bytes}</td></tr>`).join('') || `<tr><td colspan="3" class="muted">${esc(t('licencia_conexiones_ninguna'))}</td></tr>`;
      };
      render(await api('/api/licencia'));
      $('l-probar').addEventListener('click', async () => {
        $('l-probar-msg').textContent = '…';
        try { render(await api('/api/licencia/probar', { method: 'POST' })); $('l-probar-msg').textContent = ''; }
        catch (err) { $('l-probar-msg').textContent = String(err.message || err); }
      });
      $('l-clave').addEventListener('submit', async (e) => {
        e.preventDefault();
        $('l-clave-msg').textContent = '…';
        try { render(await api('/api/licencia/activar-clave', { method: 'POST', body: { clave: new FormData($('l-clave')).get('clave') } })); $('l-clave-msg').textContent = t('licencia_activada'); }
        catch (err) { $('l-clave-msg').textContent = String(err.message || err); render(await api('/api/licencia')); }
      });
      $('l-archivo').addEventListener('submit', async (e) => {
        e.preventDefault();
        try { render(await api('/api/licencia/activar-archivo', { method: 'POST', body: { texto: new FormData($('l-archivo')).get('texto') } })); $('l-archivo-msg').textContent = t('licencia_activada'); }
        catch (err) { $('l-archivo-msg').textContent = String(err.message || err); }
      });
    },
    async verify() {
      const load = async () => { $('v-texto').textContent = '…'; const r = await api('/api/verify'); $('v-texto').textContent = r.texto; };
      $('v-reload').addEventListener('click', () => load().catch(console.error));
      await load();
    },
    async estado() {
      paintChanges().catch(() => {});
      const r = await api('/api/estado');
      $('e-version').textContent = r.version + (r.clave_dev ? ' · ' + t('clave_dev') : '');
      $('e-escucha').textContent = r.escucha.join(', ');
      $('e-upstream').textContent = r.upstream.join(', ');
      const paintDns = (on) => { $('e-dns').textContent = on ? t('dns_aplicado') : t('dns_no_aplicado'); $('e-dns-apply').classList.toggle('hidden', on); $('e-dns-restore').classList.toggle('hidden', !on); };
      paintDns(r.dns_aplicado);
      $('e-dns-apply').addEventListener('click', async () => { $('e-dns-msg').textContent = t('dns_aplicando'); try { const x = await api('/api/dns/aplicar', { method: 'POST' }); $('e-dns-msg').textContent = x.mensaje; paintDns(x.dns_aplicado); paintChanges().catch(() => {}); } catch (err) { $('e-dns-msg').textContent = String(err.message || err); } });
      $('e-dns-restore').addEventListener('click', async () => { $('e-dns-msg').textContent = '…'; try { const x = await api('/api/dns/restaurar', { method: 'POST' }); $('e-dns-msg').textContent = x.mensaje; paintDns(x.dns_aplicado); paintChanges().catch(() => {}); } catch (err) { $('e-dns-msg').textContent = String(err.message || err); } });
      $('e-listas').innerHTML = r.listas.map((l) => `<tr><td>${esc(l.id)}</td><td>${l.entries}</td><td class="muted">${esc(l.fetched)}</td></tr>`).join('');
    },
  };

  // The texts arrive from /api/textos, and until they do the page still shows the Spanish written
  // into the HTML. On an English panel that is half a screen in the wrong language on every load,
  // which is what «it mixes everything» looked like. The last answer is kept per language and
  // applied before asking again, so only the very first load can show the fallback.
  const CACHE = 'guardiana_textos_' + LANG;
  function cached() {
    try { const raw = localStorage.getItem(CACHE); return raw ? JSON.parse(raw) : null; } catch (_) { return null; }
  }
  function remember() {
    try { localStorage.setItem(CACHE, JSON.stringify(T)); } catch (_) {}
  }

  async function main() {
    document.documentElement.lang = LANG;
    const previo = cached();
    if (previo) { T = previo; applyTexts(); }
    try { T = await api('/api/textos'); remember(); } catch (_) {}
    applyTexts();
    const page = document.body.getAttribute('data-page');
    if (pages[page]) {
      try { await pages[page](); } catch (e) { console.error(e); }
    }
    if (token) {
      document.querySelectorAll('nav a').forEach((a) => { /* token stays in memory; links are plain */ });
    }
  }
  main();
})();
