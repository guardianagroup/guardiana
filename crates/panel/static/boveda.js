// The vault page. Same rules as the panel: no third-party code, no cookies; the token of this
// run arrives once in the URL, stays in this tab's memory and travels as a header.
(() => {
  'use strict';
  const params = new URLSearchParams(location.search);
  if (params.has('t')) {
    try { sessionStorage.setItem('guardiana_boveda_token', params.get('t')); } catch (_) {}
    params.delete('t');
    const rest = params.toString();
    history.replaceState(null, '', location.pathname + (rest ? '?' + rest : ''));
  }
  let token = '';
  try { token = sessionStorage.getItem('guardiana_boveda_token') || ''; } catch (_) {}
  let LANG = '';
  try { LANG = localStorage.getItem('guardiana_lang') || ''; } catch (_) {}
  if (LANG !== 'es' && LANG !== 'en') LANG = ((navigator.language || 'es').slice(0, 2) === 'en') ? 'en' : 'es';
  try { document.documentElement.lang = LANG; } catch (_) {}
  let T = { panel: {} };
  const t = (k) => (T.panel && T.panel[k]) || k;
  const $ = (id) => document.getElementById(id);
  const show = (id, on) => $(id).classList.toggle('hidden', !on);
  const esc = (s) => String(s ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
  const size = (n) => n >= 1048576 ? (n / 1048576).toFixed(1) + ' MB' : n >= 1024 ? (n / 1024).toFixed(1) + ' KB' : n + ' B';

  async function api(path, opts = {}) {
    const headers = Object.assign({ 'X-Guardiana-Token': token, 'X-Guardiana-Lang': LANG }, opts.headers || {});
    if (opts.json !== undefined) { opts.body = JSON.stringify(opts.json); headers['Content-Type'] = 'application/json'; }
    const r = await fetch(path, Object.assign({}, opts, { headers }));
    if (r.status === 401 && path === '/api/estado') { show('no-session', true); throw new Error('sin sesión'); }
    if (!r.ok) throw new Error(await r.text());
    if (r.status === 204) return null;
    const ct = r.headers.get('content-type') || '';
    return ct.includes('json') ? r.json() : r;
  }
  function applyTexts() {
    document.querySelectorAll('[data-t]').forEach((el) => { el.textContent = t(el.getAttribute('data-t')); });
  }

  let estado = { existe: false, abierta: false };
  async function refresh() {
    estado = await api('/api/estado');
    $('ruta').textContent = estado.ruta;
    show('s-crear', !estado.existe);
    show('s-abrir', estado.existe && !estado.abierta);
    // While the 24 words are on screen, nothing else: the person reads and copies them first.
    const leyendo = !$('s-palabras').classList.contains('hidden');
    show('s-abierta', estado.abierta && !leyendo);
    show('s-registro', estado.existe && !leyendo);
    if (estado.abierta) await lista();
    if (estado.existe) await registro();
  }
  async function lista() {
    const items = await api('/api/objetos');
    show('vacia', items.length === 0);
    $('objetos').innerHTML = items.map((o) => `<tr><td>${esc(o.nombre)}</td><td>${size(o.tamano)}</td><td>${esc(o.guardado.slice(0, 16).replace('T', ' '))}</td><td class="mono">${esc(o.sha256.slice(0, 16))}…</td><td><button class="secondary" data-bajar="${esc(o.id)}" data-nombre="${esc(o.nombre)}">${esc(t('bov_sacar'))}</button></td></tr>`).join('');
  }
  async function registro() {
    const r = await api('/api/registro');
    const c = $('cadena');
    c.textContent = r.cadena_ok ? t('bov_registro_ok').replace('{n}', r.total) : t('bov_registro_rota').replace('{n}', r.rota_en);
    c.className = 'lead ' + (r.cadena_ok ? 'ok' : 'bad');
    $('registro').innerHTML = r.lineas.slice().reverse().map((l) => `<tr><td>${l.n}</td><td class="mono">${esc(l.hora)}</td><td>${esc(l.accion)}</td><td class="mono">${esc(l.objeto || '—')}</td><td>${esc(l.detalle)}</td></tr>`).join('');
  }
  function same(f, a, b, msgId) {
    const x = f.elements[a].value, y = f.elements[b].value;
    if (x.length < 10) { $(msgId).textContent = t('bov_clave_corta'); return null; }
    if (x !== y) { $(msgId).textContent = t('bov_clave_distinta'); return null; }
    return x;
  }

  $('f-crear').addEventListener('submit', async (ev) => {
    ev.preventDefault(); $('m-crear').textContent = t('bov_espera'); $('m-crear').className = 'muted';
    const p = same(ev.target, 'a', 'b', 'm-crear'); if (p === null) return;
    try {
      const r = await api('/api/crear', { method: 'POST', json: { contrasena: p } });
      ev.target.reset(); $('m-crear').textContent = '';
      $('palabras').innerHTML = r.palabras.map((w) => `<li>${esc(w)}</li>`).join('');
      show('s-crear', false); show('s-palabras', true);
    } catch (e) { $('m-crear').textContent = e.message; $('m-crear').className = 'bad'; }
  });
  $('b-palabras-listo').addEventListener('click', async () => { $('palabras').innerHTML = ''; show('s-palabras', false); await refresh(); });
  $('f-abrir').addEventListener('submit', async (ev) => {
    ev.preventDefault(); $('m-abrir').textContent = t('bov_espera'); $('m-abrir').className = 'muted';
    try { await api('/api/abrir', { method: 'POST', json: { contrasena: ev.target.elements.a.value } }); ev.target.reset(); $('m-abrir').textContent = ''; await refresh(); }
    catch (e) { $('m-abrir').textContent = e.message; $('m-abrir').className = 'bad'; }
  });
  $('f-recuperar').addEventListener('submit', async (ev) => {
    ev.preventDefault(); $('m-recuperar').textContent = t('bov_espera'); $('m-recuperar').className = 'muted';
    const p = same(ev.target, 'nueva', 'nueva2', 'm-recuperar'); if (p === null) return;
    try { await api('/api/recuperar', { method: 'POST', json: { palabras: ev.target.elements.palabras.value, nueva: p } }); ev.target.reset(); $('m-recuperar').textContent = ''; await refresh(); }
    catch (e) { $('m-recuperar').textContent = e.message; $('m-recuperar').className = 'bad'; }
  });
  $('f-subir').addEventListener('submit', async (ev) => {
    ev.preventDefault(); $('m-subir').textContent = '';
    const f = ev.target.elements.archivo.files[0]; if (!f) return;
    try {
      const r = await api('/api/objetos?nombre=' + encodeURIComponent(f.name), { method: 'POST', body: f, headers: { 'Content-Type': 'application/octet-stream' } });
      $('m-subir').textContent = t('bov_guardado').replace('{nombre}', r.nombre); ev.target.reset(); await lista(); await registro();
    } catch (e) { $('m-subir').textContent = e.message; $('m-subir').className = 'bad'; }
  });
  $('objetos').addEventListener('click', async (ev) => {
    const b = ev.target.closest('[data-bajar]'); if (!b) return;
    try {
      const r = await api('/api/objetos/' + encodeURIComponent(b.getAttribute('data-bajar')));
      const blob = await r.blob();
      const a = document.createElement('a'); a.href = URL.createObjectURL(blob); a.download = b.getAttribute('data-nombre'); document.body.appendChild(a); a.click(); a.remove();
      setTimeout(() => URL.revokeObjectURL(a.href), 60000);
      await registro();
    } catch (e) { alert(e.message); }
  });
  $('f-contrasena').addEventListener('submit', async (ev) => {
    ev.preventDefault(); $('m-contrasena').textContent = '';
    const p = same(ev.target, 'nueva', 'nueva2', 'm-contrasena'); if (p === null) return;
    try { await api('/api/contrasena', { method: 'POST', json: { nueva: p } }); $('m-contrasena').textContent = t('bov_hecho'); ev.target.reset(); await registro(); }
    catch (e) { $('m-contrasena').textContent = e.message; }
  });
  $('b-cerrar').addEventListener('click', async () => { await api('/api/cerrar', { method: 'POST' }); await refresh(); });
  $('b-salir').addEventListener('click', async () => {
    try { await api('/api/salir', { method: 'POST' }); } catch (_) {}
    ['s-crear', 's-palabras', 's-abrir', 's-abierta', 's-registro'].forEach((id) => show(id, false)); show('salido', true);
  });

  (async () => {
    try { T = await api('/api/textos'); } catch (_) {}
    applyTexts();
    try { await refresh(); } catch (_) {}
    // The vault locks itself after ten idle minutes: keep the page honest about it.
    setInterval(async () => { try { const e = await api('/api/estado'); if (e.abierta !== estado.abierta) await refresh(); } catch (_) {} }, 30000);
  })();
})();
