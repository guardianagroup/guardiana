// The bridge between the browser's own pages and the program. The pages never talk to the
// internet: they send small JSON messages to the program (window.chrome.webview, which only
// the browser's own pages have) and draw what it answers. Every text comes from the program, in
// the person's language; none is written here.
'use strict';
const Z = (() => {
  const host = window.chrome && window.chrome.webview;
  const oyentes = {};
  let T = {};
  // Outside GUARDIANA ZERO (17 Oct 2026): on Linux and macOS, Comunicaciones opens in the person's
  // own browser, served on 127.0.0.1 by `guardiana comunicaciones`, run as that person. The same
  // messages go there by HTTP (see `fuera` below); in ZERO nothing of this runs.
  const local = !host && location.protocol === 'http:'
    && (location.hostname === '127.0.0.1' || location.hostname === 'localhost');
  let fuera = null;
  // «listo» carries the clock's offset so the day's figures change at the person's midnight.
  const manda = (m) => {
    if (m && m.tipo === 'listo') m.zona = -new Date().getTimezoneOffset();
    if (host) host.postMessage(m);
    else if (fuera && m) fuera(m);
  };
  const en = (tipo, f) => { (oyentes[tipo] = oyentes[tipo] || []).push(f); };
  const recibe = (m) => {
    if (!m || typeof m !== 'object') return;
    if (m.tipo === 'textos') {
      T = m.textos || {};
      document.documentElement.lang = m.idioma || 'es';
      pintaTextos(document);
    }
    if (m.tipo === 'tema') document.documentElement.dataset.tema = m.tema || '';
    // One drawing that fails must not leave the rest of the screen frozen.
    (oyentes[m.tipo] || []).forEach((f) => { try { f(m); } catch (e) { console.error(e); } });
  };
  // The pages follow Windows' day or night until the person chooses (diseno.css, data-sigue).
  document.documentElement.dataset.sigue = '';
  if (host) host.addEventListener('message', (e) => recibe(e.data));
  // A text with its {holes} filled; the key itself if the text is missing (never silent).
  const t = (k, v) => {
    let s = Object.prototype.hasOwnProperty.call(T, k) ? T[k] : k;
    if (v) for (const [a, b] of Object.entries(v)) s = s.split('{' + a + '}').join(String(b));
    return s;
  };
  // The same with the count's own form: «_uno» for one and «_cero» for none, when the text has them
  // («Se cortó 1 petición», never «Se cortaron 1 peticiones»).
  const tn = (k, cuantos, v) => {
    const c = Number(cuantos || 0);
    const clave = (c === 1 && Object.prototype.hasOwnProperty.call(T, k + '_uno')) ? k + '_uno'
      : (c === 0 && Object.prototype.hasOwnProperty.call(T, k + '_cero')) ? k + '_cero' : k;
    return t(clave, v);
  };
  // The local server's way (17 Oct 2026). Messages to the program go in order, a few at a time,
  // as POST /mensaje; what the program says comes from GET /eventos, which waits until there is
  // something (at most 25 s) and then answers with all of it. A long wait and not a WebSocket or
  // a server-sent stream: plain requests need nothing added to the program. The server's key is
  // a cookie only it can read (HttpOnly); this page never sees it. Each page has its own name,
  // so a second tab takes Comunicaciones over and the first one says so instead of both playing
  // the same call.
  if (local) {
    const pagina = Array.from(crypto.getRandomValues(new Uint8Array(16)), (b) => b.toString(16).padStart(2, '0')).join('');
    const cola = [];
    // A file goes in 64 KB pieces and a call in small ones: a few MB per request is plenty, and
    // the server takes at most 8 MB.
    const LOTE = 4 * 1024 * 1024;
    let enviando = false, escuchando = false, parado = false, desde = 0, fallos = 0, cartel = null;
    const muestra = (clave) => {
      if (!clave) { if (cartel) { cartel.remove(); cartel = null; } return; }
      if (!cartel) {
        cartel = document.createElement('div');
        cartel.setAttribute('role', 'alert');
        Object.assign(cartel.style, {
          position: 'fixed', top: '0', left: '0', right: '0', zIndex: '9999', padding: 'var(--e3) var(--e4)',
          background: 'var(--aviso-suave)', color: 'var(--fg)', borderBottom: '1px solid var(--aviso)',
          font: '500 15px/1.4 var(--texto)', textAlign: 'center',
        });
        document.body.appendChild(cartel);
      }
      cartel.textContent = t(clave);
    };
    const para = (clave) => { parado = true; cola.length = 0; muestra(clave); };
    const cabeceras = { 'Content-Type': 'application/json', 'X-Guardiana-Pagina': pagina };
    const envia = () => {
      if (enviando || parado || !cola.length) return;
      enviando = true;
      const lote = [];
      let tam = 0, listo = false;
      while (cola.length && (!lote.length || tam + cola[0].s.length < LOTE)) {
        const x = cola.shift();
        lote.push(x.s); tam += x.s.length; listo = listo || x.listo;
      }
      fetch('/mensaje', { method: 'POST', headers: cabeceras, body: '[' + lote.join(',') + ']', cache: 'no-store', credentials: 'same-origin' })
        .then((r) => {
          if (r.status === 409) para('sec_puente_otra');
          else if (r.status === 401 || r.status === 403) para('sec_puente_cerrado');
          // What the program says starts once it has taken «listo»: its texts come first.
          else if (r.ok && listo && !escuchando) { escuchando = true; escucha(); }
        })
        .catch(() => { fallos += 1; if (fallos >= 3) muestra('sec_puente_cerrado'); })
        .finally(() => { enviando = false; envia(); });
    };
    const escucha = async () => {
      while (!parado) {
        try {
          const r = await fetch('/eventos?desde=' + desde, { headers: { 'X-Guardiana-Pagina': pagina }, cache: 'no-store', credentials: 'same-origin' });
          if (r.status === 409) { para('sec_puente_otra'); return; }
          if (r.status === 401 || r.status === 403) { para('sec_puente_cerrado'); return; }
          if (!r.ok) throw new Error(String(r.status));
          const j = await r.json();
          if (fallos) { fallos = 0; muestra(null); }
          desde = Number(j.sig) || desde;
          (j.m || []).forEach(recibe);
        } catch (e) {
          // Asleep, or closed: say so after a few tries, and keep trying.
          fallos += 1;
          if (fallos >= 3) muestra('sec_puente_cerrado');
          await new Promise((res) => setTimeout(res, Math.min(5000, 500 * fallos)));
        }
      }
    };
    fuera = (m) => {
      // What only a window of ZERO can do, done here by the page itself: an invitation by
      // WhatsApp opens in a new tab, and one by mail in the mail program.
      if (m.tipo === 'navegar') {
        const u = String(m.texto || '');
        if (/^https:\/\//i.test(u)) window.open(u, '_blank', 'noopener,noreferrer');
        return;
      }
      if (m.tipo === 'abrir_correo') {
        const a = document.createElement('a');
        a.href = 'mailto:?subject=' + encodeURIComponent(m.asunto || '') + '&body=' + encodeURIComponent(m.cuerpo || '');
        a.click();
        return;
      }
      if (m.tipo !== 'listo' && m.tipo !== 'secure') return;
      if (parado) return;
      cola.push({ s: JSON.stringify(m), listo: m.tipo === 'listo' });
      // Everything the page says while it opens goes in one request, «listo» with it.
      queueMicrotask(envia);
    };
  }
  const esc =(s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
  function pintaTextos(raiz) {
    raiz.querySelectorAll('[data-t]').forEach((el) => { el.textContent = t(el.dataset.t); });
    raiz.querySelectorAll('[data-t-ph]').forEach((el) => { el.placeholder = t(el.dataset.tPh); });
    raiz.querySelectorAll('[data-t-title]').forEach((el) => { el.title = t(el.dataset.tTitle); el.setAttribute('aria-label', t(el.dataset.tTitle)); });
  }
  const $ = (id) => document.getElementById(id);
  // Numbers in the person's language, never rounded.
  const n = (x) => Number(x || 0).toLocaleString(document.documentElement.lang || 'es');
  // Day or night as the page shows it now: the person's choice, or Windows' when there is none.
  const oscuro = () => {
    const e = document.documentElement.dataset.tema;
    if (e === 'noche') return true;
    if (e === 'dia') return false;
    return window.matchMedia('(prefers-color-scheme: dark)').matches;
  };
  const ICONO = {
    sol: '<svg viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><circle cx="12" cy="12" r="4.2"/><path d="M12 2.5v2.2M12 19.3v2.2M2.5 12h2.2M19.3 12h2.2M5.3 5.3l1.6 1.6M17.1 17.1l1.6 1.6M5.3 18.7l1.6-1.6M17.1 6.9l1.6-1.6"/></svg>',
    luna: '<svg viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><path d="M20.5 14.2A8.5 8.5 0 0 1 9.8 3.5a8.5 8.5 0 1 0 10.7 10.7z"/></svg>',
  };
  // «Who followed you from web to web»: one row per company, on how many of the person's webs it
  // showed up, and whether it got through on any. Returns whether there was anyone.
  function rastro(ul, r) {
    const lista = (r && r.seguidores) || [];
    const total = Math.max(1, Number((r && r.webs) || 0));
    ul.innerHTML = lista.map((x) => {
      const ancho = Math.min(100, Math.round((100 * x.webs) / total));
      const estado = x.paso ? tn('rastro_paso', x.paso, { n: n(x.paso) }) : t('rastro_cortado');
      return `<li><button class="quien enlace-empresa" data-empresa="${esc(x.quien)}">${esc(x.quien)}</button><span class="cuantas">${esc(t('rastro_webs', { webs: n(x.webs), total: n(r.webs) }))}</span>`
        + `<span class="pista"><i style="width:${ancho}%"></i></span><span class="estado${x.paso ? ' paso' : ''}">${esc(estado)}</span></li>`;
    }).join('');
    return lista.length > 0;
  }
  // A company's name anywhere on these pages opens its profile (the owner, 12 Oct 2026). In the
  // capture phase, so the row around the name does not open or close as well.
  document.addEventListener('click', (e) => {
    const b = e.target && e.target.closest ? e.target.closest('[data-empresa]') : null;
    if (!b) return;
    e.preventDefault();
    e.stopPropagation();
    manda({ tipo: 'abrir_ficha', empresa: b.dataset.empresa });
  }, true);
  return { manda, en, t, tn, esc, $, n, pintaTextos, recibe, oscuro, ICONO, rastro };
})();
