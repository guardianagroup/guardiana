// The top of the window: tabs, the address, the shield and the guard line.
'use strict';
(() => {
  const { manda, en, t, esc, $ } = Z;
  let activa = null;      // id of the active tab
  let panel = null;       // view open in the side panel
  let cortesPrevios = -1; // to make the shield beat only when a new company is stopped
  const marcasPorPestana = new Map(); // tab id → [{t, cortado}]

  const CANDADO = {
    seguro: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="3.2" y="7" width="9.6" height="7" rx="1.6"/><path d="M5.3 7V5.2a2.7 2.7 0 0 1 5.4 0V7"/></svg>',
    abierto: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="3.2" y="7" width="9.6" height="7" rx="1.6"/><path d="M5.3 7V5.2a2.7 2.7 0 0 1 5.2-1"/></svg>',
    interno: '<svg viewBox="0 0 16 16"><circle cx="8" cy="8" r="3.4" fill="currentColor"/><circle cx="8" cy="8" r="6" fill="none" stroke="currentColor" stroke-width="1.4" opacity=".35"/></svg>',
  };

  // --- tabs ---------------------------------------------------------------------------------
  function pintaPestanas(lista) {
    const cont = $('pestanas');
    cont.querySelectorAll('.pestana').forEach((p) => p.remove());
    const nueva = $('nueva');
    for (const p of lista) {
      const el = document.createElement('div');
      el.className = 'pestana' + (p.cargando ? ' cargando' : '') + (p.mandato ? ' mandato' : '') + (p.aislada ? ' aislada' : '');
      el.setAttribute('role', 'tab');
      el.setAttribute('aria-selected', p.activa ? 'true' : 'false');
      el.dataset.id = p.id;
      el.title = p.titulo || p.url || '';
      const icono = p.icono && !p.cargando && /^data:image\/png;base64,/.test(p.icono) ? `<img class="icono" alt="" src="${esc(p.icono)}">` : '<i class="punto"></i>';
      el.innerHTML = `${icono}<span class="titulo">${esc(p.titulo || t('pestana_nueva'))}</span>`
        + `<button class="cerrar" aria-label="${esc(t('cerrar_pestana'))}" title="${esc(t('cerrar_pestana'))}"><svg viewBox="0 0 12 12" width="10" height="10" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"><path d="M2.5 2.5l7 7M9.5 2.5l-7 7"/></svg></button>`;
      el.addEventListener('mousedown', (e) => {
        if (e.button === 1) { e.preventDefault(); manda({ tipo: 'pestana_cerrar', id: p.id }); return; }
        if (!e.target.closest('.cerrar')) manda({ tipo: 'pestana_activar', id: p.id });
      });
      el.querySelector('.cerrar').addEventListener('click', () => manda({ tipo: 'pestana_cerrar', id: p.id }));
      cont.insertBefore(el, nueva);
    }
  }
  $('nueva').addEventListener('click', () => manda({ tipo: 'pestana_nueva' }));

  // --- address --------------------------------------------------------------------------------
  const campo = $('campo-dir');
  const vista = $('vista-dir');
  let urlActual = '';
  function pintaDireccion(url, interna) {
    urlActual = url || '';
    if (document.activeElement !== campo) campo.value = interna ? '' : urlActual;
    let host = '', resto = '';
    try {
      const u = new URL(urlActual);
      // The browser's own pages show the empty field with its hint, not their address.
      if (/^https?:$/.test(u.protocol) && !interna) { host = u.host; resto = (u.pathname === '/' ? '' : u.pathname) + u.search; }
    } catch (_) {}
    vista.innerHTML = host ? `<b>${esc(host)}</b><span>${esc(resto)}</span>` : '';
    const mostrarVista = !!host && document.activeElement !== campo;
    vista.classList.toggle('oculto', !mostrarVista);
    campo.style.color = mostrarVista ? 'transparent' : '';
  }
  campo.addEventListener('focus', () => { campo.value = urlActual.startsWith('https://zero.guardiana') ? '' : urlActual; campo.style.color = ''; vista.classList.add('oculto'); setTimeout(() => campo.select(), 0); });
  campo.addEventListener('blur', () => pintaDireccion(urlActual, urlActual.startsWith('https://zero.guardiana')));
  $('direccion').addEventListener('submit', (e) => {
    e.preventDefault();
    const texto = campo.value.trim();
    if (!texto) return;
    manda({ tipo: 'navegar', texto });
    campo.blur();
  });
  campo.addEventListener('keydown', (e) => { if (e.key === 'Escape') { campo.value = urlActual; campo.blur(); } });

  // --- buttons ------------------------------------------------------------------------------
  $('atras').addEventListener('click', () => manda({ tipo: 'atras' }));
  $('b-parar').addEventListener('click', () => manda({ tipo: 'mandato_terminar' }));
  $('adelante').addEventListener('click', () => manda({ tipo: 'adelante' }));
  $('recargar').addEventListener('click', () => manda({ tipo: $('ic-detener').classList.contains('oculto') ? 'recargar' : 'detener' }));
  const vistas = { 'b-escudo': 'escudo', 'b-mandato': 'mandato', 'b-tachon': 'tachon', 'b-datos': 'datos', 'b-menu': 'ajustes' };
  for (const [id, v] of Object.entries(vistas)) {
    $(id).addEventListener('click', () => manda({ tipo: 'panel', vista: panel === v ? null : v }));
  }

  // --- the shield ------------------------------------------------------------------------------
  function pintaEscudo(m) {
    const b = $('b-escudo');
    const r = m.resumen || {};
    const mirando = !m.cortando;
    b.classList.toggle('mirando', mirando);
    const cifra = mirando ? r.empresas : r.empresas_cortadas;
    $('b-escudo-n').textContent = Z.n(cifra || 0);
    if (!mirando && cortesPrevios >= 0 && (r.empresas_cortadas || 0) > cortesPrevios) {
      b.classList.remove('late'); void b.offsetWidth; b.classList.add('late');
    }
    cortesPrevios = mirando ? -1 : (r.empresas_cortadas || 0);
  }

  // --- the guard line ----------------------------------------------------------------------------
  const lienzo = $('guardia');
  const ctx = lienzo.getContext('2d');
  let animando = false;
  function color(nombre) { return getComputedStyle(document.documentElement).getPropertyValue(nombre).trim(); }
  function dibuja() {
    const ahora = Date.now();
    const marcas = (marcasPorPestana.get(activa) || []).filter((x) => ahora - x.t < 60000);
    marcasPorPestana.set(activa, marcas);
    const dpr = window.devicePixelRatio || 1;
    const w = lienzo.clientWidth, h = lienzo.clientHeight;
    if (lienzo.width !== Math.round(w * dpr) || lienzo.height !== Math.round(h * dpr)) { lienzo.width = Math.round(w * dpr); lienzo.height = Math.round(h * dpr); }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    const rojo = color('--rojo'), gris = color('--gris-suave');
    for (const x of marcas) {
      const edad = (ahora - x.t) / 60000;
      ctx.globalAlpha = Math.max(0.15, 1 - edad);
      ctx.fillStyle = x.cortado ? rojo : gris;
      ctx.fillRect(Math.round(w - edad * w) - (x.cortado ? 3 : 2), 0, x.cortado ? 3 : 2, h);
    }
    ctx.globalAlpha = 1;
    if (marcas.length) setTimeout(() => requestAnimationFrame(dibuja), 100); else animando = false;
  }
  function anima() { if (!animando) { animando = true; requestAnimationFrame(dibuja); } }

  let avisoTimer = 0;
  function aviso(texto) {
    const a = $('aviso-corte');
    a.textContent = texto;
    a.classList.add('visto');
    clearTimeout(avisoTimer);
    avisoTimer = setTimeout(() => a.classList.remove('visto'), 2600);
  }

  // --- what the program says ------------------------------------------------------------------
  en('estado', (m) => {
    activa = m.activa ? m.activa.id : null;
    panel = m.panel || null;
    pintaPestanas(m.pestanas || []);
    const a = m.activa || {};
    $('atras').disabled = !a.puede_atras;
    $('adelante').disabled = !a.puede_adelante;
    $('ic-recargar').classList.toggle('oculto', !!a.cargando);
    $('ic-detener').classList.toggle('oculto', !a.cargando);
    $('recargar').title = t(a.cargando ? 'detener' : 'recargar');
    const c = $('candado');
    c.className = a.interna ? 'interno' : (a.segura ? '' : 'abierto');
    c.innerHTML = a.interna ? CANDADO.interno : (a.segura ? CANDADO.seguro : CANDADO.abierto);
    c.title = t(a.interna ? 'pagina_interna' : (a.segura ? 'conexion_segura' : 'conexion_no_segura'));
    pintaDireccion(a.url, a.interna);
    for (const [id, v] of Object.entries(vistas)) $(id).classList.toggle('activo', panel === v);
    $('b-mandato').classList.toggle('activo', panel === 'mandato' || !!a.mandato);
    $('b-parar').classList.toggle('oculto', !a.mandato);
    anima();
  });
  en('escudo', (m) => { if (m.pestana === activa) pintaEscudo(m); });
  // Requests arrive in small batches (a few times a second): one mark each on the guard line,
  // and the last company stopped on this tab beside the address.
  en('pulsos', (m) => {
    let ultimo = null;
    const ahora = Date.now();
    for (const p of m.lista || []) {
      const lista = marcasPorPestana.get(p.pestana) || [];
      lista.push({ t: ahora, cortado: !!p.cortado });
      if (lista.length > 600) lista.splice(0, lista.length - 600);
      marcasPorPestana.set(p.pestana, lista);
      if (p.pestana === activa && p.cortado) ultimo = p.quien;
    }
    if (ultimo) aviso(t('ultimo_corte', { quien: ultimo }));
    anima();
  });
  en('foco_direccion', () => { campo.focus(); });
  manda({ tipo: 'listo', vista: 'barra' });
})();
