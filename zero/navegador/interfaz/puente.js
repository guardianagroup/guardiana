// The bridge between the browser's own pages and the program. The pages never talk to the
// internet: they send small JSON messages to the program (window.chrome.webview, which only
// the browser's own pages have) and draw what it answers. Every text comes from the program, in
// the person's language; none is written here.
'use strict';
const Z = (() => {
  const host = window.chrome && window.chrome.webview;
  const oyentes = {};
  let T = {};
  // «listo» carries the clock's offset so the day's figures change at the person's midnight.
  const manda = (m) => {
    if (m && m.tipo === 'listo') m.zona = -new Date().getTimezoneOffset();
    if (host) host.postMessage(m);
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
    (oyentes[m.tipo] || []).forEach((f) => f(m));
  };
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
  const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
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
  return { manda, en, t, tn, esc, $, n, pintaTextos, recibe, oscuro, ICONO };
})();
