// GUARDIANA SECURE — the monitor the person sees the whole time (the owner, 15 Oct 2026:
// «monitoreo en pantalla constante»). It draws exactly the `monitor` value the program sends
// (crates/secure/src/monitor.rs); a field the program does not know arrives as null and is drawn
// as «—». Nothing here is computed or guessed, and no address ever reaches this page.
'use strict';
(() => {
  const { manda, en, t, esc, $, n } = Z;
  const RAYA = '—';
  const hora = (s) => (s ? new Date(s * 1000).toLocaleTimeString(document.documentElement.lang || 'es', { hour: '2-digit', minute: '2-digit', second: '2-digit' }) : RAYA);
  const fecha = (s) => (s ? new Date(s * 1000).toLocaleDateString(document.documentElement.lang || 'es', { day: 'numeric', month: 'short', year: 'numeric' }) : RAYA);
  const fechaHora = (s) => (s ? new Date(s * 1000).toLocaleString(document.documentElement.lang || 'es') : RAYA);
  const sino = (v) => (v === true ? t('sec_si') : v === false ? t('sec_no') : RAYA);
  const num = (v, unidad) => (v === null || v === undefined ? RAYA : n(v) + (unidad || ''));
  const NIVEL = { info: 'sec_nivel_info', observada: 'sec_nivel_observada', inusual: 'sec_nivel_inusual', no_verificable: 'sec_nivel_no_verificable' };
  const CHIP = { info: '', observada: ' aviso', inusual: ' aviso', no_verificable: '' };

  const lista = (ol, vacio, sucesos) => {
    const xs = sucesos || [];
    ol.innerHTML = xs.slice().reverse().map((s) => `<li><time>${esc(hora(s.cuando))}</time><span>${esc(s.texto)}</span>`
      + `<span class="chip${CHIP[s.nivel] || ''}">${esc(t(NIVEL[s.nivel] || 'sec_nivel_info'))}</span></li>`).join('');
    vacio.hidden = xs.length > 0;
  };

  // The one state on top, from the same facts the details show. Rules agreed on 15 Oct 2026
  // with Grok and ChatGPT: green only when verified, encrypted, direct (or an accepted relay),
  // loss under 3 % and nothing observed; amber when something was seen, the relay is in use or
  // loss is 3-8 %; red when the identity is not verified, a key changed or loss stays over 8 %.
  function estadoGlobal(m) {
    const id = m.identidad, tr = m.transporte || {}, vistos = m.vigilancia || [];
    if (!id) return 'sin';
    if ((m.sesion || {}).estado === 'pausada' || id.estado === 'clave_cambiada') return 'pausada';
    const perdida = typeof tr.perdida_pct === 'number' ? tr.perdida_pct : 0;
    if (perdida > 8 || vistos.some((s) => s.nivel === 'inusual')) return 'pausada';
    if (vistos.length > 0 || tr.directo === false || perdida >= 3) return 'revisar';
    return 'protegida';
  }
  const linea = (el, clase, texto) => { el.className = clase; el.textContent = texto; };
  function pintaGlobal(m) {
    const g = estadoGlobal(m), id = m.identidad, tr = m.transporte || {}, vistos = m.vigilancia || [];
    const caja = $('g');
    caja.classList.remove('protegida', 'revisar', 'pausada', 'sin');
    caja.classList.add(g);
    $('g-palabra').textContent = t('sec_g_' + g);
    const porCausa = g === 'pausada' && id.estado !== 'clave_cambiada' && ((m.sesion || {}).motivo || {}).por !== 'clave_cambiada';
    $('g-frase').textContent = t(porCausa ? 'sec_g_pausada_frase_causa' : 'sec_g_' + g + '_frase');
    const chip = $('m-estado-chip');
    chip.textContent = g === 'sin' ? t('sec_sin_sesion') : t('sec_g_' + g);
    chip.className = 'chip' + (g === 'protegida' ? ' ok' : g === 'revisar' ? ' aviso' : g === 'pausada' ? ' alerta' : '');
    if (!id) {
      linea($('g-cifrado'), '', t('sec_l_cifrado_no'));
      linea($('g-identidad'), '', t('sec_l_sin_verificar'));
      linea($('g-conexion'), '', t('sec_l_sin_conexion'));
      linea($('g-visto'), '', t('sec_l_nada'));
    } else {
      linea($('g-cifrado'), 'ok', t('sec_l_cifrado'));
      if (id.estado === 'clave_cambiada') linea($('g-identidad'), 'alerta', t('sec_l_clave_cambiada', { n: id.nombre }));
      else linea($('g-identidad'), 'ok', t('sec_l_verificado', { n: id.nombre, f: fecha(id.verificado_en) }));
      if (tr.directo === true) linea($('g-conexion'), typeof tr.perdida_pct === 'number' && tr.perdida_pct >= 3 ? 'aviso' : 'ok', typeof tr.perdida_pct === 'number' && tr.perdida_pct >= 3 ? t('sec_l_perdida', { p: n(tr.perdida_pct) }) : t('sec_l_directa'));
      else if (tr.directo === false) linea($('g-conexion'), 'aviso', t('sec_l_rele'));
      else linea($('g-conexion'), '', t('sec_l_sin_conexion'));
      if (vistos.length === 0) linea($('g-visto'), 'ok', t('sec_l_nada'));
      else linea($('g-visto'), vistos.some((s) => s.nivel === 'inusual') ? 'alerta' : 'aviso', vistos.length === 1 ? t('sec_l_visto_uno', { t: vistos[0].texto }) : t('sec_l_visto', { n: n(vistos.length), t: vistos[vistos.length - 1].texto }));
    }
    const abierto = $('g-mas').getAttribute('aria-expanded') === 'true';
    $('g-mas').textContent = t(abierto ? 'sec_ocultar_detalles' : 'sec_ver_detalles');
  }

  // The stop card: why we stopped, what happens now, and when it resumes. Only while stopped.
  let claseSesion = 'chat';
  const tiempo = (s) => `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
  function pintaPausa(m) {
    const se = m.sesion || {};
    const caja = $('p');
    if (se.estado !== 'pausada') { caja.hidden = true; return; }
    caja.hidden = false;
    const mo = se.motivo || {};
    $('p-titulo').textContent = t('sec_p_titulo_' + claseSesion);
    $('p-motivo').textContent = mo.por === 'clave_cambiada' ? t('sec_l_clave_cambiada', { n: mo.nombre }) : (mo.texto || RAYA);
    $('p-hora').textContent = hora(se.desde);
    $('p-desde').textContent = t('sec_p_desde', { t: tiempo(Math.max(0, Math.floor(Date.now() / 1000) - (se.desde || 0))) });
    const clave = mo.por === 'clave_cambiada';
    $('p-reanuda').textContent = clave ? t('sec_p_reanuda_clave', { n: mo.nombre }) : t('sec_p_reanuda_sola', { s: n(se.comprueba_cada_s || 2) });
    const pend = se.pendiente || [];
    $('p-pendiente').innerHTML = pend.length
      ? `<li>${esc(t('sec_p_pendiente'))}</li>` + pend.map((x) => `<li>${esc(x)}</li>`).join('')
      : (clave ? '' : `<li class="lista">${esc(t('sec_p_lista'))}</li>`);
    $('p-reanudar').hidden = !se.puede_reanudar; $('p-reanudar').textContent = t('sec_p_reanudar');
    $('p-nota').hidden = !se.puede_reanudar;
    $('p-verificar').hidden = !clave; $('p-verificar').textContent = t('sec_p_verificar');
    $('p-prueba').textContent = t('sec_p_prueba');
    $('p-terminar').textContent = t('sec_p_terminar');
  }
  $('p-reanudar').addEventListener('click', () => manda({ tipo: 'secure', accion: 'reanudar' }));
  $('p-verificar').addEventListener('click', () => manda({ tipo: 'secure', accion: 'verificar' }));
  $('p-prueba').addEventListener('click', () => manda({ tipo: 'secure', accion: 'prueba' }));
  $('p-terminar').addEventListener('click', () => manda({ tipo: 'secure', accion: 'terminar' }));
  // The «paused for» clock ticks on its own; the program resends the state every second anyway.
  setInterval(() => { if (!$('p').hidden) pintaPausa(ultimo); }, 1000);

  function pinta(m) {
    if (m.clase) claseSesion = m.clase;
    pintaGlobal(m);
    pintaPausa(m);
    const id = m.identidad;
    $('m-latido').classList.toggle('vivo', !!id);
    $('m-nombre').textContent = id ? id.nombre : RAYA;
    $('m-estado').textContent = id ? t(id.estado === 'clave_cambiada' ? 'sec_estado_cambiada' : 'sec_estado_verificado') : RAYA;
    $('m-verificado').textContent = id ? fechaHora(id.verificado_en) : RAYA;
    $('m-huella').textContent = id ? id.huella : RAYA;
    $('m-propia').textContent = id ? id.propia : RAYA;
    $('m-otra').textContent = id ? id.contacto : RAYA;

    const c = m.cifrado || {};
    $('m-algoritmo').textContent = c.algoritmo || RAYA;
    $('m-enviados').textContent = num(c.enviados);
    $('m-recibidos').textContent = num(c.recibidos);
    $('m-ultimo').textContent = hora(c.ultimo);
    $('m-sframe').textContent = sino(c.sframe);

    const tr = m.transporte || {};
    $('m-directo').textContent = tr.directo === true ? t('sec_transporte_directo') : tr.directo === false ? t('sec_transporte_rele') : RAYA;
    $('m-dtls').textContent = sino(tr.dtls_verificado);
    $('m-latencia').textContent = num(tr.latencia_ms, ' ms');
    $('m-perdida').textContent = num(tr.perdida_pct, ' %');
    $('m-jitter').textContent = num(tr.jitter_ms, ' ms');
    $('m-audio').textContent = tr.codec_audio ? `${tr.codec_audio} · ${num(tr.kbps_audio, ' kb/s')}` : RAYA;
    $('m-video').textContent = tr.codec_video ? `${tr.codec_video} · ${tr.resolucion || RAYA} · ${num(tr.kbps_video, ' kb/s')}` : RAYA;

    lista($('m-vigilancia'), $('m-vigilancia-vacia'), m.vigilancia);
    lista($('m-historico'), $('m-historico-vacio'), m.historico);
  }

  // The numbers only when asked for. The choice is remembered on this computer only.
  $('g-mas').addEventListener('click', () => {
    const abierto = $('g-mas').getAttribute('aria-expanded') !== 'true';
    $('g-mas').setAttribute('aria-expanded', String(abierto));
    $('detalles').hidden = !abierto;
    $('g-mas').textContent = t(abierto ? 'sec_ocultar_detalles' : 'sec_ver_detalles');
    try { localStorage.setItem('secure-detalles', abierto ? '1' : ''); } catch (_) {}
  });
  try { if (localStorage.getItem('secure-detalles') === '1') { $('g-mas').setAttribute('aria-expanded', 'true'); $('detalles').hidden = false; } } catch (_) {}
  let ultimo = {};
  en('monitor', (m) => { ultimo = m.monitor || {}; pinta(ultimo); });
  // The texts arrive after the page: draw again so «—» and the chips speak the person's language.
  en('textos', () => pinta(ultimo));
  manda({ tipo: 'listo', vista: 'secure' });
})();
