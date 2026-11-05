// The top of the window: tabs, the address, the shield and the guard line.
'use strict';
(() => {
  const { manda, en, t, tn, esc, $ } = Z;
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
  let arrastrada = null;
  function pintaPestanas(lista) {
    const cont = $('pestanas');
    cont.querySelectorAll('.pestana').forEach((p) => p.remove());
    const nueva = $('nueva');
    // 1.0.46: only the browser's own tabs; the products are in the selector above (Z3).
    const propias = lista.filter((p) => !p.producto || p.producto === 'zero');
    for (const p of propias) {
      const el = document.createElement('div');
      el.className = 'pestana' + (p.cargando ? ' cargando' : '') + (p.aislada ? ' aislada' : '') + (p.fija ? ' fija' : '');
      el.setAttribute('role', 'tab');
      el.setAttribute('aria-selected', p.activa ? 'true' : 'false');
      el.dataset.id = p.id;
      el.title = p.titulo || p.url || '';
      const letra = String(p.titulo || t('pestana_nueva')).trim().charAt(0).toUpperCase();
      const icono = p.icono && !p.cargando && /^data:image\/png;base64,/.test(p.icono) ? `<img class="icono" alt="" src="${esc(p.icono)}">` : `<i class="punto">${p.cargando ? '' : esc(letra)}</i>`;
      el.innerHTML = `${icono}<span class="titulo">${esc(p.titulo || t('pestana_nueva'))}</span>`
        + (p.fija ? '' : `<button class="cerrar" aria-label="${esc(t('cerrar_pestana'))}" title="${esc(t('cerrar_pestana'))}"><svg viewBox="0 0 12 12" width="10" height="10" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"><path d="M2.5 2.5l7 7M9.5 2.5l-7 7"/></svg></button>`);
      el.addEventListener('mousedown', (e) => {
        if (e.button === 1 && !p.fija) { e.preventDefault(); manda({ tipo: 'pestana_cerrar', id: p.id }); return; }
        if (!e.target.closest('.cerrar')) manda({ tipo: 'pestana_activar', id: p.id });
      });
      el.querySelector('.cerrar')?.addEventListener('click', () => manda({ tipo: 'pestana_cerrar', id: p.id }));
      // Drag a tab to put it where you like (16 Oct 2026): Radiografías, Navegador ZERO and
      // Comunicaciones too; the program remembers their order.
      el.draggable = true;
      el.addEventListener('dragstart', (e) => { arrastrada = p.id; el.classList.add('arrastrando'); try { e.dataTransfer.setData('text/plain', String(p.id)); e.dataTransfer.effectAllowed = 'move'; } catch (_) {} });
      el.addEventListener('dragend', () => { arrastrada = null; el.classList.remove('arrastrando'); cont.querySelectorAll('.pestana').forEach((x) => x.classList.remove('antes', 'despues')); });
      el.addEventListener('dragover', (e) => {
        if (arrastrada === null || arrastrada === p.id) return;
        e.preventDefault();
        const r = el.getBoundingClientRect(), despues = e.clientX > r.left + r.width / 2;
        el.classList.toggle('antes', !despues); el.classList.toggle('despues', despues);
      });
      el.addEventListener('dragleave', () => el.classList.remove('antes', 'despues'));
      el.addEventListener('drop', (e) => {
        e.preventDefault();
        if (arrastrada === null || arrastrada === p.id) return;
        const r = el.getBoundingClientRect(), despues = e.clientX > r.left + r.width / 2;
        const ids = lista.map((x) => x.id).filter((x) => x !== arrastrada);
        const a = ids.indexOf(p.id) + (despues ? 1 : 0);
        manda({ tipo: 'pestana_mover', id: arrastrada, a });
        el.classList.remove('antes', 'despues');
      });
      cont.insertBefore(el, nueva);
    }
  }
  $('nueva').addEventListener('click', () => manda({ tipo: 'pestana_nueva' }));

  // --- address --------------------------------------------------------------------------------
  const campo = $('campo-dir');
  const vista = $('vista-dir');
  const formDir = $('direccion');
  let urlActual = '';
  let propiaActual = false;
  // The person is typing in the address. It holds while the focus moves inside the address (to the
  // star or to «Ir», with the keyboard): what was typed stays there until the focus leaves it.
  let editando = false;
  const dentro = (el) => !!el && formDir.contains(el);
  function pintaDireccion(url, interna, propia) {
    urlActual = url || '';
    propiaActual = !!propia;
    if (!editando) campo.value = interna || propia ? '' : urlActual;
    // GUARDIANA's own pages on this computer read «🔒 GUARDIANA», never 127.0.0.1:7443 (Z3).
    if (propia) {
      vista.innerHTML = `<span class="propia">GUARDIANA</span>`;
      const ver = !editando;
      vista.classList.toggle('oculto', !ver);
      campo.style.color = ver ? 'transparent' : '';
      return;
    }
    let host = '', resto = '';
    try {
      const u = new URL(urlActual);
      // The browser's own pages show the empty field with its hint, not their address.
      if (/^https?:$/.test(u.protocol) && !interna) { host = u.host; resto = (u.pathname === '/' ? '' : u.pathname) + u.search; }
    } catch (_) {}
    vista.innerHTML = host ? `<b>${esc(host)}</b><span>${esc(resto)}</span>` : '';
    const mostrarVista = !!host && !editando;
    vista.classList.toggle('oculto', !mostrarVista);
    campo.style.color = mostrarVista ? 'transparent' : '';
  }
  campo.addEventListener('focus', (e) => {
    if (editando && dentro(e.relatedTarget)) return;
    editando = true;
    campo.value = urlActual.startsWith('https://zero.guardiana') || propiaActual ? '' : urlActual; campo.style.color = ''; vista.classList.add('oculto'); setTimeout(() => campo.select(), 0);
  });
  formDir.addEventListener('focusout', (e) => {
    if (dentro(e.relatedTarget)) return;
    editando = false;
    pintaDireccion(urlActual, urlActual.startsWith('https://zero.guardiana'), propiaActual);
  });
  // «Ir» (2 Nov 2026, the owner, 16:50: «falta el botón de Enter… tiene que estar en el navegador»):
  // a submit button of this same form, so it does exactly what Enter does. Pressed with the mouse
  // it does not take the focus from the field, as Enter would not.
  $('ir').addEventListener('mousedown', (e) => e.preventDefault());
  formDir.addEventListener('submit', (e) => {
    e.preventDefault();
    const texto = campo.value.trim();
    if (!texto) return;
    manda({ tipo: 'navegar', texto });
    // Done typing, wherever the focus was (the field, or «Ir» reached with the keyboard).
    const enFoco = document.activeElement;
    if (dentro(enFoco)) enFoco.blur();
  });
  campo.addEventListener('keydown', (e) => { if (e.key === 'Escape') { campo.value = propiaActual ? '' : urlActual; campo.blur(); } });

  // --- buttons ------------------------------------------------------------------------------
  $('atras').addEventListener('click', () => manda({ tipo: 'atras' }));
  $('adelante').addEventListener('click', () => manda({ tipo: 'adelante' }));
  $('recargar').addEventListener('click', () => manda({ tipo: $('ic-detener').classList.contains('oculto') ? 'recargar' : 'detener' }));
  const vistas = { 'b-escudo': 'escudo', 'b-menu': 'ajustes' };
  for (const [id, v] of Object.entries(vistas)) {
    $(id).addEventListener('click', () => {
      // On the browser's own pages the shield is the day's: it leads to the day's list of cuts.
      if (id === 'b-escudo' && enPropia) { manda({ tipo: 'abrir_cortes' }); return; }
      manda({ tipo: 'panel', vista: panel === v ? null : v });
    });
  }

  // --- the shield ------------------------------------------------------------------------------
  // 1.0.46 (Z1): on a web, the attempts cut on this page («23 en esta página»); on the browser's
  // own pages, the day's («1.284 hoy»). Watching without cutting, the companies seen, in amber.
  let escudoUltimo = null, hoyUltimo = null, enPropia = true;
  function pintaPastilla() {
    const b = $('b-escudo');
    if (enPropia) {
      const h = hoyUltimo || {};
      const total = Number((h.hoy && h.hoy.cortadas) || 0);
      b.classList.toggle('mirando', h.cortando === false);
      b.classList.toggle('cero', total === 0);
      $('b-escudo-n').textContent = Z.n(total);
      $('b-escudo-que').textContent = t('z1_pastilla_hoy');
      return;
    }
    const m = escudoUltimo || {};
    const r = m.resumen || {};
    const mirando = !m.cortando;
    const cifra = mirando ? (r.empresas || 0) : (r.cortadas || 0);
    b.classList.toggle('mirando', mirando);
    b.classList.toggle('cero', cifra === 0);
    $('b-escudo-n').textContent = Z.n(cifra);
    $('b-escudo-que').textContent = t(mirando ? 'z1_pastilla_empresas' : 'z1_pastilla_pagina');
  }
  function pintaEscudo(m) {
    const b = $('b-escudo');
    const r = m.resumen || {};
    const mirando = !m.cortando;
    escudoUltimo = m;
    pintaPastilla();
    if (!mirando && !enPropia && cortesPrevios >= 0 && (r.cortadas || 0) > cortesPrevios) {
      b.classList.remove('late'); void b.offsetWidth; b.classList.add('late');
    }
    cortesPrevios = mirando ? -1 : (r.cortadas || 0);
    // A pop-up just not opened on this page: said beside the address.
    const clave = `${m.pestana}:${m.carga}`;
    const e = m.emergentes || 0;
    if (clave === emergentesDe && e > emergentesPrevios) aviso(t('emergente_aviso'));
    emergentesDe = clave;
    emergentesPrevios = e;
  }
  let emergentesDe = '';
  let emergentesPrevios = 0;

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

  // --- the subscription ---------------------------------------------------------------------------
  // The browser protects the same at every plan (2 Nov 2026, the owner, 17:01: «El navegador, sí, también quiero que lo
  // dejes gratis»). The bar says the plan only where it said it before: in the trial's last two days, while the licence
  // file cannot be read, and, quietly, the free plan once the trial or the subscription is over.
  let licencia = null;
  function pintaLicencia(m) {
    licencia = m;
    const b = $('b-licencia');
    const ilegible = !!m.ilegible || m.estado === 'ilegible';
    const gratis = m.nivel === 'gratis' && !ilegible;
    let texto = '';
    if (ilegible) texto = t('licencia_pastilla_ilegible');
    else if (gratis) texto = t('licencia_pastilla_gratis');
    else if (m.estado === 'prueba' && m.dias <= 2) texto = tn('licencia_pastilla_dias', m.dias, { d: Z.n(m.dias) });
    b.textContent = texto;
    b.classList.toggle('oculto', !texto);
    b.classList.toggle('gratis', gratis);
    // While the file cannot be read, the whole sentence (with where it is) on hover.
    const titulo = ilegible ? t(m.estado === 'ilegible' ? 'licencia_ilegible_fin_gratis' : 'licencia_ilegible_plus', { datos: m.donde_datos || '' }) : t('licencia_boton_titulo');
    b.title = titulo;
    b.setAttribute('aria-label', titulo);
  }
  $('b-licencia').addEventListener('click', () => manda({ tipo: 'panel', vista: panel === 'licencia' ? null : 'licencia' }));

  // --- ZERO's three protections, beside «Protegido» (2 Nov 2026, the owner, 16:50: «está demasiado escondido… que
  // aparezca ya en la pantalla del navegador») ---------------------------------------------------------------------------
  // The same settings as «Ajustes» and the shield (`ajuste`). What the program says always wins: the switch shows the
  // state the session sends back in «estado», so the three places never disagree.
  const PROTECCIONES = [...document.querySelectorAll('#protecciones .prot')];
  for (const l of PROTECCIONES) {
    const x = l.querySelector('input');
    x.addEventListener('change', () => manda({ tipo: 'ajuste', clave: l.dataset.clave, valor: x.checked }));
  }
  function nombraProtecciones() {
    $('protecciones').setAttribute('aria-label', t('barra_protecciones'));
    for (const l of PROTECCIONES) {
      // Its own name, even when only its drawing shows; the setting's whole sentence on hover.
      l.querySelector('input').setAttribute('aria-label', t(l.dataset.nombre));
      l.title = t(l.dataset.explica);
    }
  }
  function pintaProtecciones(p) {
    if (!p) return;
    for (const l of PROTECCIONES) l.querySelector('input').checked = p[l.dataset.clave] !== false;
  }
  en('estado', (m) => pintaProtecciones(m.protecciones));
  en('textos', nombraProtecciones);

  // --- GUARDIANA's VPN (2 Nov 2026, the VPN review) -----------------------------------------------------------------------
  // While it connects, is connected or its server does not answer, its state and «Desconectar». The program asks the
  // service through the panel's own tab on this computer: no page of the web, so it works with no internet at all.
  // 3 Nov 2026, strict mode (ronda 91): «reconectando», its server silent and the tunnel down: nothing leaves until it
  // comes back or the person turns it off, and the chip says so with «Desconectar».
  const VPN_FASES = { conectando: 'vpn_conectando', conectada: 'vpn_conectada', sin_respuesta: 'vpn_sin_respuesta', reconectando: 'vpn_reconectando' };
  let vpnUltimo = null;
  function pintaVpn(m) {
    vpnUltimo = m;
    const caja = $('vpn');
    // 5 Nov 2026 (second review of 1.0.56): with the VPN stopped by an error and its block still in Windows, the chip
    // stays with «Desconectar», the only way out while no page can load.
    const bloqueada = !VPN_FASES[m.fase] && !!m.desconectar;
    const clave = VPN_FASES[m.fase] || (bloqueada ? 'vpn_bloqueo_puesto' : null);
    caja.classList.toggle('oculto', !clave);
    document.body.classList.toggle('con-vpn', !!clave);
    if (!clave) return;
    caja.dataset.fase = m.fase;
    caja.setAttribute('aria-label', t('vpn_titulo'));
    const estado = $('vpn-estado');
    estado.textContent = m.aviso ? t(m.aviso) : t(clave);
    estado.title = estado.textContent;
    const b = $('vpn-desconectar');
    b.disabled = !!m.desconectando;
    b.textContent = t(m.desconectando ? 'vpn_desconectando' : 'vpn_desconectar');
    const callada = m.fase === 'sin_respuesta' || m.fase === 'reconectando';
    caja.title = callada ? t(m.estricto ? 'vpn_estricto_no_conecta' : 'vpn_sin_respuesta_explica') : bloqueada ? t('vpn_bloqueo_puesto_explica') : '';
  }
  $('vpn-desconectar').addEventListener('click', () => manda({ tipo: 'vpn_desconectar' }));
  en('vpn', pintaVpn);
  en('textos', () => { if (vpnUltimo) pintaVpn(vpnUltimo); });

  // --- 1.0.46: the top row, GUARDIANA's in the three products ---------------------------------
  // The selector is the only navigation between them (Z3); the state says, in words, what ZERO
  // does now: cutting, or only watching (the person said no). At every plan (2 Nov 2026).
  for (const b of document.querySelectorAll('#productos [data-producto]')) {
    b.addEventListener('click', () => manda({ tipo: 'producto', producto: b.dataset.producto }));
  }
  function pintaEstadoZ() {
    const el = $('estado-z');
    const h = hoyUltimo || {};
    const mirando = h.cortando === false;
    el.classList.toggle('mirando', mirando);
    el.querySelector('span').textContent = t(mirando ? 'z1_estado_mirando' : 'z1_estado_protegido');
  }
  en('hoy', (m) => { hoyUltimo = m; pintaPastilla(); pintaEstadoZ(); });
  en('marco', (m) => { document.body.classList.toggle('solo-arriba', Number(m.barra) > 0 && Number(m.barra) <= 56); });
  en('textos', () => { $('productos').setAttribute('aria-label', t('z1_productos')); pintaPastilla(); pintaEstadoZ(); });
  en('licencia', pintaLicencia);
  en('textos', () => { if (licencia) pintaLicencia(licencia); });

  // --- what the program says ------------------------------------------------------------------
  $('estrella').addEventListener('click', () => manda({ tipo: 'favorito' }));
  en('estado', (m) => {
    activa = m.activa ? m.activa.id : null;
    panel = m.panel || null;
    for (const b of document.querySelectorAll('#productos [data-producto]')) {
      b.setAttribute('aria-current', String(b.dataset.producto === (m.producto || 'zero')));
    }
    pintaPestanas(m.pestanas || []);
    const a = m.activa || {};
    $('atras').disabled = !a.puede_atras;
    $('adelante').disabled = !a.puede_adelante;
    $('ic-recargar').classList.toggle('oculto', !!a.cargando);
    $('ic-detener').classList.toggle('oculto', !a.cargando);
    $('recargar').title = t(a.cargando ? 'detener' : 'recargar');
    const c = $('candado');
    // GUARDIANA's own panel is on this computer and never goes out: a green closed lock, not the
    // amber of an unencrypted website (17 Oct 2026, the owner).
    c.className = a.interna ? 'interno' : (a.segura || a.local ? '' : 'abierto');
    c.innerHTML = a.interna ? '' : (a.segura || a.local ? CANDADO.seguro : CANDADO.abierto);
    c.title = t(a.interna ? 'pagina_interna' : a.local ? 'conexion_local' : (a.segura ? 'conexion_segura' : 'conexion_no_segura'));
    pintaDireccion(a.url, a.interna, a.guardiana);
    enPropia = !!a.interna || !a.url;
    pintaPastilla();
    const e = $('estrella');
    e.classList.toggle('oculto', !!a.interna || !a.url);
    e.classList.toggle('puesta', !!a.favorito);
    e.title = t(a.favorito ? 'favorito_quitar' : 'favorito_guardar');
    e.setAttribute('aria-label', e.title);
    e.setAttribute('aria-pressed', String(!!a.favorito));
    for (const [id, v] of Object.entries(vistas)) $(id).classList.toggle('activo', panel === v);
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
