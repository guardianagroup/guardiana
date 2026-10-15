// GUARDIANA SECURE — the monitor the person sees the whole time (the owner, 15 Oct 2026:
// «monitoreo en pantalla constante»). It draws exactly the `monitor` value the program sends
// (crates/secure/src/monitor.rs); a field the program does not know arrives as null and is drawn
// as «—». Nothing here is computed or guessed, and no address ever reaches this page.
'use strict';
(() => {
  const { manda, en, t, esc, $, n } = Z;
  const RAYA = '—';
  const hora = (s) => (s ? new Date(s * 1000).toLocaleTimeString(document.documentElement.lang || 'es', { hour: '2-digit', minute: '2-digit', second: '2-digit' }) : RAYA);
  const fecha = (s) => (s ? new Date(s * 1000).toLocaleString(document.documentElement.lang || 'es') : RAYA);
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

  function pinta(m) {
    const id = m.identidad;
    $('m-latido').classList.toggle('vivo', !!id);
    const chip = $('m-estado-chip');
    if (!id) {
      chip.textContent = t('sec_sin_sesion'); chip.className = 'chip';
    } else if (id.estado === 'clave_cambiada') {
      chip.textContent = t('sec_estado_cambiada'); chip.className = 'chip alerta';
    } else {
      chip.textContent = t('sec_estado_verificado'); chip.className = 'chip ok';
    }
    $('m-nombre').textContent = id ? id.nombre : RAYA;
    $('m-estado').textContent = id ? t(id.estado === 'clave_cambiada' ? 'sec_estado_cambiada' : 'sec_estado_verificado') : RAYA;
    $('m-verificado').textContent = id ? fecha(id.verificado_en) : RAYA;
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

  let ultimo = {};
  en('monitor', (m) => { ultimo = m.monitor || {}; pinta(ultimo); });
  // The texts arrive after the page: draw again so «—» and the chips speak the person's language.
  en('textos', () => pinta(ultimo));
  manda({ tipo: 'listo', vista: 'secure' });
})();
