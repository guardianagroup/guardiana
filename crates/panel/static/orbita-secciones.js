// GUARDIANA · «Órbita»: the common section and its shows (prototype, 19 Oct 2026).
//
// The owner: the nine doors open the same way, with the same symmetry and colours, and «sobre todo
// el espectáculo visual… como si pusiera una película». Agreed by Claude, ChatGPT and Grok: three
// symmetric zones (figures on the left, the show in the middle, what just happened on the right)
// and one line below (a note, the way of seeing it, the main action). Each section brings its own
// show, drawn only from real events; with no events the show rests and says so.
//
// First section: ZERO, the browser, in three ways to choose from: «Constelación» (each website a
// star, the companies it calls around it, a cut breaks the line in red), «Frontera» (ZERO at the
// centre behind a glass ring, each request arrives from its country's direction; what is cut
// shatters on the glass) and «Río» (every request flows left to right; what is cut leaves the river
// in red and falls into its company's channel). In the design review (`data-demo`) the events are
// invented and the screen says «Demostración»; in the program they will come from ZERO's own record.
(() => {
  'use strict';
  const $ = (id) => document.getElementById(id);
  const RAIZ = document.documentElement;
  const DEMO = RAIZ.dataset.demo === '1';
  const QUIETO = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const ROJO = '224,122,108', ACERO = '156,195,228', BLANCO = '232,236,241';
  const cv = $('espectaculo'), g = cv.getContext('2d');
  let W = 1, H = 1, DPR = 1, activo = null, modo = 'constelacion', raf = 0, t0 = 0, ultimo = 0;
  const nombrePais = (() => { let dn = null; try { dn = new Intl.DisplayNames(['es'], { type: 'region' }); } catch (_) {} return (c) => { try { return dn ? dn.of(c) : c; } catch (_) { return c; } }; })();
  const hora = () => { const d = new Date(); return String(d.getHours()).padStart(2, '0') + ':' + String(d.getMinutes()).padStart(2, '0'); };
  // A country's direction around the centre, from its longitude: the show keeps real geography.
  const LON = { US: -98, CA: -100, MX: -102, BR: -51, AR: -64, CL: -71, GB: -2, IE: -8, FR: 2, DE: 10, NL: 5, SE: 15, ES: -4, IT: 12, IN: 78, SG: 104, JP: 138, KR: 128, CN: 104, AU: 134, ZA: 25, IL: 35, RU: 90 };

  function mide() {
    const r = cv.getBoundingClientRect();
    DPR = Math.min(devicePixelRatio || 1, 2);
    W = Math.max(1, r.width); H = Math.max(1, r.height);
    cv.width = Math.round(W * DPR); cv.height = Math.round(H * DPR);
    g.setTransform(DPR, 0, 0, DPR, 0, 0);
  }

  // ---------- The events (ZERO) ----------
  const WEBS = ['noticias.ejemplo', 'tienda.ejemplo', 'recetas.ejemplo', 'deportes.ejemplo', 'clima.ejemplo', 'video.ejemplo', 'viajes.ejemplo', 'foro.ejemplo'];
  const EMPRESAS = [
    { n: 'Red publicitaria', p: 'US', c: 'Publicidad' }, { n: 'Corredor de datos', p: 'US', c: 'Corredor' },
    { n: 'Medición de audiencia', p: 'DE', c: 'Rastreador' }, { n: 'Subasta de anuncios', p: 'NL', c: 'Publicidad' },
    { n: 'Huella del dispositivo', p: 'IE', c: 'Rastreador' }, { n: 'Analítica de terceros', p: 'SG', c: 'Rastreador' },
    { n: 'Perfilado comercial', p: 'GB', c: 'Corredor' }, { n: 'Red social (botón)', p: 'IE', c: 'Rastreador' },
    { n: 'Vídeo publicitario', p: 'FR', c: 'Publicidad' }, { n: 'Recomendaciones', p: 'JP', c: 'Rastreador' },
  ];
  const PROPIOS = [{ n: 'Servidor de la web', p: 'US', c: '' }, { n: 'Contenido (CDN)', p: 'NL', c: '' }, { n: 'Fuentes y estilos', p: 'DE', c: '' }];
  const cifras = { webs: new Set(), cortes: 0, empresas: new Set(), paises: new Set(), pasan: 0 };
  let generador = 0;
  function evento(webFija, corteFijo) {
    const web = webFija || WEBS[Math.random() * WEBS.length | 0];
    const corte = corteFijo != null ? corteFijo : Math.random() < 0.55;
    const e = corte ? EMPRESAS[Math.random() * EMPRESAS.length | 0] : PROPIOS[Math.random() * PROPIOS.length | 0];
    const ev = { web, empresa: e.n, pais: e.p, cat: e.c, corte, t: performance.now() / 1000 };
    cifras.webs.add(web);
    if (corte) { cifras.cortes++; cifras.empresas.add(e.n); cifras.paises.add(e.p); } else cifras.pasan++;
    llega(ev);
    pintaCifras();
    if (corte) { anota(ev); if (pagina) golpeEscudo(); }
  }
  function pintaCifras() {
    const dl = $('pl-cifras');
    const filas = pagina
      ? [['Cortes en esta web', cifras.cortes, 'rojo'], ['Empresas cortadas', cifras.empresas.size, ''], ['Países', cifras.paises.size, ''], ['Dejadas pasar', cifras.pasan, '']]
      : [['Webs visitadas', cifras.webs.size, ''], ['Cortes', cifras.cortes, 'rojo'], ['Empresas cortadas', cifras.empresas.size, ''], ['Países', cifras.paises.size, ''], ['Dejadas pasar', cifras.pasan, '']];
    dl.innerHTML = '';
    for (const [k, v, cl] of filas) { const dt = document.createElement('dt'); dt.textContent = k; const dd = document.createElement('dd'); dd.textContent = v.toLocaleString('es-ES'); if (cl) dd.className = cl; dl.append(dt, dd); }
  }
  function anota(ev) {
    const ul = $('pl-lista');
    const li = document.createElement('li');
    const i = document.createElement('i'); i.textContent = 'CORTADO · ' + hora();
    const b = document.createElement('b'); b.textContent = ev.empresa;
    const s = document.createElement('span'); s.textContent = ev.web + ' → ' + nombrePais(ev.pais) + ' · ' + ev.cat;
    li.append(i, b, s);
    ul.insertBefore(li, ul.firstChild);
    while (ul.children.length > 7) ul.lastChild.remove();
  }

  // ---------- The three shows ----------
  const S = { estrellas: new Map(), empresas: new Map(), lineas: [], particulas: [], chispas: [], rotulos: [], rio: [], canales: new Map() };
  const reinicia = () => { S.lineas.length = 0; S.particulas.length = 0; S.chispas.length = 0; S.rotulos.length = 0; S.rio.length = 0; };
  function llega(ev) {
    const t = ev.t;
    if (modo === 'constelacion') {
      const st = S.estrellas.get(ev.web) || { n: 0 }; st.n++; S.estrellas.set(ev.web, st);
      const em = S.empresas.get(ev.empresa) || { n: 0, corte: ev.corte, pais: ev.pais }; em.n++; S.empresas.set(ev.empresa, em);
      S.lineas.push({ web: ev.web, emp: ev.empresa, corte: ev.corte, t0: t });
      if (S.lineas.length > 160) S.lineas.shift();
    } else if (modo === 'frontera') {
      const lon = LON[ev.pais] ?? 0;
      const ang = (lon / 180) * Math.PI - Math.PI / 2 + (Math.random() - 0.5) * 0.25;
      S.particulas.push({ ang, t0: t, dur: 2.6 + Math.random() * 0.8, corte: ev.corte, ev });
    } else {
      S.rio.push({ y: (Math.random() - 0.5), t0: t, v: 0.16 + Math.random() * 0.08, corte: ev.corte, ev, cae: 0.35 + Math.random() * 0.3 });
      if (ev.corte) { const c = S.canales.get(ev.empresa) || { n: 0 }; c.n++; S.canales.set(ev.empresa, c); }
    }
  }
  function brillo(x, y, r, color, a) {
    const gr = g.createRadialGradient(x, y, 0, x, y, r);
    gr.addColorStop(0, `rgba(${BLANCO},${a})`); gr.addColorStop(0.25, `rgba(${color},${a * 0.6})`); gr.addColorStop(1, `rgba(${color},0)`);
    g.fillStyle = gr; g.beginPath(); g.arc(x, y, r, 0, 7); g.fill();
  }
  function chispas(x, y, t) { if (QUIETO) return; for (let i = 0; i < 16; i++) { const a = Math.random() * 7, v = 30 + Math.random() * 110; S.chispas.push({ x, y, vx: Math.cos(a) * v, vy: Math.sin(a) * v, t0: t, vida: 0.5 + Math.random() * 0.6 }); } }
  function pintaChispas(t) {
    for (let i = S.chispas.length - 1; i >= 0; i--) {
      const c = S.chispas[i], e = (t - c.t0) / c.vida; if (e >= 1) { S.chispas.splice(i, 1); continue; }
      const tt = t - c.t0; g.fillStyle = `rgba(${e < 0.3 ? '255,214,190' : ROJO},${(1 - e).toFixed(3)})`;
      g.beginPath(); g.arc(c.x + c.vx * tt, c.y + c.vy * tt + 20 * tt * tt, 1.3 * (1 - e) + 0.4, 0, 7); g.fill();
    }
  }
  function rotulo(x, y, ev, a) {
    g.globalAlpha = a; g.textAlign = 'center';
    g.font = '500 9.5px "IBM Plex Mono", monospace'; g.fillStyle = `rgb(${ROJO})`; g.fillText('CORTADO', x, y - 22);
    g.font = '600 12.5px "Hanken Grotesk", sans-serif'; g.fillStyle = `rgb(${BLANCO})`; g.fillText(ev.empresa, x, y - 8);
    g.font = '400 11px "Hanken Grotesk", sans-serif'; g.fillStyle = '#A9B3BE'; g.fillText(nombrePais(ev.pais), x, y + 6);
    g.globalAlpha = 1;
  }

  // «Constelación»: the websites on an inner circle, the companies on an outer one; each request a
  // line between them, red and broken in the middle when it is cut. The whole sky turns slowly.
  function constelacion(t) {
    const cx = W / 2, cy = H / 2, R = Math.min(W, H) * 0.44, r = R * 0.48, giro = QUIETO ? 0 : t * 0.025;
    const webs = [...S.estrellas.keys()], emps = [...S.empresas.keys()];
    const posW = new Map(), posE = new Map();
    webs.forEach((w, i) => { const a = giro + i / Math.max(1, webs.length) * Math.PI * 2 - Math.PI / 2; posW.set(w, [cx + Math.cos(a) * r, cy + Math.sin(a) * r]); });
    emps.forEach((e, i) => { const a = -giro * 0.6 + i / Math.max(1, emps.length) * Math.PI * 2 - Math.PI / 2 + 0.2; posE.set(e, [cx + Math.cos(a) * R, cy + Math.sin(a) * R]); });
    // guides: two thin rings, symmetric
    g.strokeStyle = 'rgba(156,195,228,.10)'; g.lineWidth = 1;
    g.beginPath(); g.arc(cx, cy, r, 0, 7); g.stroke(); g.beginPath(); g.arc(cx, cy, R, 0, 7); g.stroke();
    g.globalCompositeOperation = 'lighter';
    for (let i = S.lineas.length - 1; i >= 0; i--) {
      const l = S.lineas[i], e = (t - l.t0) / 26; if (e >= 1) { S.lineas.splice(i, 1); continue; }
      const a = posW.get(l.web), b = posE.get(l.emp); if (!a || !b) continue;
      const fresco = Math.max(0, 1 - (t - l.t0) / 1.2);
      const mx = (a[0] + b[0]) / 2 + (cy - (a[1] + b[1]) / 2) * 0.12, my = (a[1] + b[1]) / 2 + ((a[0] + b[0]) / 2 - cx) * 0.12;
      const q = (s) => [(1 - s) * (1 - s) * a[0] + 2 * (1 - s) * s * mx + s * s * b[0], (1 - s) * (1 - s) * a[1] + 2 * (1 - s) * s * my + s * s * b[1]];
      const alfa = (l.corte ? 0.5 : 0.28) * (1 - e) + fresco * 0.5;
      g.strokeStyle = `rgba(${l.corte ? ROJO : ACERO},${alfa.toFixed(3)})`; g.lineWidth = 1 + fresco * 1.5;
      const tramo = (s0, s1) => { g.beginPath(); for (let k = 0; k <= 12; k++) { const p = q(s0 + (s1 - s0) * k / 12); k ? g.lineTo(p[0], p[1]) : g.moveTo(p[0], p[1]); } g.stroke(); };
      if (l.corte) { tramo(0, 0.44); tramo(0.56, 1); } else tramo(0, 1);
      if (l.corte && fresco > 0.98 && !l.hecho) { l.hecho = true; const p = q(0.5); chispas(p[0], p[1], t); S.rotulos.push({ x: p[0], y: p[1], ev: { empresa: l.emp, pais: S.empresas.get(l.emp).pais }, t0: t }); if (S.rotulos.length > 2) S.rotulos.shift(); }
      if (fresco > 0) { const p = q(l.corte ? Math.min(0.44, (t - l.t0) * 0.9) : Math.min(1, (t - l.t0) * 0.9)); brillo(p[0], p[1], 10, l.corte ? ROJO : ACERO, fresco); }
    }
    for (const [w, p] of posW) { const n = S.estrellas.get(w).n; brillo(p[0], p[1], 8 + Math.min(18, Math.sqrt(n) * 3), ACERO, 0.9); }
    for (const [e, p] of posE) { const em = S.empresas.get(e); brillo(p[0], p[1], 6 + Math.min(14, Math.sqrt(em.n) * 2.5), em.corte ? ROJO : ACERO, 0.75); }
    g.globalCompositeOperation = 'source-over';
    g.font = '500 10px "IBM Plex Mono", monospace'; g.fillStyle = 'rgba(232,236,241,.75)';
    for (const [w, p] of posW) { g.textAlign = p[0] < cx ? 'right' : 'left'; g.fillText(w.replace('.ejemplo', '').toUpperCase(), p[0] + (p[0] < cx ? -14 : 14), p[1] + 3); }
    g.font = '400 11px "Hanken Grotesk", sans-serif'; g.fillStyle = 'rgba(169,179,190,.85)';
    for (const [e, p] of posE) { g.textAlign = p[0] < cx ? 'right' : 'left'; g.fillText(e, p[0] + (p[0] < cx ? -14 : 14), p[1] + 4); }
    // ZERO at the heart
    brillo(cx, cy, 34, ACERO, 0.55);
    g.textAlign = 'center'; g.font = '700 12px "Schibsted Grotesk", sans-serif'; g.fillStyle = `rgb(${BLANCO})`; g.fillText('Z E R O', cx, cy + 4);
  }

  // «Frontera»: ZERO behind a glass ring; each request comes from its country's direction. What is
  // let through crosses the glass; what is cut breaks on it.
  const golpes = [];
  function frontera(t) {
    const cx = W / 2, cy = H / 2, R = Math.min(W, H) * 0.3, lejos = Math.min(W, H) * 0.62;
    // the glass: a ring of 72 segments; those hit glow red for a moment
    for (let i = 0; i < 72; i++) {
      const a0 = i / 72 * Math.PI * 2, a1 = a0 + Math.PI * 2 / 72 * 0.8;
      let calor = 0; for (const h of golpes) { const d = Math.abs(((h.ang - a0 + Math.PI * 3) % (Math.PI * 2)) - Math.PI); if (d < 0.12) calor = Math.max(calor, 1 - (t - h.t) / 1.6); }
      g.strokeStyle = calor > 0 ? `rgba(${ROJO},${(0.25 + calor * 0.75).toFixed(3)})` : 'rgba(156,195,228,.22)';
      g.lineWidth = calor > 0 ? 3 : 2;
      g.beginPath(); g.arc(cx, cy, R, a0, a1); g.stroke();
    }
    for (let i = golpes.length - 1; i >= 0; i--) if (t - golpes[i].t > 1.6) golpes.splice(i, 1);
    g.strokeStyle = 'rgba(156,195,228,.07)'; g.lineWidth = 1; g.beginPath(); g.arc(cx, cy, R * 1.55, 0, 7); g.stroke();
    g.globalCompositeOperation = 'lighter';
    for (let i = S.particulas.length - 1; i >= 0; i--) {
      const p = S.particulas[i], s = (t - p.t0) / p.dur;
      const ux = Math.cos(p.ang), uy = Math.sin(p.ang);
      if (p.corte) {
        const stop = (lejos - R) / (lejos - R * 0.25);
        const ss = Math.min(s, stop), d = lejos - (lejos - R * 0.25) * ss;
        if (s < stop) { const x = cx + ux * d, y = cy + uy * d; g.strokeStyle = `rgba(${ACERO},.5)`; g.lineWidth = 1.5; g.beginPath(); g.moveTo(x + ux * 40, y + uy * 40); g.lineTo(x, y); g.stroke(); brillo(x, y, 9, ACERO, 0.9); }
        else if (!p.hecho) { p.hecho = true; const x = cx + ux * R, y = cy + uy * R; golpes.push({ ang: (p.ang + Math.PI * 2) % (Math.PI * 2), t }); chispas(x, y, t); S.rotulos.push({ x: cx + ux * (R + 56), y: cy + uy * (R + 56), ev: p.ev, t0: t }); if (S.rotulos.length > 2) S.rotulos.shift(); }
        if (s > stop + 1.5) S.particulas.splice(i, 1);
      } else {
        if (s >= 1) { S.particulas.splice(i, 1); continue; }
        const d = lejos - (lejos - 18) * s, x = cx + ux * d, y = cy + uy * d;
        g.strokeStyle = `rgba(${ACERO},.35)`; g.lineWidth = 1; g.beginPath(); g.moveTo(x + ux * 30, y + uy * 30); g.lineTo(x, y); g.stroke(); brillo(x, y, 6, ACERO, 0.7);
      }
    }
    g.globalCompositeOperation = 'source-over';
    const late = 0.5 + 0.5 * Math.sin(t * 1.6);
    brillo(cx, cy, 70, ACERO, 0.25 + 0.1 * late); brillo(cx, cy, 26, ACERO, 0.7);
    g.textAlign = 'center'; g.font = '700 13px "Schibsted Grotesk", sans-serif'; g.fillStyle = `rgb(${BLANCO})`; g.fillText('Z E R O', cx, cy + 5);
    // the four cardinal marks keep the symmetry and say where the requests come from
    g.font = '500 9.5px "IBM Plex Mono", monospace'; g.fillStyle = 'rgba(138,149,162,.8)';
    g.fillText('ASIA', cx, cy + R * 1.55 + 18); g.fillText('AMÉRICA', cx - R * 1.55 - 6, cy + 3); g.fillText('EUROPA', cx, cy - R * 1.55 - 10);
  }

  // «Río»: every request flows from left to right; what is cut leaves the river in red and falls
  // into its company's channel below, where the day accumulates.
  function rio(t) {
    const cy = H * 0.36, banda = H * 0.12, base = H * 0.86;
    // the banks
    g.strokeStyle = 'rgba(156,195,228,.10)'; g.lineWidth = 1;
    for (const k of [-1, 1]) { g.beginPath(); for (let x = 0; x <= W; x += 12) { const y = cy + k * banda + Math.sin(x / 140 + t * 0.6) * 6; x ? g.lineTo(x, y) : g.moveTo(x, y); } g.stroke(); }
    const canales = [...S.canales.entries()].sort((a, b) => b[1].n - a[1].n).slice(0, 6);
    const ancho = W / 6, maxN = Math.max(4, ...canales.map((c) => c[1].n));
    const x0 = (W - ancho * canales.length) / 2;
    const xCanal = new Map(); canales.forEach(([n], i) => xCanal.set(n, x0 + ancho * (i + 0.5)));
    g.globalCompositeOperation = 'lighter';
    for (let i = S.rio.length - 1; i >= 0; i--) {
      const p = S.rio[i], tt = t - p.t0, x = tt * p.v * W, yRio = cy + p.y * banda * 1.6 + Math.sin(x / 140 + t * 0.6) * 6;
      if (!p.corte || x < W * p.cae) {
        if (x > W + 20) { S.rio.splice(i, 1); continue; }
        g.strokeStyle = `rgba(${ACERO},.35)`; g.lineWidth = 1.2; g.beginPath(); g.moveTo(x - 36, yRio); g.lineTo(x, yRio); g.stroke(); brillo(x, yRio, 6, ACERO, 0.8);
      } else {
        // leaves the river: a red arc down to its channel
        if (!p.x0) { p.x0 = x; p.y0 = yRio; p.tc = t; chispas(x, yRio, t); }
        const e = Math.min(1, (t - p.tc) / 1.4), tx = xCanal.get(p.ev.empresa) ?? p.x0, ty = base - 6;
        const bx = p.x0 + (tx - p.x0) * e, by = p.y0 + (ty - p.y0) * e * e;
        g.strokeStyle = `rgba(${ROJO},${(0.6 * (1 - e) + 0.15).toFixed(3)})`; g.lineWidth = 1.5;
        g.beginPath(); g.moveTo(p.x0, p.y0); g.quadraticCurveTo(bx, p.y0, bx, by); g.stroke();
        brillo(bx, by, 8, ROJO, 0.9 * (1 - e) + 0.1);
        if (e >= 1) S.rio.splice(i, 1);
      }
    }
    g.globalCompositeOperation = 'source-over';
    // the channels: what was cut today, by company, growing from the floor
    canales.forEach(([n, c]) => {
      const x = xCanal.get(n), h = (base - cy - banda * 2.2) * (c.n / maxN);
      g.fillStyle = 'rgba(224,122,108,.12)'; g.fillRect(x - ancho * 0.16, base - h, ancho * 0.32, h);
      g.fillStyle = `rgba(${ROJO},.9)`; g.fillRect(x - ancho * 0.16, base - h, ancho * 0.32, 2);
      g.textAlign = 'center'; g.font = '600 18px "Schibsted Grotesk", sans-serif'; g.fillStyle = `rgb(${BLANCO})`; g.fillText(String(c.n), x, base - h - 10);
      g.font = '400 11px "Hanken Grotesk", sans-serif'; g.fillStyle = '#A9B3BE'; g.fillText(n, x, base + 18);
    });
    g.font = '500 9.5px "IBM Plex Mono", monospace'; g.fillStyle = 'rgba(138,149,162,.8)'; g.textAlign = 'left';
    g.fillText('LO QUE TUS WEBS PIDEN →', 14, cy - banda - 14); g.textAlign = 'center'; g.fillText('LO QUE ZERO CORTÓ, POR EMPRESA', W / 2, base + 44);
  }

  function cuadro(now) {
    raf = requestAnimationFrame(cuadro);
    if (document.hidden) return;
    const t = now / 1000;
    if (now - ultimo < 15) return; ultimo = now;
    g.clearRect(0, 0, W, H);
    if (modo === 'constelacion') constelacion(t); else if (modo === 'frontera') frontera(t); else rio(t);
    pintaChispas(t);
    for (let i = S.rotulos.length - 1; i >= 0; i--) { const r = S.rotulos[i], e = t - r.t0; if (e > 3.2) { S.rotulos.splice(i, 1); continue; } rotulo(r.x, r.y, r.ev, Math.min(1, e / 0.2) * Math.min(1, (3.2 - e) / 0.6)); }
  }

  // ---------- ZERO: the browser on screen (owner, 20 Oct 2026) ----------
  // Search, Enter: the web appears in the middle and, on the right, what GUARDIANA cuts on it.
  // Inside ZERO's own window the program does it for real: the same two messages its address bar
  // sends (navigate, then open the shield's panel on the right). In the design review a sample page
  // stands in for the web and the cuts are invented, and the screen says so.
  const HOST = window.chrome && window.chrome.webview;
  let pagina = null, ritmo = 0;
  const esDireccion = (t) => /^https?:\/\//i.test(t) || /^[^\s/]+\.[a-z]{2,}(\/\S*)?$/i.test(t);
  const dominioDe = (t) => { try { return new URL(/^https?:/i.test(t) ? t : 'https://' + t).hostname.replace(/^www\./, ''); } catch (_) { return t; } };
  function golpeEscudo() {
    const n = $('pl-escudo-n'); n.textContent = String(cifras.cortes);
    const e = $('pl-escudo'); e.classList.remove('late'); void e.offsetWidth; e.classList.add('late');
    const f = $('pl-filo'); f.classList.remove('late'); void f.offsetWidth; f.classList.add('late');
  }
  function navega(texto) {
    if (HOST) { HOST.postMessage({ tipo: 'navegar', texto }); HOST.postMessage({ tipo: 'panel', vista: 'escudo' }); return; }
    if (!DEMO) { $('pl-vacio').hidden = false; $('pl-vacio').textContent = 'Para navegar con GUARDIANA cortando, abre ZERO: la web aparece en el centro y lo que se corta, a la derecha.'; return; }
    const dir = esDireccion(texto), dom = dir ? dominioDe(texto) : 'buscador.ejemplo';
    pagina = { dom, texto };
    clearInterval(generador); clearTimeout(ritmo);
    cifras.webs.clear(); cifras.cortes = 0; cifras.empresas.clear(); cifras.paises.clear(); cifras.pasan = 0;
    $('pl-lista').innerHTML = ''; pintaCifras();
    $('pl-k1').textContent = 'ESTA WEB · DEMOSTRACIÓN'; $('pl-k2').textContent = 'LO QUE CORTA GUARDIANA';
    $('plantilla').classList.add('navegando');
    $('pl-visor').hidden = false; $('pl-atras').hidden = false; $('pl-escudo').hidden = false; $('pl-escudo-n').textContent = '0';
    $('pl-q').value = dir ? dom : texto; $('pl-q').blur();
    const carga = $('pl-carga'); carga.style.opacity = '1'; carga.style.width = '0';
    requestAnimationFrame(() => { carga.style.width = '72%'; });
    $('pl-web').onload = () => { carga.style.width = '100%'; setTimeout(() => { carga.style.opacity = '0'; }, 500); };
    $('pl-web').src = 'static/demo-web.html?d=' + encodeURIComponent(dom) + (dir ? '' : '&q=' + encodeURIComponent(texto));
    // what a page asks for while it loads, then a trickle while it is open
    const n = 12 + (Math.random() * 12 | 0);
    for (let i = 0; i < n; i++) setTimeout(() => { if (pagina && pagina.dom === dom) evento(dom, Math.random() < 0.7); }, 250 + i * (2600 / n) + Math.random() * 120);
    const sigue = () => { if (!pagina || pagina.dom !== dom) return; evento(dom, Math.random() < 0.6); ritmo = setTimeout(sigue, 1800 + Math.random() * 2600); };
    ritmo = setTimeout(sigue, 3400);
  }
  function nuevaPestana() {
    pagina = null; clearTimeout(ritmo);
    $('plantilla').classList.remove('navegando');
    $('pl-visor').hidden = true; $('pl-web').src = 'about:blank'; $('pl-atras').hidden = true; $('pl-escudo').hidden = true;
    $('pl-q').value = '';
    $('pl-k1').textContent = DEMO ? 'HOY · DEMOSTRACIÓN' : 'HOY'; $('pl-k2').textContent = 'EN DIRECTO';
    cifras.webs.clear(); cifras.cortes = 0; cifras.empresas.clear(); cifras.paises.clear(); cifras.pasan = 0;
    $('pl-lista').innerHTML = ''; pintaCifras();
    clearInterval(generador); if (DEMO) generador = setInterval(evento, 900);
  }
  $('pl-busca').addEventListener('submit', (e) => { e.preventDefault(); const t = $('pl-q').value.trim(); if (t) navega(t); });
  $('pl-atras').addEventListener('click', nuevaPestana);

  const MODOS = [['constelacion', 'Constelación'], ['frontera', 'Frontera'], ['rio', 'Río']];
  function ponModo(m) {
    modo = m; reinicia(); S.estrellas.clear(); S.empresas.clear(); S.canales.clear();
    for (const b of $('pl-modos').children) b.setAttribute('aria-selected', String(b.dataset.m === m));
    try { localStorage.setItem('guardiana_orbita_zero_modo', m); } catch (_) {}
  }
  function abre(id) {
    if (id !== 'zero') return false;
    activo = id;
    $('pl-k1').textContent = DEMO ? 'HOY · DEMOSTRACIÓN' : 'HOY';
    $('pl-nota').textContent = DEMO ? 'DATOS DE DEMOSTRACIÓN · EN ZERO, LOS DE TU NAVEGACIÓN' : 'LO QUE ZERO HA CORTADO HOY';
    const acc = $('pl-accion'); acc.textContent = 'Abrir ZERO'; acc.onclick = (e) => { e.preventDefault(); };
    const mods = $('pl-modos'); mods.innerHTML = '';
    let guardado = 'constelacion'; try { guardado = localStorage.getItem('guardiana_orbita_zero_modo') || guardado; } catch (_) {}
    for (const [m, n] of MODOS) { const b = document.createElement('button'); b.type = 'button'; b.setAttribute('role', 'tab'); b.dataset.m = m; b.textContent = n; b.addEventListener('click', () => ponModo(m)); mods.appendChild(b); }
    ponModo(guardado);
    cifras.webs.clear(); cifras.cortes = 0; cifras.empresas.clear(); cifras.paises.clear(); cifras.pasan = 0;
    $('pl-lista').innerHTML = ''; pintaCifras();
    $('plantilla').hidden = false;
    $('pl-busca').hidden = false; pagina = null; $('plantilla').classList.remove('navegando'); $('pl-visor').hidden = true; $('pl-atras').hidden = true; $('pl-escudo').hidden = true; $('pl-q').value = '';
    $('pl-k2').textContent = 'EN DIRECTO';
    setTimeout(() => { if (activo === 'zero') $('pl-q').focus({ preventScroll: true }); }, 1000);
    requestAnimationFrame(() => { mide(); if (!raf) raf = requestAnimationFrame(cuadro); });
    clearInterval(generador);
    if (DEMO) { for (let i = 0; i < 6; i++) setTimeout(evento, 400 + i * 260); generador = setInterval(evento, 900); }
    else { $('pl-vacio').hidden = false; $('pl-vacio').textContent = 'Aquí aparecerá cada corte que haga ZERO mientras navegas.'; }
    return true;
  }
  function cierra() {
    activo = null; clearInterval(generador); clearTimeout(ritmo); pagina = null;
    $('pl-busca').hidden = true; $('pl-visor').hidden = true; $('pl-web').src = 'about:blank'; $('plantilla').classList.remove('navegando');
    if (raf) cancelAnimationFrame(raf); raf = 0;
    $('plantilla').hidden = true;
  }
  addEventListener('resize', () => { if (activo) mide(); });
  window.GuardianaSecciones = { abre, cierra };
})();
