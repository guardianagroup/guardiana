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
    // Spanish or Portuguese browsers get their language; any other one gets English, not Spanish
    // (28 Sep 2026: a browser in German got the whole panel in Spanish).
    const dos = (navigator.language || 'es').slice(0, 2).toLowerCase();
    LANG = (dos === 'es' || dos === 'pt') ? dos : 'en';
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
  // A text that counts: its `_uno` form when there is one thing (until 1.0.2 the panel said
  // «1 reglas deshechas», «1 nombres distintos»), with {n} filled in.
  // Who made a rule, in words: the stored codes («usuario (panel)») were shown as they are, in
  // Spanish on every panel (review of 5 Oct 2026).
  const creadaPor = (c) => (c === 'usuario (panel)' ? t('creada_panel') : c === 'usuario (dispositivo)' ? t('creada_dispositivo') : c === 'protección máxima' ? t('creada_maxima') : c);
  const tn = (k, n) => t(n === 1 && T.panel && T.panel[k + '_uno'] ? k + '_uno' : k).replace('{n}', n);

  // ----- "Guardar en PDF" ----------------------------------------------------
  // The PDF is written here, in the page, with no third-party code (brief §8): A4 pages,
  // Helvetica (a font every PDF reader carries, so nothing is embedded) and WinAnsi text,
  // which covers Spanish. Tables break across pages; every page gets a footer. Saving goes
  // through the browser's "save as" dialog where it exists (Chrome and Edge on a PC) and
  // through a normal download elsewhere (Safari, Firefox, phones).
  // A4. El margen de arriba es mayor que el de los lados a propósito: con los 40 del resto, el
  // título salía pegado al borde de la hoja y el papel parecía un volante (lo vio el responsable
  // al abrir su extracto, 21 sep 2026).
  const PDF_W = 595.28, PDF_H = 841.89, PDF_M = 40, PDF_ARRIBA = 62;
  // Helvetica advance widths, thousandths of the size, WinAnsi code points 32-255.
  const HELV = '278,278,355,556,556,889,667,191,333,333,389,584,278,333,278,278,556,556,556,556,556,556,556,556,556,556,278,278,584,584,584,556,1015,667,667,722,722,667,611,778,722,278,500,667,556,833,722,778,667,778,722,667,611,722,667,944,667,667,611,278,278,278,469,556,333,556,556,500,556,556,278,556,556,222,222,500,222,833,556,556,556,556,333,500,278,556,500,722,500,500,500,334,260,334,584,376,744,556,222,556,333,1000,556,556,333,1000,667,333,1000,556,611,556,556,222,222,333,333,350,556,1000,333,1000,500,333,944,556,500,667,278,333,556,556,556,556,260,556,333,737,370,556,584,0,737,333,400,549,333,333,333,576,537,278,333,333,365,556,834,834,834,611,667,667,667,667,667,667,1000,722,667,667,667,667,278,278,278,278,722,722,778,778,778,778,778,584,778,722,722,722,722,667,667,611,556,556,556,556,556,556,889,500,556,556,556,556,278,278,278,278,556,556,556,556,556,556,556,549,611,556,556,556,556,500,556,500'.split(',').map(Number);
  // Unicode code points that WinAnsi keeps in the 0x80-0x9F slots, plus spaces and hyphens
  // that toLocaleString() produces in some locales.
  const WINANSI = { 0x20AC: 0x80, 0x201A: 0x82, 0x0192: 0x83, 0x201E: 0x84, 0x2026: 0x85, 0x2020: 0x86, 0x2021: 0x87, 0x02C6: 0x88, 0x2030: 0x89, 0x0160: 0x8A, 0x2039: 0x8B, 0x0152: 0x8C, 0x017D: 0x8E, 0x2018: 0x91, 0x2019: 0x92, 0x201C: 0x93, 0x201D: 0x94, 0x2022: 0x95, 0x2013: 0x96, 0x2014: 0x97, 0x02DC: 0x98, 0x2122: 0x99, 0x0161: 0x9A, 0x203A: 0x9B, 0x0153: 0x9C, 0x017E: 0x9E, 0x0178: 0x9F, 0xA0: 0x20, 0x2009: 0x20, 0x202F: 0x20, 0x2010: 0x2D, 0x2011: 0x2D, 0x2212: 0x2D };
  // What WinAnsi cannot hold is written the nearest way it can: the letter without its accent
  // (ř → r, ș → s), a few letters by hand (ł → l), Cyrillic in Latin letters, arrows and signs in
  // ASCII. Until 1.0.16 all of it became «?», and a Czech or Polish name came out of the deletion
  // letter as «Dvo?ák» (review of 12 Oct 2026). What still has no form (Chinese, emoji) stays «?»;
  // `faltaEnPdf()` says so before a letter is made.
  const A_MANO = { 'ł': 'l', 'Ł': 'L', 'đ': 'd', 'Đ': 'D', 'ı': 'i', 'ħ': 'h', 'Ħ': 'H', 'ŧ': 't', 'Ŧ': 'T', 'ŋ': 'n', 'ĸ': 'k', 'ſ': 's', '→': '->', '←': '<-', '↑': '^', '↓': 'v', '≥': '>=', '≤': '<=', '≠': '!=', '≈': '~', '′': "'", '″': '"', '‐': '-', '−': '-' };
  const CIRILICO = 'a,b,v,g,d,e,zh,z,i,y,k,l,m,n,o,p,r,s,t,u,f,kh,ts,ch,sh,shch,,y,,e,yu,ya'.split(',');
  function aWinAnsi(ch) {
    const cp = ch.codePointAt(0);
    if (cp < 128 || (cp >= 160 && cp < 256) || WINANSI[cp]) return ch;
    if (A_MANO[ch] !== undefined) return A_MANO[ch];
    if (cp >= 0x410 && cp <= 0x44F) {
      const i = (cp - 0x410) % 32;
      const l = CIRILICO[i];
      return cp < 0x430 && l ? l.charAt(0).toUpperCase() + l.slice(1) : l;
    }
    if (cp === 0x401) return 'Yo';
    if (cp === 0x451) return 'yo';
    const sin = ch.normalize('NFD').replace(/[\u0300-\u036f]/g, '');
    if (sin && [...sin].every((c) => { const p = c.codePointAt(0); return p < 128 || (p >= 160 && p < 256) || WINANSI[p]; })) return sin;
    return '?';
  }
  // Text as a "binary string": one character per byte, ready for a PDF string.
  function winAnsi(s) {
    let out = '';
    for (const ch0 of String(s ?? '')) {
      for (const ch of aWinAnsi(ch0)) {
        const cp = ch.codePointAt(0);
        let b = cp < 128 || (cp >= 160 && cp < 256) ? cp : (WINANSI[cp] || 63);
        if (b < 32 && b !== 10) b = 32;
        out += String.fromCharCode(b);
      }
    }
    return out;
  }
  // The characters of `s` the PDF cannot write in any form, and whether it changes any.
  const faltaEnPdf = (s) => [...String(s ?? '')].filter((c) => c !== '?' && aWinAnsi(c) === '?');
  const cambiaEnPdf = (s) => [...String(s ?? '')].some((c) => aWinAnsi(c) !== c);
  const winAnsiLegible = (s) => [...String(s ?? '')].map(aWinAnsi).join('');
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
    const ink = (color) => Array.isArray(color) ? color.map(n).join(' ') + ' rg' : n(color || 0) + ' g';
    const text = (x, yy, bin, size, bold, color) => { ops.push(ink(color) + ' BT /' + (bold ? 'F2' : 'F1') + ' ' + n(size) + ' Tf ' + n(x) + ' ' + n(yy) + ' Td (' + pdfEscape(bin) + ') Tj ET'); };
    const rect = (x, yy, w, h, color) => ops.push(ink(color) + ' ' + n(x) + ' ' + n(yy) + ' ' + n(w) + ' ' + n(h) + ' re f');
    const newPage = () => {
      ops = []; pages.push(ops);
      rect(0, PDF_H - 38, PDF_W, 38, [0.043, 0.063, 0.125]);
      rect(0, PDF_H - 41, PDF_W, 3, [0.122, 0.294, 1]);
      text(PDF_M, PDF_H - 24, winAnsi('GUARDIANA'), 10, true, [1, 1, 1]);
      text(PDF_W - PDF_M - textWidth(winAnsi('PRIVACIDAD Y SEGURIDAD'), 7), PDF_H - 23, winAnsi('PRIVACIDAD Y SEGURIDAD'), 7, false, [0.76, 0.81, 1]);
      y = PDF_H - PDF_ARRIBA;
    };
    const need = (h) => { if (y - h < PDF_M + 18) newPage(); };
    newPage();
    const doc = {
      line(s, size = 10, bold = false, gray = 0) {
        const lh = size * 1.25;
        for (const l of wrapText(winAnsi(s), size, usable)) { need(lh); y -= lh; text(PDF_M, y + size * 0.3, l, size, bold, gray); }
        return doc;
      },
      gap(h = 8) { y -= h; return doc; },
      // A clear section band makes long reports navigable on screen and on paper.
      seccion(titulo, lead) {
        doc.gap(18); need(lead ? 58 : 38);
        y -= 30; rect(PDF_M, y, usable, 26, [0.95, 0.965, 1]); rect(PDF_M, y, 4, 26, [0.122, 0.294, 1]);
        text(PDF_M + 12, y + 8, winAnsi(titulo), 11, true, [0.043, 0.063, 0.125]);
        if (lead) { y -= 4; doc.line(lead, 9, false, 0.38); }
        doc.gap(8); return doc;
      },
      regla() { need(12); y -= 7; ops.push('0.78 G 0.6 w ' + n(PDF_M) + ' ' + n(y) + ' m ' + n(PDF_M + usable) + ' ' + n(y) + ' l S'); return doc; },
      // Pide sitio para lo que viene: sin esto, el nombre de una empresa se quedaba al final de
      // una página y la frase que explica qué vende se iba a la siguiente.
      juntos(h) { need(h); return doc; },
      // cols: [{ title, w (fraction of the width) }]; rows: arrays of strings, or { cells, bold }.
      table(cols, rows, size = 9) {
        const lh = size * 1.25, pad = 3;
        const widths = cols.map((c) => c.w * usable);
        const layout = (cells, bold) => {
          const cellLines = cells.map((c, i) => wrapText(winAnsi(c), size, widths[i] - 2 * pad));
          return { cellLines, bold, h: Math.max(1, ...cellLines.map((l) => l.length)) * lh + 2 * pad };
        };
        const draw = (row, kind) => {
          if (kind === 'header') rect(PDF_M, y - row.h, usable, row.h, [0.043, 0.063, 0.125]);
          else if (kind === 'stripe') rect(PDF_M, y - row.h, usable, row.h, [0.965, 0.972, 0.988]);
          let x = PDF_M;
          row.cellLines.forEach((lines, i) => { lines.forEach((l, k) => text(x + pad, y - pad - lh * (k + 1) + size * 0.3, l, size, row.bold, kind === 'header' ? [1, 1, 1] : [0.043, 0.063, 0.125])); x += widths[i]; });
          y -= row.h;
          ops.push('0.82 G 0.35 w ' + n(PDF_M) + ' ' + n(y) + ' m ' + n(PDF_M + usable) + ' ' + n(y) + ' l S');
        };
        const header = layout(cols.map((c) => c.title), true);
        need(header.h + lh); draw(header, 'header');
        rows.forEach((r, index) => {
          const row = layout(r.cells || r, !!r.bold);
          if (y - row.h < PDF_M + 18) { newPage(); draw(header, 'header'); }
          draw(row, index % 2 ? 'stripe' : 'row');
        });
        return doc;
      },
      // `footer2`, when given, is a second line just above the first, on every page: where the
      // server countries of the document come from (decision 203).
      bytes(footer, footer2) {
        const total = pages.length;
        pages.forEach((p, i) => {
          p.push('0.78 G 0.5 w ' + n(PDF_M) + ' ' + n(PDF_M + 14) + ' m ' + n(PDF_W - PDF_M) + ' ' + n(PDF_M + 14) + ' l S');
          if (footer2) {
            for (const [k, l] of wrapText(winAnsi(footer2), 7, usable).slice(0, 2).entries()) {
              p.push('0.45 g BT /F1 7 Tf ' + n(PDF_M) + ' ' + n(PDF_M - 4 + 8 * (1 - k)) + ' Td (' + pdfEscape(l) + ') Tj ET');
            }
          }
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
        const { name, doc, pie2 } = build();
        const outcome = await savePdfFile(name, doc.bytes(t('pdf_pie'), pie2));
        if (msg) { msg.textContent = outcome === 'cancelado' ? '' : t('pdf_' + outcome).replace('{nombre}', name); setTimeout(() => { msg.textContent = ''; }, 8000); }
      } catch (e) { if (msg) msg.textContent = String(e.message || e); }
    });
  }
  // The day in the file name is the person's day, not UTC's: at eight in the evening in Bogotá
  // the file said tomorrow (review of 8 Oct 2026).
  const pdfName = (what) => { const d = new Date(); const dd = (n) => String(n).padStart(2, '0'); return 'guardiana-' + what + '-' + d.getFullYear() + '-' + dd(d.getMonth() + 1) + '-' + dd(d.getDate()) + '.pdf'; };
  const pdfHead = (doc, heading) => doc.line('GUARDIANA · ' + heading, 16, true).gap(3)
    .line(t('pdf_generado').replace('{fecha}', new Date().toLocaleString(LOC, DIA_HORA)), 9, false, 0.45)
    .regla().gap(14);
  const catName = (c) => T.categorias[c] || c;
  const verdictName = (v) => T.veredictos[v] || v;
  // One name per device on every page: its own; «this PC»; or «new device on your Wi‑Fi» with
  // its address, never the stored `mac:…`/`ip:…` id, which made one device two names depending
  // on the page (review of 10 Oct 2026, panel 10).
  const nombreAparato = (id, name, ip) => name || (id === 'self' ? t('este_computador') : t('dispositivo_nuevo') + ' · ' + (ip || String(id || '').replace(/^(mac|ip):/, '')));
  const deviceName = (ev) => nombreAparato(ev.device_id, ev.device_name);
  const nameCell = (ev) => [ev.qname + (ev._n > 1 ? ' ×' + ev._n : '') + (ev.ia ? ' · ' + t('ia_prefijo') + ': ' + ev.ia : '')].concat((ev.frases || []).map((f) => '· ' + f)).concat(ev.disfraz ? ['· ' + t('disfraz_etiqueta') + ': ' + ev.disfraz.frase] : []).concat(ev.servidor ? ['· ' + ev.servidor.frase] : []).join('\n');
  // El PDF tiene que decir lo MISMO que la pantalla. Enseñaba `catName(ev.category)`, la
  // categoría en bruto, así que todo lo que en pantalla pone «entrega», «red local» o «de
  // Google» salía en el papel como «sin clasificar»; y el país no salía en absoluto (lo vio el
  // responsable al abrir su extracto en PDF, 21 sep 2026).
  const eventCols = (withDevice) => withDevice
    ? [{ title: t('col_hora'), w: 0.10 }, { title: t('col_nombre'), w: 0.28 }, { title: t('col_empresa_n'), w: 0.15 }, { title: t('col_categoria'), w: 0.13 }, { title: t('col_pais'), w: 0.12 }, { title: t('col_dispositivo'), w: 0.12 }, { title: t('col_veredicto'), w: 0.10 }]
    : [{ title: t('col_hora'), w: 0.12 }, { title: t('col_nombre'), w: 0.33 }, { title: t('col_empresa_n'), w: 0.17 }, { title: t('col_categoria'), w: 0.15 }, { title: t('col_pais'), w: 0.13 }, { title: t('col_veredicto'), w: 0.10 }];
  // En el papel no hay dos líneas por celda: la ciudad va en la misma, debajo, como en la
  // pantalla, porque la columna de país se lee de arriba abajo.
  const dondeCell = (ev) => [ev.pais || (ev.pais_dominio ? t('pais_dominio_texto').replace('{pais}', ev.pais_dominio) : ''), ev.ciudad || ''].filter(Boolean).join('\n');
  // En el papel, la etiqueta «compra y venta de datos» va debajo del nombre de la empresa, como
  // en la pantalla va a su lado.
  const empresaTexto = (ev) => (ev.empresa || (ev.alojado ? t('alojado_en').replace('{nube}', ev.alojado) : '')) + (ev.grupo ? '\n' + t('grupo_de').replace('{grupo}', ev.grupo) : '') + (ev.corredor ? '\n' + t('corredor_etiqueta') : '');
  const eventCells = (ev, withDevice) => withDevice
    ? [clock(ev.ts), nameCell(ev), empresaTexto(ev), catTexto(ev), dondeCell(ev), deviceName(ev), verdictName(ev.verdict)]
    : [clock(ev.ts), nameCell(ev), empresaTexto(ev), catTexto(ev), dondeCell(ev), verdictName(ev.verdict)];


  // «Quién hay detrás», al final del PDF: las empresas que de verdad salen en estas consultas,
  // con lo que vende cada una en una línea. Un extracto lleno de nombres como
  // `ats-wrapper.privacymanager.io` no le dice nada a nadie; «LiveRamp · convierte a quien
  // inicia sesión en un identificador que sirve para reconocerte en webs distintas» sí (pedido
  // del responsable, 21 sep 2026). Solo salen las que tienen su frase escrita y comprobada: de
  // las demás se dice el nombre en la tabla y nada más, que es lo honesto.
  function pdfQuienHayDetras(doc, eventos) {
    const por = new Map();
    for (const ev of eventos || []) {
      if (!ev.empresa || !ev.oficio) continue;
      const e = por.get(ev.empresa) || { n: 0, pais: ev.pais || '', oficio: ev.oficio };
      e.n += 1;
      por.set(ev.empresa, e);
    }
    if (!por.size) return;
    const lista = [...por.entries()].sort((a, b) => b[1].n - a[1].n).slice(0, 12);
    doc.seccion(t('pdf_detras_t'), t('pdf_detras_lead'));
    for (const [nombre, e] of lista) {
      // «1 consultas» no lo escribe nadie.
      const veces = e.n === 1 ? t('pdf_detras_una') : t('pdf_detras_veces').replace('{n}', e.n);
      const cabeza = nombre + (e.pais ? ' · ' + e.pais : '') + ' · ' + veces;
      // El nombre y su frase no se separan nunca: se pide de una vez el alto de las dos.
      doc.juntos(10 * 1.25 + 9 * 1.25 * 2).line(cabeza, 10, true).line(e.oficio, 9, false, 0.25).gap(3);
    }
  }
  // The phone's steps as numbered steps, the address to type set apart (the owner, 16 Oct 2026:
  // «hay que explicar cómo conectar el teléfono al Modo Hogar para que sea fácil»).
  function pasosTel(el, texto, ip) {
    const [pasos, ...cola] = texto.split(/(?<=\.)\s+(?=[A-ZÁÉÍÓÚ])/);
    const resalta = (s) => esc(s).split(esc('{ip}')).join(`<b class="ip-tel">${esc(ip)}</b>`);
    el.innerHTML = pasos.replace(/\.$/, '').split(' › ').map((x) => `<li>${resalta(x)}</li>`).join('')
      + (cola.length ? `<li class="nota-tel">${resalta(cola.join(' '))}</li>` : '');
  }
  async function api(path, opts = {}) {
    // The browser's offset in minutes east of UTC, so the dates written into sentences are the
    // person's own day (getTimezoneOffset counts the other way round).
    const headers = Object.assign({ 'X-Guardiana-Token': token, 'X-Guardiana-Lang': LANG, 'X-Guardiana-Tz': String(-new Date().getTimezoneOffset()) }, opts.headers || {});
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
  // /api/estado once per page: the banner and the page share the answer. Each call can cost a
  // PowerShell on Windows, and the radiography asked twice on every load (review of 10 Oct 2026,
  // panel 15).
  let estadoPromesa = null;
  const estadoBase = () => {
    if (!estadoPromesa) estadoPromesa = api('/api/estado').catch((e) => { estadoPromesa = null; throw e; });
    return estadoPromesa;
  };
  // A refresh that stops while the tab is hidden and catches up when it is shown again: the
  // radiography asked every two seconds and the phone's page every five, in a pocket too
  // (review of 10 Oct 2026, panel 25).
  function cadaTanto(fn, ms) {
    let id = null;
    const arranca = () => { if (id === null) id = setInterval(() => { fn().catch(() => {}); }, ms); };
    const para = () => { if (id !== null) { clearInterval(id); id = null; } };
    document.addEventListener('visibilitychange', () => {
      if (document.hidden) { para(); return; }
      fn().catch(() => {});
      arranca();
    });
    if (!document.hidden) arranca();
  }
  const $ = (id) => document.getElementById(id);
  // Rows of devices that keep their detail to themselves: a number, never the names.
  const ocultosTexto = (n) => (n ? ' ' + t(n === 1 ? 'mostrando_ocultos_uno' : 'mostrando_ocultos').replace('{n}', n) : '');
  // «Mostrando 1 de 1 eventos» read wrong: the noun follows the total (panel 22).
  const mostrandoTexto = (r) => t(r.total === 1 ? 'mostrando_uno' : 'mostrando').replace('{n}', Number(r.eventos.length).toLocaleString(LOC)).replace('{total}', Number(r.total).toLocaleString(LOC)) + ocultosTexto(r.ocultos);
  const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
  // A company's name opens its profile (the owner, 12 Oct 2026: touch it and see everything about
  // it). Same tab: the session lives in this tab, and «Volver» comes back here.
  const empresaLink = (nombre) => (nombre ? `<a class="enlace-empresa" href="/empresa?e=${encodeURIComponent(nombre)}" title="${esc(t('ficha_ver_titulo').replace('{empresa}', nombre))}">${esc(nombre)}</a>` : '');
  // Dates follow the panel's language, not the computer's: someone reading the panel in English
  // on a Spanish machine was getting «18/9/2026», and in English that reads as the 9th of a month
  // that does not exist here. English gets a named month so the two can never be swapped.
  const LOC = LANG === 'en' ? 'en-GB' : (LANG === 'pt' ? 'pt-BR' : 'es');
  const DIA = LANG === 'en' ? { day: 'numeric', month: 'short', year: 'numeric' } : undefined;
  const DIA_HORA = LANG === 'en' ? { day: 'numeric', month: 'short', year: 'numeric', hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false } : undefined;
  // La hora sola solo vale para hoy: con un filtro, 200 filas abarcan varios días y se leían
  // como si fueran uno (revisión del 8 oct 2026). Si la fila no es de hoy, lleva el día delante.
  const esHoy = (ms) => {
    const d = new Date(ms);
    const hoy = new Date();
    return d.getFullYear() === hoy.getFullYear() && d.getMonth() === hoy.getMonth() && d.getDate() === hoy.getDate();
  };
  const clock = (ms) => {
    const d = new Date(ms);
    return esHoy(ms) ? d.toLocaleTimeString(LOC) : d.toLocaleDateString(LOC, { day: 'numeric', month: 'numeric' }) + ' ' + d.toLocaleTimeString(LOC);
  };
  // Figures as the language writes them (5.000 / 5,000), the same on every page.
  const num = (n) => Number(n || 0).toLocaleString(LOC);
  const when = (ms) => new Date(ms).toLocaleString(LOC, DIA_HORA);
  // «Unknown» was 88% of every table: a word that told the person nothing while the program
  // already knew who owned 86% of those names. The stored category does not change -- the ledger
  // keeps what was decided at the time -- but what is SHOWN now says what is actually known:
  // the network that delivers somebody else's content, the company the name belongs to, or,
  // when nothing is known, that no open list knows it either. Never a verdict (brief §6).
  // Ad auctions name their servers with the words of the trade: prebid, rtb, ssp, dsp, cookie
  // syncing. No list has caught them all, but the word is in the name itself, so the panel says
  // that (and only that): what the name says, not a verdict (6 Oct 2026: they were most of what
  // was still "unclassified").
  const PALABRAS_SUBASTA = /(^|[.-])(prebid|rtb|ortb|dsp|ssp|usync|csync|pbs|bids?|hb-?bid|adserver)([.-]|$)|cookie-?sync|user-?sync|pixel-?sync/;
  // «Servicio» tampoco decía nada (8 oct 2026: el responsable lo vio como el «sin clasificar» de
  // antes con otro nombre). El propio nombre casi siempre dice para qué es: «login.», «api.»,
  // «cdn.», «config.», «analytics.», «push.», «update.»… Se mira palabra a palabra en lo que va
  // delante del dominio y se dice esa palabra, nada más: una pista que da el nombre, no una
  // comprobación. El orden importa: «cuenta» o «medición» antes que «contenido» o «web».
  const PISTAS = [
    ['anuncios', /^(prebid|rtb|ortb|ssp|usync|csync|bids?|hb-?bid|adserver|ads?|adsystem|banners?|cookiesync)$/],
    ['pago', /^(checkout|pay|payments?|billing|invoices?|cart|orders?|subscribe|subscriptions?)$/],
    ['cuenta', /^(accounts?|login|signin|sign-?in|logon|auth|authn?|oauth2?|oauthaccountmanager|sso|identity|idp|passport|myaccount|appleid|tokens?|session|profile)$/],
    ['medicion', /^(analytics|metrics?|stats?|statistics|telemetry|pixels?|beacons?|track(ing|er)?|trk|collect(or)?|logs?|logging|events?|insights|measure|reports?|monitor(ing)?|probe|diagnostics?|crash(es|lytics)?|perf|rum|nel|sentry|bugsnag|px)$/],
    ['notificaciones', /^(push|notify|notifications?|wns|fcm|gcm|apns|alerts?)$/],
    ['actualizaciones', /^(updates?|upgrade|autoupdate|swupdate|patch(es)?|release|dist|installer|download(s|center)?|dl)$/],
    ['seguridad', /^(safebrowsing|safe|antiphishing|smartscreen|threat|security|protect(ion)?|urs|shield|captcha|recaptcha|hcaptcha)$/],
    ['correo', /^(mail|smtp|imap|pop3?|mx\d*|webmail|email)$/],
    ['mensajes', /^(chat|msg|messages?|messaging|im|xmpp|mqtt)$/],
    ['sincronizacion', /^(backup|drive|onedrive|dropbox)$/],
    ['contenido', /^(cdn|static|assets?|img|images?|image|media|fonts?|video|videos?|content|files?|storage|cache|js|css|thumbs?|pics?|photos?|resources?|scripts?|styles?|s3|blob)$/],
    ['configuracion', /^(config|configuration|settings?|flags?|features?|remoteconfig|clients?\d*|bootstrap|discovery|geo|ip|whatismyip)$/],
    ['app', /^(api|apis|ws|wss|rpc|grpc|gateway|gw|backend|bridge|graph|mobile|app|apps|edge|services?|srv|data|query|search|sockets?|websockets?|realtime|live|ec2|compute|lb|elb)$/],
    ['web', /^(www\d*|web|home|portal)$/],
  ];
  // The registrable part of a name: two labels, or three under a country code with a generic
  // second level (mercadolibre.com.co, bbc.co.uk).
  const nLabelsDominio = (labels) => (labels.length >= 3 && labels[labels.length - 1].length === 2
    && /^(com|co|net|org|gov|edu|ac|or|ne|gob)$/.test(labels[labels.length - 2]) ? 3 : 2);
  const pista = (ev) => {
    if (ev.category !== 'desconocido' || ev.local || ev.entrega) return '';
    const q = String(ev.qname || '').toLowerCase().replace(/\.$/, '');
    if (!ev.empresa && !ev.ia && PALABRAS_SUBASTA.test(q)) return 'anuncios';
    // Lo que dice el nombre vale también cuando no se sabe de quién es: «www.realmadrid.com» es
    // una web aunque ninguna lista diga de quién (el responsable, 9 oct 2026: «al cliente no le
    // sirve un sin clasificar»). De quién es sigue sin decirse: la columna de empresa queda vacía
    // y el color se queda en naranja hasta que se sepa.
    // Only what comes before the registrable domain: the last two labels are the company's name.
    const todas = q.split('.');
    const labels = todas.slice(0, -nLabelsDominio(todas));
    // The bare domain of a known company or AI service is its own site or app; so is a
    // language code alone in front of it (es.tradingview.com).
    if (!labels.length || (labels.length === 1 && /^(es|en|pt|fr|de|it|br|mx|ar)$/.test(labels[0]))) return 'web';
    const words = [];
    // Trailing digits are a counter (api2, cdn3); they go unless what is left is too short to
    // mean anything (s3, ec2, us4 stay as they are).
    labels.forEach((l) => l.split(/[-_]+/).forEach((w) => { if (w) { const sin = w.replace(/\d+$/, ''); words.push(sin.length >= 3 ? sin : w); } }));
    for (const [clave, re] of PISTAS) if (words.some((w) => re.test(w))) return clave;
    return '';
  };
  const catTexto = (ev) => {
    if (ev.category !== 'desconocido') return T.categorias[ev.category] || ev.category;
    if (ev.local) return t('cat_red_local');
    if (ev.entrega) return t('cat_entrega');
    // «de ByteDance» dejó de tener sentido el día que la empresa ganó su propia columna: la fila
    // decía «ByteDance | de ByteDance». Pero «sin clasificar» al lado de «Microsoft» tampoco: el
    // 6 oct 2026 era el 90 % de las filas de un PC normal y el responsable lo vio como «no sabe
    // nada». Si se sabe de quién es, es un servicio de esa empresa (su web, su app, sus
    // servidores), y eso es lo que dice; «sin clasificar» queda para lo que nadie conoce.
    const p = pista(ev);
    if (ev.sector && (!p || p === 'web')) return t('sector_' + ev.sector);
    if (p) return t('cat_pista_' + p);
    if (ev.empresa || ev.ia) return t('cat_servicio');
    return T.categorias.desconocido;
  };
  // El borde discontinuo se reserva para lo que de verdad no se sabe de quién es: si la columna
  // de empresa dice un nombre, la fila ya no es un hueco y no hace falta subrayarla.
  const catClase = (ev) => (ev.category !== 'desconocido' ? '' : (ev.local || ev.entrega) ? ' entrega' : (ev.empresa || ev.ia || ev.sector || pista(ev)) ? ' propio' : ' sinlista');
  // Qué quiere decir esa palabra, en el propio sitio donde está: la leyenda del pie existía,
  // pero está lejos de la tabla y el responsable tuvo que preguntar qué era «entrega».
  const catQue = (ev) => {
    const q = T.categorias_que || {};
    if (ev.category !== 'desconocido') return q[ev.category] || '';
    if (ev.local) return q.red_local || '';
    if (ev.entrega) return q.entrega || '';
    const p = pista(ev);
    if (ev.sector && (!p || p === 'web')) return (q.sector || '').replace('{sector}', t('sector_' + ev.sector));
    if (p) return q['pista_' + p] || '';
    if (ev.empresa || ev.ia) return q.de_empresa || '';
    return q.desconocido || '';
  };
  // Tres colores, con su leyenda en la propia página (petición del responsable, 8 oct 2026):
  // verde para lo normal, naranja para lo que conviene mirar, rojo para estar alerta. Es una
  // forma de leer la categoría de un vistazo, no un veredicto nuevo: cada color sale de lo que
  // ya dice la etiqueta (la lista que lo reconoce, la palabra del nombre, el registro de
  // corredores, la señal de evasión), y la leyenda dice exactamente qué entra en cada uno.
  const nivel = (ev) => {
    if (ev.category === 'rastreador' || ev.corredor || (ev.signals || []).includes('evasion_dns')) return 'rojo';
    if (ev.category === 'publicidad' || ev.category === 'telemetria') return 'naranja';
    if (ev.category === 'desconocido' && !ev.local && !ev.entrega) {
      const p = pista(ev);
      if (p === 'anuncios' || p === 'medicion') return 'naranja';
      if (!ev.empresa && !ev.ia && !ev.sector) return 'naranja';
    }
    return 'verde';
  };
  // The registrable domain of the name (bing.com, mercadolibre.com.co): where «its website» is.
  const dominioDe = (qname) => {
    const todas = String(qname || '').toLowerCase().replace(/\.$/, '').split('.');
    return todas.slice(-nLabelsDominio(todas)).join('.');
  };
  // The chip links somewhere useful: «its website» opens that website; every other label opens
  // the page that explains what the word means (the person asked where to click to know more).
  const catEnlace = (ev) => {
    const pw = pista(ev);
    if (ev.category === 'desconocido' && !ev.local && !ev.entrega && (ev.empresa || ev.ia || ev.sector) && (pw === 'web' || (ev.sector && !pw))) {
      return { href: 'https://' + dominioDe(ev.qname), titulo: t('cat_enlace_web').replace('{dominio}', dominioDe(ev.qname)) };
    }
    let ancla = ev.category;
    if (ev.category === 'desconocido') {
      const p = pista(ev);
      ancla = ev.local ? 'red-local' : ev.entrega ? 'entrega' : p ? 'pista-' + p : (ev.empresa || ev.ia) ? 'servicio' : 'sin-clasificar';
    }
    return { href: t('guia_categorias_url') + '#' + ancla, titulo: t('cat_enlace_guia') };
  };
  const cat = (ev) => {
    const q = catQue(ev);
    const n = nivel(ev);
    const e = catEnlace(ev);
    const titulo = (q ? q + ' ' : '') + '(' + t('nivel_' + n) + ') · ' + e.titulo;
    return `<a class="tag ${esc(ev.category)}${catClase(ev)} nivel-${n}" href="${esc(e.href)}" target="_blank" rel="noopener noreferrer" title="${esc(titulo)}">${esc(catTexto(ev))}</a>`;
  };
  const verdict = (v) => `<span class="verdict ${esc(v)}">${esc(T.veredictos[v] || v)}</span>`;
  // The trade goes first: reading «AppsFlyer» tells a person nothing, and reading what
  // AppsFlyer sells tells them everything they need to decide. It is a fact about the
  // company, not about this query, and it is only printed where one is written.
  const phrases = (ev) => (ev.oficio ? `<span class="phrase oficio">· ${esc(ev.oficio)}</span>` : '')
    + declara(ev)
    + (ev.frases || []).map((f) => `<span class="phrase">· ${esc(f)}</span>`).join('')
    + disfrazFrase(ev)
    + servidorFrase(ev);
  // Un nombre disfrazado (decisión 205): se presenta como la web, pero responde un rastreador de
  // otra empresa. La frase viene hecha del servidor; la explicación larga, en el título.
  const disfrazFrase = (ev) => (ev.disfraz ? `<span class="phrase disfraz" title="${esc(ev.disfraz.titulo)}">· <b>${esc(t('disfraz_etiqueta'))}</b> ${esc(ev.disfraz.frase)}</span>` : '');
  // A dónde mandó la respuesta al aparato (decisión 203): por qué red se sirve y en qué país está
  // el servidor según la base que va dentro del programa, o por qué no se dice (anycast, red
  // local). La dirección y la fuente van en el título, para quien quiera comprobarlas.
  const fechaBase = (f) => {
    const [y, m] = String((f && f.fecha) || '').split('-').map(Number);
    return y && m ? new Date(Date.UTC(y, m - 1, 15)).toLocaleDateString(LOC, { month: 'long', year: 'numeric', timeZone: 'UTC' }) : '';
  };
  const textoFuenteRed = (f) => (f.muestra ? t('destinos_muestra') + ' ' : '') + t('destinos_fuente').replace('{fuente}', f.fuente).replace('{fecha}', fechaBase(f)).replace('{licencia}', f.licencia);
  const servidorFrase = (ev) => {
    const s = ev.servidor;
    if (!s) return '';
    const titulo = t('servidor_titulo').replace('{base}', s.base || '?');
    return `<span class="phrase servidor${s.anycast ? ' anycast' : ''}" title="${esc(titulo)}">· ${esc(s.frase)}</span>`;
  };
  // La línea de la fuente, debajo de cada tabla o tarjeta que enseña un país de servidor, con el
  // enlace que pide su licencia; y, si lo que va dentro es la muestra de pruebas, dicho delante.
  function pintaFuenteRed(id, f) {
    if (!f) return;
    const el = $(id);
    if (!el || el.dataset.hecho === f.fecha + f.muestra) return;
    el.dataset.hecho = f.fecha + f.muestra;
    el.classList.remove('hidden');
    el.innerHTML = (f.muestra ? `<strong class="bad">${esc(t('destinos_muestra'))}</strong> ` : '')
      + esc(t('destinos_fuente').replace('{fuente}', f.fuente).replace('{fecha}', fechaBase(f)).replace('{licencia}', f.licencia))
      + ` <a href="${esc(f.web)}" target="_blank" rel="noopener noreferrer">${esc(f.atribucion)}</a>`;
  }
  const pieRed = (f) => (f ? textoFuenteRed(f) + ' ' + f.atribucion + ' (' + f.web + ')' : '');
  // Lo que la empresa marcó ella misma en el registro de corredores de datos (ubicación exacta,
  // menores, a quién vendió): una frase debajo del nombre, solo cuando marcó algo. Es su propia
  // declaración, copiada; no una conclusión de Guardiana (pedido del responsable, 6 oct 2026).
  const declara = (ev) => (ev.corredor && ev.corredor.declara && ev.corredor.declara.length
    ? `<span class="phrase">· ${esc(t('corredor_declara').replace('{lista}', ev.corredor.declara.join(', ')))}</span>` : '');
  // El país va pegado a la empresa, y es el de la EMPRESA, no el del servidor que responde:
  // casi todo lo grande contesta desde un servidor cercano, así que el país de la dirección IP
  // diría «Colombia» de algo cuyos datos acaban en Estados Unidos. Lo que se puede sostener es
  // a quién pertenece el nombre y bajo qué leyes está (pedido del responsable, 21 sep 2026).
  // Columna propia, no etiqueta pegada al nombre: el responsable la quiere «al lado de
  // categoría y dispositivo», que se lee mucho mejor en una tabla (21 sep 2026).
  // El país dice bajo qué leyes está la empresa; la ciudad, debajo y en gris, lo hace concreto:
  // «Estados Unidos» son muchos sitios, «Menlo Park» es uno (pedido del responsable, 21 sep 2026).
  const paisCelda = (ev) => `<td class="pais"${ev.pais ? ` title="${esc(t('pais_titulo'))}"` : ''}>`
    + (ev.pais ? esc(ev.pais) : ev.pais_dominio ? `<span class="muted" title="${esc(t('pais_dominio_titulo').replace('{pais}', ev.pais_dominio))}">${esc(t('pais_dominio_texto').replace('{pais}', ev.pais_dominio))}</span>` : '<span class="muted">—</span>')
    + (ev.ciudad ? `<span class="ciudad">${esc(ev.ciudad)}</span>` : '') + '</td>';
  // El servicio de IA sigue pegado al nombre —es el nombre dicho de otra manera—, pero la
  // EMPRESA pasa a su propia columna, al lado de la categoría y el país: así la tabla dice
  // quién es, qué es y de dónde, cada cosa en su sitio (pedido del responsable, 21 sep 2026).
  const company = (ev) => (ev.ia && !(ev.category === 'desconocido' && !ev.entrega && !ev.empresa) ? ` <span class="tag ia">${esc(ev.ia)}</span>` : '');
  // Al lado de la empresa, a lo que se dedica cuando está inscrita como corredora de datos:
  // «compra y venta de datos», con el nombre con el que se inscribió y su país en el título.
  // El registro es público y la frase es la de la ley; cortarla o no es decisión de la persona
  // (pedido del responsable, 6 oct 2026).
  const corredorTag = (ev) => (ev.corredor
    ? ` <span class="tag corredor" title="${esc(t('corredor_titulo').replace('{nombre}', ev.corredor.nombre).replace('{pais}', ev.corredor.pais ? ' (' + ev.corredor.pais + ')' : ''))}">${esc(t('corredor_etiqueta'))}</span>`
    : '');
  // A rented server on a cloud is whoever rents it: the cell says where it is hosted, not that
  // the cloud is the company (review of 9 Oct 2026).
  const empresaCelda = (ev) => `<td class="empresa-col">${ev.empresa ? empresaLink(ev.empresa) : ev.alojado ? `<span class="muted">${esc(t('alojado_en').replace('{nube}', ev.alojado))}</span>` : '<span class="muted">—</span>'}${corredorTag(ev)}${ev.grupo ? `<span class="grupo" title="${esc(t('grupo_titulo'))}">${esc(t('grupo_de').split('{grupo}')[0])}${empresaLink(ev.grupo)}${esc(t('grupo_de').split('{grupo}').slice(1).join(''))}</span>` : ''}</td>`;
  // Dónde acaba lo de este aparato, sumado: los países de las empresas dueñas de los nombres.
  const paisesDe = (l) => (l && l.paises && l.paises.length
    ? `<br><span class="phrase">${esc(t('lectura_paises'))} ` + l.paises.map((x) => `${esc(x[0])} <b>${x[1]}</b>`).join(' · ') + '</span>'
    : '');
  const hints = (l) => [l && l.callado_min != null ? t('lectura_callado').replace('{min}', l.callado_min) : '', l && l.relay ? t('lectura_relay') : '', l && l.evasiones ? (l.evasiones === 1 ? t('lectura_evasion_una') : t('lectura_evasiones').replace('{n}', l.evasiones)) : ''].filter(Boolean);
  const device = (ev) => esc(deviceName(ev)) + programa(ev);
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

  // Names this person has already cut, so the state survives a redraw. The snapshot table
  // repaints every two seconds: without this, a button turned red by a cut went back to grey
  // as if nothing had happened, and the person would cut the same name twice.
  // Un mapa y no un conjunto: para deshacer un corte desde el mismo botón hace falta el número
  // de la regla que lo hizo (petición del responsable, 21 sep 2026).
  const CORTADOS = new Map();
  // Y las reglas que permiten un nombre concreto: son las que deshace «bloquear» en una fila
  // que se desbloqueó a mano (8 oct 2026).
  const PERMITIDOS = new Map();
  const claveCorte = (dev, nombre) => dev + '|' + nombre;
  const sinPunto = (n) => String(n || '').toLowerCase().replace(/\.$/, '');
  // Every rule id the page can see, and the active ones: a row that was blocked by a rule
  // undone since (the table keeps the query for a minute) is not blocked any more, and
  // offering «unblock» there created a permanent allow (review of 9 Oct 2026).
  const REGLAS_VISTAS = new Set();
  const REGLAS_ACTIVAS = new Set();
  // Rules this device did not make for itself (the household panel set them on it): on the
  // device's own page they are the home's decision, which only the home's panel undoes.
  const REGLAS_DE_LA_CASA = new Set();
  const esDeLaCasa = (x) => x.created_by !== 'usuario (dispositivo)';
  // Names let through the declared scope from this page since it loaded.
  const DEJADOS_PASAR = new Set();
  // When each rule was made and undone (Unix ms), and when a name was let into the scope here:
  // the status says «bloqueado a las…» / «desbloqueado a las…» with the time of that change
  // (the responsible, 9 Oct 2026: the state and its time, nothing more).
  const HORAS_REGLA = new Map();
  const HORA_DEJADO = new Map();
  // The latest time a block of each name was undone (device|name → Unix ms): a row that only
  // passed is «desbloqueado a las…» once its name was blocked and unblocked after it.
  const DESBLOQUEOS = new Map();
  const anotaDesbloqueo = (k, ms) => { if (typeof ms === 'number' && !(DESBLOQUEOS.get(k) >= ms)) DESBLOQUEOS.set(k, ms); };
  async function cargarCortados(path = '/api/reglas') {
    try {
      const r = await api(path);
      CORTADOS.clear(); PERMITIDOS.clear(); REGLAS_VISTAS.clear(); REGLAS_ACTIVAS.clear();
      DESBLOQUEOS.clear(); REGLAS_DE_LA_CASA.clear();
      (r.reglas || []).forEach((x) => {
        REGLAS_VISTAS.add(x.id);
        if (esDeLaCasa(x)) REGLAS_DE_LA_CASA.add(x.id);
        HORAS_REGLA.set(x.id, { creada: x.created_at, deshecha: x.undone_at ?? null });
        if (x.activa) REGLAS_ACTIVAS.add(x.id);
        if (x.match_kind === 'domain' && x.action === 'cortar' && x.undone_at != null) {
          anotaDesbloqueo(claveCorte(x.device_id || 'home', sinPunto(x.pattern)), x.undone_at);
        }
        if (!x.activa || x.match_kind !== 'domain') return;
        const mapa = x.action === 'cortar' ? CORTADOS : x.action === 'permitir' ? PERMITIDOS : null;
        if (mapa) mapa.set(claveCorte(x.device_id || 'home', sinPunto(x.pattern)), x.id);
      });
    } catch (_) {}
  }
  function reglaDe(mapa, ev) {
    const propia = mapa.get(claveCorte(ev.device_id, sinPunto(ev.qname)));
    return propia === undefined ? mapa.get(claveCorte('home', sinPunto(ev.qname))) : propia;
  }
  const reglaDeCorte = (ev) => reglaDe(CORTADOS, ev);
  const reglaDePermiso = (ev) => reglaDe(PERMITIDOS, ev);
  // "1 horas de 24" no lo escribe nadie: una hora es singular.
  function mirando(h) {
    return t(h === 1 ? 'observando_una' : 'observando').replace('{h}', h);
  }
  // Cada fila tiene su botón, siempre (petición del responsable, 8 oct 2026: una fila decía
  // «bloqueado» y no había forma de desbloquearla). Qué hace depende de por qué está así:
  //  · bloqueada por una regla de este nombre → «desbloquear» deshace esa regla;
  //  · bloqueada por el Modo Vigilante (fuera del alcance) → «desbloquear» mete el nombre en
  //    el alcance de ese aparato;
  //  · bloqueada por una regla ancha (categoría, sufijo, toda la casa) → «desbloquear» crea una
  //    regla que permite este nombre en este aparato, que gana a la ancha por ser más concreta;
  //  · no bloqueada → «bloquear» crea la regla de este nombre (y deshace antes el permiso a
  //    mano, si lo había).
  // Bloquear un nombre no espera a las 24 horas (decisión del responsable, 20 sep 2026): el
  // servidor pide confirmación mientras lleve menos de un día mirando. Lo ancho sigue esperando.
  // A blocked query whose block was lifted since it was written down: an allow of this name, the
  // name let into the scope from this page, or the rule that blocked it undone.
  function levantado(ev) {
    if (ev.verdict !== 'cortado' || reglaDeCorte(ev) !== undefined) return false;
    if (reglaDePermiso(ev) !== undefined || DEJADOS_PASAR.has(claveCorte(ev.device_id, sinPunto(ev.qname)))) return true;
    return ev.rule_id != null && REGLAS_VISTAS.has(ev.rule_id) && !REGLAS_ACTIVAS.has(ev.rule_id);
  }
  // The status beside the button says what holds NOW, so the two never contradict each other:
  // a row read «bloqueado» next to a red «Bloquear» because the name had been unblocked after
  // that query (the responsible, 9 Oct 2026: «no se puede bloquear una cosa que ya está
  // bloqueada»). What happened at the time of the query is still on the row, in small letters
  // under the status, and the export keeps it exactly as it was.
  // When the block of this row was lifted: the allow made for it, the scope it was let into
  // from this page, or the undoing of the rule that blocked it — the latest of those.
  function horaLevantado(ev) {
    const k = claveCorte(ev.device_id, sinPunto(ev.qname));
    const momentos = [HORA_DEJADO.get(k)];
    const permiso = reglaDePermiso(ev);
    if (permiso !== undefined) momentos.push(HORAS_REGLA.get(permiso)?.creada);
    if (ev.rule_id != null) momentos.push(HORAS_REGLA.get(ev.rule_id)?.deshecha);
    const hay = momentos.filter((m) => typeof m === 'number');
    return hay.length ? Math.max(...hay) : null;
  }
  // «a las 17:25:51» today; another day says which: «a las 9/10 17:25:51» read as a time
  // (review of 10 Oct 2026, panel 22).
  const aLas = (ms) => {
    if (ms == null) return '';
    const d = new Date(ms);
    const texto = esHoy(ms)
      ? t('estado_a_las').replace('{hora}', d.toLocaleTimeString(LOC))
      : t('estado_el').replace('{fecha}', d.toLocaleDateString(LOC, DIA)).replace('{hora}', d.toLocaleTimeString(LOC));
    // On the status itself, as its tooltip: a second line made the row taller after every
    // click and pushed every row below it down (the owner, 10 Oct 2026: «no se puede mover»).
    return texto;
  };
  function estadoCelda(ev) {
    if (levantado(ev)) {
      return `<span class="verdict levantado" title="${esc(aLas(horaLevantado(ev)))}">${esc(t('estado_desbloqueado'))}</span>`;
    }
    const corte = reglaDeCorte(ev);
    if (corte !== undefined) {
      // Blocked by a rule of this very name: blocked since that rule was made.
      return `<span title="${esc(aLas(HORAS_REGLA.get(corte)?.creada ?? null))}">${verdict('cortado')}</span>`;
    }
    // Blocked and unblocked after this query: unblocked, and when.
    const n = sinPunto(ev.qname);
    const des = Math.max(DESBLOQUEOS.get(claveCorte(ev.device_id, n)) ?? -1, DESBLOQUEOS.get(claveCorte('home', n)) ?? -1);
    if (ev.verdict !== 'cortado' && des >= Number(ev.ts)) {
      return `<span class="verdict levantado" title="${esc(aLas(des))}">${esc(t('estado_desbloqueado'))}</span>`;
    }
    return verdict(ev.verdict);
  }
  // Every row carries what its status and its button are drawn from, so a click can redraw both
  // (and those of every other row of the same name) at once. Before, the click changed only the
  // button: a row the person had just unblocked kept reading «bloqueado» beside a red
  // «Bloquear» until the next reload (the responsible, 9 Oct 2026, twice).
  function filaAttrs(ev) {
    return `data-dev="${esc(ev.device_id)}" data-q="${esc(ev.qname)}" data-v="${esc(ev.verdict)}" data-r="${ev.rule_id == null ? '' : esc(ev.rule_id)}" data-by="${esc(ev.decided_by || '')}" data-ts="${esc(ev.ts)}" data-k="${esc([ev.device_id, ev.qname, ev.verdict, ev._t0 ?? ev.ts].join('|'))}"`;
  }
  function evDeFila(tr) {
    const d = tr.dataset;
    return { device_id: d.dev, qname: d.q, verdict: d.v, rule_id: d.r === '' ? null : Number(d.r), decided_by: d.by, ts: Number(d.ts) };
  }
  const celdasEstado = (ev) => `<td class="estado">${estadoCelda(ev)}</td><td class="accion">${cutCell(ev)}</td>`;
  function redibujaNombre(nombre) {
    const n = sinPunto(nombre);
    document.querySelectorAll('tr[data-q]').forEach((tr) => {
      if (sinPunto(tr.dataset.q) !== n) return;
      const ev = evDeFila(tr);
      const est = tr.querySelector('td.estado'), acc = tr.querySelector('td.accion');
      if (est) est.innerHTML = estadoCelda(ev);
      if (acc) acc.innerHTML = cutCell(ev);
    });
  }
  function cutCell(ev) {
    const d = `data-device="${esc(ev.device_id)}" data-name="${esc(ev.qname)}"`;
    const enSuPagina = document.body.getAttribute('data-page') === 'miDispositivo';
    const corte = reglaDeCorte(ev);
    if (corte !== undefined) {
      // A block the household panel set on this device: its own page cannot lift it (the
      // server refuses it too), so the row says where it is changed instead of a button.
      if (enSuPagina && REGLAS_DE_LA_CASA.has(corte)) return `<span class="muted">${esc(t('mi_bloqueo_de_la_casa'))}</span>`;
      return `<button class="cut cortado" data-deshacer="${corte}" ${d}>${esc(t('desbloquear_boton'))}</button>`;
    }
    // Blocking a name undoes the hand-made allow first, but only one this page may undo.
    const permiso = reglaDePermiso(ev);
    const permisoPropio = permiso !== undefined && !(enSuPagina && REGLAS_DE_LA_CASA.has(permiso));
    const bloquear = `<button class="secondary cut" ${permisoPropio ? `data-permiso="${permiso}" ` : ''}${d}>${esc(t('cortar'))}</button>`;
    if (ev.verdict === 'cortado') {
      // Lifted since: the status cell says «desbloqueado» (estadoCelda), and the button offers
      // what can be done now.
      if (levantado(ev)) return bloquear;
      if (ev.decided_by === 'alcance_declarado') {
        // The scope belongs to the home's panel: from the phone's own page there is no key for
        // it, so the row says where to change it instead of a button that would fail.
        if (enSuPagina) return `<span class="muted">${esc(t('mi_fuera_alcance'))}</span>`;
        return `<button class="cut cortado" data-alcance="1" ${d}>${esc(t('desbloquear_boton'))}</button>`;
      }
      // Blocked by a rule this page does not hold: on the phone's own page that is the home's
      // rule, which only the home's panel lifts (the server refuses it too).
      if (enSuPagina) return `<span class="muted">${esc(t('mi_bloqueo_de_la_casa'))}</span>`;
      return `<button class="cut cortado" data-permitir="1" ${d}>${esc(t('desbloquear_boton'))}</button>`;
    }
    return bloquear;
  }
  // Una sola ventana al cortar, y la pregunta la escribe el servidor, que es quien sabe si este
  // aparato lleva más o menos de un día mirado. Antes preguntaba el panel y, encima, el servidor
  // volvía a preguntar: dos ventanas seguidas para un botón que se deshace con el mismo clic.
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
      const resumen = tn('rafagas_resumen', r.nombres.length).replace('{seg}', (r.duracion_ms / 1000).toFixed(1));
      const quien = esc(nombreAparato(r.device_id, r.device_name));
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
      const quien = esc(nombreAparato(r.device_id, r.device_name));
      const lineas = [];
      lineas.push(esc(tn('recibo_nombres', r.nombres)));
      const emp = r.empresas_con_oficio
        ? tn('recibo_empresas', r.empresas).replace('{m}', r.empresas_con_oficio)
        : tn('recibo_empresas_sin', r.empresas);
      lineas.push(esc(emp) + (r.oficios.length ? `<br><span class="muted">${esc(r.oficios.join(' · '))}</span>` : ''));
      if (r.latidos.length) {
        const lista = r.latidos.slice(0, 3).map(([n, m]) => t('recibo_latido').replace('{nombre}', n).replace('{min}', m)).join(' · ');
        lineas.push(esc(t('recibo_latidos').replace('{lista}', lista)));
      }
      if (r.evasiones) lineas.push(esc(tn('recibo_evasiones', r.evasiones)));
      if (r.cortadas) lineas.push(esc(tn('recibo_cortadas', r.cortadas)));
      if (r.rafaga) {
        lineas.push(esc(tn('recibo_rafaga', r.rafaga.nombres.length).replace('{seg}', (r.rafaga.duracion_ms / 1000).toFixed(1)))
          + (r.rafaga.empresas.length ? `<br><span class="muted">${esc(r.rafaga.empresas.slice(0, 6).join(' · '))}</span>` : ''));
      }
      return `<section class="card disp"><div class="disp-cab"><h3>${quien}</h3>`
        + `<span class="muted small">${esc(t('recibo_desde').replace('{fecha}', when(r.desde)))}</span></div>`
        + `<ul class="recibo">${lineas.map((l) => `<li>${l}</li>`).join('')}</ul></section>`;
    }).join('');
  }

  function bindCutButtons(container, path) {
    container.addEventListener('click', async (e) => {
      const b = e.target.closest('button.cut');
      if (!b) return;
      const name = b.dataset.name, dev = b.dataset.device;
      const k = claveCorte(dev, sinPunto(name)), kCasa = claveCorte('home', sinPunto(name));
      const deshecha = (id) => {
        REGLAS_ACTIVAS.delete(Number(id));
        const h = HORAS_REGLA.get(Number(id));
        HORAS_REGLA.set(Number(id), { creada: h ? h.creada : null, deshecha: Date.now() });
      };
      const creada = (id) => {
        REGLAS_VISTAS.add(Number(id)); REGLAS_ACTIVAS.add(Number(id));
        HORAS_REGLA.set(Number(id), { creada: Date.now(), deshecha: null });
      };
      b.disabled = true;
      try {
        if (b.dataset.deshacer) {
          // Desbloquear: se deshace la regla de este nombre.
          await api(`${path}/${b.dataset.deshacer}/deshacer`, { method: 'POST' });
          deshecha(b.dataset.deshacer);
          anotaDesbloqueo(CORTADOS.get(k) === Number(b.dataset.deshacer) ? k : kCasa, Date.now());
          if (CORTADOS.get(k) === Number(b.dataset.deshacer)) CORTADOS.delete(k); else CORTADOS.delete(kCasa);
        } else if (b.dataset.alcance) {
          // Fuera del alcance declarado: el nombre entra en el alcance de ese aparato.
          await api('/api/ia/alcance/anadir', { method: 'POST', body: { device_id: dev, nombre: name } });
          DEJADOS_PASAR.add(k);
          HORA_DEJADO.set(k, Date.now());
        } else if (b.dataset.permitir) {
          // The map is from when the page loaded: a rule of this name made since then (another
          // tab, the Rules page) is undone, not covered with an allow (second pass, 8 Oct 2026).
          await cargarCortados(path);
          const corte = CORTADOS.get(k) ?? CORTADOS.get(kCasa);
          if (corte !== undefined) {
            await api(`${path}/${corte}/deshacer`, { method: 'POST' });
            deshecha(corte);
            anotaDesbloqueo(CORTADOS.get(k) === corte ? k : kCasa, Date.now());
            CORTADOS.delete(k); CORTADOS.delete(kCasa);
          } else {
            // Bloqueado por una regla ancha: una regla concreta que lo permite gana a la ancha.
            const nueva = await createRule({ scope: 'device', device_id: dev, match_kind: 'domain', pattern: name, action: 'permitir' }, path);
            if (nueva) { PERMITIDOS.set(k, nueva.id); creada(nueva.id); }
          }
        } else {
          // Sin confirm aquí: lo pide el servidor con la frase que corresponda (una sola ventana).
          const nueva = await createRule({ scope: 'device', device_id: dev, match_kind: 'domain', pattern: name, action: 'cortar' }, path);
          // The allow made by hand goes only once the block exists: undone first, a cancelled
          // confirmation left the wide rule blocking the name again (review of 9 Oct 2026).
          if (nueva && b.dataset.permiso) {
            await api(`${path}/${b.dataset.permiso}/deshacer`, { method: 'POST' });
            deshecha(b.dataset.permiso);
            PERMITIDOS.delete(k);
          }
          if (nueva) { CORTADOS.set(k, nueva.id); creada(nueva.id); DEJADOS_PASAR.delete(k); }
        }
      } catch (err) {
        alert(String(err.message || err));
      }
      // The button turns into what can be done now and the status beside it says what holds
      // now, on this row and on every other row of the same name, in every table of the page.
      b.disabled = false;
      // The button is drawn anew: the focus stays on this row's button, not on the page.
      const fila = b.closest('tr');
      redibujaNombre(name);
      const nuevo = fila && fila.querySelector('td.accion button');
      if (nuevo) nuevo.focus({ preventScroll: true });
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
        if (!last.servidor && ev.servidor) last.servidor = ev.servidor;
        if (!last.disfraz && ev.disfraz) last.disfraz = ev.disfraz;
        last.ts = Math.max(last.ts, ev.ts);
        continue;
      }
      out.push(Object.assign({}, ev, { _n: 1, _t0: ev.ts, _types: ev.qtype ? [ev.qtype] : [] }));
    }
    return out;
  }
  // A table that refreshes by itself (the 60-second snapshot every two seconds, the phone's own
  // page every five) never moves under the person (the responsible, 9 Oct 2026: «la página no se
  // puede mover sola, la tiene que mover ella»):
  //  · while the pointer is over it, it was touched in the last few seconds or a control in it has
  //    the keyboard focus, new rows wait and a small notice at the bottom of the window says how
  //    many there are; pressing it, or moving the pointer away, brings them in;
  //  · when they come in and the person has scrolled into the table, the row at the top of the
  //    window stays exactly where it was: the new rows go in above it, out of sight.
  function tablaQuieta(tbody) {
    const aviso = document.createElement('button');
    aviso.type = 'button';
    aviso.className = 'nuevas hidden';
    document.body.appendChild(aviso);
    // Once the person touches the page (a click, a tap, the wheel, a key), the table stops
    // moving for good: new rows wait behind the button, and the person scrolls by themselves
    // (the owner, 10 Oct 2026: «cuando yo toco, ya deja de moverse… en todo momento»). Until
    // then it is a live view that fills itself. Reloading the page makes it live again.
    let quieta = false, mostrada = null, pendiente = null;
    const claves = (html) => [...String(html).matchAll(/data-k="([^"]*)"/g)].map((m) => m[1]);
    const ocupada = () => quieta;
    const para = (e) => { if (!aviso.contains(e.target)) quieta = true; };
    for (const ev of ['pointerdown', 'wheel', 'touchstart', 'keydown']) window.addEventListener(ev, para, { capture: true, passive: true });
    function aplica(html) {
      const antes = tbody.getBoundingClientRect();
      let ancla = null;
      if (antes.top < 0 && antes.bottom > 0) {
        for (const tr of tbody.querySelectorAll('tr[data-k]')) {
          const r = tr.getBoundingClientRect();
          if (r.bottom > 0) { ancla = { k: tr.dataset.k, top: r.top }; break; }
        }
      }
      tbody.innerHTML = html;
      mostrada = html; pendiente = null;
      aviso.classList.add('hidden');
      if (antes.bottom <= 0) {
        // Reading something below the table: it stays where it was however the table grew.
        window.scrollBy(0, tbody.getBoundingClientRect().bottom - antes.bottom);
      } else if (ancla) {
        const tr = [...tbody.querySelectorAll('tr[data-k]')].find((x) => x.dataset.k === ancla.k);
        if (tr) window.scrollBy(0, tr.getBoundingClientRect().top - ancla.top);
      }
    }
    // The browser's own scroll anchoring would correct the same jump a second time.
    document.documentElement.style.overflowAnchor = 'none';
    // The button brings the new rows in once; the table stays still afterwards.
    aviso.addEventListener('click', () => { if (pendiente !== null) aplica(pendiente); });
    return {
      get quieta() { return quieta; },
      pinta(html) {
        if (html === mostrada) { pendiente = null; aviso.classList.add('hidden'); return; }
        if (mostrada === null || !ocupada()) { aplica(html); return; }
        pendiente = html;
        const ya = new Set(claves(mostrada));
        const n = claves(html).filter((k) => !ya.has(k)).length;
        aviso.textContent = n ? tn('tabla_nuevas', n) : t('tabla_cambios');
        aviso.classList.remove('hidden');
      },
    };
  }
  function eventRows(events) {
    return foldEvents(events).map((ev) => {
      const veces = ev._n > 1 ? ` <span class="muted">×${ev._n}</span>` : '';
      const como = ev._types.length ? ` title="${esc(ev._types.join(', '))}"` : '';
      return `<tr ${filaAttrs(ev)}><td class="mono">${clock(ev.ts)}</td><td><span class="mono"${como}>${esc(ev.qname)}</span>${veces}${trampa(ev)}${company(ev)}${phrases(ev)}</td>${empresaCelda(ev)}<td>${cat(ev)}</td>${paisCelda(ev)}<td>${device(ev)}</td>${celdasEstado(ev)}</tr>`;
    }).join('');
  }

  // «Protección máxima» (the owner, 10 Oct 2026): one button for the whole home, off until the
  // person turns it on, and «Desactivar protección máxima» undoes exactly what it set (api::cambia_maxima).
  // The same card on the X-ray and on Rules; `alCambiar` reloads the page's own list.
  async function tarjetaMaxima(alCambiar) {
    if (!$('maxima')) return;
    const pinta = (m) => {
      $('maxima').classList.remove('hidden');
      $('maxima-activar').classList.toggle('hidden', m.activa);
      $('maxima-activa').classList.toggle('hidden', !m.activa);
      $('maxima-quitar').classList.toggle('hidden', !m.activa);
      $('maxima-msg').className = 'muted';
      $('maxima-msg').textContent = m.activa && m.esperando ? t('maxima_espera') : '';
    };
    const cambia = async (activa, b) => {
      b.disabled = true;
      try {
        pinta(await api('/api/proteccion-maxima', { method: 'POST', body: { activa } }));
        // The button pressed is gone: the focus goes to the one that undoes it.
        $(activa ? 'maxima-quitar' : 'maxima-activar').focus();
        if (alCambiar) await alCambiar();
      } catch (err) { $('maxima-msg').className = 'bad'; $('maxima-msg').textContent = String(err.message || err); } finally { b.disabled = false; }
    };
    $('maxima-activar').addEventListener('click', (e) => cambia(true, e.currentTarget));
    $('maxima-quitar').addEventListener('click', (e) => cambia(false, e.currentTarget));
    try { pinta(await api('/api/proteccion-maxima')); } catch (_) {}
  }
  const PAGE_TITLE = { radiografia: 'nav_radiografia', ia: 'nav_ia', antivirus: 'nav_antivirus', extracto: 'nav_extracto', dispositivos: 'nav_dispositivos', sabeDeTi: 'nav_sabe', estado: 'nav_estado', hogar: 'nav_hogar', miDispositivo: 'nav_mi', informe: 'nav_informe', reglas: 'nav_reglas', verify: 'nav_verify', licencia: 'nav_licencia', comprobador: 'comprobador_titulo', borrado: 'nav_borrado', empresa: 'ficha_titulo', expedientes: 'nav_expedientes' };
  function applyTexts() {
    document.querySelectorAll('[data-t]').forEach((el) => { el.textContent = t(el.getAttribute('data-t')); });
    document.querySelectorAll('[data-t-html]').forEach((el) => { el.innerHTML = t(el.getAttribute('data-t-html')); });
    document.querySelectorAll('[data-t-title]').forEach((el) => { el.title = t(el.getAttribute('data-t-title')); });
    document.querySelectorAll('[data-t-placeholder]').forEach((el) => { el.placeholder = t(el.getAttribute('data-t-placeholder')); });
    // A name for what a screen reader cannot read by itself: the share card, the QR code.
    document.querySelectorAll('[data-t-label]').forEach((el) => { el.setAttribute('aria-label', t(el.getAttribute('data-t-label'))); });
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
    if (PAGE_TITLE[page]) document.title = 'GUARDIANA · ' + t(PAGE_TITLE[page]);
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
      // A moon by day (to go to night) and a sun by night: the icon says where it takes you,
      // the name stays in the tooltip and for screen readers.
      const ICONO_TEMA = {
        noche: '<svg viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><path d="M20.5 14.2A8.5 8.5 0 0 1 9.8 3.5a8.5 8.5 0 1 0 10.7 10.7z"/></svg>',
        dia: '<svg viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><circle cx="12" cy="12" r="4.2"/><path d="M12 2.5v2.2M12 19.3v2.2M2.5 12h2.2M19.3 12h2.2M5.3 5.3l1.6 1.6M17.1 17.1l1.6 1.6M5.3 18.7l1.6-1.6M17.1 6.9l1.6-1.6"/></svg>',
      };
      n.innerHTML = ICONO_TEMA[OTRO];
      n.title = t(OTRO === 'noche' ? 'tema_noche' : 'tema_dia');
      n.setAttribute('aria-label', n.title);
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
  // The card people share. Until 1.0.2 its last row sat on top of the date, it used the system
  // font and colours outside the palette (review of 28 Sep 2026, G2): now the panel's own fonts,
  // loaded before drawing (a canvas draws with whatever is loaded at that moment), and its colour
  // variables, so day and night look like the panel.
  async function drawCard(canvas, r) {
    try {
      await Promise.all([
        document.fonts.load("600 88px Unbounded"),
        document.fonts.load("400 40px 'IBM Plex Sans'"),
        document.fonts.load("500 30px 'IBM Plex Mono'"),
      ]);
    } catch (_) { /* the fallbacks below still draw */ }
    const css = getComputedStyle(document.documentElement);
    const v = (name) => css.getPropertyValue(name).trim();
    const ctx = canvas.getContext('2d');
    const W = canvas.width, H = canvas.height;
    const DISPLAY = "Unbounded, 'IBM Plex Sans', sans-serif";
    const TEXTO = "'IBM Plex Sans', system-ui, sans-serif";
    ctx.fillStyle = v('--bg');
    ctx.fillRect(0, 0, W, H);
    ctx.fillStyle = v('--fg');
    ctx.font = `600 60px ${DISPLAY}`;
    ctx.fillText('GUARDIANA', 80, 140);
    ctx.fillStyle = v('--muted');
    ctx.font = `400 40px ${TEXTO}`;
    ctx.fillText(t('tarjeta_titulo'), 80, 205);
    const rows = [
      [r.servicios, 'c_servicios', '--fg'],
      [r.rastreadores, 'c_rastreadores', '--rastreador'],
      [r.publicidad, 'c_publicidad', '--publicidad'],
      [r.destinos_nuevos, 'c_nuevos', '--fg'],
      [r.esperados, 'c_esperados', '--esperado'],
      [r.cortados, 'c_cortados', '--cortado'],
    ];
    // Six rows of 108 px from 330: the last baseline is 870, well above the date (H − 120).
    let y = 330;
    for (const [n, key, color] of rows) {
      const num = Number(n || 0).toLocaleString(LOC);
      ctx.fillStyle = v(color);
      ctx.font = `600 84px ${DISPLAY}`;
      ctx.fillText(num, 80, y);
      const x = Math.max(380, 80 + ctx.measureText(num).width + 36);
      ctx.fillStyle = v('--muted');
      ctx.font = `400 40px ${TEXTO}`;
      ctx.fillText(t(key), x, y - 10);
      y += 108;
    }
    ctx.fillStyle = v('--muted');
    ctx.font = `500 30px 'IBM Plex Mono', monospace`;
    ctx.fillText(new Date(r.hasta).toLocaleString(LOC, DIA_HORA), 80, H - 120);
    ctx.font = `400 32px ${TEXTO}`;
    ctx.fillText(t('tarjeta_pie'), 80, H - 64);
  }
  const waLink = (text) => 'https://wa.me/?text=' + encodeURIComponent(text);
  // The person's own mail program, with the subject and the text written: like WhatsApp, Guardiana
  // sends nothing; whoever presses "send" there is the person (6 Oct 2026, the owner asked for it).
  const mailLink = (text) => 'mailto:?subject=' + encodeURIComponent(t('informe_correo_asunto')) + '&body=' + encodeURIComponent(text);

  // ----- pages -------------------------------------------------------------
  const pages = {
    async informe() {
      const r = await api('/api/informe');
      $('i-periodo').textContent = t('informe_periodo').replace('{desde}', new Date(r.desde).toLocaleDateString(LOC, DIA)).replace('{hasta}', new Date(r.hasta).toLocaleDateString(LOC, DIA));
      // Free plan: the table is a marked example and the Plus card shows, with "Ahora no" (decision 53).
      $('i-whatsapp').classList.toggle('hidden', !r.plus);
      const row = (label, f) => `<td>${label}</td><td>${num(f.consultas)}</td><td>${num(f.rastreadores)}</td><td>${num(f.publicidad)}</td><td>${num(f.telemetria)}</td><td>${num(f.esperados)}</td><td>${num(f.desconocidos)}</td><td>${num(f.cortados)}</td>`;
      // Stood aside, there is no week to report: a table of zeros would be figures nobody counted.
      $('i-rows').innerHTML = r.dispositivos.map((d) => `<tr>${row(esc(nombreAparato(d.id, d.name)), d.fila)}</tr>`).join('') || `<tr><td colspan="8" class="muted">${esc(t(r.plus ? 'informe_sin_datos' : 'informe_apartado'))}</td></tr>`;
      $('i-total').innerHTML = r.plus ? row(`<strong>${esc(t('total'))}</strong>`, r.total) : '';
      $('i-texto').textContent = r.texto_whatsapp;
      $('i-wa').href = waLink(r.texto_whatsapp);
      $('i-correo').href = mailLink(r.texto_whatsapp);
      // Lo que solo Plus puede contestar, porque hace falta memoria: qué cambió respecto a la
      // semana pasada y qué destinos son nuevos. Con el plan gratis se dice por qué no está,
      // en vez de enseñar una sección vacía que se leería como «no ha cambiado nada».
      const comparacion = (f) => {
        const campos = [['col_consultas', 'consultas'], ['c_rastreadores', 'rastreadores'], ['col_publicidad', 'publicidad'], ['col_telemetria', 'telemetria'], ['col_desconocidos', 'desconocidos'], ['col_cortados', 'cortados']];
        return campos.map(([clave, campo]) => {
          const d = r.total[campo] - f[campo];
          const frase = d === 0 ? t('cambio_igual') : t(d > 0 ? 'cambio_mas' : 'cambio_menos').replace('{n}', Math.abs(d));
          return `<li>${esc(t(clave))}: <strong>${num(r.total[campo])}</strong> · ${esc(frase)}</li>`;
        }).join('');
      };
      $('i-cambio').classList.toggle('hidden', !r.anterior);
      if (r.anterior) $('i-cambio-lista').innerHTML = comparacion(r.anterior);
      // Until the week before this one was watched whole there is nothing to compare with, and
      // every destination would read as "new": the page says from when it will be there.
      $('i-novedades').classList.toggle('hidden', !r.plus);
      if (r.plus && r.comparar_desde) {
        $('i-cambio').classList.remove('hidden');
        $('i-cambio-lista').innerHTML = `<li class="muted">${esc(t('informe_comparar_desde').replace('{fecha}', new Date(r.comparar_desde).toLocaleDateString(LOC, DIA)))}</li>`;
        $('i-novedades').classList.add('hidden');
      }
      // Las empresas inscritas como corredoras de datos con las que habló la casa: nombre, país,
      // cuántas veces, lo que declaran y dónde pedir que borren tus datos (decisión 194).
      const derechos = (c) => (c.derechos ? `<a href="${esc(c.derechos)}" target="_blank" rel="noopener noreferrer">${esc(t('corredor_derechos'))}</a>` : '<span class="muted">—</span>');
      $('i-corredores').classList.toggle('hidden', !r.plus);
      if (r.plus) {
        $('i-corredores-total').textContent = r.corredores.length ? tn('informe_corredores_texto', r.corredores.length).replace('{consultas}', tn('informe_n_consultas', r.corredores_consultas)) : '';
        $('i-corredores-rows').innerHTML = r.corredores.map((c) => `<tr><td>${esc(c.nombre)}</td><td>${c.pais ? esc(c.pais) : '<span class="muted">—</span>'}</td><td>${num(c.consultas)}</td><td>${num(c.nombres)}</td><td>${c.declara.length ? esc(c.declara.join(', ')) : '<span class="muted">—</span>'}</td><td>${derechos(c)}</td></tr>`).join('')
          || `<tr><td colspan="6" class="muted">${esc(t('informe_corredores_ninguna'))}</td></tr>`;
      }
      // A dónde viajan las consultas de la semana (decisión 203): el país del servidor según la
      // base que va dentro del programa, y los grupos dueños de los nombres. Las cifras vienen
      // hechas del servidor; aquí solo se dibujan. Cada país lleva al Extracto filtrado por él.
      const d = r.destinos;
      $('i-destinos').classList.toggle('hidden', !d);
      const barra = (nombre, pct, consultas, enlace) => `<li>${enlace ? `<a href="${esc(enlace)}">${esc(nombre)}</a>` : `<span>${esc(nombre)}</span>`}<span class="barra" aria-hidden="true"><span data-pct="${Math.max(0, Math.min(100, Number(pct) || 0))}"></span></span><span class="cifra"><b>${pct} %</b> <span class="muted">${esc(tn('informe_n_consultas', consultas))}</span></span></li>`;
      const grupoDicho = (g) => g.nombre + (g.empresas.length ? ' (' + g.empresas.join(', ') + ')' : '');
      if (d) {
        pintaFuenteRed('fuente-red', d.fuente);
        $('i-paises-frase').textContent = d.con_direccion ? tn('destinos_paises_frase', d.n_paises) : t('destinos_sin');
        $('i-paises-lead').textContent = t('destinos_paises_lead').replace('{total}', num(d.consultas)).replace('{con}', num(d.con_direccion)) + ' ' + t('destinos_paises_pulsa');
        $('i-paises-lead').classList.toggle('hidden', !d.con_direccion);
        $('i-paises').innerHTML = d.paises.map((p) => barra(p.nombre, p.pct, p.consultas, '/extracto?pais=' + encodeURIComponent(p.clave))).join('');
        $('i-grupos-frase').textContent = d.top_n ? tn('destinos_grupos_frase', d.top_n).replace('{pct}', d.top_pct).replace('{total}', num(d.consultas)).replace('{grupos}', d.grupos.slice(0, d.top_n).map(grupoDicho).join(', ')) : '';
        $('i-grupos').innerHTML = d.grupos.slice(0, 12).map((g) => barra(grupoDicho(g), g.pct, g.consultas)).join('');
        // The panel's CSP refuses style attributes; the width is set through the DOM instead.
        for (const b of document.querySelectorAll('ul.barras .barra > span[data-pct]')) b.style.width = b.dataset.pct + '%';
      }
      // Las medidas de la decisión 205, hechas en el servidor con las filas que la casa puede
      // leer: las noches, las empresas de cada día, cada cuánto llaman y los nombres disfrazados.
      // La página no se refresca sola: nada se mueve mientras se lee.
      const m = r.medidas;
      for (const id of ['i-noche', 'i-empresas', 'i-cadencia', 'i-disfraz']) $(id).classList.toggle('hidden', !m);
      const hhmm = (min) => String(Math.floor(min / 60)).padStart(2, '0') + ':' + String(min % 60).padStart(2, '0');
      // Each day is named by its weekday, read at noon so the browser's own zone cannot slip it.
      const diaCorto = (ms) => new Date(ms + 12 * 3600 * 1000).toLocaleDateString(LOC, { weekday: 'short', day: 'numeric' });
      const nada = '<span class="muted">—</span>';
      const mananas = (n) => n.mananas.map((x, i) => (x === null ? '' : diaCorto(m.dias[i]) + ' ' + hhmm(x))).filter(Boolean);
      const empresaO = (e) => (e ? empresaLink(e) : nada);
      if (m) {
        $('i-noche-frase').textContent = m.noche_total ? tn('noche_frase', m.noche_total) : '';
        $('i-noche-rows').innerHTML = m.noche.map((n) => `<tr><td>${esc(nombreAparato(n.id, n.name))}</td><td>${num(n.consultas)}${n.cortadas ? ` <span class="muted">· ${esc(tn('noche_cortadas', n.cortadas))}</span>` : ''}</td><td>${num(n.noches)}</td><td>${n.empresas.length ? esc(n.empresas.join(', ')) : nada}</td><td class="horas">${mananas(n).map(esc).join('<br>') || nada}</td></tr>`).join('')
          || `<tr><td colspan="5" class="muted">${esc(t('noche_ninguna'))}</td></tr>`;
        $('i-empresas-cab').innerHTML = `<th>${esc(t('col_dispositivo'))}</th>` + m.dias.map((d) => `<th>${esc(diaCorto(d))}</th>`).join('');
        $('i-empresas-hoy').textContent = m.empresas_hoy.length ? tn('empresas_hoy', m.empresas_hoy.length).replace('{grupos}', m.empresas_hoy.join(', ')) : '';
        $('i-empresas-rows').innerHTML = m.empresas.map((e) => `<tr><td>${esc(nombreAparato(e.id, e.name))}</td>${e.por_dia.map((x, i) => `<td${i === e.por_dia.length - 1 && e.hoy.length ? ` title="${esc(e.hoy.join(', '))}"` : ''}>${x ? num(x) : nada}</td>`).join('')}</tr>`).join('')
          || `<tr><td colspan="${m.dias.length + 1}" class="muted">${esc(t('empresas_ninguna'))}</td></tr>`;
        $('i-cadencia-rows').innerHTML = m.cadencias.map((c) => `<tr><td class="mono">${esc(c.nombre)}</td><td>${empresaO(c.empresa)}</td><td>${num(c.contactos)}</td><td>${esc(c.frase)}</td></tr>`).join('')
          || `<tr><td colspan="4" class="muted">${esc(t('cadencia_ninguna'))}</td></tr>`;
        $('i-disfraz-total').textContent = m.disfrazados.length ? tn('disfraz_total', m.disfrazados.length).replace('{consultas}', tn('informe_n_consultas', m.disfrazados_consultas)) : '';
        $('i-disfraz-rows').innerHTML = m.disfrazados.map((x) => `<tr><td><span class="mono">${esc(x.nombre)}</span><span class="phrase disfraz">· ${esc(x.frase)}</span></td><td>${empresaO(x.empresa)}</td><td>${num(x.consultas)}</td><td>${num(x.aparatos)}</td></tr>`).join('')
          || `<tr><td colspan="4" class="muted">${esc(t('disfraz_ninguno'))}</td></tr>`;
      }
      if (r.plus && !r.comparar_desde) {
        $('i-nuevos-rows').innerHTML = r.novedades.map((n2) => `<tr><td class="mono">${esc(n2.nombre)}</td><td>${empresaLink(n2.empresa)}</td><td>${esc(catName(n2.categoria))}</td><td>${esc(nombreAparato(n2.device_id, n2.device_name))}</td><td>${num(n2.consultas)}</td></tr>`).join('') || `<tr><td colspan="5" class="muted">${esc(t('informe_nuevos_ninguno'))}</td></tr>`;
      }
      savePdf('i-pdf', () => {
        const doc = pdfDocument('GUARDIANA · ' + t('nav_informe'));
        pdfHead(doc, t('nav_informe'));
        doc.line($('i-periodo').textContent, 10);
        doc.gap(6);
        // Stood aside there is no week: the paper says so, as the screen does, instead of a row
        // of zeros nobody counted (review of 10 Oct 2026, panel 9).
        if (!r.plus) {
          doc.line(t('informe_apartado'), 10);
          return { name: pdfName('informe-semanal'), doc };
        }
        const cols = [{ title: t('col_dispositivo'), w: 0.16 }, { title: t('col_consultas'), w: 0.12 }, { title: t('c_rastreadores'), w: 0.12 }, { title: t('col_publicidad'), w: 0.12 }, { title: t('col_telemetria'), w: 0.12 }, { title: t('col_esperados'), w: 0.12 }, { title: t('col_desconocidos'), w: 0.12 }, { title: t('col_cortados'), w: 0.12 }];
        const cells = (label, f) => [label].concat([f.consultas, f.rastreadores, f.publicidad, f.telemetria, f.esperados, f.desconocidos, f.cortados].map(num));
        const rows = r.dispositivos.map((d) => cells(nombreAparato(d.id, d.name), d.fila));
        rows.push({ cells: cells(t('total'), r.total), bold: true });
        doc.table(cols, rows, 8);
        if (r.anterior) {
          doc.seccion(t('informe_cambio_titulo'), t('informe_cambio_lead'));
          for (const [clave, campo] of [['col_consultas', 'consultas'], ['c_rastreadores', 'rastreadores'], ['col_publicidad', 'publicidad'], ['col_telemetria', 'telemetria'], ['col_desconocidos', 'desconocidos'], ['col_cortados', 'cortados']]) {
            const d = r.total[campo] - r.anterior[campo];
            const frase = d === 0 ? t('cambio_igual') : t(d > 0 ? 'cambio_mas' : 'cambio_menos').replace('{n}', Math.abs(d));
            doc.line('· ' + t(clave) + ': ' + num(r.total[campo]) + ' · ' + frase, 10);
          }
        }
        if (r.plus && r.comparar_desde) {
          doc.seccion(t('informe_cambio_titulo')).line(t('informe_comparar_desde').replace('{fecha}', new Date(r.comparar_desde).toLocaleDateString(LOC, DIA)), 10);
        } else if (r.plus) {
          doc.seccion(t('informe_nuevos_titulo'), t('informe_nuevos_lead'));
          if (r.novedades.length) {
            doc.table([{ title: t('col_nombre'), w: 0.34 }, { title: t('col_empresa_n'), w: 0.2 }, { title: t('col_categoria'), w: 0.16 }, { title: t('col_dispositivo'), w: 0.18 }, { title: t('col_consultas'), w: 0.12 }],
              r.novedades.map((n2) => [n2.nombre, n2.empresa, catName(n2.categoria), nombreAparato(n2.device_id, n2.device_name), num(n2.consultas)]), 8);
          } else {
            doc.line(t('informe_nuevos_ninguno'), 10);
          }
        }
        if (r.plus) {
          doc.seccion(t('informe_corredores_titulo'), t('informe_corredores_lead'));
          if (r.corredores.length) {
            doc.table([{ title: t('col_empresa_n'), w: 0.26 }, { title: t('col_pais'), w: 0.14 }, { title: t('col_consultas'), w: 0.1 }, { title: t('col_nombres_n'), w: 0.1 }, { title: t('col_declara'), w: 0.4 }],
              r.corredores.map((c) => [c.nombre, c.pais || '', num(c.consultas), num(c.nombres), c.declara.join(', ')]), 8);
          } else {
            doc.line(t('informe_corredores_ninguna'), 10);
          }
        }
        if (d) {
          doc.seccion(t('destinos_titulo'));
          doc.line(t('destinos_paises_titulo'), 11, true).gap(2);
          doc.line(d.con_direccion ? tn('destinos_paises_frase', d.n_paises) : t('destinos_sin'), 10);
          if (d.con_direccion) {
            doc.gap(2).line(t('destinos_paises_lead').replace('{total}', num(d.consultas)).replace('{con}', num(d.con_direccion)), 9, false, 0.45).gap(4);
            doc.table([{ title: t('col_pais'), w: 0.6 }, { title: '%', w: 0.15 }, { title: t('col_consultas'), w: 0.25 }], d.paises.map((p) => [p.nombre, p.pct + ' %', num(p.consultas)]), 8);
          }
          doc.gap(8).line(t('destinos_grupos_titulo'), 11, true).gap(2);
          if (d.top_n) doc.line(tn('destinos_grupos_frase', d.top_n).replace('{pct}', d.top_pct).replace('{total}', num(d.consultas)).replace('{grupos}', d.grupos.slice(0, d.top_n).map(grupoDicho).join(', ')), 10);
          doc.gap(2).line(t('destinos_grupos_lead'), 9, false, 0.45).gap(4);
          if (d.grupos.length) doc.table([{ title: t('col_grupo'), w: 0.6 }, { title: '%', w: 0.15 }, { title: t('col_consultas'), w: 0.25 }], d.grupos.slice(0, 12).map((g) => [grupoDicho(g), g.pct + ' %', num(g.consultas)]), 8);
          // La fuente con su fecha, al pie del documento y de cada página.
          doc.gap(8).line(pieRed(d.fuente), 8, false, 0.45);
        }
        if (m) {
          doc.seccion(t('noche_titulo'), t('noche_lead'));
          if (m.noche.length) {
            if (m.noche_total) doc.line(tn('noche_frase', m.noche_total), 10).gap(3);
            doc.table([{ title: t('col_dispositivo'), w: 0.2 }, { title: t('noche_col_consultas'), w: 0.14 }, { title: t('noche_col_noches'), w: 0.1 }, { title: t('noche_col_empresas'), w: 0.24 }, { title: t('noche_col_mananas'), w: 0.32 }],
              m.noche.map((n) => [nombreAparato(n.id, n.name), num(n.consultas) + (n.cortadas ? ' · ' + tn('noche_cortadas', n.cortadas) : ''), num(n.noches), n.empresas.join(', '), mananas(n).join('\n')]), 8);
          } else {
            doc.line(t('noche_ninguna'), 10);
          }
          doc.seccion(t('empresas_titulo'), t('empresas_lead'));
          if (m.empresas_hoy.length) doc.line(tn('empresas_hoy', m.empresas_hoy.length).replace('{grupos}', m.empresas_hoy.join(', ')), 10).gap(3);
          if (m.empresas.length) {
            const w = 0.8 / m.dias.length;
            doc.table([{ title: t('col_dispositivo'), w: 0.2 }].concat(m.dias.map((d) => ({ title: diaCorto(d), w }))), m.empresas.map((e) => [nombreAparato(e.id, e.name)].concat(e.por_dia.map((x) => num(x)))), 8);
          } else {
            doc.line(t('empresas_ninguna'), 10);
          }
          doc.seccion(t('cadencia_titulo'), t('cadencia_lead'));
          if (m.cadencias.length) {
            doc.table([{ title: t('col_nombre'), w: 0.34 }, { title: t('col_empresa_n'), w: 0.16 }, { title: t('col_contactos'), w: 0.12 }, { title: t('col_cadencia'), w: 0.38 }], m.cadencias.map((c) => [c.nombre, c.empresa || '', num(c.contactos), c.frase]), 8);
          } else {
            doc.line(t('cadencia_ninguna'), 10);
          }
          doc.seccion(t('disfraz_seccion'), t('disfraz_lead'));
          if (m.disfrazados.length) {
            doc.line(tn('disfraz_total', m.disfrazados.length).replace('{consultas}', tn('informe_n_consultas', m.disfrazados_consultas)), 10).gap(3);
            doc.table([{ title: t('col_nombre'), w: 0.56 }, { title: t('col_empresa_n'), w: 0.2 }, { title: t('col_consultas'), w: 0.12 }, { title: t('col_aparatos_n'), w: 0.12 }], m.disfrazados.map((x) => [x.nombre + '\n' + x.frase, x.empresa || '', num(x.consultas), num(x.aparatos)]), 8);
          } else {
            doc.line(t('disfraz_ninguno'), 10);
          }
        }
        if (r.plus) doc.seccion(t('informe_whatsapp_titulo')).line(r.texto_whatsapp, 10);
        return { name: pdfName('informe-semanal'), doc, pie2: d ? pieRed(d.fuente) : '' };
      });
      $('i-copiar').addEventListener('click', async () => {
        try { await navigator.clipboard.writeText(r.texto_whatsapp); $('i-copiado').textContent = t('informe_copiado'); } catch (_) {}
      });
    },
    async reglas() {
      const devs = await api('/api/dispositivos');
      const sel = $('n-device');
      devs.forEach((d) => { const o = document.createElement('option'); o.value = d.id; o.textContent = nombreAparato(d.id, d.name, d.last_ip) + (d.puede_cortar ? '' : ' · ' + mirando(d.horas_observadas)); sel.appendChild(o); });
      $('n-scope').addEventListener('change', () => $('n-device-wrap').classList.toggle('hidden', $('n-scope').value !== 'device'));
      // The category is written the way the panel shows it, in the panel's language («tracker»,
      // «advertising»): until 1.0.1 only the Spanish code was accepted, and the English page even
      // suggested the Spanish words. Both are understood; the server gets the code.
      const CATS_CORTABLES = ['rastreador', 'publicidad', 'telemetria', 'desconocido'];
      const ejemplo = { es: 'ejemplo.com', en: 'example.com', pt: 'exemplo.com' }[LANG] || 'example.com';
      const pista = () => { $('n-pattern').placeholder = $('n-kind').value === 'category' ? CATS_CORTABLES.map(catName).join(' / ') : ejemplo; };
      const categoriaDe = (texto) => {
        const x = texto.trim().toLowerCase();
        const sinTilde = (v) => v.normalize('NFD').replace(/[\u0300-\u036f]/g, '');
        for (const [codigo, nombre] of Object.entries(T.categorias || {})) {
          if (sinTilde(x) === sinTilde(codigo) || sinTilde(x) === sinTilde(String(nombre).toLowerCase())) return codigo;
        }
        return texto.trim();
      };
      $('n-kind').addEventListener('change', pista);
      pista();
      const load = async () => {
        const r = await api('/api/reglas');
        document.querySelectorAll('input[name=modo]').forEach((i) => { i.checked = i.value === r.modo_bloqueo; });
        $('r-ocultas').textContent = r.ocultas ? t(r.ocultas === 1 ? 'reglas_ocultas_uno' : 'reglas_ocultas').replace('{n}', r.ocultas) : '';
        $('r-rows').innerHTML = r.reglas.map((x) => `<tr><td>${x.id}</td><td>${x.activa ? esc(t('regla_activa')) : esc(t('regla_deshecha'))}</td><td>${x.scope === 'home' ? esc(t('alcance_casa')) : esc(x.device_id ? nombreAparato(x.device_id, x.device_name) : '')}</td><td>${esc(t('tipo_' + x.match_kind))}</td><td class="mono">${esc(x.pattern)}</td><td class="verdict ${x.action === 'cortar' ? 'cortado' : ''}">${esc(t('accion_' + x.action))}</td><td>${esc(creadaPor(x.created_by))}</td><td class="muted">${when(x.created_at)}</td><td>${x.activa ? `<button class="secondary undo" data-id="${x.id}">${esc(t('deshacer'))}</button>` : ''}</td></tr>`).join('') || `<tr><td colspan="9" class="muted">${esc(t('reglas_ninguna'))}</td></tr>`;
      };
      $('nueva').addEventListener('submit', async (e) => {
        e.preventDefault();
        const f = new FormData($('nueva'));
        const kind = f.get('match_kind');
        const pattern = kind === 'category' ? categoriaDe(f.get('pattern') || '') : f.get('pattern');
        const body = { scope: f.get('scope'), device_id: f.get('device_id') || null, match_kind: kind, pattern, action: f.get('action') };
        try {
          const created = await createRule(body);
          $('n-msg').textContent = created ? t('regla_creada') : '';
          if (created) { $('n-pattern').value = ''; await load(); }
        } catch (err) { $('n-msg').textContent = String(err.message || err); }
      });
      // Every action says when it failed and leaves its button usable: a 409 or a 500 used to
      // leave it grey for good, or say nothing, and the person believed it was done (review of
      // 10 Oct 2026, panel 14).
      $('r-rows').addEventListener('click', async (e) => {
        const b = e.target.closest('button.undo');
        if (!b) return;
        b.disabled = true;
        try {
          await api('/api/reglas/' + b.dataset.id + '/deshacer', { method: 'POST' });
          await load();
        } catch (err) { $('undo-msg').className = 'bad'; $('undo-msg').textContent = String(err.message || err); } finally { b.disabled = false; }
      });
      $('undo-today').addEventListener('click', async () => {
        const b = $('undo-today');
        b.disabled = true;
        try {
          const n = await api('/api/reglas/deshacer-hoy', { method: 'POST' });
          $('undo-msg').className = 'ok';
          $('undo-msg').textContent = tn('deshechas', n);
          await load();
        } catch (err) { $('undo-msg').className = 'bad'; $('undo-msg').textContent = String(err.message || err); } finally { b.disabled = false; }
      });
      $('modo-guardar').addEventListener('click', async () => {
        const b = $('modo-guardar');
        const modo = (document.querySelector('input[name=modo]:checked') || {}).value || 'nxdomain';
        b.disabled = true;
        try {
          await api('/api/ajustes/bloqueo', { method: 'POST', body: { modo } });
          $('modo-msg').className = 'ok';
          $('modo-msg').textContent = t('guardado');
        } catch (err) { $('modo-msg').className = 'bad'; $('modo-msg').textContent = String(err.message || err); } finally { b.disabled = false; }
      });
      await tarjetaMaxima(load);
      await load();
    },
    async radiografia() {
      tarjetaMaxima();
      let last = null;
      savePdf('r-pdf', () => {
        const doc = pdfDocument('GUARDIANA · ' + t('radiografia_titulo'));
        // Before the first figures arrive there is nothing to save: zeros would be figures
        // nobody counted (review of 10 Oct 2026, panel 9).
        if (!last) throw new Error(t('pdf_sin_datos'));
        pdfHead(doc, t('radiografia_titulo'));
        const r = last;
        doc.line([['c_servicios', r.servicios], ['c_rastreadores', r.rastreadores], ['c_publicidad', r.publicidad], ['c_nuevos', r.destinos_nuevos], ['c_esperados', r.esperados], ['c_cortados', r.cortados]].map(([k, v]) => t(k) + ': ' + v).join(' · '), 10);
        if (r.hueco) doc.line(t('hueco').replace('{desde}', when(r.hueco.desde)).replace('{hasta}', when(r.hueco.hasta)), 9, false, 0.4);
        doc.gap(6).table(eventCols(true), r.eventos.length ? foldEvents(r.eventos).map((ev) => eventCells(ev, true)) : [[ '', t('sin_consultas_aun'), '', '', '', '', '' ]]);
        pdfQuienHayDetras(doc, r.eventos);
        return { name: pdfName('radiografia'), doc, pie2: pieRed(r.fuente_red) };
      });
      await cargarCortados();
      // Until the system DNS points at Guardiana, this PC's own queries never arrive: say so and offer the change (brief §4).
      try {
        const est = await estadoBase();
        // With the trial over and no Plus, Guardiana is not watching: offering to point the DNS at
        // it only broke names (review of 28 Sep 2026, G6).
        $('dns-card').classList.toggle('hidden', est.dns_aplicado || est.caducado);
        $('dns-apply').addEventListener('click', async () => {
          $('dns-msg').textContent = t('dns_aplicando'); $('dns-apply').disabled = true;
          try { const r = await api('/api/dns/aplicar', { method: 'POST' }); $('dns-msg').textContent = r.mensaje; $('dns-apply').classList.add('hidden'); }
          catch (err) { $('dns-msg').textContent = String(err.message || err); $('dns-apply').disabled = false; }
        });
      } catch (_) {}
      bindCutButtons($('live'), '/api/reglas');
      const vivo = tablaQuieta($('live'));
      $('share-open').addEventListener('click', async () => {
        if (!last) return;
        await drawCard($('share-canvas'), last);
        $('share-download').href = $('share-canvas').toDataURL('image/png');
        const text = t('compartir_texto').replace('{servicios}', num(last.servicios)).replace('{rastreadores}', num(last.rastreadores)).replace('{nuevos}', num(last.destinos_nuevos)).replace('{esperados}', num(last.esperados)).replace('{cortados}', num(last.cortados));
        $('share-wa').href = waLink(text);
        $('share').classList.remove('hidden');
      });
      $('share-close').addEventListener('click', () => $('share').classList.add('hidden'));
      const refresh = async () => {
        const r = await api('/api/radiografia?segundos=60');
        last = r;
        pintaFuenteRed('fuente-red', r.fuente_red);
        $('c-servicios').textContent = r.servicios;
        $('c-rastreadores').textContent = r.rastreadores;
        $('c-publicidad').textContent = r.publicidad;
        $('c-nuevos').textContent = r.destinos_nuevos;
        $('c-esperados').textContent = r.esperados;
        $('c-cortados').textContent = r.cortados;
        vivo.pinta(eventRows(r.eventos) || `<tr><td colspan="8" class="muted">${esc(t('sin_consultas_aun'))}</td></tr>`, r.eventos.length);
        // A line that appears or goes would move everything below it: not once the person is
        // using the page.
        if (!vivo.quieta) {
          $('hueco').textContent = r.hueco ? t('hueco').replace('{desde}', when(r.hueco.desde)).replace('{hasta}', when(r.hueco.hasta)) : '';
          $('hueco').classList.toggle('hidden', !r.hueco);
        }
      };
      await refresh();
      cadaTanto(refresh, 2000);
      // Bursts change slowly and cost a query over the whole day: painted once, not every
      // two seconds like the live table.
      pintarRafagas().catch(() => {});
    },
    async extracto() {
      const form = $('filters');
      paintChanges().catch(() => {});
      await cargarCortados();
      let lastR = null, lastFilters = '';
      savePdf('export-pdf', () => {
        const doc = pdfDocument('GUARDIANA · ' + t('nav_extracto'));
        if (!lastR) throw new Error(t('pdf_sin_datos'));
        pdfHead(doc, t('nav_extracto'));
        const r = lastR;
        doc.line(mostrandoTexto(r), 10);
        if (lastFilters) doc.line(lastFilters, 9, false, 0.4);
        (r.huecos || []).forEach((h) => doc.line(t('hueco').replace('{desde}', when(h.desde)).replace('{hasta}', when(h.hasta)), 9, false, 0.4));
        doc.gap(6).table(eventCols(true), r.eventos.length ? foldEvents(r.eventos).map((ev) => eventCells(ev, true)) : [[ '', t('sin_resultados'), '', '', '', '', '' ]]);
        pdfQuienHayDetras(doc, r.eventos);
        return { name: pdfName('extracto'), doc, pie2: pieRed(r.fuente_red) };
      });
      bindCutButtons($('rows'), '/api/reglas');
      const load = async () => {
        const q = new URLSearchParams(new FormData(form));
        for (const [k, v] of [...q]) if (!v) q.delete(k);
        const r = await api('/api/extracto?' + q.toString());
        lastR = r;
        pintaFuenteRed('fuente-red', r.fuente_red);
        // The server countries the extract holds, for the filter; the one asked for in the
        // address (a country pressed in the weekly report) is chosen (decision 203).
        const selP = $('f-pais');
        if (selP && !selP.dataset.lleno) {
          selP.dataset.lleno = '1';
          for (const [clave, nombre] of r.paises_servidor || []) {
            const ya = [...selP.options].find((o) => o.value === clave);
            if (ya) { ya.textContent = nombre; selP.appendChild(ya); continue; }
            const o = document.createElement('option'); o.value = clave; o.textContent = nombre; selP.appendChild(o);
          }
        }
        lastFilters = [...q].filter(([k]) => k !== 'limit').map(([k, v]) => t('f_' + ({ device_id: 'dispositivo', category: 'categoria', signal: 'senal', verdict: 'veredicto', pais: 'pais_servidor' }[k] || k)) + ': ' + (k === 'device_id' ? ($('f-device').selectedOptions[0] || {}).textContent || v : k === 'pais' ? ($('f-pais').selectedOptions[0] || {}).textContent || v : k === 'category' ? catName(v) : k === 'verdict' ? verdictName(v) : v)).join(' · ');
        $('rows').innerHTML = eventRows(r.eventos) || `<tr><td colspan="8" class="muted">${esc(t('sin_resultados'))}</td></tr>`;
        $('total').textContent = mostrandoTexto(r);
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
        const out = $('check-result');
        $('check').disabled = true;
        try {
          const r = await api('/api/extracto/comprobar');
          if (r.ok) {
            out.className = 'ok';
            out.textContent = (r.anchor_is_genesis ? tn('cadena_ok', r.checked) : t('cadena_ok_recortada').replace('{n}', r.checked)).replace('{p}', r.pruned);
          } else {
            out.className = 'bad';
            out.textContent = t('cadena_rota').replace('{id}', r.fallo.id).replace('{motivo}', r.fallo.motivo);
          }
        } catch (err) { out.className = 'bad'; out.textContent = String(err.message || err); } finally { $('check').disabled = false; }
      });
      const devs = await api('/api/dispositivos');
      const sel = $('f-device');
      // A device that keeps its detail to itself says so here: choosing it shows no rows, and
      // that has a reason (review of 10 Oct 2026, panel 30).
      devs.forEach((d) => { const o = document.createElement('option'); o.value = d.id; o.textContent = nombreAparato(d.id, d.name, d.last_ip) + (d.detalle_visible ? '' : t('extracto_no_comparte')); sel.appendChild(o); });
      // Arriving from a country of the weekly report: the filter starts on it. The option is put
      // in before the first load, so the request carries it.
      const pedido = new URLSearchParams(location.search).get('pais');
      if (pedido && $('f-pais')) { const o = document.createElement('option'); o.value = pedido; o.textContent = pedido; $('f-pais').appendChild(o); $('f-pais').value = pedido; }
      await load();
    },
    async dispositivos() {
      pintarRecibos().catch(() => {});
      // One card per device (the responsible, 9 Oct 2026: the ten-column table «no guarda
      // geometría»): its name and address, when it was last heard and whether its detail can be
      // seen from here; its figures as the same counters the home page uses, and since when they
      // count; then the companies and the countries, each on its own line. And a field to name
      // it from here: the brief says a device is named from the panel or from itself, and only
      // the second way existed (review of 10 Oct 2026, panel 10).
      const cat = (k) => (T.categorias && T.categorias[k]) || k;
      const cifra = (n, l, cls = '') => `<div class="counter${cls}"><div class="n">${num(n)}</div><div class="l">${esc(l)}</div></div>`;
      const pintar = async () => {
        const devs = await api('/api/dispositivos');
        $('devices').innerHTML = devs.map((d) => {
          const nombre = esc(nombreAparato(d.id, d.name, d.last_ip));
          const emp = d.lectura.empresas || [];
          const pistas = hints(d.lectura).map((x) => `<li>${esc(x)}</li>`).join('');
          const paises = d.lectura && d.lectura.paises && d.lectura.paises.length
            ? `<div class="disp-linea"><span class="k">${esc(t('lectura_paises').replace(/:\s*$/, ''))}</span><span>${d.lectura.paises.map((x) => `${esc(x[0])} <b>${num(x[1])}</b>`).join(' · ')}</span></div>`
            : '';
          return `<section class="card disp">
            <div class="disp-cab">
              <div><h3>${nombre}</h3>${d.last_ip ? `<span class="mono muted">${esc(d.last_ip)}</span>` : ''}</div>
              <dl class="disp-meta"><div><dt>${esc(t('col_ultima'))}</dt><dd>${when(d.last_seen)}</dd></div><div><dt>${esc(t('col_detalle'))}</dt><dd>${esc(t(d.detalle_visible ? 'detalle_si' : 'detalle_no'))}</dd></div></dl>
            </div>
            <p class="muted small">${esc(t('dispositivo_desde').replace('{fecha}', when(d.first_seen)))}</p>
            <div class="counters disp-cifras">
              ${cifra(d.totales.consultas, t('col_consultas'))}${cifra(d.totales.cortados, t('col_cortados'), ' bloq')}
              ${cifra(d.totales.rastreadores, cat('rastreador'), ' c-rastreador')}${cifra(d.totales.publicidad, cat('publicidad'), ' c-publicidad')}
              ${cifra(d.totales.telemetria, cat('telemetria'), ' c-telemetria')}${cifra(d.totales.esperados, cat('esperado'), ' c-esperado')}
            </div>
            <div class="disp-linea"><span class="k">${esc(t('col_empresas'))}</span><span title="${esc(emp.map((e) => e[0] + ' ' + e[1]).join(', '))}"><b>${num(d.lectura.empresas_total)}</b>${emp.length ? ' · ' + esc(emp.slice(0, 6).map((e) => e[0]).join(', ')) : ''}</span></div>
            ${paises}
            ${pistas ? `<ul class="disp-pistas">${pistas}</ul>` : ''}
            <form class="filters disp-nombre" data-id="${esc(d.id)}"><label><span>${esc(t('mi_nombre'))}</span><input name="name" maxlength="60" autocomplete="off" value="${esc(d.name || '')}"></label><button type="submit" class="secondary">${esc(t('mi_nombre_guardar'))}</button> <span class="ok" role="status" aria-live="polite"></span></form>
          </section>`;
        }).join('') || `<div class="card muted">${esc(t('sin_dispositivos'))}</div>`;
      };
      $('devices').addEventListener('submit', async (e) => {
        const f = e.target.closest('form.disp-nombre');
        if (!f) return;
        e.preventDefault();
        const msg = f.querySelector('[role=status]');
        const b = f.querySelector('button');
        b.disabled = true;
        try {
          await api('/api/dispositivos/' + encodeURIComponent(f.dataset.id) + '/nombre', { method: 'POST', body: { name: f.querySelector('input').value } });
          await pintar();
          const otra = [...document.querySelectorAll('form.disp-nombre')].find((x) => x.dataset.id === f.dataset.id);
          const m2 = otra && otra.querySelector('[role=status]');
          if (m2) { m2.className = 'ok'; m2.textContent = t('mi_nombre_guardado'); setTimeout(() => { m2.textContent = ''; }, 3000); }
        } catch (err) { msg.className = 'bad'; msg.textContent = String(err.message || err); b.disabled = false; }
      });
      await pintar();
    },
    async sabeDeTi() {
      const r = await api('/api/sabe-de-ti');
      $('s-eventos').textContent = r.eventos; $('s-dispositivos').textContent = r.dispositivos;
      $('s-reglas').textContent = r.reglas; $('s-nombres').textContent = r.nombres_vistos;
      $('s-ruta').textContent = r.ruta;
      $('s-retencion').textContent = r.retencion;
      $('outbound').innerHTML = r.outbound.map((o) => `<tr><td>${when(o.ts)}</td><td>${esc(t('proposito_' + o.purpose))}</td><td class="mono">${esc(o.host)}</td><td>${o.bytes}</td></tr>`).join('') || `<tr><td colspan="4" class="ok">${esc(t('outbound_vacio'))}</td></tr>`;
      $('wipe').addEventListener('click', async () => {
        // The word is the one the question asks for in the panel's language (BORRAR, DELETE,
        // APAGAR). Until 1.0.1 only BORRAR was accepted, so in English and Portuguese typing the
        // word the panel asked for did nothing at all. The server keeps its one fixed word.
        const word = prompt(t('borrar_confirmar'));
        if ((word || '').trim().toUpperCase() !== t('borrar_palabra').toUpperCase()) return;
        try {
          await api('/api/sabe-de-ti/borrar', { method: 'POST', body: { confirmacion: 'BORRAR' } });
          location.reload();
        } catch (err) { $('wipe-msg').textContent = String(err.message || err); }
      });
    },
    async hogar() {
      // On a Mac, say it plainly: Home Mode works there but is not part of 1.0, and the site says
      // the same. Offering it in silence would make the program and the page disagree.
      estadoBase().then((e) => {
        if (e && e.so === 'macos') $('hogar-mac').classList.remove('hidden');
      }).catch(() => {});
      const render = (r, aviso) => {
        $('h-estado').textContent = r.encendido
          ? t('hogar_estado_on').replace('{fecha}', r.desde ? when(r.desde) : '').replace('{ip}', r.ip || '')
          : r.aparcado ? t('hogar_estado_aparcado') : t('hogar_estado_off');
        if (r.encendido && r.fuera_de_casa) $('h-estado').textContent = t('hogar_estado_fuera');
        const ipKey = r.ip_dinamica === true ? 'hogar_ip_dinamica' : r.ip_dinamica === false ? 'hogar_ip_fija' : 'hogar_ip_desconocida';
        $('h-ip-aviso').textContent = r.ip ? t(ipKey).replace('{ip}', r.ip) : t('hogar_sin_lan');
        const sv = (n) => n === 0 ? t('hogar_suspension_nunca_valor') : tn('hogar_suspension_valor', n);
        const sl = r.suspension;
        $('h-suspension').textContent = !sl ? '' : (sl[0] === 0 && sl[1] === 0) ? t('hogar_suspension_nunca') : t('hogar_suspension').replace('{ac}', sv(sl[0])).replace('{dc}', sv(sl[1]));
        $('h-toggle').textContent = r.encendido ? t('hogar_desactivar') : r.aparcado ? t('hogar_desactivar_relevo') : t('hogar_activar');
        if (r.licencia) { $('h-ip-aviso').textContent += ' ' + r.licencia; }
        $('h-toggle').dataset.on = (r.encendido || r.aparcado) ? '1' : '0';
        $('h-aviso').textContent = aviso || '';
        $('h-conectar').classList.toggle('hidden', !r.encendido);
        $('h-primero').classList.toggle('hidden', !!r.encendido);
        if (r.encendido) {
          $('h-qr').innerHTML = r.qr_svg || '';
          $('h-url').textContent = r.url || '';
          $('h-router').textContent = t('hogar_router_texto').replace('{ip}', r.ip || '');
          pasosTel($('h-iphone'), t('hogar_iphone_pasos'), r.ip || '');
          pasosTel($('h-android'), t('hogar_android_pasos'), r.ip || '');
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
        if (!lastR) throw new Error(t('pdf_sin_datos'));
        pdfHead(doc, t('mi_titulo'));
        const r = lastR;
        if (r.dispositivo) doc.line(nombreAparato(r.dispositivo.id, r.dispositivo.name, r.dispositivo.last_ip) + (r.dispositivo.last_ip ? ' · ' + r.dispositivo.last_ip : ''), 10, true);
        doc.line([['col_consultas', r.totales.consultas], ['c_rastreadores', r.totales.rastreadores], ['c_esperados', r.totales.esperados], ['c_cortados', r.totales.cortados]].map(([k, v]) => t(k) + ': ' + num(v)).join(' · '), 10);
        doc.gap(6).table(eventCols(false), r.eventos.length ? foldEvents(r.eventos).map((ev) => eventCells(ev, false)) : [[ '', t('sin_consultas_aun'), '', '' ]]);
        if (lastRules.length) {
          doc.seccion(t('mi_reglas_titulo'));
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
        // It goes through once its queries arrive; a phone that only named itself has not yet.
        // Until then the steps are right here, with the address the phone reached this page by:
        // the QR used to open the home panel's page, which needs a key the phone does not have
        // (review of 10 Oct 2026, panel 1).
        const pasa = !!(r.dispositivo && r.totales.consultas > 0);
        $('mi-pasa').textContent = pasa ? t('mi_pasa') : t('mi_no_pasa');
        $('mi-pasa').className = pasa ? 'ok' : 'muted';
        // The address only when the page was reached by it (the QR's link); by a name, the
        // steps would name no address at all.
        const ip = /^\d{1,3}(\.\d{1,3}){3}$/.test(location.hostname) ? location.hostname : '';
        $('mi-guia').classList.toggle('hidden', pasa || !ip);
        if (!pasa && ip) {
          pasosTel($('mi-iphone'), t('hogar_iphone_pasos'), ip);
          pasosTel($('mi-android'), t('hogar_android_pasos'), ip);
        }
        $('m-consultas').textContent = num(r.totales.consultas); $('m-rastreadores').textContent = num(r.totales.rastreadores);
        $('m-esperados').textContent = num(r.totales.esperados); $('m-cortados').textContent = num(r.totales.cortados);
        // Never overwrite what the person is typing: the page reloads every few seconds.
        const nameInput = document.querySelector('#mi-nombre input');
        if (r.dispositivo && document.activeElement !== nameInput && !nameInput.dataset.dirty) { nameInput.value = r.dispositivo.name || ''; }
        if (r.dispositivo && document.activeElement !== $('mi-compartir')) { $('mi-compartir').checked = !!r.dispositivo.share_detail_with_home; }
        // Known only by its address on the network: say what that means for this checkbox.
        $('mi-por-ip').classList.toggle('hidden', !(r.dispositivo && String(r.dispositivo.id || '').startsWith('ip:')));
        const me = r.dispositivo;
        // Un nombre suelto se corta desde el primer minuto (el servidor pide confirmación si el
        // aparato lleva menos de un día); arriba se dice cuánto lleva mirando, como información.
        $('mi-gate').textContent = me && !r.puede_cortar ? mirando(r.horas_observadas) : '';
        const lect = r.lectura || {};
        $('mi-empresas').innerHTML = (lect.empresas && lect.empresas.length) ? lect.empresas.map((e) => `<li><b>${esc(e[0])}</b> · ${e[1]}</li>`).join('') + `<li class="muted">${esc(tn('lectura_empresas_total', lect.empresas_total))}</li>` : `<li class="muted">${esc(t('lectura_empresas_ninguna'))}</li>`;
        $('mi-avisos').innerHTML = hints(lect).map((h) => `<p class="limit">${esc(h)}</p>`).join('');
        await cargarCortados('/api/mi-dispositivo/reglas');
        miTabla.pinta(r.eventos.map((ev) => `<tr ${filaAttrs(ev)}><td class="mono">${clock(ev.ts)}</td><td><span class="mono">${esc(ev.qname)}</span>${company(ev)}${phrases(ev)}</td>${empresaCelda(ev)}<td>${cat(ev)}</td>${paisCelda(ev)}${celdasEstado(ev)}</tr>`).join('') || `<tr><td colspan="7" class="muted">${esc(t('sin_consultas_aun'))}</td></tr>`, r.eventos.length);
        const rules = await api('/api/mi-dispositivo/reglas');
        lastRules = rules.reglas;
        $('mi-reglas').innerHTML = rules.reglas.map((x) => `<tr><td>${esc(t('tipo_' + x.match_kind))}</td><td class="mono">${esc(x.pattern)}</td><td>${esc(t('accion_' + x.action))}</td><td>${x.activa ? esc(t('regla_activa')) : esc(t('regla_deshecha'))}</td><td>${!x.activa ? '' : esDeLaCasa(x) ? `<span class="muted">${esc(t('mi_regla_de_la_casa'))}</span>` : `<button class="secondary undo" data-id="${x.id}">${esc(t('deshacer'))}</button>`}</td></tr>`).join('') || `<tr><td colspan="5" class="muted">${esc(t('reglas_ninguna'))}</td></tr>`;
      };
      bindCutButtons($('mi-rows'), '/api/mi-dispositivo/reglas');
      const miTabla = tablaQuieta($('mi-rows'));
      $('mi-reglas').addEventListener('click', async (e) => {
        const b = e.target.closest('button.undo');
        if (!b) return;
        b.disabled = true;
        try {
          await api('/api/mi-dispositivo/reglas/' + b.dataset.id + '/deshacer', { method: 'POST' });
          $('mi-reglas-msg').textContent = '';
          await load();
        } catch (err) { $('mi-reglas-msg').textContent = String(err.message || err); } finally { b.disabled = false; }
      });
      document.querySelector('#mi-nombre input').addEventListener('input', (e) => { e.target.dataset.dirty = '1'; });
      $('mi-nombre').addEventListener('submit', async (e) => { e.preventDefault(); const inp = document.querySelector('#mi-nombre input'); try { await api('/api/mi-dispositivo/nombre', { method: 'POST', body: { name: inp.value } }); delete inp.dataset.dirty; inp.blur(); $('mi-nombre-msg').textContent = t('mi_nombre_guardado'); setTimeout(() => { $('mi-nombre-msg').textContent = ''; }, 3000); } catch (err) { $('mi-nombre-msg').textContent = String(err.message || err); } await load(); });
      // The box goes back to what it was when the change was not saved: left ticked, it said the
      // detail was shared when it was not (review of 10 Oct 2026, panel 14).
      $('mi-compartir').addEventListener('change', async (e) => {
        const c = e.target;
        c.disabled = true;
        try {
          await api('/api/mi-dispositivo/compartir', { method: 'POST', body: { compartir: c.checked } });
          $('mi-nombre-msg').textContent = '';
        } catch (err) { c.checked = !c.checked; $('mi-nombre-msg').className = 'bad'; $('mi-nombre-msg').textContent = String(err.message || err); } finally { c.disabled = false; }
      });
      await load();
      cadaTanto(load, 5000);
    },
    // «GUARDIANA por dentro» (1.0.22): the universe lives in its own script (por-dentro.js), which
    // the page loads before this one; here it gets the session, the texts and the language.
    async 'por-dentro'() {
      if (!window.GuardianaUniverso) return;
      await window.GuardianaUniverso.iniciar({ api, t, tn, esc, LOC, LANG, cadaTanto, pdf: { pdfDocument, savePdfFile } });
    },
    // «Revisión del ordenador» (1.0.24): its own script (revision.js), loaded before this one.
    async revision() {
      if (!window.GuardianaRevision) return;
      await window.GuardianaRevision.iniciar({ api, t, tn, esc, LOC, LANG, pdf: { pdfDocument, savePdfFile } });
    },
    async antivirus() {
      let pais = (c) => c;
      try { const dn = new Intl.DisplayNames([LOC], { type: 'region' }); pais = (c) => { try { return dn.of(c) || c; } catch (_) { return c; } }; } catch (_) {}
      const catNombre = (k) => (T.categorias && T.categorias[k]) || k;
      const enlace = (url, txt) => `<a href="${esc(url)}" target="_blank" rel="noopener noreferrer">${esc(txt)}</a>`;
      // Lo que el fabricante dice de un nombre, con su página; o que no lo dice, y de quién es
      // el nombre cuando las listas lo saben. Un nombre sin documentar es una pregunta, no una
      // acusación: así se escribe.
      const queEs = (d) => (d.uso
        ? `<span class="tag av-doc">${esc(t('av_uso_' + d.uso))}</span> ${enlace(d.fuente, t('av_fuente_fabricante'))}`
        : `<span class="tag av-nodoc">${esc(t('av_no_documentado'))}</span>`
          + (d.empresa ? ` <span class="muted">${esc(t('av_nombre_de').replace('{empresa}', d.empresa))}</span>` : '')
          + (d.categoria !== 'desconocido' && d.categoria !== 'esperado' ? ` <span class="tag ${esc(d.categoria)}">${esc(catNombre(d.categoria))}</span>` : ''));
      const firma = (p) => `<span class="tag app">${esc(p.nombre)} ×${num(p.consultas)}</span> <span class="muted">${esc(p.firma ? t('av_firmado_por').replace('{firma}', p.firma) : t('av_sin_firma'))}</span>`;
      // El corte limpio (1.0.22): solo en este equipo, siempre decisión de la persona, siempre
      // reversible. Si no está disponible, se dice por qué en vez de esconder el botón.
      let corte = { disponible: false, cortados: [] };
      const controlCorte = (v) => {
        if (v.device_id !== 'self') return '';
        const on = corte.cortados.includes(v.producto);
        if (!corte.disponible) return `<p class="limit">${esc(t('av_cortar_no_disponible'))}</p>`;
        return `<p class="vigilante"><label><input type="checkbox" class="av-cortar" data-producto="${esc(v.producto)}"${on ? ' checked' : ''}> <b>${esc(t('av_cortar'))}</b></label>
          <br><span class="${on ? 'bad' : 'muted'}">${esc(t(on ? 'av_cortar_on' : 'av_cortar_off'))}</span>
          <br><span class="muted">${esc(t('av_cortar_aviso'))}</span></p>`;
      };
      const visto = (v) => `<div class="card">
        <h3>${esc(v.producto)} <span class="muted">· ${esc(v.empresa)} (${esc(pais(v.pais))}) · ${esc(nombreAparato(v.device_id, v.device_name))}</span></h3>
        ${controlCorte(v)}
        <p>${v.programas.map(firma).join('<br>')}</p>
        <p><b>${esc(tn('av_consultas', v.consultas))}</b> · ${esc(tn('av_nombres', v.destinos_total))} · ${esc(tn('av_nocturnas', v.nocturnas))}${v.cortadas ? ' · ' + esc(tn('av_cortadas', v.cortadas)) : ''} · <span class="muted">${esc(t('col_ultima'))}: ${when(v.ultima)}</span></p>
        <p>${esc(tn('av_documentados', v.documentados))} · <b>${esc(tn('av_no_documentados', v.no_documentados))}</b></p>
        <div class="wrap"><table><thead><tr><th>${esc(t('av_col_nombre'))}</th><th>${esc(t('av_col_que_es'))}</th><th>${esc(t('col_consultas'))}</th><th>${esc(t('av_col_noche'))}</th><th>${esc(t('col_ultima'))}</th></tr></thead><tbody>
        ${v.destinos.map((d) => `<tr><td class="mono">${esc(d.qname)}</td><td>${queEs(d)}</td><td>${num(d.consultas)}${d.cortadas ? ` <span class="muted">(${esc(tn('av_cortadas', d.cortadas))})</span>` : ''}</td><td>${num(d.nocturnas)}</td><td class="muted">${when(d.ultima)}</td></tr>`).join('')}
        </tbody></table></div>
        ${v.destinos_total > v.destinos.length ? `<p class="muted">${esc(t('av_mas').replace('{n}', num(v.destinos_total - v.destinos.length)))}</p>` : ''}
      </div>`;
      // El importe viaja como «16500000 USD» y se escribe en la moneda y el idioma de quien lo lee.
      const importe = (x) => { const [n, m] = String(x).split(' '); try { return new Intl.NumberFormat(LOC, { style: 'currency', currency: m, maximumFractionDigits: 0 }).format(Number(n)); } catch (_) { return x; } };
      const fecha = (d) => { try { return new Date(d + 'T12:00:00').toLocaleDateString(LOC, { day: 'numeric', month: 'long', year: 'numeric' }); } catch (_) { return d; } };
      const sancion = (s) => `<li><b>${esc(fecha(s.fecha))}</b> · ${esc(s.autoridad)} (${esc(pais(s.pais))}) · <span class="tag av-${esc(s.tipo)}">${esc(t('av_tipo_' + s.tipo))}</span>${s.importe ? ' · <b>' + esc(importe(s.importe)) + '</b>' : ''}<br>${esc(t('av_sancion_' + s.clave))} ${enlace(s.fuente, t('av_fuente_oficial'))}</li>`;
      // Lo que dicen sus propios documentos (13 oct 2026): resumido en palabras llanas, cada punto
      // con el enlace a la página del fabricante que lo dice. Un resumen, no asesoría legal.
      const L3 = (o) => (o ? o[LANG] || o.es || '' : '');
      const terminos = (x) => (!x ? '' : `<details class="av-terminos"><summary><b>${esc(t('av_terminos_titulo'))}</b> <span class="muted">· ${esc(t('av_terminos_revisado').replace('{f}', fecha(x.revisado)))}</span></summary>
        <ul>${x.puntos.map((p) => `<li><span class="tag">${esc(t('av_tema_' + p.tema))}</span> ${esc(L3(p))} ${enlace(p.fuente, t('av_terminos_fuente'))}</li>`).join('')}</ul>
        ${x.como_limitar ? `<p><b>${esc(t('av_terminos_limitar'))}</b> ${esc(L3(x.como_limitar))} ${enlace(x.como_limitar.fuente, t('av_terminos_fuente'))}</p>` : ''}
        <p class="muted">${esc(t('av_terminos_leidos'))} ${x.documentos.map((d) => enlace(d.url, d.titulo + (d.actualizado ? ' (' + d.actualizado + ')' : ''))).join(' · ')}</p>
        <p class="muted">${esc(t('av_terminos_nota'))}</p></details>`);
      const ficha = (f) => `<div class="card">
        <h3>${esc(f.producto)} <span class="muted">· ${esc(f.empresa)} (${esc(pais(f.pais))})</span></h3>
        <p class="muted mono">${f.programas.map(esc).join(' · ')}</p>
        ${terminos(f.terminos)}
        ${f.sanciones.length ? `<ul>${f.sanciones.map(sancion).join('')}</ul>` : `<p class="muted">${esc(t('av_sin_sanciones'))}</p>`}
      </div>`;
      // Bytes en la unidad de quien lee, sin inventar precisión: lo que la pila dio por enviado.
      const bytes = (n) => {
        n = Number(n || 0);
        const u = ['B', 'kB', 'MB', 'GB', 'TB'];
        let i = 0;
        while (n >= 1000 && i < u.length - 1) { n /= 1000; i += 1; }
        return (i === 0 ? n : n.toFixed(n < 10 ? 1 : 0)) + ' ' + u[i];
      };
      // Un país es «XX», o una marca (@local, @anycast) que se dice en palabras.
      const paisTrafico = (c) => (!c ? t('av_pais_desconocido') : c === '@local' ? t('av_local') : c === '@anycast' ? t('av_anycast') : pais(c));
      const filaTrafico = (d, etiqueta) => `<tr><td>${esc(etiqueta)}</td><td><b>${esc(bytes(d.subidos))}</b> ${esc(t('av_sube'))}</td><td>${esc(bytes(d.bajados))} ${esc(t('av_baja'))}</td></tr>`;
      const bloqueTrafico = (p) => `<div class="card">
        <h3>${esc(p.producto)}</h3>
        <p><b>${esc(bytes(p.subidos))}</b> ${esc(t('av_subidos_total'))} · ${esc(bytes(p.bajados))} ${esc(t('av_bajados_total'))}${p.subidos_noche ? ' · ' + esc(t('av_de_noche_trafico').replace('{x}', bytes(p.subidos_noche))) : ''}</p>
        <h4 data-t="av_por_pais">${esc(t('av_por_pais'))}</h4>
        <div class="wrap"><table><tbody>${p.paises.map((x) => filaTrafico(x, paisTrafico(x.pais))).join('')}</tbody></table></div>
        <h4 data-t="av_por_destino">${esc(t('av_por_destino'))}</h4>
        <div class="wrap"><table><tbody>${p.destinos.map((x) => filaTrafico(x, (x.destino || x.red || t('av_sin_nombre')) + (x.destino && x.pais && x.pais[0] !== '@' ? ' · ' + paisTrafico(x.pais) : ''))).join('')}</tbody></table></div>
      </div>`;
      const reloj = (ms) => { try { return new Date(ms).toLocaleTimeString(LOC, { hour: '2-digit', minute: '2-digit' }); } catch (_) { return ''; } };
      // «Controles avanzados» se abre solo una vez, al abrir la página, si hay un corte puesto.
      let avanzadoVisto = false;
      // El panel en directo: cada minuto reciente de un antivirus hablando, lo nuevo arriba.
      const filaViva = (v) => `<tr><td class="muted">${esc(reloj(v.minuto))}</td><td><span class="tag ia">${esc(v.producto)}</span></td><td class="mono">${esc(v.destino || v.red || t('av_sin_nombre'))}</td><td>${esc(v.pais && v.pais[0] !== '@' ? paisTrafico(v.pais) : paisTrafico(v.red))}</td><td><b>${esc(bytes(v.subidos))}</b></td><td>${esc(bytes(v.bajados))}</td></tr>`;
      const paint = (r) => {
        corte = { disponible: !!r.cortar_disponible, cortados: r.cortados || [] };
        // El resumen de 24 h, siempre arriba, y los dos mandos: vigilar y, plegado en «Controles
        // avanzados», cortar todo lo que envía el antivirus.
        const m = r.resumen || {};
        $('av-resumen').innerHTML = (m.productos && m.productos.length)
          ? `<p class="av-cifras"><b>${esc(bytes(m.subidos))}</b> ${esc(t('av_resumen_enviado'))} · <b>${esc(bytes(m.bajados))}</b> ${esc(t('av_resumen_recibido'))} · ${esc(tn('av_resumen_paises', m.paises))} · ${esc(tn('av_resumen_destinos', m.destinos))}</p>`
            + (m.subidos_noche ? `<p class="muted">${esc(t('av_resumen_noche').replace('{x}', bytes(m.subidos_noche)))}</p>` : '')
            + `<p class="muted">${esc(t('av_resumen_productos').replace('{p}', m.productos.join(', ')))}</p>`
          : `<p class="muted">${esc(t('av_resumen_nada'))}</p>`;
        if (!r.vigilando) $('av-resumen').innerHTML += `<p class="bad">${esc(t('av_resumen_pausado'))}</p>`;
        $('av-vigilar').checked = !!r.vigilando;
        $('av-vigilar-txt').textContent = t(r.vigilando ? 'av_vigilar_on' : 'av_vigilar_off');
        // «Cortar todo lo que envía el antivirus» (hasta el 13 oct 2026 «Protección máxima del
        // antivirus», un nombre que callaba que así el antivirus protege menos) vive plegado en
        // «Controles avanzados». Puesto, una línea fuera del pliegue lo dice siempre, y al abrir la
        // página el pliegue se abre solo: nadie tiene que buscar cómo quitarlo.
        const hayCorte = !!r.maxima;
        $('av-max-fuera').textContent = hayCorte ? t('av_max_activa') : '';
        $('av-max-fuera').classList.toggle('hidden', !hayCorte);
        $('av-max-estado').textContent = corte.disponible ? '' : t('av_cortar_no_disponible');
        $('av-max-estado').className = corte.disponible ? 'hidden' : 'muted';
        $('av-max-activar').classList.toggle('hidden', hayCorte || !corte.disponible);
        $('av-max-quitar').classList.toggle('hidden', !corte.cortados.length);
        if (!avanzadoVisto) { avanzadoVisto = true; if (hayCorte || corte.cortados.length) $('av-avanzado').open = true; }
        $('av-vistos').innerHTML = r.vistos.length ? r.vistos.map(visto).join('')
          : `<div class="card"><p class="muted">${esc(t('av_ninguno').replace('{dias}', r.dias))}</p></div>`;
        $('av-ocultos').textContent = r.ocultos ? tn('av_ocultos', r.ocultos) : '';
        $('av-ocultos').classList.toggle('hidden', !r.ocultos);
        // Panel en directo.
        const vivo = r.vivo || [];
        $('av-vivo').innerHTML = vivo.map(filaViva).join('');
        $('av-vivo-vacio').classList.toggle('hidden', vivo.length > 0);
        const sube = vivo.reduce((s, v) => s + Number(v.subidos || 0), 0);
        const baja = vivo.reduce((s, v) => s + Number(v.bajados || 0), 0);
        $('av-vivo-resumen').textContent = vivo.length ? t('av_vivo_resumen').replace('{sube}', bytes(sube)).replace('{baja}', bytes(baja)) : '';
        $('av-trafico-lead').textContent = r.trafico_horas ? t('av_trafico_lead').replace('{h}', r.trafico_horas) : t('av_trafico_vacio');
        $('av-trafico').innerHTML = r.trafico.length ? r.trafico.map(bloqueTrafico).join('') : '';
        $('av-export').classList.toggle('hidden', !r.trafico.length);
        // Primero los que tienen decisiones oficiales, luego el resto, cada grupo por nombre.
        const cat = r.catalogo.slice().sort((a, b) => (b.sanciones.length > 0) - (a.sanciones.length > 0) || a.producto.localeCompare(b.producto));
        $('av-catalogo').innerHTML = cat.map(ficha).join('');
      };
      // Cortar / dejar pasar: la casilla manda la decisión; si Windows no deja, vuelve atrás y
      // se dice por qué.
      $('av-vistos').addEventListener('change', async (e) => {
        const c = e.target.closest('input.av-cortar');
        if (!c) return;
        c.disabled = true;
        try {
          const x = await api('/api/antivirus/cortar', { method: 'POST', body: { producto: c.dataset.producto, cortar: c.checked } });
          corte.cortados = x.cortados || [];
          paint(await api('/api/antivirus'));
        } catch (err) {
          c.checked = !c.checked;
          $('av-detectar-msg').textContent = String(err.message || err);
        } finally { c.disabled = false; }
      });
      // Exportar: se pide con el token en la cabecera (nunca en la dirección) y se descarga.
      $('av-export').addEventListener('click', async (e) => {
        e.preventDefault();
        try {
          const r = await fetch('/api/antivirus/export', { headers: { 'X-Guardiana-Token': token, 'X-Guardiana-Tz': String(-new Date().getTimezoneOffset()) } });
          if (!r.ok) throw new Error(await r.text());
          const blob = await r.blob();
          const a = document.createElement('a'); a.href = URL.createObjectURL(blob); a.download = 'guardiana-antivirus-trafico.csv';
          document.body.appendChild(a); a.click(); a.remove(); setTimeout(() => URL.revokeObjectURL(a.href), 10000);
        } catch (err) { $('av-detectar-msg').textContent = String(err.message || err); }
      });
      // Qué antivirus hay en el equipo, preguntado a Windows al abrir y cada pocos minutos: sin
      // botón, porque es información que siempre debe estar a la vista.
      const detectado = (d) => `<span>${d.activo ? '🟢' : '⚪'} <b>${esc(d.nombre)}</b> · ${esc(d.activo ? t('av_activo') : t('av_inactivo'))} · ${esc(d.al_dia ? t('av_al_dia') : t('av_desactualizado'))}${d.conocido ? '' : ` ${esc(t('av_no_en_lista'))}`}</span>`;
      const detecta = async () => {
        try {
          const d = await api('/api/antivirus/detectar');
          $('av-detectados').innerHTML = d.detectados.length
            ? `<p>${esc(t('av_instalados'))}<br>${d.detectados.map(detectado).join('<br>')}</p>`
            : `<p>${esc(t(d.aviso === 'no_windows' ? 'av_detectar_no_windows' : 'av_detectar_ninguno'))}</p>`;
        } catch (_) { /* si Windows no contesta, la línea queda como estaba */ }
      };
      detecta();
      cadaTanto(detecta, 120000);
      // Vigilar a mi antivirus: apagado, GUARDIANA deja de medir; no deshace ningún corte.
      $('av-vigilar').addEventListener('change', async (e) => {
        const c = e.target;
        c.disabled = true;
        try {
          await api('/api/antivirus/vigilar', { method: 'POST', body: { activa: c.checked } });
          paint(await api('/api/antivirus'));
        } catch (err) { c.checked = !c.checked; $('av-detectar-msg').textContent = String(err.message || err); } finally { c.disabled = false; }
      });
      // Cortar todo lo que envía el antivirus: pide confirmación una vez (lleva un aviso), y se
      // quita de un clic.
      const maximaSet = async (activa) => {
        $('av-max-confirma').classList.add('hidden');
        try {
          const x = await api('/api/antivirus/maxima', { method: 'POST', body: { activa } });
          corte.cortados = x.cortados || [];
          $('av-detectar-msg').textContent = '';
          paint(await api('/api/antivirus'));
          // El botón pulsado desaparece: el foco va al que lo deshace.
          $(activa ? 'av-max-quitar' : 'av-max-activar').focus();
        } catch (err) { $('av-detectar-msg').textContent = String(err.message || err); }
      };
      $('av-max-activar').addEventListener('click', () => { $('av-max-confirma').classList.remove('hidden'); $('av-max-si').focus(); });
      $('av-max-no').addEventListener('click', () => { $('av-max-confirma').classList.add('hidden'); $('av-max-activar').focus(); });
      $('av-max-si').addEventListener('click', () => maximaSet(true));
      $('av-max-quitar').addEventListener('click', () => maximaSet(false));
      // El mercado del antivirus: cifras de fuentes oficiales o reconocidas, con su enlace.
      const mercado = [
        { n: 1, fuente: 'Gartner / F‑Secure', url: 'https://investors.f-secure.com/investors/as_an_investment/market' },
        { n: 2, fuente: 'Gen Digital (año fiscal 2025)', url: 'https://newsroom.gendigital.com/2025-05-06-Gen-Delivers-Record-Q4-and-Full-Year-Fiscal-2025-Results' },
        { n: 3, fuente: 'Kaspersky (2024)', url: 'https://www.kaspersky.com/about/press-releases/kaspersky-reports-2024-financial-results-with-record-revenue' },
        { n: 4, fuente: 'FTC (2024)', url: 'https://www.ftc.gov/news-events/news/press-releases/2024/06/ftc-finalizes-order-avast-banning-it-selling-or-licensing-web-browsing-data-advertising-requiring-it' },
        { n: 5, fuente: 'Microsoft', url: 'https://learn.microsoft.com/en-us/defender-endpoint/microsoft-defender-antivirus-windows' },
        { n: 6, fuente: 'CMA (2021)', url: 'https://www.gov.uk/cma-cases/anti-virus-software' },
      ];
      $('av-mercado').innerHTML = mercado.map((m) => `<div class="card av-dato">
        <div class="av-dato-cifra">${esc(t('av_mercado_cifra_' + m.n))}</div>
        <p>${esc(t('av_mercado_' + m.n))}</p>
        <p class="muted">${esc(t('av_mercado_fuente'))}: <a href="${esc(m.url)}" target="_blank" rel="noopener noreferrer">${esc(m.fuente)}</a></p>
      </div>`).join('');
      paint(await api('/api/antivirus'));
      cadaTanto(async () => paint(await api('/api/antivirus')), 10000);
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
          <td>${esc(nombreAparato(x.device_id, x.device_name))}${quien(x)}</td>
          <td>${num(x.nombres)}</td><td>${num(x.consultas)}</td><td class="muted">${when(x.ultima)}</td></tr>`).join('')
          || `<tr><td colspan="5" class="muted">${esc(t('ia_sin_servicios'))}</td></tr>`;
        // The devices that keep their detail to themselves: their services are not listed, and
        // the page says how many devices that is (review of 10 Oct 2026, panel 2).
        $('ia-ocultos').textContent = r.ocultos ? tn('ia_ocultos', r.ocultos) : '';
        $('ia-ocultos').classList.toggle('hidden', !r.ocultos);
        // A device that does not share: what it asked is not read from here, so there are no
        // counts and no «nothing beyond the list», which would be a zero nobody counted (panel 3).
        const cifrasAlcance = (a) => (a.comparte
          ? `<p>${esc(tn('alcance_dentro', a.dentro_total))} · <b>${esc(tn('alcance_fuera', a.fuera_total))}</b></p>`
          : `<p class="limit">${esc(t('alcance_no_comparte'))}</p>`);
        const fueraAlcance = (a) => {
          if (!a.comparte) return '';
          return a.fuera.length ? `<div class="wrap"><table><tbody>${a.fuera.map((f) => `<tr><td class="mono">${esc(f[0])}</td><td>${num(f[1])}</td>`
            + `<td><button class="secondary a-anadir" data-device="${esc(a.device_id)}" data-name="${esc(f[0])}">${esc(t('alcance_anadir'))}</button></td></tr>`).join('')}</tbody></table></div>`
            : `<p class="ok">${esc(t('alcance_fuera_ninguno'))}</p>`;
        };
        $('alcances').innerHTML = r.alcances.map((a) => `<div class="card">
          <h3>${esc(nombreAparato(a.device_id, a.device_name))}</h3>
          <p class="mono muted">${a.patrones.map(esc).join(' · ')}</p>
          ${cifrasAlcance(a)}
          <p class="vigilante"><label><input type="checkbox" class="a-cortar" data-device="${esc(a.device_id)}"${a.cortar ? ' checked' : ''}> <b>${esc(t('alcance_cortar'))}</b></label>
            <br><span class="muted">${esc(t(a.cortar ? 'alcance_cortar_on' : 'alcance_cortar_off'))}</span>
            <br><span class="muted">${esc(t('alcance_cortar_todo'))}</span></p>
          ${a.pase_hasta ? `<p class="ok">${esc(t('alcance_pase_activo').replace('{hora}', when(a.pase_hasta)))}</p>`
            : (a.cortar ? `<p><button class="secondary a-pase" data-device="${esc(a.device_id)}" data-horas="4">${esc(t('alcance_pase').replace('{h}', 4))}</button></p>` : '')}
          ${fueraAlcance(a)}
          <p class="limit">${esc(t('alcance_esperado_nota'))}</p>
          ${a.cortar ? `<p class="limit">${esc(t('alcance_cortado_aviso'))}</p>` : ''}</div>`).join('');
        const opts = r.alcances.map((a) => [a.device_id, a.device_name, a.patrones.join('\n')])
          .concat(r.sin_alcance.map((d) => [d[0], d[1], '']));
        const sel = $('a-device');
        const keep = sel.value;
        sel.innerHTML = opts.map(([id, name]) => `<option value="${esc(id)}">${esc(nombreAparato(id, name))}${opts.find((o) => o[0] === id)[2] ? '' : ' · ' + t('alcance_sin_declarar')}</option>`).join('');
        if (keep && opts.some((o) => o[0] === keep)) sel.value = keep;
        const current = opts.find((o) => o[0] === sel.value);
        $('a-patrones').value = current ? current[2] : '';
        sel.onchange = () => { const c = opts.find((o) => o[0] === sel.value); $('a-patrones').value = c ? c[2] : ''; marcaRapidos(); };
        marcaRapidos();
      };
      // The usual AIs, one tap each (the owner, 16 Oct 2026: «hazlo super fácil»): a chip adds its
      // services to the list below, or takes them out; the list stays the source of truth.
      const lineas = () => $('a-patrones').value.split(/\n+/).map((x) => x.trim().toLowerCase()).filter(Boolean);
      const marcaRapidos = () => {
        const hay = new Set(lineas());
        document.querySelectorAll('#a-rapidos .chip').forEach((c) => c.setAttribute('aria-pressed', String(c.dataset.dominios.split(' ').every((d) => hay.has(d)))));
      };
      $('a-rapidos').addEventListener('click', (e) => {
        const c = e.target.closest('.chip'); if (!c) return;
        const ds = c.dataset.dominios.split(' ');
        let ls = lineas();
        if (c.getAttribute('aria-pressed') === 'true') ls = ls.filter((x) => !ds.includes(x));
        else ds.forEach((d) => { if (!ls.includes(d)) ls.push(d); });
        $('a-patrones').value = ls.join('\n');
        marcaRapidos();
      });
      $('a-patrones').addEventListener('input', marcaRapidos);
      paint(await api('/api/ia'));
      $('alcance-form').addEventListener('submit', async (e) => {
        e.preventDefault();
        const patrones = $('a-patrones').value;
        let r;
        try {
          r = await api('/api/ia/alcance', { method: 'POST', body: { device_id: $('a-device').value, patrones } });
        } catch (err) {
          // A line that is no name is said, and nothing is saved: the person fixes it and saves.
          $('a-msg').textContent = String(err.message || err);
          return;
        }
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
        // A device that does not share its detail has no last day to read from here: the warning
        // says so instead of «0 names would have been blocked» (review of 10 Oct 2026, panel 3).
        if (c.checked && yo) {
          const ejemplos = (yo.fuera || []).slice(0, 4).map((f) => f[0]).join(', ') || '—';
          const aviso = yo.comparte
            ? tn('alcance_cortar_aviso', yo.fuera_total).replace('{ejemplos}', ejemplos)
            : t('alcance_cortar_aviso_sin_detalle');
          if (!confirm(aviso)) { c.checked = false; return; }
        }
        try {
          const r = await api('/api/ia/alcance', {
            method: 'POST',
            body: { device_id: dev, patrones: (yo ? yo.patrones : []).join('\n'), modo: c.checked ? 'cortar' : 'observar' },
          });
          paint(r);
        } catch (err) {
          // El servidor no deja encenderlo antes de las 24 horas: se dice por qué y el
          // interruptor vuelve solo a su sitio, en vez de quedarse encendido sin serlo.
          c.checked = false;
          $('a-msg').textContent = String(err.message || err);
          setTimeout(() => { $('a-msg').textContent = ''; }, 8000);
        }
      });
      // A refused pass or a failed «let it through» says why, and the button comes back: it used
      // to stay grey for good (review of 10 Oct 2026, panel 14).
      $('alcances').addEventListener('click', async (e) => {
        const b = e.target.closest('button.a-pase, button.a-anadir');
        if (!b) return;
        b.disabled = true;
        try {
          if (b.classList.contains('a-pase')) {
            const actual = await api('/api/ia');
            const yo = (actual.alcances || []).find((x) => x.device_id === b.dataset.device);
            paint(await api('/api/ia/alcance', {
              method: 'POST',
              body: { device_id: b.dataset.device, patrones: (yo ? yo.patrones : []).join('\n'), pase_horas: Number(b.dataset.horas) },
            }));
          } else {
            paint(await api('/api/ia/alcance/anadir', { method: 'POST', body: { device_id: b.dataset.device, nombre: b.dataset.name } }));
          }
        } catch (err) {
          $('a-msg').textContent = String(err.message || err);
          setTimeout(() => { $('a-msg').textContent = ''; }, 8000);
        } finally {
          b.disabled = false;
        }
      });
    },
    // A company's profile (the owner, 12 Oct 2026: «que quede todo perfecto, igual que GUARDIANA
    // ZERO»): what the lists say of it, its checked profile with every fact's public source, what
    // the devices of this house asked of it according to the extract, and the letter that asks it
    // to delete the person's data, as a PDF to attach or written in the person's own mail.
    async empresa() {
      const pedida = new URLSearchParams(location.search).get('e') || '';
      $('f-volver').addEventListener('click', (e) => { if (history.length > 1) { e.preventDefault(); history.back(); } });
      let d;
      try {
        d = await api('/api/empresa?' + new URLSearchParams({ e: pedida }).toString());
      } catch (err) {
        if (!/sesi/.test(String(err.message))) $('f-no').classList.remove('hidden');
        return;
      }
      const f = d.ficha || null;
      const fx = (k, o) => Object.entries(o || {}).reduce((s, [a, b]) => s.split('{' + a + '}').join(String(b)), t(k));
      const fn = (k, n, o) => fx(n === 1 && T.panel && T.panel[k + '_uno'] ? k + '_uno' : k, Object.assign({ n: num(n) }, o));
      const L = (o) => (o && typeof o === 'object' ? o[LANG] || o.es || o.en || o.pt || '' : String(o || ''));
      const web = (u) => typeof u === 'string' && /^https:\/\//.test(u);
      const enlace = (u, texto) => (web(u) ? `<a href="${esc(u)}" target="_blank" rel="noopener noreferrer">${esc(texto || u)}</a>` : '');
      let nombrePais = (c) => c || '';
      try { const dn = new Intl.DisplayNames([LOC], { type: 'region' }); nombrePais = (c) => { try { return dn.of(c) || c; } catch (_) { return c; } }; } catch (_) {}
      // Money as people say it: «1.930 millones de USD». Never converted: a rate would be ours.
      const dinero = (valor, moneda) => {
        const v = Number(valor);
        if (!Number.isFinite(v)) return '';
        const m = String(moneda || '').toUpperCase().slice(0, 3);
        if (Math.abs(v) === 1e6) return fx('ficha_millon', { moneda: m });
        if (Math.abs(v) >= 1e6) return fx('ficha_millones', { n: (v / 1e6).toLocaleString(LOC, { maximumFractionDigits: Math.abs(v) >= 1e10 ? 0 : 1, useGrouping: 'always' }), moneda: m });
        try { return v.toLocaleString(LOC, { style: 'currency', currency: m, maximumFractionDigits: 0 }); } catch (_) { return num(v) + ' ' + m; }
      };
      const fecha = (x) => {
        const s = String(x || '');
        if (/^\d{4}-\d{2}-\d{2}$/.test(s)) return new Date(s + 'T12:00:00').toLocaleDateString(LOC, { day: 'numeric', month: 'long', year: 'numeric' });
        if (/^\d{4}-\d{2}$/.test(s)) return new Date(s + '-15T12:00:00').toLocaleDateString(LOC, { month: 'long', year: 'numeric' });
        return s;
      };
      const dia = (ms) => new Date(ms).toLocaleDateString(LOC, DIA);
      // A fine as imposed; cut on appeal, the amount that stands; annulled, said so.
      const importe = (x) => {
        if (!x.importe) return { puesta: '', queda: '' };
        const puesta = dinero(x.importe, x.moneda);
        if (x.vigente === false) return { puesta, queda: t('ficha_anulada') };
        if (x.importe_final) return { puesta, queda: dinero(x.importe_final, x.moneda) };
        return { puesta: '', queda: puesta };
      };
      // The fines that stand, added up by currency; an annulled one does not count.
      const totales = (lista) => {
        const suma = new Map();
        for (const x of lista) {
          if (x.vigente === false) continue;
          const v = Number(x.importe_final || x.importe);
          if (!Number.isFinite(v) || !v || !x.moneda) continue;
          suma.set(x.moneda, (suma.get(x.moneda) || 0) + v);
        }
        return suma.size ? fx('ficha_total_sanciones', { total: Array.from(suma, ([m, v]) => dinero(v, m)).join(' + ') }) : '';
      };

      // What the page says, once, for the screen and for the PDF: every source numbered in the
      // order it is first cited.
      const fuentes = [];
      const cita = (u) => { if (!web(u)) return 0; let i = fuentes.indexOf(u); if (i < 0) { fuentes.push(u); i = fuentes.length - 1; } return i + 1; };
      const sede = [d.ciudad, d.pais_por_dominio ? fx('pais_dominio_texto', { pais: d.pais }) : d.pais].filter(Boolean).join(', ');
      const sanciones = f && Array.isArray(f.sanciones) ? f.sanciones : [];
      const filtraciones = f && Array.isArray(f.filtraciones) ? f.filtraciones : [];
      const bloques = [];
      if (f) {
        if (f.que_hace) bloques.push({ titulo: t('ficha_que_hace'), textos: [[L(f.que_hace), 0], [L(f.como_gana), cita(f.fuente_que_hace)]].filter((x) => x[0]) });
        if (f.datos) bloques.push({ titulo: t('ficha_datos'), textos: [[L(f.datos), cita(f.fuente_datos)]] });
        for (const [k, titulo] of [['como_usa_datos', 'ficha_como_usa'], ['sociedad', 'ficha_sociedad'], ['fiscal', 'ficha_fiscal'], ['monopolio', 'ficha_monopolio']]) {
          if (f[k]) bloques.push({ titulo: t(titulo), textos: [[L(f[k]), cita(f['fuente_' + k])]] });
        }
        const cifras = Array.isArray(f.cifras) ? f.cifras : [];
        if (cifras.length) {
          // Whose figures they are: a group's results (Alphabet's, on Google's profile) say so in
          // the heading when all of them are the group's, and beside each one when they mix.
          const de = (x) => (typeof x.de === 'string' ? x.de.trim() : '');
          const todasDe = new Set(cifras.map(de)).size === 1 ? de(cifras[0]) : '';
          bloques.push({ titulo: todasDe ? fx('ficha_dinero_de', { entidad: todasDe }) : t('ficha_dinero'), cifras: cifras.map((x) => ({ valor: x.moneda ? dinero(x.valor, x.moneda) : Number(x.valor || 0).toLocaleString(LOC, { useGrouping: 'always' }), que: fx('ficha_cifra_' + x.clave, { anio: String(x.anio || '') }) + (!todasDe && de(x) ? ' · ' + fx('ficha_cifra_de', { entidad: de(x) }) : ''), n: cita(x.fuente) })) });
        }
        // An old fine is what an authority decided then, not proof of what the company does now.
        bloques.push({ titulo: t('ficha_sanciones'), nota: t('ficha_sanciones_nota'), total: totales(sanciones), vacio: fx('ficha_sin_sanciones', { fecha: fecha(d.revisada) }),
          filas: sanciones.map((x) => Object.assign({ cab: [x.autoridad || '', x.pais ? nombrePais(x.pais) : '', fecha(x.fecha)].filter(Boolean).join(' · '), texto: [L(x.motivo), L(x.estado)].filter(Boolean).join(' '), n: cita(x.fuente), roja: true }, importe(x))) });
        bloques.push({ titulo: t('ficha_filtraciones'), vacio: fx('ficha_sin_filtraciones', { fecha: fecha(d.revisada) }),
          filas: filtraciones.map((x) => ({ cab: fecha(x.fecha), queda: x.afectados ? fn('ficha_afectados', Number(x.afectados)) : '', puesta: '', texto: L(x.que), n: cita(x.fuente) })) });
        if (web(f.web)) bloques.push({ titulo: t('ficha_web'), enlace: f.web });
      } else {
        bloques.push({ titulo: '', textos: [[fx(d.conocida ? 'ficha_pendiente' : 'ficha_pendiente_desconocida', { empresa: d.empresa }), 0]], vaga: true });
      }
      const borrarPagina = (f && web(f.borrar) && f.borrar) || (d.corredor && web(d.corredor.url) && d.corredor.url) || '';
      cita(borrarPagina);

      // ----- the screen -----
      document.title = d.empresa + ' · GUARDIANA';
      $('f-nombre').textContent = d.empresa;
      const partes = [];
      if (d.pais_por_dominio) partes.push(`<b>${esc(sede)}</b>`);
      else if (sede) partes.push(`${esc(t('ficha_sede'))}: <b>${esc(sede)}</b>`);
      if (d.grupo) partes.push(`${esc(t('ficha_grupo'))}: ${d.grupo_conocido ? empresaLink(d.grupo) : `<b>${esc(d.grupo)}</b>`}`);
      if (f && f.fundada) partes.push(`${esc(t('ficha_fundada'))}: <b>${esc(String(f.fundada))}</b>`);
      if (f && f.bolsa) partes.push(`${esc(t('ficha_bolsa'))}: <b>${esc(f.bolsa)}</b>`);
      if (!d.conocida && !d.pais_por_dominio) partes.push(esc(t('ficha_no_identificada')));
      $('f-sede').innerHTML = partes.join(' · ');
      if (d.pais_por_dominio) $('f-sede').title = fx('pais_dominio_titulo', { pais: d.pais });
      const chips = [];
      if (d.corredor) chips.push(`<span class="tag corredor-chip" title="${esc(fx('ficha_corredor_titulo', { nombre: d.corredor.nombre }))}">${esc(t('corredor_etiqueta'))}</span>`);
      if (sanciones.length) chips.push(`<span class="tag rastreador">${esc(fn('ficha_n_sanciones', sanciones.length))}</span>`);
      if (filtraciones.length) chips.push(`<span class="tag publicidad">${esc(fn('ficha_n_filtraciones', filtraciones.length))}</span>`);
      $('f-chips').innerHTML = chips.join(' ');
      $('f-oficio').textContent = (!f || !f.que_hace) && d.oficio ? d.oficio : '';
      $('f-oficio').classList.toggle('hidden', !$('f-oficio').textContent);

      const c = d.casa;
      $('f-consultas').textContent = num(c.consultas);
      $('f-cortadas').textContent = num(c.cortadas);
      $('f-aparatos').textContent = num(c.aparatos.length);
      const aparatos = c.aparatos.map((a) => nombreAparato(a[0], a[1])).join(', ');
      const callados = c.callados ? ' ' + fn('ficha_casa_callados', c.callados) : '';
      $('f-casa-texto').textContent = (c.consultas
        ? (dia(c.primera) === dia(c.ultima) ? fx('ficha_casa_periodo_dia', { dia: dia(c.primera), aparatos }) : fx('ficha_casa_periodo', { desde: dia(c.primera), hasta: dia(c.ultima), aparatos }))
        : t('ficha_casa_nada')) + callados;
      $('f-nombres-det').classList.toggle('hidden', !c.nombres.length);
      $('f-nombres-titulo').textContent = fn('ficha_casa_ver_nombres', c.n_nombres);
      $('f-nombres').innerHTML = c.nombres.map((x) => `<tr><td class="mono">${esc(x.nombre)}</td><td>${esc(fn('ficha_casa_n_consultas', x.consultas))}${x.cortadas ? ` <span class="muted">· ${esc(fn('ficha_casa_n_cortadas', x.cortadas))}</span>` : ''}</td><td class="muted">${esc(dia(x.primera))} – ${esc(dia(x.ultima))}</td></tr>`).join('')
        + (c.n_nombres > c.nombres.length ? `<tr><td colspan="3" class="muted">${esc(fx('ficha_y_mas', { n: num(c.n_nombres - c.nombres.length) }))}</td></tr>` : '');

      const sup = (n) => (n ? ` <sup class="f-cita"><a href="${esc(fuentes[n - 1])}" target="_blank" rel="noopener noreferrer">[${n}]</a></sup>` : '');
      $('f-cuerpo').innerHTML = bloques.map((b) => {
        let h = b.titulo ? `<h2>${esc(b.titulo)}</h2>` : '';
        if (b.textos) h += b.textos.map(([x, n]) => `<p${b.vaga ? ' class="f-vacia"' : ''}>${esc(x)}${sup(n)}</p>`).join('');
        if (b.cifras) h += `<div class="counters f-dinero">${b.cifras.map((x) => `<div class="counter"><div class="n">${esc(x.valor)}</div><div class="l">${esc(x.que)}${sup(x.n)}</div></div>`).join('')}</div>`;
        if (b.filas) {
          h += b.filas.length
            ? (b.nota ? `<p class="muted">${esc(b.nota)}</p>` : '') + (b.total ? `<p class="f-total">${esc(b.total)}</p>` : '') + `<ul class="f-lista">${b.filas.map((x) => `<li class="${x.roja ? 'roja' : 'ambar'}"><div class="f-l1"><span>${esc(x.cab)}</span><span class="f-importe">${x.puesta ? `<s>${esc(x.puesta)}</s> ` : ''}${esc(x.queda)}</span></div><div class="f-l2">${esc(x.texto)}${sup(x.n)}</div></li>`).join('')}</ul>`
            : `<p class="f-vacia">${esc(b.vacio)}</p>`;
        }
        if (b.enlace) h += `<p>${enlace(b.enlace)}</p>`;
        return `<section class="f-seccion">${h}</section>`;
      }).join('');
      if (borrarPagina) {
        $('f-borrar-pagina').innerHTML = `${esc(t('ficha_borrar_explica'))} ${enlace(borrarPagina, t('ficha_borrar_enlace'))}`;
        $('f-borrar-pagina').classList.remove('hidden');
      }
      $('f-s-dominios').classList.toggle('hidden', !d.dominios.length);
      $('f-dominios').textContent = d.dominios.join(' · ') + (d.n_dominios > d.dominios.length ? ' · ' + fx('ficha_y_mas', { n: num(d.n_dominios - d.dominios.length) }) : '');
      const pintaFuentes = () => {
        $('f-s-fuentes').classList.toggle('hidden', !fuentes.length);
        $('f-fuentes').innerHTML = fuentes.map((u) => `<li>${enlace(u)}</li>`).join('');
      };
      pintaFuentes();
      $('f-nota').textContent = f && d.revisada ? fx('ficha_nota', { fecha: fecha(d.revisada) }) : t('ficha_nota_sin');
      $('f-todo').classList.remove('hidden');

      // ----- the profile as a PDF -----
      const slug = (s) => String(s).normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '').slice(0, 60) || 'empresa';
      savePdf('f-pdf', () => {
        const doc = pdfDocument(d.empresa + ' · ' + t('ficha_titulo'));
        pdfHead(doc, t('ficha_titulo'));
        doc.line(d.empresa, 20, true).gap(4);
        if (sede || d.grupo) doc.line([sede ? t('ficha_sede') + ': ' + sede : '', d.grupo ? t('ficha_grupo') + ': ' + d.grupo : '', f && f.fundada ? t('ficha_fundada') + ': ' + f.fundada : '', f && f.bolsa ? t('ficha_bolsa') + ': ' + f.bolsa : ''].filter(Boolean).join(' · '), 10, false, 0.3);
        const etiquetas = [d.corredor ? t('corredor_etiqueta') : '', sanciones.length ? fn('ficha_n_sanciones', sanciones.length) : '', filtraciones.length ? fn('ficha_n_filtraciones', filtraciones.length) : ''].filter(Boolean);
        if (etiquetas.length) doc.gap(4).line(etiquetas.join(' · '), 10, true);
        if ((!f || !f.que_hace) && d.oficio) doc.gap(6).line(d.oficio, 10);
        doc.seccion(t('ficha_casa_titulo'));
        doc.line([fn('ficha_pdf_consultas', c.consultas), fn('ficha_pdf_cortadas', c.cortadas), fn('ficha_pdf_aparatos', c.aparatos.length)].join(' · '), 10, true);
        doc.gap(2).line($('f-casa-texto').textContent, 9, false, 0.35);
        const n = (k) => (k ? ' [' + k + ']' : '');
        for (const b of bloques) {
          if (b.titulo) doc.seccion(b.titulo);
          if (b.textos) for (const [x, k] of b.textos) doc.line(x + n(k), 10).gap(3);
          if (b.cifras) doc.table([{ title: t('ficha_pdf_concepto'), w: 0.6 }, { title: t('ficha_pdf_cifra'), w: 0.4 }], b.cifras.map((x) => [x.que + n(x.n), x.valor]), 9);
          if (b.filas) {
            if (!b.filas.length) doc.line(b.vacio, 10, false, 0.35);
            else {
              if (b.nota) doc.line(b.nota, 9, false, 0.35).gap(4);
              if (b.total) doc.line(b.total, 10, true).gap(4);
              doc.table([{ title: t('ficha_pdf_cuando'), w: 0.3 }, { title: t('ficha_pdf_importe'), w: 0.18 }, { title: t('ficha_pdf_que'), w: 0.52 }],
                b.filas.map((x) => [x.cab, (x.puesta ? x.puesta + ' -> ' : '') + x.queda, x.texto + n(x.n)]), 8);
            }
          }
          if (b.enlace) doc.line(b.enlace, 10);
        }
        if (borrarPagina) doc.seccion(t('ficha_borrar')).line(t('ficha_borrar_explica') + ' ' + borrarPagina, 10);
        if (d.dominios.length) doc.seccion(t('ficha_dominios')).line($('f-dominios').textContent, 8, false, 0.3);
        if (fuentes.length) { doc.seccion(t('ficha_fuentes')); fuentes.forEach((u, i) => doc.line((i + 1) + '. ' + u, 8, false, 0.3)); }
        doc.gap(10).line($('f-nota').textContent, 8, false, 0.45);
        return { name: 'guardiana-' + slug(d.empresa) + '.pdf', doc };
      });

      // ----- share: a few lines anyone can read, and the public page when there is one -----
      const enlacePublico = () => {
        const s = f && typeof f.slug === 'string' && /^[a-z0-9-]+$/.test(f.slug) ? f.slug : '';
        const pre = LANG === 'es' ? '' : LANG + '/';
        return s ? `https://guardianagroup.com/${pre}empresas/${s}.html` : 'https://guardianagroup.com/' + (LANG === 'es' ? '' : LANG + '/');
      };
      const copia = async (s) => {
        try { await navigator.clipboard.writeText(s); return true; } catch (_) {}
        const area = document.createElement('textarea');
        area.value = s; area.setAttribute('readonly', ''); area.style.position = 'fixed'; area.style.left = '-9999px';
        document.body.appendChild(area); area.select();
        let ok = false;
        try { ok = document.execCommand('copy'); } catch (_) {}
        area.remove();
        return ok;
      };
      $('f-compartir').addEventListener('click', async () => {
        const lineas = [d.empresa + (sede ? ` (${sede})` : '')];
        if (f && f.que_hace) lineas.push(L(f.que_hace));
        for (const x of sanciones.slice(0, 3)) lineas.push('• ' + [x.autoridad, fecha(x.fecha), x.importe ? dinero(x.importe, x.moneda) : ''].filter(Boolean).join(' · ') + ': ' + L(x.motivo));
        for (const x of filtraciones.slice(0, 2)) lineas.push('• ' + fecha(x.fecha) + ': ' + L(x.que));
        if (c.consultas) lineas.push(fn('ficha_compartir_casa', c.consultas, { c: num(c.cortadas) }));
        const texto = lineas.join('\n');
        const url = enlacePublico();
        if (navigator.share) {
          try { await navigator.share({ title: d.empresa, text: texto, url }); return; } catch (e) { if (e && e.name === 'AbortError') return; }
        }
        $('f-pdf-msg').textContent = t((await copia(texto + '\n' + url)) ? 'ficha_copiado' : 'ficha_no_copiado');
        setTimeout(() => { $('f-pdf-msg').textContent = ''; }, 6000);
      });

      // ----- the letter -----
      // The places come with the profile: «Haz que te borren»'s summary would read the whole
      // statement again only for this list (review of 12 Oct 2026).
      const R = { jurisdicciones: d.jurisdicciones || [] };
      if (!R.jurisdicciones.length) { $('f-borrar').classList.add('hidden'); return; }
      const RECUERDO = 'guardiana_borrado_datos';
      const MAILTO_MAX = 2000;
      const sel = $('f-pais');
      sel.innerHTML = R.jurisdicciones.map((j) => `<option value="${esc(j.codigo)}">${esc(j.nombre)}</option>`).join('');
      const partesIdioma = (navigator.language || '').split('-');
      const region = (partesIdioma[1] || '').toUpperCase();
      const jPorDefecto = (region === 'CA' && (partesIdioma[0] || '').toLowerCase() === 'fr') ? 'ca_qc'
        : ((R.jurisdicciones.find((x) => x.regiones.includes(region)) || {}).codigo || (LANG === 'pt' ? 'br' : (LANG === 'en' ? 'us' : 'es')));
      sel.value = jPorDefecto;
      const jur = () => R.jurisdicciones.find((j) => j.codigo === sel.value) || R.jurisdicciones[0];
      let guardado = null;
      try { guardado = JSON.parse(localStorage.getItem(RECUERDO) || 'null'); } catch (_) {}
      if (guardado && typeof guardado === 'object') { $('f-cnombre').value = String(guardado.nombre || ''); $('f-ccorreo').value = String(guardado.correo || ''); }
      // Kept in this browser only when the person already asked for it on «Haz que te borren».
      const recuerda = () => { if (!guardado) return; try { localStorage.setItem(RECUERDO, JSON.stringify({ nombre: $('f-cnombre').value, correo: $('f-ccorreo').value })); } catch (_) {} };
      let C = null;
      // One request per law and choice of evidence: each one reads the statement.
      const cartas = new Map();
      const pide = (evidencia) => {
        const k = sel.value + '|' + $('f-tipo').value + '|' + (evidencia ? 1 : 0);
        if (!cartas.has(k)) cartas.set(k, api('/api/borrado/carta?' + new URLSearchParams({ empresa: d.empresa, jurisdiccion: sel.value, idioma: jur().idioma, evidencia: evidencia ? '1' : '0', tipo: $('f-tipo').value }).toString()).catch((e) => { cartas.delete(k); throw e; }));
        return cartas.get(k);
      };
      const rellena = (cuerpo, carta) => cuerpo.split('[[NOMBRE]]').join($('f-cnombre').value.trim() || carta.hueco_nombre).split('[[CORREO]]').join($('f-ccorreo').value.trim() || carta.hueco_correo);
      const pinta = () => {
        if (!C) return;
        const j = jur();
        $('f-ley').innerHTML = (j.derecho !== 'borrado' ? `<span class="bad">${esc(t('borrado_sin_derecho_' + j.derecho))}</span><br>` : '') + esc($('f-tipo').value === 'acceso' ? j.corta_acceso : j.corta);
        $('f-asunto').textContent = C.asunto;
        $('f-texto').value = rellena(C.cuerpo, C);
        $('f-canal').innerHTML = C.para ? esc(t('borrado_para').replace('{correo}', C.para))
          : C.url ? `${esc(t('ficha_carta_formulario'))} ${enlace(C.url)}` : esc(t('ficha_carta_sin_correo'));
      };
      const carga = async () => { C = await pide($('f-evidencia').checked); pinta(); };
      if (d.borrado) {
        $('f-en-borrado').innerHTML = `${esc(t('ficha_carta_en_borrado'))} <a href="/borrado">${esc(t('nav_borrado'))}</a>`;
        $('f-en-borrado').classList.remove('hidden');
      }
      const avisa = (k, o) => { $('f-carta-msg').textContent = fx(k, o); setTimeout(() => { $('f-carta-msg').textContent = ''; }, 8000); };
      const faltan = () => {
        const vacio = !$('f-cnombre').value.trim() ? $('f-cnombre') : (!$('f-ccorreo').value.trim() ? $('f-ccorreo') : null);
        if (!vacio) return false;
        avisa('borrado_faltan_datos');
        vacio.focus();
        return true;
      };
      $('f-carta-pdf').addEventListener('click', async () => {
        if (faltan()) return;
        // The PDF writes Latin letters only: a name it cannot write at all goes by mail or copy;
        // one it writes without its accents is said (review of 12 Oct 2026).
        const quien = $('f-cnombre').value.trim() + ' ' + $('f-ccorreo').value.trim();
        const faltan2 = faltaEnPdf(quien);
        if (faltan2.length) { avisa('ficha_carta_pdf_no', { letras: faltan2.join(' ') }); return; }
        try {
          await carga();
          const doc = pdfDocument(C.asunto);
          if (C.para) doc.line(t('ficha_carta_pdf_para').replace('{correo}', C.para), 9, false, 0.4).gap(10);
          doc.line(C.asunto, 12, true).gap(12);
          for (const l of rellena(C.cuerpo, C).split('\n')) { if (l.trim()) doc.line(l, 10.5); else doc.gap(7); }
          const name = 'carta-borrado-' + slug(d.empresa) + '.pdf';
          const r = await savePdfFile(name, doc.bytes(''));
          if (r !== 'cancelado') {
            avisa('pdf_' + r, { nombre: name });
            if (cambiaEnPdf(quien)) setTimeout(() => avisa('ficha_carta_pdf_cambia', { nombre: winAnsiLegible($('f-cnombre').value.trim()) }), 50);
          }
        } catch (err) { $('f-carta-msg').textContent = String(err.message || err); }
      });
      $('f-carta-correo').addEventListener('click', async () => {
        if (faltan()) return;
        try {
          await carga();
          const para = C.para || '';
          const mailto = (asunto, cuerpo) => 'mailto:' + para + '?subject=' + encodeURIComponent(asunto) + '&body=' + encodeURIComponent(cuerpo);
          let url = mailto(C.asunto, rellena(C.cuerpo, C));
          let pista = para ? 'borrado_pista_correo' : 'ficha_carta_pista_sin_para';
          if (url.length > MAILTO_MAX && $('f-evidencia').checked) {
            const sin = await pide(false);
            url = mailto(sin.asunto, rellena(sin.cuerpo, sin));
            pista = pista + '|borrado_pista_sin_evidencia';
          }
          if (url.length > MAILTO_MAX) {
            await copia(rellena(C.cuerpo, C));
            url = mailto(C.asunto, t('borrado_mailto_pega'));
            pista = 'borrado_pista_pegar_correo';
          }
          location.href = url;
          $('f-carta-msg').textContent = pista.split('|').map((k) => t(k)).join(' ');
        } catch (err) { $('f-carta-msg').textContent = String(err.message || err); }
      });
      $('f-carta-copiar').addEventListener('click', async () => {
        try { await carga(); avisa((await copia(C.asunto + '\n\n' + rellena(C.cuerpo, C))) ? 'borrado_copiado' : 'ficha_no_copiado'); } catch (err) { $('f-carta-msg').textContent = String(err.message || err); }
      });
      sel.addEventListener('change', () => { carga().catch(() => {}); });
      $('f-tipo').addEventListener('change', () => { carga().catch(() => {}); });
      $('f-evidencia').addEventListener('change', () => { carga().catch(() => {}); });
      // The request becomes a case file, prepared: its clock starts when the person says it went.
      $('f-expediente').addEventListener('click', async () => {
        try {
          const cuerpo = { jurisdiccion: sel.value, tipo: $('f-tipo').value, estado: 'preparada' };
          if (d.borrado) cuerpo.corredor = d.borrado; else cuerpo.empresa = d.empresa;
          await api('/api/expedientes', { method: 'POST', body: cuerpo });
          $('f-carta-msg').innerHTML = `${esc(t('exp_abierto_ok'))} <a href="/expedientes">${esc(t('exp_aviso_ver'))}</a>`;
        } catch (err) { $('f-carta-msg').textContent = String(err.message || err); }
      });
      ['f-cnombre', 'f-ccorreo'].forEach((id) => $(id).addEventListener('input', () => { pinta(); recuerda(); }));
      $('f-carta-form').addEventListener('submit', (e) => e.preventDefault());
      await carga().catch((e) => { $('f-carta-msg').textContent = String(e.message || e); });
    },
    // «Expedientes» (the owner, 12 Oct 2026): every request made to a company, with its legal
    // deadline, the answer and, once overdue, the complaint to the authority. Only the company,
    // the place, the kind, the dates, a result and a note are kept; the name and the e-mail live
    // in this page's inputs (and in this browser only if «Haz que te borren» was told to remember).
    async expedientes() {
      let R = await api('/api/expedientes');
      const RECUERDO = 'guardiana_borrado_datos';
      const MAILTO_MAX = 2000;
      const fx = (k, o) => Object.entries(o || {}).reduce((s, [a, b]) => s.split('{' + a + '}').join(String(b)), t(k));
      const enlace = (url, texto) => (/^https:\/\//.test(String(url || '')) ? `<a href="${esc(url)}" target="_blank" rel="noopener noreferrer">${esc(texto || url)}</a>` : '');
      let filtro = 'todos';
      // The place: from the browser's region, as on «Haz que te borren».
      const sel = $('x-pais');
      sel.innerHTML = R.jurisdicciones.map((j) => `<option value="${esc(j.codigo)}">${esc(j.nombre)}</option>`).join('');
      const partes = (navigator.language || '').split('-');
      const region = (partes[1] || '').toUpperCase();
      sel.value = (region === 'CA' && (partes[0] || '').toLowerCase() === 'fr') ? 'ca_qc'
        : ((R.jurisdicciones.find((x) => x.regiones.includes(region)) || {}).codigo || (LANG === 'pt' ? 'br' : (LANG === 'en' ? 'us' : 'es')));
      const jur = (c) => R.jurisdicciones.find((j) => j.codigo === c) || R.jurisdicciones[0];
      const pintaLey = () => {
        const j = jur(sel.value);
        $('x-ley').innerHTML = (j.derecho !== 'borrado' ? `<span class="bad">${esc(t('borrado_sin_derecho_' + j.derecho))}</span><br>` : '') + esc($('x-tipo').value === 'acceso' ? j.corta_acceso : j.corta);
      };
      // The companies to pick from: those the house asked for, then those of «Haz que te borren».
      const opciones = [];
      const vistos = new Set();
      for (const e of R.vistas.concat(R.corredores)) { if (!vistos.has(e.empresa)) { vistos.add(e.empresa); opciones.push(e); } }
      $('x-empresas').innerHTML = opciones.map((e) => `<option value="${esc(e.empresa)}">${e.consultas ? esc(tn('ficha_casa_n_consultas', e.consultas).replace(String(e.consultas), num(e.consultas))) : ''}</option>`).join('');
      $('x-fecha').max = R.hoy;
      $('x-fecha').value = R.hoy;
      $('x-estado').addEventListener('change', () => $('x-fecha-l').classList.toggle('hidden', $('x-estado').value !== 'enviada'));
      sel.addEventListener('change', pintaLey);
      $('x-tipo').addEventListener('change', pintaLey);
      pintaLey();
      // The person's details, for the letters: as on «Haz que te borren».
      try {
        const g = JSON.parse(localStorage.getItem(RECUERDO) || 'null');
        if (g && typeof g === 'object') { $('x-nombre').value = String(g.nombre || ''); $('x-correo').value = String(g.correo || ''); }
      } catch (_) {}
      const recuerda = () => { try { if (localStorage.getItem(RECUERDO)) localStorage.setItem(RECUERDO, JSON.stringify({ nombre: $('x-nombre').value, correo: $('x-correo').value })); } catch (_) {} };
      ['x-nombre', 'x-correo'].forEach((id) => $(id).addEventListener('input', recuerda));
      $('x-datos').addEventListener('submit', (e) => e.preventDefault());
      const rellena = (c) => c.cuerpo.split('[[NOMBRE]]').join($('x-nombre').value.trim() || c.hueco_nombre).split('[[CORREO]]').join($('x-correo').value.trim() || c.hueco_correo);
      const faltan = () => {
        if ($('x-nombre').value.trim() && $('x-correo').value.trim()) return false;
        $('x-datos-det').open = true;
        ($('x-nombre').value.trim() ? $('x-correo') : $('x-nombre')).focus();
        return true;
      };
      const copia = async (s) => {
        try { await navigator.clipboard.writeText(s); return true; } catch (_) {}
        const area = document.createElement('textarea');
        area.value = s; area.setAttribute('readonly', ''); area.style.position = 'fixed'; area.style.left = '-9999px';
        document.body.appendChild(area); area.select();
        let ok = false;
        try { ok = document.execCommand('copy'); } catch (_) {}
        area.remove();
        return ok;
      };
      const slug = (s) => String(s).normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '').slice(0, 50) || 'empresa';

      const abiertos = () => R.expedientes.filter((e) => e.estado === 'enviada').length;
      const pintaCifras = () => {
        $('x-abiertos').textContent = num(abiertos());
        $('x-vencidos').textContent = num(R.expedientes.filter((e) => e.vencido).length);
        $('x-resueltos').textContent = num(R.expedientes.filter((e) => e.estado === 'respondida' || e.estado === 'cerrada').length);
      };
      const pasa = (e) => filtro === 'todos' || (filtro === 'abiertos' && (e.estado === 'enviada' || e.estado === 'preparada'))
        || (filtro === 'vencidos' && e.vencido) || (filtro === 'cerrados' && (e.estado === 'respondida' || e.estado === 'cerrada'));
      const fechas = (e) => [fx('exp_creada', { fecha: e.creada_texto }), e.enviada_texto ? fx('exp_enviada_el', { fecha: e.enviada_texto }) : '', e.cerrada_texto ? fx('exp_cerrada_el', { fecha: e.cerrada_texto }) : ''].filter(Boolean).join(' · ');
      const botones = (e) => {
        const b = [];
        if (e.estado === 'preparada') b.push(`<button type="button" data-accion="enviada" data-id="${e.id}">${esc(t('exp_ya_envie'))}</button>`);
        if (e.estado === 'enviada') b.push(`<button type="button" data-accion="respondida" data-id="${e.id}">${esc(t('exp_respondieron'))}</button>`);
        if (e.estado !== 'cerrada') b.push(`<button type="button" class="secondary" data-accion="cerrada" data-id="${e.id}">${esc(t('exp_cerrar'))}</button>`);
        else b.push(`<button type="button" class="secondary" data-accion="reabrir" data-id="${e.id}">${esc(t('exp_reabrir'))}</button>`);
        b.push(`<button type="button" class="secondary" data-accion="quitar" data-id="${e.id}">${esc(t('exp_quitar'))}</button>`);
        return b.join(' ');
      };
      const pinta = () => {
        pintaCifras();
        const lista = R.expedientes.filter(pasa);
        $('x-vacio').classList.toggle('hidden', R.expedientes.length > 0);
        $('x-lista').innerHTML = lista.map((e) => `<div class="card x-exp${e.vencido ? ' vencido' : e.aviso ? ' pronto' : ''}" data-id="${e.id}">
          <div class="b-cab"><div><b>${empresaLink(e.empresa)}</b> <span class="tag empresa">${esc(e.tipo_texto)}</span> <span class="tag x-estado ${esc(e.estado)}">${esc(e.estado_texto)}</span>
            <div class="muted small">${esc(e.jurisdiccion_nombre)} · ${esc(fechas(e))}</div></div>
            <div class="b-acciones">${botones(e)}</div></div>
          <p><b>${esc(e.frase)}</b>${e.resultado_texto ? ` <span class="muted">· ${esc(e.resultado_texto)}</span>` : ''}</p>
          ${e.prorroga ? `<p class="muted">${esc(e.prorroga)}</p>` : ''}
          ${e.notas_plazo.map((n) => `<p class="muted small">${esc(n)}</p>`).join('')}
          <div class="x-paso hidden" data-id="${e.id}"></div>
          ${e.vencido ? `<div class="x-queja"><h3>${esc(t('exp_queja'))}</h3><p>${esc(t('exp_queja_explica'))}</p>
            ${e.queja ? `<p class="muted">${esc(e.queja)}</p>` : ''}
            ${e.autoridad || e.autoridad_url ? `<p>${esc(t('exp_queja_donde'))}: ${e.autoridad_url ? enlace(e.autoridad_url, e.autoridad || e.autoridad_url) : esc(e.autoridad)}</p>` : ''}
            <p class="b-acciones"><button type="button" data-doc="queja" data-que="pdf" data-id="${e.id}">${esc(t('ficha_carta_pdf'))}</button>
              <button type="button" class="secondary" data-doc="queja" data-que="correo" data-id="${e.id}">${esc(t('ficha_carta_correo'))}</button>
              <button type="button" class="secondary" data-doc="queja" data-que="copiar" data-id="${e.id}">${esc(t('borrado_copiar'))}</button></p></div>` : ''}
          <details class="b-plegable"><summary>${esc(t('exp_carta'))}</summary>
            <p class="muted">${esc(t('exp_carta_explica'))}</p>
            ${e.url ? `<p>${enlace(e.url, t('exp_pagina_empresa'))}</p>` : ''}
            <p class="b-acciones"><button type="button" data-doc="carta" data-que="pdf" data-id="${e.id}">${esc(t('ficha_carta_pdf'))}</button>
              <button type="button" class="secondary" data-doc="carta" data-que="correo" data-id="${e.id}">${esc(t('ficha_carta_correo'))}</button>
              <button type="button" class="secondary" data-doc="carta" data-que="copiar" data-id="${e.id}">${esc(t('borrado_copiar'))}</button></p></details>
          <details class="b-plegable"><summary>${esc(t('exp_notas'))}</summary>
            <textarea class="wide x-notas" rows="3" maxlength="600" data-id="${e.id}" placeholder="${esc(t('exp_notas_ph'))}">${esc(e.notas)}</textarea>
            <p class="b-acciones"><button type="button" class="secondary" data-accion="notas" data-id="${e.id}">${esc(t('exp_guardar_notas'))}</button></p></details>
          <p class="x-msg muted" role="status" aria-live="polite" data-id="${e.id}"></p>
        </div>`).join('');
      };
      const msg = (id, texto) => { const m = document.querySelector(`.x-msg[data-id="${id}"]`); if (m) { m.textContent = texto; setTimeout(() => { m.textContent = ''; }, 8000); } };
      const cambia = async (id, cuerpo) => { R.expedientes = await api('/api/expedientes/' + id, { method: 'POST', body: cuerpo }); pinta(); };
      // A step asks for its day (today by default) and, for an answer, what they answered.
      const pidePaso = (id, accion) => {
        const caja = document.querySelector(`.x-paso[data-id="${id}"]`);
        document.querySelectorAll('.x-paso').forEach((c) => { if (c !== caja) { c.classList.add('hidden'); c.innerHTML = ''; } });
        const resultados = accion === 'respondida' || accion === 'cerrada'
          ? `<label><span>${esc(t('exp_resultado_campo'))}</span><select class="x-res"><option value=""></option>${R.resultados.map(([k, v]) => `<option value="${esc(k)}">${esc(v)}</option>`).join('')}</select></label>` : '';
        caja.innerHTML = `<form class="filters x-paso-form" data-id="${id}" data-accion="${accion}">
          <label><span>${esc(t(accion === 'enviada' ? 'exp_campo_fecha' : 'exp_fecha_otro'))}</span><input type="date" class="x-dia" value="${esc(R.hoy)}" max="${esc(R.hoy)}"></label>
          ${resultados}
          <button type="submit">${esc(t('exp_confirmar'))}</button> <button type="button" class="secondary x-cancela">${esc(t('exp_cancelar'))}</button></form>`;
        caja.classList.remove('hidden');
      };
      const doc = async (id, que, como) => {
        const e = R.expedientes.find((x) => String(x.id) === String(id));
        if (!e || faltan()) return;
        const j = jur(e.jurisdiccion);
        const pide = (ev) => api(`/api/expedientes/${id}/${que}?` + new URLSearchParams({ idioma: j.idioma, evidencia: ev ? '1' : '0' }).toString());
        let C = await pide(true);
        const texto = rellena(C);
        if (como === 'pdf') {
          const quien = $('x-nombre').value.trim() + ' ' + $('x-correo').value.trim();
          const no = faltaEnPdf(quien);
          if (no.length) { msg(id, fx('ficha_carta_pdf_no', { letras: no.join(' ') })); return; }
          const d = pdfDocument(C.asunto);
          if (C.para) d.line(t('ficha_carta_pdf_para').replace('{correo}', C.para), 9, false, 0.4).gap(10);
          d.line(C.asunto, 12, true).gap(12);
          for (const l of texto.split('\n')) { if (l.trim()) d.line(l, 10.5); else d.gap(7); }
          const name = (que === 'queja' ? 'queja-' : 'carta-') + slug(e.empresa) + '.pdf';
          const r = await savePdfFile(name, d.bytes(''));
          if (r !== 'cancelado') msg(id, t('pdf_' + r).replace('{nombre}', name) + (cambiaEnPdf(quien) ? ' ' + fx('ficha_carta_pdf_cambia', { nombre: winAnsiLegible($('x-nombre').value.trim()) }) : ''));
        } else if (como === 'correo') {
          const mailto = (a, b) => 'mailto:' + (C.para || '') + '?subject=' + encodeURIComponent(a) + '&body=' + encodeURIComponent(b);
          let url = mailto(C.asunto, texto);
          const pistas = [C.para ? 'borrado_pista_correo' : 'ficha_carta_pista_sin_para'];
          if (url.length > MAILTO_MAX && que === 'carta') { C = await pide(false); url = mailto(C.asunto, rellena(C)); pistas.push('borrado_pista_sin_evidencia'); }
          if (url.length > MAILTO_MAX) { await copia(rellena(C)); url = mailto(C.asunto, t('borrado_mailto_pega')); pistas.splice(0, pistas.length, 'borrado_pista_pegar_correo'); }
          location.href = url;
          msg(id, pistas.map((k) => t(k)).join(' '));
        } else {
          msg(id, t((await copia(C.asunto + '\n\n' + texto)) ? 'borrado_copiado' : 'ficha_no_copiado'));
        }
      };
      const lista = $('x-lista');
      lista.addEventListener('click', async (ev) => {
        const b = ev.target.closest('button');
        if (!b) return;
        const id = b.dataset.id;
        try {
          if (b.classList.contains('x-cancela')) { const c = b.closest('.x-paso'); c.classList.add('hidden'); c.innerHTML = ''; return; }
          if (b.dataset.doc) { await doc(id, b.dataset.doc, b.dataset.que); return; }
          const a = b.dataset.accion;
          if (a === 'enviada' || a === 'respondida' || a === 'cerrada') pidePaso(id, a);
          else if (a === 'reabrir') await cambia(id, { estado: 'enviada' });
          else if (a === 'notas') { await cambia(id, { notas: document.querySelector(`.x-notas[data-id="${id}"]`).value }); msg(id, t('exp_guardado')); }
          else if (a === 'quitar') { if (b.dataset.seguro) { R.expedientes = await api('/api/expedientes/' + id + '/quitar', { method: 'POST' }); pinta(); } else { b.dataset.seguro = '1'; b.textContent = t('exp_quitar_seguro'); b.classList.add('danger'); } }
        } catch (err) { msg(id, String(err.message || err)); }
      });
      lista.addEventListener('submit', async (ev) => {
        const f = ev.target.closest('.x-paso-form');
        if (!f) return;
        ev.preventDefault();
        const cuerpo = { estado: f.dataset.accion, fecha: f.querySelector('.x-dia').value };
        const r = f.querySelector('.x-res');
        if (r && r.value) cuerpo.resultado = r.value;
        try { await cambia(f.dataset.id, cuerpo); } catch (err) { msg(f.dataset.id, String(err.message || err)); }
      });
      document.querySelectorAll('.x-filtros button').forEach((b) => b.addEventListener('click', () => {
        filtro = b.dataset.filtro;
        document.querySelectorAll('.x-filtros button').forEach((x) => x.setAttribute('aria-pressed', String(x === b)));
        pinta();
      }));
      $('x-nuevo').addEventListener('submit', async (ev) => {
        ev.preventDefault();
        $('x-msg').textContent = '';
        const nombre = $('x-empresa').value.trim();
        if (!nombre) { $('x-empresa').focus(); return; }
        const conocida = opciones.find((o) => o.empresa.toLowerCase() === nombre.toLowerCase());
        const cuerpo = { jurisdiccion: sel.value, tipo: $('x-tipo').value, estado: $('x-estado').value };
        if (conocida && conocida.corredor) cuerpo.corredor = conocida.corredor; else cuerpo.empresa = conocida ? conocida.empresa : nombre;
        if (cuerpo.estado === 'enviada') cuerpo.fecha = $('x-fecha').value;
        try {
          R.expedientes = await api('/api/expedientes', { method: 'POST', body: cuerpo });
          $('x-empresa').value = '';
          pinta();
        } catch (err) { $('x-msg').textContent = /unknown company/.test(String(err.message)) ? t('exp_empresa_desconocida') : String(err.message || err); }
      });
      // From a profile or another page: /expedientes?e=Criteo fills the company in.
      const pedida = new URLSearchParams(location.search).get('e');
      if (pedida) $('x-empresa').value = pedida;
      pinta();
    },
    // «Haz que te borren» (decision 204). One button per company sends the letter as far as a
    // browser may without GUARDIANA sending anything: the person's own mail client opens with the
    // letter written, or the company's form opens with the letter on the clipboard. The name and
    // the e-mail live in two inputs of this page; they are never sent to the panel's server, and
    // only if the person ticks the box are they kept in this browser's own storage.
    async borrado() {
      let R = await api('/api/borrado');
      let C = null;
      // Mail clients cut or refuse longer mailto: links (the old 2,083-character limit of Windows
      // is still the safe one): above this, the letter goes by the clipboard.
      const MAILTO_MAX = 2000;
      const RECUERDO = 'guardiana_borrado_datos';
      const enlace = (url, texto) => `<a href="${esc(url)}" target="_blank" rel="noopener noreferrer">${esc(texto || url)}</a>`;
      const porDefecto = () => {
        const partes = (navigator.language || '').split('-');
        const idioma = (partes[0] || '').toLowerCase();
        const region = (partes[1] || '').toUpperCase();
        if (region === 'CA' && idioma === 'fr') return 'ca_qc';
        const j = R.jurisdicciones.find((x) => x.regiones.includes(region));
        if (j) return j.codigo;
        return LANG === 'pt' ? 'br' : (LANG === 'en' ? 'us' : 'es');
      };
      const sel = $('b-pais');
      sel.innerHTML = R.jurisdicciones.map((j) => `<option value="${esc(j.codigo)}">${esc(j.nombre)}</option>`).join('');
      sel.value = porDefecto();
      const jur = () => R.jurisdicciones.find((j) => j.codigo === sel.value) || R.jurisdicciones[0];
      const corredorDe = (id) => R.corredores.find((c) => c.id === id);
      // The letter's language follows the place (what the company there reads); still changeable.
      const idiomaDelLugar = () => { $('b-idioma').value = jur().idioma; $('b-qc').classList.toggle('hidden', jur().codigo !== 'ca_qc'); };
      idiomaDelLugar();
      // The person's details: in memory; in this browser only when asked to.
      try {
        const guardado = JSON.parse(localStorage.getItem(RECUERDO) || 'null');
        if (guardado && typeof guardado === 'object') {
          $('b-nombre').value = String(guardado.nombre || '');
          $('b-correo').value = String(guardado.correo || '');
          $('b-recordar').checked = true;
        }
      } catch (_) {}
      const recuerda = () => {
        try {
          if ($('b-recordar').checked) localStorage.setItem(RECUERDO, JSON.stringify({ nombre: $('b-nombre').value, correo: $('b-correo').value }));
          else localStorage.removeItem(RECUERDO);
        } catch (_) {}
      };
      const pintaLey = () => {
        const j = jur();
        const autoridad = j.autoridad
          ? (j.autoridad_url ? enlace(j.autoridad_url, j.autoridad) : esc(j.autoridad))
          : (j.autoridad_url ? enlace(j.autoridad_url, t('borrado_autoridad_lista')) : esc(t('borrado_autoridad_ninguna')));
        $('b-ley-corta').innerHTML = (j.derecho !== 'borrado' ? `<span class="bad">${esc(t('borrado_sin_derecho_' + j.derecho))}</span><br>` : '') + esc(j.corta);
        $('b-ley').innerHTML = `<h3>${esc(j.nombre)}</h3>
          <p><span class="muted">${esc(t('borrado_etq_ley'))}</span> ${esc(j.ley)}</p>
          <p><span class="muted">${esc(t('borrado_etq_articulo'))}</span> ${esc(j.articulo)}</p>
          <p><span class="muted">${esc(t('borrado_etq_plazo'))}</span> ${esc(j.plazo_texto)}</p>
          <p>${esc(j.nota)}</p>
          <p><span class="muted">${esc(t('borrado_etq_autoridad'))}</span> ${autoridad}</p>
          <details><summary class="muted">${esc(t('borrado_etq_fuentes'))}</summary><ul class="mono small">${j.fuentes.map((u) => `<li>${enlace(u)}</li>`).join('')}</ul></details>`;
        $('b-drop').classList.toggle('hidden', j.codigo !== 'us_ca');
      };
      // The address the company gives for this place, decided here so that a form can be opened
      // in the same click (a window opened later is blocked as a pop-up).
      const correoDe = (c) => { const x = c.correos.find((k) => k.ambitos.includes('*') || k.ambitos.includes(sel.value)); return x ? x.correo : null; };
      // The last letter on the clock for a company under the law chosen now: under another law it
      // is another letter, so the row offers to send it.
      const envioDe = (id) => R.envios.filter((e) => e.corredor === id && e.jurisdiccion === sel.value).sort((a, b) => b.id - a.id)[0];
      const estadoEnvio = (e) => (e.dias === null ? t('borrado_fila_sin_plazo') : e.dias > 0 ? tn('borrado_fila_quedan', e.dias) : e.dias === 0 ? t('borrado_fila_hoy') : t('borrado_fila_vencido'));
      const accion = (c, clase) => {
        if (c.estado) return `<span class="bad">${esc(c.estado_texto)}</span>`;
        const e = envioDe(c.id);
        if (e) {
          return `<span class="b-enviada"><b>${esc(t('borrado_fila_enviada').replace('{fecha}', e.enviada_texto).replace('{estado}', estadoEnvio(e)))}</b>
            <button type="button" class="secondary b-enviar" data-id="${esc(c.id)}">${esc(t('borrado_volver_enviar'))}</button></span>`;
        }
        return `<button type="button" class="${clase} b-enviar" data-id="${esc(c.id)}">${esc(t('borrado_enviar'))}</button>`;
      };
      const pregunta = (c) => `<div class="b-pregunta hidden" data-id="${esc(c.id)}"></div>`;
      const quien = (c) => `<b>${esc(c.nombre)}</b>${c.pais ? ' <span class="muted">· ' + esc(c.pais) + '</span>' : ''} <span class="tag empresa">${esc(c.tipo_texto)}</span>`;
      const vistas = R.corredores.filter((c) => c.evidencia.consultas > 0);
      const otras = R.corredores.filter((c) => c.evidencia.consultas === 0);
      const pintaEmpresas = () => {
        $('b-vistas').innerHTML = vistas.map((c) => {
          const ev = c.evidencia;
          const aparatos = c.aparatos.length
            ? `<p class="muted">${esc(t('borrado_aparatos').replace('{lista}', c.aparatos.map((a) => nombreAparato(a[0], a[1])).join(', ')))}</p>` : '';
          const nombres = `<details><summary class="muted">${esc(t('borrado_ver_nombres'))}</summary><div class="wrap"><table><tbody>${ev.nombres.map((n) => `<tr><td class="mono">${esc(n.nombre)}</td><td>${num(n.consultas)}</td><td class="muted">${esc(new Date(n.primera).toLocaleDateString(LOC, DIA))} – ${esc(new Date(n.ultima).toLocaleDateString(LOC, DIA))}</td></tr>`).join('')}</tbody></table></div></details>`;
          return `<div class="card b-vista"><div class="b-cab"><div class="b-quien">${quien(c)}</div><div class="b-accion">${accion(c, '')}</div></div>
            ${pregunta(c)}<p>${esc(c.frase)}</p>${aparatos}${nombres}</div>`;
        }).join('');
        const filtro = $('b-buscar').value;
        $('b-lista').innerHTML = otras.map((c) => `<li data-nombre="${esc(c.nombre)}"><span class="b-quien">${quien(c)}</span><span class="b-accion">${accion(c, 'secondary')}</span>${pregunta(c)}</li>`).join('');
        if (filtro) $('b-buscar').dispatchEvent(new Event('input'));
      };
      $('b-empresa-lead').textContent = t(vistas.length ? 'borrado_empresa_lead' : 'borrado_ninguna_vista');
      $('b-callados').textContent = R.callados ? tn('borrado_callados', R.callados) : '';
      $('b-callados').classList.toggle('hidden', !R.callados);
      $('b-otras-titulo').textContent = tn(vistas.length ? 'borrado_otras_titulo' : 'borrado_todas_titulo', otras.length);
      $('b-otras').classList.toggle('hidden', !otras.length);
      $('b-otras').open = !vistas.length;
      const plano = (s) => String(s || '').normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
      $('b-buscar').addEventListener('input', () => {
        const q = plano($('b-buscar').value.trim());
        let visibles = 0;
        $('b-lista').querySelectorAll('li').forEach((li) => {
          const si = !q || plano(li.getAttribute('data-nombre')).includes(q);
          li.classList.toggle('hidden', !si);
          if (si) visibles += 1;
        });
        $('b-lista-vacia').classList.toggle('hidden', visibles > 0);
      });
      $('b-buscar-form').addEventListener('submit', (e) => e.preventDefault());
      const elegibles = R.corredores.filter((c) => !c.estado);
      $('b-corredor').innerHTML = elegibles.map((c) => `<option value="${esc(c.id)}">${esc(c.nombre)}</option>`).join('');
      // The letter, filled in here and only here.
      const rellena = (cuerpo) => {
        const nombre = $('b-nombre').value.trim() || C.hueco_nombre;
        const correo = $('b-correo').value.trim() || C.hueco_correo;
        return cuerpo.split('[[NOMBRE]]').join(nombre).split('[[CORREO]]').join(correo);
      };
      const texto = () => (C ? rellena(C.cuerpo) : '');
      const pintaCarta = () => {
        if (!C) return;
        $('b-asunto').textContent = C.asunto;
        $('b-texto').value = texto();
        $('b-canal').textContent = C.para ? t('borrado_para').replace('{correo}', C.para) : t('borrado_sin_correo');
      };
      const pideCarta = (id, evidencia) => api('/api/borrado/carta?' + new URLSearchParams({ corredor: id, jurisdiccion: sel.value, idioma: $('b-idioma').value, evidencia: evidencia ? '1' : '0' }).toString());
      const cargaCarta = async () => {
        const id = $('b-corredor').value;
        if (!id) return;
        C = await pideCarta(id, $('b-evidencia').checked);
        $('b-msg').textContent = '';
        pintaCarta();
      };
      const copia = async (s) => {
        try { await navigator.clipboard.writeText(s); return true; } catch (_) {}
        // Home Mode serves the panel over plain http on the LAN, where the clipboard API is not
        // offered: the old way still works there.
        const area = document.createElement('textarea');
        area.value = s; area.setAttribute('readonly', ''); area.style.position = 'fixed'; area.style.left = '-9999px';
        document.body.appendChild(area); area.select();
        let ok = false;
        try { ok = document.execCommand('copy'); } catch (_) {}
        area.remove();
        return ok;
      };
      const mailto = (para, asunto, cuerpo) => 'mailto:' + para + '?subject=' + encodeURIComponent(asunto) + '&body=' + encodeURIComponent(cuerpo);
      // After «Send the letter»: what happens next, and «did you send it?» in the same row.
      const muestraPregunta = (id, pista, url) => {
        document.querySelectorAll('.b-pregunta').forEach((p) => { p.classList.add('hidden'); p.innerHTML = ''; });
        const caja = document.querySelector(`.b-pregunta[data-id="${CSS.escape(id)}"]`);
        if (!caja) return;
        caja.innerHTML = `<p class="ok">${esc(pista)}</p>
          ${url ? `<p class="muted">${enlace(url, t('borrado_no_se_abrio'))}</p>` : ''}
          <p><b>${esc(t('borrado_pregunta'))}</b></p>
          <p class="b-acciones"><button type="button" class="b-si" data-id="${esc(id)}">${esc(t('borrado_si'))}</button>
            <button type="button" class="secondary b-no">${esc(t('borrado_todavia'))}</button></p>
          <p><a href="#" class="b-otro-dia">${esc(t('borrado_otro_dia'))}</a></p>
          <form class="filters hidden b-fecha-form" data-id="${esc(id)}"><label><span>${esc(t('borrado_campo_enviada'))}</span><input type="date" value="${esc(R.hoy)}" max="${esc(R.hoy)}"></label>
            <button type="submit" class="secondary">${esc(t('borrado_guardar_fecha'))}</button></form>`;
        caja.classList.remove('hidden');
      };
      const faltanDatos = () => {
        const vacio = !$('b-nombre').value.trim() ? $('b-nombre') : (!$('b-correo').value.trim() ? $('b-correo') : null);
        if (!vacio) return false;
        $('b-datos-msg').textContent = t('borrado_faltan_datos');
        $('b-datos-msg').classList.remove('hidden');
        $('b-datos').scrollIntoView({ behavior: 'smooth', block: 'start' });
        vacio.focus({ preventScroll: true });
        return true;
      };
      const enviar = async (id) => {
        if (faltanDatos()) return;
        const c = corredorDe(id);
        const para = correoDe(c);
        // A form opens in this same click, or the browser takes it for a pop-up.
        const ventana = !para && c.url ? window.open('about:blank', '_blank') : null;
        $('b-corredor').value = id;
        try {
          C = await pideCarta(id, $('b-evidencia').checked);
        } catch (err) {
          if (ventana) ventana.close();
          $('b-msg').textContent = String(err.message || err);
          return;
        }
        pintaCarta();
        const cuerpo = texto();
        if (para) {
          let url = mailto(para, C.asunto, cuerpo);
          let pista = t('borrado_pista_correo');
          if (url.length > MAILTO_MAX && $('b-evidencia').checked) {
            const sin = await pideCarta(id, false);
            url = mailto(para, sin.asunto, rellena(sin.cuerpo));
            pista += ' ' + t('borrado_pista_sin_evidencia');
          }
          if (url.length > MAILTO_MAX) {
            await copia(cuerpo);
            url = mailto(para, C.asunto, t('borrado_mailto_pega'));
            pista = t('borrado_pista_pegar_correo');
          }
          location.href = url;
          muestraPregunta(id, pista, null);
        } else if (c.url) {
          const copiada = await copia(cuerpo);
          if (ventana) { ventana.opener = null; ventana.location.href = c.url; }
          muestraPregunta(id, t(copiada ? 'borrado_pista_formulario' : 'borrado_pista_formulario_sin_copia'), c.url);
        } else {
          await copia(cuerpo);
          muestraPregunta(id, t('borrado_pista_sin_canal'), null);
        }
      };
      const apunta = async (id, fecha) => {
        $('b-envio-msg').textContent = '';
        try {
          // Only the company, the place and the day: the name and the e-mail stay in the inputs.
          const lista = await api('/api/borrado/envios', { method: 'POST', body: { corredor: id, jurisdiccion: sel.value, enviada: fecha } });
          R.envios = lista;
          pintaEnvios();
          pintaEmpresas();
        } catch (err) { $('b-envio-msg').textContent = String(err.message || err); }
      };
      const pintaEnvios = () => {
        const envios = R.envios;
        $('b-envios').innerHTML = envios.length ? envios.slice().reverse().map((e) => {
          const autoridad = e.autoridad ? (e.autoridad_url ? enlace(e.autoridad_url, e.autoridad) : esc(e.autoridad)) : (e.autoridad_url ? enlace(e.autoridad_url) : '');
          return `<div class="card reloj${e.vencido ? ' vencido' : ''}">
            <h3>${esc(e.corredor_nombre)} · ${esc(e.jurisdiccion_nombre)}</h3>
            <p class="muted">${esc(t('borrado_enviada').replace('{fecha}', e.enviada_texto))}</p>
            <p><b>${esc(e.frase)}</b></p>
            ${e.prorroga ? `<p class="muted">${esc(e.prorroga)}</p>` : ''}
            ${e.notas.map((n) => `<p class="muted small">${esc(n)}</p>`).join('')}
            ${e.vencido ? `<p><strong>${esc(t('borrado_reloj_queja_titulo'))}</strong><br>${esc(e.queja || '')}${autoridad ? '<br>' + autoridad : ''}</p>` : ''}
            <p><button type="button" class="secondary b-quitar" data-id="${e.id}">${esc(t('borrado_quitar'))}</button></p></div>`;
        }).join('') : `<p class="muted">${esc(t('borrado_reloj_vacio'))}</p>`;
      };
      pintaLey();
      pintaEmpresas();
      pintaEnvios();
      await cargaCarta().catch((e) => { $('b-msg').textContent = String(e.message || e); });
      sel.addEventListener('change', () => { pintaLey(); idiomaDelLugar(); pintaEmpresas(); cargaCarta().catch(() => {}); });
      ['b-corredor', 'b-idioma', 'b-evidencia'].forEach((id) => $(id).addEventListener('change', () => { cargaCarta().catch(() => {}); }));
      ['b-nombre', 'b-correo'].forEach((id) => $(id).addEventListener('input', () => {
        pintaCarta();
        recuerda();
        if ($('b-nombre').value.trim() && $('b-correo').value.trim()) $('b-datos-msg').classList.add('hidden');
      }));
      $('b-recordar').addEventListener('change', recuerda);
      $('b-datos-form').addEventListener('submit', (e) => e.preventDefault());
      $('b-carta-form').addEventListener('submit', (e) => e.preventDefault());
      const main = document.querySelector('main');
      main.addEventListener('click', (e) => {
        const b = e.target.closest('.b-enviar');
        if (b) { enviar(b.getAttribute('data-id')).catch((err) => { $('b-msg').textContent = String(err.message || err); }); return; }
        const si = e.target.closest('.b-si');
        if (si) { apunta(si.getAttribute('data-id'), R.hoy); return; }
        if (e.target.closest('.b-no')) { const caja = e.target.closest('.b-pregunta'); caja.classList.add('hidden'); caja.innerHTML = ''; return; }
        const otro = e.target.closest('.b-otro-dia');
        if (otro) { e.preventDefault(); otro.closest('.b-pregunta').querySelector('.b-fecha-form').classList.remove('hidden'); }
      });
      main.addEventListener('submit', (e) => {
        const f = e.target.closest('.b-fecha-form');
        if (!f) return;
        e.preventDefault();
        apunta(f.getAttribute('data-id'), f.querySelector('input').value);
      });
      $('b-copiar').addEventListener('click', async () => {
        const boton = $('b-copiar');
        if (!(await copia(texto()))) return;
        boton.textContent = t('borrado_copiado');
        setTimeout(() => { boton.textContent = t('borrado_copiar'); }, 2000);
      });
      // The letter as a PDF, ready to attach to an e-mail (the owner, 12 Oct 2026: «que
      // solamente lo adjunten»); until 1.0.16 it was a .txt.
      $('b-descargar').addEventListener('click', async () => {
        if (!C) return;
        const doc = pdfDocument(C.asunto);
        if (C.para) doc.line(t('ficha_carta_pdf_para').replace('{correo}', C.para), 9, false, 0.4).gap(10);
        doc.line(C.asunto, 12, true).gap(12);
        for (const l of texto().split('\n')) { if (l.trim()) doc.line(l, 10.5); else doc.gap(7); }
        const name = 'carta-borrado-' + ($('b-corredor').value || 'guardiana') + '-' + R.hoy + '.pdf';
        try {
          const r = await savePdfFile(name, doc.bytes(''));
          if (r !== 'cancelado') { $('b-msg').textContent = t('pdf_' + r).replace('{nombre}', name); setTimeout(() => { $('b-msg').textContent = ''; }, 8000); }
        } catch (err) { $('b-msg').textContent = String(err.message || err); }
      });
      $('b-envios').addEventListener('click', async (e) => {
        const b = e.target.closest('.b-quitar');
        if (!b) return;
        try {
          R.envios = await api('/api/borrado/envios/' + encodeURIComponent(b.getAttribute('data-id')) + '/quitar', { method: 'POST' });
          pintaEnvios();
          pintaEmpresas();
        } catch (err) { $('b-envio-msg').textContent = String(err.message || err); }
      });
    },
    async comprobador() {},
    async licencia() {
      const render = (r) => {
        $('l-estado').textContent = r.texto;
        // The mark is named only when it is really there, and a paid licence says where its copy
        // is kept: nothing the program leaves on the machine goes unsaid.
        const plan = r.estado.plan.plan;
        $('l-prueba').textContent = plan === 'prueba'
          ? t(r.marca_puesta ? 'licencia_prueba_texto' : 'licencia_prueba_sin_marca').replace('{donde}', r.marca_prueba || '')
          : plan === 'plus' ? t(r.marca_licencia_puesta ? 'licencia_ancla_texto' : 'licencia_ancla_sin_marca').replace('{donde}', r.marca_licencia || '') : '';
        $('l-dev').classList.toggle('hidden', !r.clave_dev);
        $('l-clave-lead').textContent = t('licencia_clave_lead').replace('{host}', r.host_activacion);
        $('l-conexiones').innerHTML = r.conexiones.map((o) => `<tr><td>${when(o.ts)}</td><td class="mono">${esc(o.host)}</td><td>${num(o.bytes)}</td></tr>`).join('') || `<tr><td colspan="3" class="muted">${esc(t('licencia_conexiones_ninguna'))}</td></tr>`;
      };
      // Where to buy: the page of the website in the panel's language. A plain link the person
      // clicks; the panel itself still sends nothing anywhere.
      $('l-comprar').href = t('licencia_comprar_url');
      render(await api('/api/licencia'));
      $('l-clave').addEventListener('submit', async (e) => {
        e.preventDefault();
        // One activation at a time: a second Enter during the gateway's wait spent a second
        // activation (second pass, 8 Oct 2026).
        if ($('l-clave').dataset.enviando) return;
        // Read before disabling: a disabled field is left out of FormData, and the key went out
        // empty («write the key») on every activation.
        const clave = new FormData($('l-clave')).get('clave');
        $('l-clave').dataset.enviando = '1';
        $('l-clave').querySelectorAll('button,input').forEach((el) => { el.disabled = true; });
        $('l-clave-msg').textContent = '…';
        try {
          render(await api('/api/licencia/activar-clave', { method: 'POST', body: { clave } }));
          // The service sees the new licence on its next read of the ledger (seconds) and starts
          // watching again; until then every page still has the "not watching" strip it drew when
          // it loaded. Wait for the service and reload, so nobody has to refresh by hand
          // (5 Oct 2026: the owner had to).
          $('l-clave-msg').textContent = t('licencia_arrancando');
          const hasta = Date.now() + 60000;
          const mirar = async () => {
            let e = null;
            try { e = await api('/api/estado'); } catch (_) {}
            // The licence is good as soon as the panel says so; the service reads it within five
            // seconds and puts the DNS back, so the reload waits that long more.
            if ((e && !e.caducado) || Date.now() > hasta) { setTimeout(() => location.reload(), 6000); return; }
            setTimeout(mirar, 2000);
          };
          setTimeout(mirar, 1500);
        }
        catch (err) {
          $('l-clave-msg').textContent = String(err.message || err);
          try { render(await api('/api/licencia')); } catch (_) {}
        }
        finally {
          // Whatever happened, the form can be used again.
          delete $('l-clave').dataset.enviando;
          $('l-clave').querySelectorAll('button,input').forEach((el) => { el.disabled = false; });
        }
      });
    },
    async verify() {
      const load = async () => { $('v-texto').textContent = '…'; const r = await api('/api/verify'); $('v-texto').textContent = r.texto; };
      $('v-reload').addEventListener('click', () => load().catch(console.error));
      await load();
    },
    async estado() {
      paintChanges().catch(() => {});
      const r = await api('/api/estado?detalle=1');
      $('e-version').textContent = r.version + (r.clave_dev ? ' · ' + t('clave_dev') : '');
      $('e-escucha').textContent = r.escucha.join(', ');
      $('e-upstream').textContent = r.upstream.join(', ');
      const paintDns = (on, cubre) => { $('e-dns').textContent = on ? (cubre === false ? t('dns_aplicado_falta') : t('dns_aplicado')) : t('dns_no_aplicado'); $('e-dns-apply').classList.toggle('hidden', on || r.caducado); $('e-dns-restore').classList.toggle('hidden', !on); };
      paintDns(r.dns_aplicado, r.dns_cubre);
      $('e-dns-apply').addEventListener('click', async (e) => { e.target.disabled = true; $('e-dns-msg').textContent = t('dns_aplicando'); try { const x = await api('/api/dns/aplicar', { method: 'POST' }); $('e-dns-msg').textContent = x.mensaje; paintDns(x.dns_aplicado); paintChanges().catch(() => {}); } catch (err) { $('e-dns-msg').textContent = String(err.message || err); } e.target.disabled = false; });
      $('e-dns-restore').addEventListener('click', async () => { $('e-dns-msg').textContent = '…'; try { const x = await api('/api/dns/restaurar', { method: 'POST' }); $('e-dns-msg').textContent = x.mensaje; paintDns(x.dns_aplicado); paintChanges().catch(() => {}); } catch (err) { $('e-dns-msg').textContent = String(err.message || err); } });
      // On a Mac only: the note about Home Mode on a Mac (panel 23).
      $('e-limite').classList.toggle('hidden', r.so !== 'macos');
      // Lists by their name, not their id; GUARDIANA's own two have no download date because they
      // travel inside the program (review of 10 Oct 2026, panel 23).
      $('e-listas').innerHTML = r.listas.map((l) => `<tr><td>${esc((T.panel && T.panel['lista_' + l.id]) || l.id)}</td><td>${num(l.entries)}</td><td class="muted">${esc(l.fetched || t('lista_incluida'))}</td></tr>`).join('');
      pintaFuenteRed('fuente-red', r.fuente_red);
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

  // Una franja arriba de TODAS las páginas cuando el programa se ha apartado, y una cuenta atrás
  // honesta mientras dura la prueba. Que la prueba se acabe no puede ser una sorpresa, y que el
  // programa haya dejado de mirar tiene que verse en la página en la que estés, no solo en la de
  // la licencia.
  // Overdue case files, on every page but their own: as visible as the trial (1.0.17).
  function franjaExpedientes(e) {
    if (document.body.getAttribute('data-page') === 'expedientes') return;
    if (!e.exp_vencidos && !e.exp_pronto) return;
    const caja = document.createElement('div');
    caja.className = 'card franja-exp';
    const partes = [];
    if (e.exp_vencidos) partes.push(`<strong>${esc(tn('exp_aviso_vencidos', e.exp_vencidos))}</strong>`);
    if (e.exp_pronto) partes.push(`<span>${esc(tn('exp_aviso_pronto', e.exp_pronto))}</span>`);
    caja.innerHTML = partes.join(' · ') + ` <a href="/expedientes">${esc(t('exp_aviso_ver'))}</a>`;
    const m = document.querySelector('main');
    if (m) m.insertBefore(caja, m.firstChild);
  }
  async function franjaLicencia() {
    let e;
    try { e = await estadoBase(); } catch (_) { return; }
    franjaExpedientes(e);
    if (!e.caducado && !(e.prueba_dias !== null && e.prueba_dias !== undefined)) return;
    const caja = document.createElement('div');
    caja.className = 'card ' + (e.caducado ? 'caducado' : 'prueba');
    if (e.caducado) {
      caja.innerHTML = `<strong>${esc(t(e.caducado_plus ? 'caducado_titulo_plus' : 'caducado_titulo'))}</strong> <span>${esc(t('caducado_texto'))}</span>${e.hogar_aparcado ? ` <span>${esc(t('caducado_hogar_relevo'))}</span>` : ''}
        <p><a class="btn" href="${esc(t('licencia_comprar_url'))}" target="_blank" rel="noopener">${esc(t('caducado_comprar'))}</a>
        <a class="btn secondary" href="/licencia">${esc(t('caducado_activar'))}</a>
        <a class="btn secondary" href="/extracto">${esc(t('caducado_exportar'))}</a></p>`;
    } else {
      const d = e.prueba_dias;
      caja.innerHTML = `<strong>${esc(t(d === 1 ? 'prueba_queda_uno' : 'prueba_quedan').replace('{d}', d))}</strong>
        <span>${esc(t('prueba_franja_texto'))}</span>
        <a href="/licencia">${esc(t('prueba_franja_enlace'))}</a>`;
    }
    const main2 = document.querySelector('main');
    if (main2) main2.insertBefore(caja, main2.firstChild);
  }

  async function main() {
    document.documentElement.lang = LANG;
    const previo = cached();
    if (previo) { T = previo; applyTexts(); }
    try { T = await api('/api/textos'); remember(); } catch (_) {}
    applyTexts();
    // The page is shown once its texts, the trial banner and its first figures are in place
    // (or after a second and a half at most), so it opens in one go instead of jumping as each
    // piece arrives (arranque.js held it back).
    const franja = franjaLicencia().catch(() => {});
    const page = document.body.getAttribute('data-page');
    const inicio = pages[page] ? pages[page]().catch((e) => console.error(e)) : Promise.resolve();
    await Promise.race([Promise.all([franja, inicio]), new Promise((r) => setTimeout(r, 1500))]);
    document.documentElement.classList.remove('cargando');
    await inicio;
    if (token) {
      document.querySelectorAll('nav a').forEach((a) => { /* token stays in memory; links are plain */ });
    }
  }
  main();
})();
