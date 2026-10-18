// The new tab: a greeting, the search, and what today's pages tried and what was stopped, live.
'use strict';
(() => {
  const { manda, en, t, tn, $, n } = Z;
  // «Your month in data», in the count's own form for each number.
  const fraseMes = (mes, nombreMes) => `${tn('mes_frase_empresas', mes.empresas, { mes: nombreMes, empresas: n(mes.empresas) })} ${tn('mes_frase_cortadas', mes.empresas_cortadas, { cortadas: n(mes.empresas_cortadas) })}`;
  let mes = null;
  let primeraVez = true;

  $('buscar').addEventListener('submit', (e) => {
    e.preventDefault();
    const texto = $('q').value.trim();
    if (texto) manda({ tipo: 'navegar', texto });
  });
  document.querySelectorAll('.acciones button').forEach((b) => b.addEventListener('click', () => manda({ tipo: 'panel', vista: b.dataset.vista })));
  $('encender').addEventListener('click', () => manda({ tipo: 'ajuste', clave: 'cortar_seguimiento', valor: true }));
  // Choosing the engine right in the search box; the program keeps it and says where searches go.
  $('motor').addEventListener('change', (e) => {
    manda({ tipo: 'ajuste', clave: 'buscador', valor: e.target.value });
    $('q').focus();
  });
  function pintaMotores(m) {
    const sel = $('motor');
    if (!m.motores || sel.matches(':focus')) { if (m.motor_id) sel.value = m.motor_id; return; }
    const opcion = (b) => `<option value="${Z.esc(b.id)}"${b.id === m.motor_id ? ' selected' : ''}>${Z.esc(b.nombre)}</option>`;
    // One plain list: the engines that say they keep no record of who searches what go first,
    // with no heading (the owner, 10 Oct 2026: a heading repeating their claim is not ours to make).
    sel.innerHTML = m.motores.filter((b) => b.privado).map(opcion).join('')
      + m.motores.filter((b) => !b.privado).map(opcion).join('');
    sel.value = m.motor_id;
  }

  // The sun or the moon: shows where a click takes the page, and the program keeps the choice
  // for every page of the browser.
  function pintaTema() {
    const noche = Z.oscuro();
    const b = $('tema');
    b.innerHTML = noche ? Z.ICONO.sol : Z.ICONO.luna;
    b.title = t(noche ? 'tema_dia' : 'tema_noche');
    b.setAttribute('aria-label', b.title);
  }
  $('tema').addEventListener('click', () => manda({ tipo: 'ajuste', clave: 'tema', valor: Z.oscuro() ? 'dia' : 'noche' }));
  en('tema', pintaTema);
  en('textos', pintaTema);
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', pintaTema);
  pintaTema();

  // Favourites: eight at a glance, all on request. Each one opens in this tab; its × forgets it.
  let favs = { lista: [], importar: [] };
  let todos = false;
  const VISIBLES = 8;
  function pintaFavoritos() {
    const lista = favs.lista || [];
    const ver = todos ? lista : lista.slice(0, VISIBLES);
    $('favs-rejilla').innerHTML = ver.map((f, i) => {
      let host = '';
      try { host = new URL(f.url).hostname.replace(/^www\./, ''); } catch (_) {}
      const nombre = f.titulo || host || f.url;
      const ic = f.icono ? `<img src="${Z.esc(f.icono)}" alt="">` : Z.esc((host || nombre).charAt(0).toUpperCase());
      return `<a class="fav" href="#" data-i="${i}" title="${Z.esc(nombre)}\n${Z.esc(f.url)}"><span class="fav-ic">${ic}</span><span>${Z.esc(nombre)}</span><button class="quita" data-q="${i}" title="${Z.esc(t('favorito_quitar'))}" aria-label="${Z.esc(t('favorito_quitar'))}">×</button></a>`;
    }).join('');
    $('favs-vacio').classList.toggle('oculto', lista.length > 0);
    const imp = (favs.importar || []).map((n) => `<button class="enlace" data-de="${Z.esc(n)}">${Z.esc(t('favoritos_importar_de', { navegador: n }))}</button>`).join(' · ');
    $('favs-importar').innerHTML = lista.length ? '' : imp;
    const b = $('favs-todos');
    b.classList.toggle('oculto', lista.length <= VISIBLES);
    b.textContent = todos ? t('favoritos_ver_menos') : t('favoritos_ver_todos', { n: n(lista.length) });
  }
  $('favs-rejilla').addEventListener('click', (e) => {
    const q = e.target.closest('.quita');
    const a = e.target.closest('.fav');
    if (!a) return;
    e.preventDefault();
    const lista = favs.lista || [];
    if (q) { const f = lista[Number(q.dataset.q)]; if (f) manda({ tipo: 'favorito_quitar', url: f.url }); return; }
    const f = lista[Number(a.dataset.i)];
    if (f) manda({ tipo: 'navegar', texto: f.url });
  });
  $('favs-importar').addEventListener('click', (e) => {
    const b = e.target.closest('[data-de]');
    if (b) manda({ tipo: 'importar_favoritos', de: b.dataset.de });
  });
  $('favs-todos').addEventListener('click', () => { todos = !todos; pintaFavoritos(); });
  en('favoritos', (m) => { favs = m; pintaFavoritos(); });
  en('favoritos_importados', (m) => { $('favs-hecho').textContent = m.texto || ''; });
  en('textos', pintaFavoritos);

  // The greeting and the date follow the clock, so the top of the page is never stale.
  function pintaSaludo() {
    const ahora = new Date();
    const h = ahora.getHours();
    $('saludo').textContent = t(h >= 5 && h < 12 ? 'saludo_manana' : (h >= 12 && h < 20 ? 'saludo_tarde' : 'saludo_noche'));
    const f = ahora.toLocaleDateString(document.documentElement.lang || 'es', { weekday: 'long', day: 'numeric', month: 'long' });
    $('fecha').textContent = f.charAt(0).toLocaleUpperCase() + f.slice(1);
  }
  en('textos', pintaSaludo);
  setInterval(pintaSaludo, 30000);

  // The numbers count up once, the first time the page opens: the one moment of motion here.
  function cifra(el, valor) {
    const fin = Number(valor || 0);
    if (!primeraVez || fin === 0) { el.textContent = n(fin); return; }
    const t0 = performance.now();
    const paso = (ahora) => {
      const k = Math.min(1, (ahora - t0) / 700);
      el.textContent = n(Math.round(fin * (1 - Math.pow(1 - k, 3))));
      if (k < 1) requestAnimationFrame(paso);
    };
    requestAnimationFrame(paso);
  }

  // The summary of «Lo que se cortó», right here, for the period chosen in the card's header (the
  // owner, 12 Oct 2026: «que aparezca directo en pantalla … en una sale todo»). Today stays live;
  // the list of every cut, one by one, is still one click away.
  let periodo = 'hoy';
  let ultimoHoy = null;
  let resumen = null;
  let pedido = 0;
  let pendiente = 0;
  let todasLasEmpresas = false;
  const MAX_BARRAS = 60;
  let pais = (c) => c || '';
  const lang = () => document.documentElement.lang || 'es';
  const nombrePais = (c) => (!c || c === '!dueno' ? t('cortes_sin_dueno') : c === '!sede' ? t('cortes_sin_sede') : c === '!anycast' ? t('cortes_servidor_anycast') : c[0] === '~' ? t('cortes_sin_identificar', { pais: pais(c.slice(1)) }) : c[0] === '@' ? t('cortes_servidor_en', { pais: pais(c.slice(1)) }) : pais(c));
  function pideResumen() {
    pedido = Date.now();
    manda({ tipo: 'cortes', periodo, sin_lista: true });
  }
  // Today changes as the pages work: asked again at most every five seconds.
  function pideLuego() {
    if (pendiente) return;
    pendiente = setTimeout(() => { pendiente = 0; pideResumen(); }, Math.max(0, 5000 - (Date.now() - pedido)));
  }
  document.querySelectorAll('.periodos button').forEach((b) => b.addEventListener('click', () => {
    if (periodo === b.dataset.periodo) return;
    periodo = b.dataset.periodo;
    clearTimeout(pendiente);
    pendiente = 0;
    document.querySelectorAll('.periodos button').forEach((x) => x.setAttribute('aria-pressed', String(x === b)));
    const esHoy = periodo === 'hoy';
    $('hoy').classList.toggle('pasado', !esHoy);
    $('hoy-titulo').textContent = t(esHoy ? 'inicio_en_directo' : 'inicio_resumen');
    $('solo-hoy').classList.toggle('oculto', !esHoy);
    $('periodo-frase').classList.toggle('oculto', esHoy);
    todasLasEmpresas = false;
    if (esHoy && ultimoHoy) pintaHoy(ultimoHoy);
    pideResumen();
  }));
  function sombraAbajo() {
    const ul = $('r-empresas');
    ul.classList.toggle('desborda', ul.scrollHeight > ul.clientHeight + 2);
    ul.classList.toggle('al-final', ul.scrollTop + ul.clientHeight >= ul.scrollHeight - 2);
  }
  $('r-empresas').addEventListener('scroll', sombraAbajo, { passive: true });
  window.addEventListener('resize', sombraAbajo);
  function barras(lista) {
    const max = Math.max(1, ...lista.map((x) => x.n));
    const corte = todasLasEmpresas ? lista.length : MAX_BARRAS;
    const resto = lista.slice(corte).reduce((a, x) => a + x.n, 0);
    const li = lista.slice(0, corte).map((x) => `<li><span class="nombre"><button class="enlace-empresa" data-empresa="${Z.esc(x.quien)}">${Z.esc(x.quien)}</button>${x.pais ? `<small>${Z.esc(nombrePais(x.pais))}</small>` : ''}</span><span class="n">${n(x.n)}</span><span class="barra"><i style="width:${Math.max(2, Math.round(100 * x.n / max))}%"></i></span></li>`);
    if (resto) li.push(`<li><span class="nombre"><button class="enlace" id="r-empresas-todas">${Z.esc(tn('cortes_ver_otras', lista.length - corte, { n: n(lista.length - corte) }))}</button></span><span class="n">${n(resto)}</span></li>`);
    return li.join('');
  }
  const simple = (lista, nombre) => lista.slice(0, 10).map((x) => `<li><span title="${Z.esc(nombre(x))}">${Z.esc(nombre(x))}</span><span class="n">${n(x.n)}</span></li>`).join('');
  function pintaResumen() {
    const d = resumen;
    if (!d || d.periodo !== periodo) return;
    const dia = d.dia || {};
    $('r-n-empresas').textContent = n(d.empresas.length);
    $('r-n-paises').textContent = n(d.paises.filter((x) => x.pais && x.pais[0] !== '~' && x.pais[0] !== '!').length);
    $('r-n-webs').textContent = n(d.paginas.filter((x) => x.pagina).length);
    $('r-n-cookies').textContent = n(dia.avisos_cookies);
    $('r-n-emergentes').textContent = n(dia.emergentes || 0);
    $('resumen').classList.toggle('oculto', !d.total);
    $('r-empresas').innerHTML = barras(d.empresas);
    const mas = $('r-empresas-todas');
    if (mas) mas.addEventListener('click', () => { todasLasEmpresas = true; pintaResumen(); });
    sombraAbajo();
    $('r-paises').innerHTML = simple(d.paises, (x) => nombrePais(x.pais));
    $('r-paises-nota').classList.toggle('oculto', !d.paises.slice(0, 10).some((x) => !x.pais || x.pais[0] === '!' || x.pais[0] === '@'));
    $('r-paginas').innerHTML = simple(d.paginas, (x) => x.pagina || t('pestana_aislada'));
    $('r-motivos').innerHTML = d.motivos.map((x) => `<span class="chip rojo">${Z.esc(t('motivo_' + x.motivo))}<b>${n(x.n)}</b></span>`).join('');
    $('r-tipos').innerHTML = d.tipos.map((x) => `<span class="chip">${Z.esc(t('recurso_' + x.recurso))}<b>${n(x.n)}</b></span>`).join('');
    if (periodo !== 'hoy') {
      // Another period: the four figures and the trail are that period's.
      $('h-cortadas').textContent = n(dia.cortadas || d.total);
      $('h-datos').textContent = n(dia.datos_salvados);
      $('h-parametros').textContent = n(dia.parametros_quitados);
      $('h-corredores').textContent = n(dia.corredores);
      $('rastro-caja').classList.toggle('oculto', !Z.rastro($('rastro'), d.rastro));
      const f = (x) => new Date(x + 'T12:00:00').toLocaleDateString(lang(), { day: 'numeric', month: 'long' });
      const cortadas = dia.cortadas || d.total;
      const rango = d.desde === d.hasta ? t('inicio_periodo_el', { dia: f(d.desde) }) : t('inicio_periodo_del', { desde: f(d.desde), hasta: f(d.hasta) });
      $('periodo-frase').textContent = d.total
        ? `${rango}: ${tn('inicio_periodo_cortadas', cortadas, { n: n(cortadas) })} ${tn('inicio_periodo_empresas', d.empresas.length, { n: n(d.empresas.length) })}.`
        : t('cortes_vacio');
    }
  }
  let reintentos = 0;
  en('cortes', (m) => {
    resumen = m; pintaResumen();
    if (m.pendientes > 0 && reintentos < 10) { reintentos += 1; setTimeout(() => manda({ tipo: 'cortes', periodo, sin_lista: true }), 2000); } else if (!m.pendientes) reintentos = 0;
  });
  en('textos', () => {
    try { const dn = new Intl.DisplayNames([lang()], { type: 'region' }); pais = (c) => { try { return dn.of(c); } catch (_) { return c; } }; } catch (_) {}
    $('hoy-titulo').textContent = t(periodo === 'hoy' ? 'inicio_en_directo' : 'inicio_resumen');
    if (resumen) pintaResumen(); else pideResumen();
  });
  document.addEventListener('visibilitychange', () => { if (document.visibilityState === 'visible') pideResumen(); });

  en('hoy', (m) => {
    ultimoHoy = m;
    pintaHoy(m);
    // A tab nobody is looking at does not ask: it catches up when it is shown again.
    if (periodo === 'hoy' && document.visibilityState === 'visible') pideLuego();
  });
  function pintaHoy(m) {
    const h = m.hoy || {};
    const frase = $('frase');
    if (h.empresas) {
      frase.classList.remove('cero');
      frase.innerHTML = `${Z.esc(tn('inicio_hoy_1', h.empresas, { empresas: n(h.empresas) }))} <b class="rojo">${Z.esc(tn('inicio_hoy_2', h.empresas_cortadas, { cortadas: n(h.empresas_cortadas) }))}</b>`;
      $('barra').style.width = `${Math.round((100 * (h.empresas_cortadas || 0)) / h.empresas)}%`;
      $('l-cortadas').textContent = tn('inicio_barra_cortadas', h.empresas_cortadas, { n: n(h.empresas_cortadas) });
      $('l-intentaron').textContent = tn('inicio_barra_intentaron', h.empresas, { n: n(h.empresas) });
    } else {
      frase.classList.add('cero');
      frase.textContent = t('inicio_hoy_cero');
    }
    // Live while protecting; amber, not green, when the cuts are off.
    $('hoy').classList.toggle('apagada', !m.cortando);
    if (periodo === 'hoy') {
      cifra($('h-cortadas'), h.cortadas);
      cifra($('h-datos'), h.datos_salvados);
      cifra($('h-parametros'), h.parametros_quitados);
      cifra($('h-corredores'), h.corredores);
      primeraVez = false;
      $('rastro-caja').classList.toggle('oculto', !Z.rastro($('rastro'), m.rastro_hoy));
    }
    $('apagado').classList.toggle('oculto', !!m.cortando);
    mes = m.mes || {};
    const nombreMes = new Date().toLocaleDateString(document.documentElement.lang, { month: 'long' });
    $('mes-frase').textContent = fraseMes(mes, nombreMes);
    const rm = m.rastro_mes || {};
    const primero = (rm.seguidores || [])[0];
    $('mes-rastro').classList.toggle('oculto', !primero);
    if (primero) $('mes-rastro').textContent = t('mes_rastro', { quien: primero.quien, webs: n(primero.webs), total: n(rm.webs) });
    pintaMotores(m);
  }

  // «Your month in data» as an image: only the month's figures, never a website or the history.
  $('mes-guardar').addEventListener('click', async () => {
    const c = $('lienzo');
    const x = c.getContext('2d');
    await document.fonts.ready;
    x.fillStyle = '#06080C'; x.fillRect(0, 0, 1200, 630);
    x.strokeStyle = 'rgba(11,16,32,.05)';
    for (let i = 0; i < 1200; i += 44) { x.beginPath(); x.moveTo(i, 0); x.lineTo(i, 630); x.stroke(); }
    for (let j = 0; j < 630; j += 44) { x.beginPath(); x.moveTo(0, j); x.lineTo(1200, j); x.stroke(); }
    x.fillStyle = '#9CC3E4'; x.beginPath(); x.arc(84, 84, 9, 0, Math.PI * 2); x.fill();
    x.fillStyle = '#E8ECF1'; x.font = "600 24px Schibsted Grotesk"; x.fillText('GUARDIANA ZERO', 108, 93);
    const nombreMes = new Date().toLocaleDateString(document.documentElement.lang, { month: 'long', year: 'numeric' });
    x.fillStyle = '#A9B3BE'; x.font = '500 22px "Hanken Grotesk"'; x.fillText(t('mes_titulo') + ' · ' + nombreMes, 72, 170);
    x.fillStyle = '#E8ECF1'; x.font = "600 46px Schibsted Grotesk";
    const frase = fraseMes(mes, new Date().toLocaleDateString(document.documentElement.lang, { month: 'long' }));
    const palabras = frase.split(' '); let linea = ''; let y = 250;
    for (const p of palabras) {
      const prueba = linea ? linea + ' ' + p : p;
      if (x.measureText(prueba).width > 1056) { x.fillText(linea, 72, y); y += 60; linea = p; } else linea = prueba;
    }
    x.fillText(linea, 72, y);
    x.fillStyle = '#9CC3E4'; x.font = '500 20px "Hanken Grotesk"'; x.fillText('guardianagroup.com', 72, 570);
    x.fillStyle = '#A9B3BE'; x.font = '400 18px "Hanken Grotesk"'; x.fillText(t('lema'), 72, 540);
    manda({ tipo: 'guardar_imagen', nombre: 'guardiana-zero-mes.png', datos: c.toDataURL('image/png') });
  });

  en('guardado', (m) => { const g = $('guardada'); g.textContent = m.texto || ''; g.classList.remove('oculto'); });

  manda({ tipo: 'listo', vista: 'inicio' });
})();
