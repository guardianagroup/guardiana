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
  // 1.0.46 (C2): two initials, «Laura Méndez» → «LM», «Oscar» → «O».
  const iniciales = (x) => { const p = String(x || '').trim().split(/\s+/).filter(Boolean); return p.length ? (Array.from(p[0])[0] + (p.length > 1 ? Array.from(p[p.length - 1])[0] : '')).toUpperCase() : '·'; };
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
    if (vistos.length > 0 || perdida >= 3) return 'revisar';
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
    chip.textContent = t('sec_g_' + g);
    chip.className = 'chip' + (g === 'protegida' ? ' ok' : g === 'revisar' ? ' aviso' : g === 'pausada' ? ' alerta' : '');
    if (!id) {
      linea($('g-cifrado'), '', t('sec_l_cifrado_no'));
      linea($('g-identidad'), '', t('sec_l_sin_verificar'));
      linea($('g-nivel'), '', t('sec_l_nivel_pc'));
      linea($('g-cuantico'), '', t('sec_l_cuantico_no'));
      linea($('g-conexion'), '', t('sec_l_sin_conexion'));
      linea($('g-visto'), '', t('sec_l_nada'));
    } else {
      linea($('g-cifrado'), 'ok', t('sec_l_cifrado'));
      if (id.estado === 'clave_cambiada') linea($('g-identidad'), 'alerta', t('sec_l_clave_cambiada', { n: id.nombre }));
      else linea($('g-identidad'), 'ok', t('sec_l_verificado', { n: id.nombre, f: fecha(id.verificado_en) }));
      // The level comes from the two ends: both computers, the highest; a phone on either side
      // is a web page inside Safari or Chrome, strong but a step below (16 Oct 2026).
      const conMovil = !!(E.otro_movil || ((E.movil || {}).habla));
      linea($('g-nivel'), 'ok', t(conMovil ? 'sec_l_nivel_movil' : 'sec_l_nivel_pc'));
      // The post-quantum lock (16 Oct 2026): on once both sides agreed it inside Olm.
      linea($('g-cuantico'), E.cuantico ? 'ok' : '', t(E.cuantico ? 'sec_l_cuantico' : 'sec_l_cuantico_no'));
      if (tr.directo === true) linea($('g-conexion'), typeof tr.perdida_pct === 'number' && tr.perdida_pct >= 3 ? 'aviso' : 'ok', typeof tr.perdida_pct === 'number' && tr.perdida_pct >= 3 ? t('sec_l_perdida', { p: n(tr.perdida_pct) }) : t('sec_l_directa'));
      else if (tr.directo === false) linea($('g-conexion'), typeof tr.perdida_pct === 'number' && tr.perdida_pct >= 3 ? 'aviso' : 'ok', t('sec_l_rele'));
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
    // The incident card says it all; the stop card would repeat it.
    if (se.estado !== 'pausada' || (E && E.incidente)) { caja.hidden = true; return; }
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
    // 1.0.42: no way to skip a stop; it resumes on its own when the cause is gone.
    $('p-reanudar').hidden = true;
    $('p-nota').hidden = true;
    $('p-verificar').hidden = !clave; $('p-verificar').textContent = t('sec_p_verificar');
    $('p-prueba').textContent = t('sec_p_prueba');
    $('p-terminar').textContent = t('sec_p_terminar');
  }
  $('p-verificar').addEventListener('click', () => manda({ tipo: 'secure', accion: 'verificar' }));
  $('p-prueba').addEventListener('click', () => manda({ tipo: 'secure', accion: 'prueba' }));
  $('p-terminar').addEventListener('click', () => manda({ tipo: 'secure', accion: 'cerrar' }));
  // The «paused for» clock ticks on its own; the program resends the state every second anyway.
  setInterval(() => { if (!$('p').hidden) pintaPausa(ultimo); }, 1000);

  // The incident card (16 Oct 2026): a stop that looks like an interference. Red, on top, with
  // what tripped and by which path: «directo por la misma red», «directo» or «por relé · ciudad».
  // Never the other end's address, not on the card and not in the saved report (18 Oct 2026, the
  // owner: «nunca guardar IPs»). Nothing is kept unless the person saves the report.
  let I = null;
  function pintaIncidente(m) {
    const i = m.incidente || null;
    const caja = $('inc');
    if (!i) { I = null; caja.hidden = true; return; }
    caja.hidden = false;
    if (I && I.cuando === i.cuando && I.senal === i.senal) return;
    I = i;
    const filas = [
      [t('sec_i_que'), t('sec_i_s_' + i.senal) + (i.fallos > 1 ? ` (${n(i.fallos)})` : '')],
      [t('sec_i_cuando'), hora(i.cuando)],
      [t('sec_identidad_contacto'), i.contacto || RAYA],
      [t('sec_i_camino'), caminoInc(i)],
    ];
    if (i.zona_otra) filas.push([t('sec_i_zona'), i.zona_otra]);
    $('inc-datos').innerHTML = filas.map(([a, b]) => `<dt>${esc(a)}</dt><dd>${esc(b)}</dd>`).join('');
  }
  // The path as the card says it: «directo por la misma red» or «por relé · ciudad»; the
  // other end's address is never on the card nor in the report (18 Oct 2026, the owner).
  function caminoInc(i) { return t('sec_i_c_' + i.transporte) + (i.ciudad_rele ? ' · ' + i.ciudad_rele : ''); }
  en('secure_incidente', (m) => {
    descarga(`guardiana-secure-incidente-${sello()}.json`, 'application/json', m.json);
    let j = {}; try { j = JSON.parse(m.json); } catch (_) {}
    const i = j.incidente || {}, pr = j.prueba || {};
    const L = []; let y = 790;
    L.push({ x: 56, y, tam: 18, negrita: true, texto: 'GUARDIANA SECURE' }); y -= 26;
    L.push({ x: 56, y, tam: 13, negrita: true, texto: t('sec_i_titulo') }); y -= 18;
    L.push({ x: 56, y, tam: 9.5, texto: t('sec_r_pdf_fecha') + ' ' + new Date().toLocaleString(document.documentElement.lang || 'es') }); y -= 30;
    const filas = [[t('sec_i_que'), t('sec_i_s_' + i.senal) + (i.fallos > 1 ? ` (${i.fallos})` : '')], [t('sec_i_cuando'), hora(i.cuando)], [t('sec_identidad_contacto'), i.contacto || RAYA], [t('sec_i_camino'), caminoInc(i)], [t('sec_i_zona'), i.zona_otra || RAYA]];
    for (const [a, b] of filas) { L.push({ x: 56, y, tam: 11, negrita: true, texto: a }); for (const s of parte(b, 60)) { L.push({ x: 230, y, tam: 11, texto: s }); y -= 16; } y -= 4; }
    y -= 10;
    for (const s of parte(i.detalle || '', 95)) { L.push({ x: 56, y, tam: 10, texto: s }); y -= 14; }
    y -= 12;
    L.push({ x: 56, y, tam: 11, negrita: true, texto: t('sec_r_pdf_firmado') }); y -= 16;
    for (const s of parte(pr.firmante || '', 80)) { L.push({ x: 56, y, tam: 8.5, texto: s }); y -= 12; }
    y -= 6;
    for (const s of parte(pr.firma || '', 80)) { L.push({ x: 56, y, tam: 8.5, texto: s }); y -= 12; }
    y -= 18;
    for (const s of parte(t('sec_i_pdf_nota'), 95)) { L.push({ x: 56, y, tam: 9.5, texto: s }); y -= 14; }
    const a = document.createElement('a'); a.href = URL.createObjectURL(new Blob([pdfDe(L)], { type: 'application/pdf' })); a.download = `guardiana-secure-incidente-${sello()}.pdf`; document.body.appendChild(a); a.click(); setTimeout(() => { URL.revokeObjectURL(a.href); a.remove(); }, 1000);
  });
  $('inc-guardar').addEventListener('click', () => manda({ tipo: 'secure', accion: 'incidente_guardar' }));
  $('inc-verificar').addEventListener('click', () => { manda({ tipo: 'secure', accion: 'incidente_vista' }); manda({ tipo: 'secure', accion: 'verificar' }); });
  $('inc-terminar').addEventListener('click', () => { manda({ tipo: 'secure', accion: 'incidente_vista' }); manda({ tipo: 'secure', accion: 'cerrar' }); });

  function pinta(m) {
    if (m.clase) claseSesion = m.clase;
    else if (E && E.llamada) claseSesion = E.llamada.video ? 'videollamada' : 'llamada';
    else claseSesion = 'chat';
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
  // ---------- The conversation itself (15 Oct 2026) ----------
  let E = { fase: 'sola' };
  const fasePinta = () => {
    const f = E.fase;
    $('c-conexion').hidden = !(f === 'sola' || f === 'escuchando') || (modo !== 'equipos' && modo !== 'movil');
    // «Tengo una invitación» (1.0.42): only the box for the code; inviting is «Nueva conversación».
    $('c-lado-invitar').hidden = modo === 'equipos' && f !== 'escuchando';
    const comparando = E.comparando || (f === 'comparando' ? E.numeros : null);
    $('c-comparar').hidden = !comparando;
    $('c-comparar-quien').textContent = E.comparando && E.contacto_nuevo ? t('sec_s_nuevo', { n: E.contacto_nuevo }) : '';
    $('c-espera').hidden = !(f === 'saludando' || f === 'abriendo' || f === 'buscando' || (f === 'comparando' && !E.numeros));
    // A conversation that was cut keeps its messages on screen (16 Oct 2026: «se ha eliminado
    // el chat»): read only, with a line saying so, until the person starts another one.
    const cortada = f !== 'lista' && (E.mensajes || []).length > 0 && (f === 'sola' || f === 'escuchando');
    const chatOculto = $('c-chat').hidden;
    $('c-chat').hidden = f !== 'lista' && !(cortada && (modo === 'equipos' || modo === 'movil'));
    // When the conversation appears, bring it whole into view (writing bar included).
    if (chatOculto && !$('c-chat').hidden) requestAnimationFrame(() => $('c-chat').scrollIntoView({ block: 'end' }));
    document.querySelector('main').classList.toggle('con-chat', !$('c-chat').hidden || !$('c-sala-chat').hidden);
    $('c-chat').classList.toggle('cortada', cortada);
    // The link dropped and both sides look for each other again: the chat stays, read only,
    // with a line saying so, until the link is back or five minutes pass.
    const rc = E.reconectando;
    $('c-chat').classList.toggle('cortada', cortada || !!rc);
    $('c-cortada').hidden = !cortada && !rc;
    if (rc) $('c-cortada').textContent = t('sec_reconectando', { n: E.contacto || '', i: n(rc.intentos || 1) });
    else $('c-cortada').textContent = t('sec_c_cortada');
    // The key's fingerprint is no longer shown under the invitation (the owner, 15 Oct 2026: a
    // second «clave» under the code confused people). The check that matters is the numbers.
    // Waiting: the door in the router gives a GS code; no door, the blind relay gives a GR one.
    const codigo = E.codigo || E.rele_codigo;
    // While the code is being prepared, one line; once it is on screen, the code box says it all.
    if (f === 'escuchando' && !codigo) { $('c-puerto').hidden = false; $('c-puerto').textContent = E.puerta_error && E.rele_error ? t('sec_c_puerto', { p: E.puerto }) : t('sec_c_preparando'); } else $('c-puerto').hidden = true;
    $('c-escuchar').hidden = f === 'escuchando';
    $('c-codigo-caja').hidden = !(f === 'escuchando' && codigo);
    if (codigo) $('c-codigo').textContent = codigo;
    $('c-codigo-texto').textContent = t('sec_c_listo');
    $('c-via').textContent = E.codigo ? t('sec_c_via_puerta') : E.rele_codigo ? t('sec_c_via_rele') : '';
    $('c-puerta-error').hidden = !(f === 'escuchando' && E.puerta_error && !codigo && E.rele_error);
    if (E.puerta_error) $('c-puerta-error').textContent = t('sec_c_sin_puerta', { p: E.puerto, e: E.puerta_error }) + (E.rele_error ? ' ' + (String(E.rele_error).includes('GUARDIANA Free') ? t('sala_plus_rele') : t('sec_rele_fallo', { e: E.rele_error })) : '');
    $('c-espera-texto').textContent = t(f === 'saludando' ? 'sec_e_saludando' : f === 'abriendo' ? 'sec_e_abriendo' : f === 'buscando' ? 'sec_e_buscando' : 'sec_e_esperando');
    if (comparando) $('c-numeros').innerHTML = comparando.map((x) => `<b>${esc(String(x).padStart(4, '0'))}</b>`).join('');
    if (E.comparando) { if (!$('c-nombre').dataset.nuevo) { $('c-nombre').value = ''; $('c-nombre').dataset.nuevo = '1'; } }
    else { delete $('c-nombre').dataset.nuevo; if (E.contacto && !$('c-nombre').value && f !== 'lista') $('c-nombre').value = E.contacto; }
    const g = E.grupo || {};
    const nombres = g.nombres || [];
    $('c-con').textContent = nombres.length > 2 ? t('sec_m_grupo', { n: nombres.join(' · ') }) : E.contacto ? E.contacto : '';
    // The avatar: the first letter of the name (the design the owner chose, 16 Oct 2026).
    const quien = nombres.length > 2 ? nombres.filter((x) => x !== g.yo)[0] || '' : (E.contacto || '');
    $('c-avatar-letra').textContent = iniciales(quien);
    pintaLista(E, quien, nombres);
    // Under the name, three quiet dots: encrypted, verified with you, and the level.
    const idn = (E.monitor || {}).identidad;
    const verificado = !!idn && idn.estado !== 'clave_cambiada';
    const conMovilSello = !!(E.otro_movil || ((E.movil || {}).habla));
    $('c-sello').innerHTML = f === 'lista' ? [
      `<span class="ok">${esc(t('sec_s_cifrado'))}</span>`,
      `<span class="${verificado ? 'ok' : ''}">${esc(t(verificado ? 'sec_s_verificado' : 'sec_s_sin_verificar'))}</span>`,
      `<span class="${conMovilSello ? 'suave' : 'ok'}">${esc(t(conMovilSello ? 'sec_s_nivel_movil' : 'sec_s_nivel_pc'))}</span>`,
      E.cuantico ? `<span class="ok">${esc(t('sec_s_cuantico'))}</span>` : '',
    ].join('') : '';
    // The host, once someone is in: the code stays on screen to invite more.
    const mas = $('c-mas');
    if (g.anfitrion && E.codigo_grupo && nombres.length < (g.max || 6)) { mas.hidden = false; $('c-mas-codigo').textContent = E.codigo_grupo; } else mas.hidden = true;
    pintaChat(nombres.length > 2);
  };
  en('secure_estado', (m) => { E = m; ultimo = m.monitor || {}; pinta(ultimo); pintaModo(m); fasePinta(); pintaResumen(m.resumen); pintaProgreso(m); pintaIncidente(m); pintaVoz(); $('c-voz-otro').hidden = !m.otro_voz_protegida; pintaVigia(m); pintaInforme(m); aplicaPausa(m); pintaVivo(m); });
  // How far a file is, both ways, and how long it has left (16 Oct 2026).
  const duracion = (s) => { s = Math.max(0, Math.round(s)); if (s < 60) return s + ' s'; if (s < 3600) return Math.floor(s / 60) + ' min' + (s % 60 ? ' ' + (s % 60) + ' s' : ''); const h = Math.floor(s / 3600), mi = Math.floor((s % 3600) / 60); return h + ' h' + (mi ? ' ' + mi + ' min' : ''); };
  const pintaProgreso = (m) => {
    const v = m.envio, r = m.recibiendo;
    if (v && v.nombre) {
      $('c-progreso').hidden = false;
      const queda = v.pausado ? t('sec_f_esperando_enlace') : typeof v.restante_s === 'number' ? t('sec_f_quedan', { t: duracion(v.restante_s) }) : t('sec_f_calculando');
      $('c-progreso-texto').textContent = t('sec_f_enviando', { n: v.nombre, p: v.pct }) + ' · ' + queda;
      $('c-progreso-barra').style.width = (v.pct || 0) + '%';
    } else if (r && r.nombre) {
      $('c-progreso').hidden = false;
      const p = r.bytes ? Math.min(100, Math.floor(r.hecho * 100 / r.bytes)) : 0;
      $('c-progreso-texto').textContent = t('sec_f_recibiendo', { n: r.nombre, p }) + (r.bytes ? ' · ' + tam(r.hecho) + ' / ' + tam(r.bytes) : '');
      $('c-progreso-barra').style.width = p + '%';
    } else if (!enviando) {
      $('c-progreso').hidden = true;
    }
  };

  // ---------- The chat, and the files that arrived (16 Oct 2026) ----------
  // Drawn again only when something in it changed (a call sends the state often: redrawing
  // the chat each time made the screen shake). A file that arrived is a card: a small picture
  // when it is one, its size, where it is, and Abrir / Ver en la carpeta / Eliminar.
  const minis = new Map(), pedidas = new Set();
  let chatAntes = '', ultimoSuyo = null;
  const tam = (b) => (b >= 1e9 ? (b / 1e9).toFixed(1) + ' GB' : b >= 1e6 ? (b / 1e6).toFixed(1) + ' MB' : b >= 1e3 ? Math.round(b / 1e3) + ' KB' : b + ' B');
  // Voice notes: the player, or a quiet «…» while the sound comes from the program.
  const audios = new Map(), notasPropias = new Map();
  const notaVoz = (url, nombre) => `<div class="nota-voz">${url ? `<audio controls preload="metadata" src="${esc(url)}"></audio>` : `<span class="nv-etiqueta">${esc(t('sec_nv_nota'))} · …</span>`}${/cambiada/.test(nombre || '') ? `<span class="nv-etiqueta">${esc(t('sec_nv_cambiada_corta'))}</span>` : ''}</div>`;
  const tarjetaArchivo = (r) => {
    const ext = (r.nombre.split('.').pop() || '').slice(0, 4).toUpperCase();
    const mini = r.imagen && !r.borrado && minis.has(r.id) ? `<img src="${minis.get(r.id)}" alt="">` : esc(ext || '·');
    const botones = r.borrado ? '' : `<div class="a-botones">${r.abrible ? `<button type="button" class="boton chico" data-archivo="archivo_abrir" data-id="${r.id}">${esc(t('sec_f_abrir'))}</button>` : ''}<button type="button" class="boton chico secundario" data-archivo="archivo_carpeta" data-id="${r.id}">${esc(t('sec_f_carpeta'))}</button><button type="button" class="boton chico secundario" data-archivo="archivo_borrar" data-id="${r.id}">${esc(t('sec_f_eliminar'))}</button></div>`;
    return `<div class="archivo${r.borrado ? ' borrado' : ''}"><div class="a-mini">${mini}</div><div class="a-info"><b>${esc(r.nombre)}</b><span>${esc(tam(r.bytes))} · ${esc(t(r.borrado ? 'sec_f_eliminado' : 'sec_f_en_descargas'))}</span></div>${botones}</div>`;
  };
  // The state of a message of ours, as the big apps show it (16 Oct 2026): 🕓 pending,
  // ✓ sent, ✓✓ delivered, ✓✓ green seen. Plus «editado», «Mensaje eliminado», the line of a
  // call and the reactions.
  const TICK = { pendiente: '<span class="tick">🕓</span>', enviado: '<span class="tick">✓</span>', entregado: '<span class="tick">✓✓</span>', visto: '<span class="tick visto">✓✓</span>' };
  const llamadaTexto = (l) => {
    const d = l.segundos != null ? ' · ' + duracion(l.segundos) : '';
    const k = l.segundos != null ? (l.video ? 'sec_ll_video' : 'sec_ll_voz') : l.fin === 'perdida' ? (l.video ? 'sec_ll_video_perdida' : 'sec_ll_voz_perdida') : l.fin === 'sin_respuesta' ? 'sec_ll_sin_respuesta' : (l.fin === 'rechazada' || l.fin === 'rechazada_aqui') ? 'sec_ll_rechazada' : 'sec_ll_cancelada';
    return t(k) + d;
  };
  // 1.0.46 (C2-a): a file this side sent, as a card. Nothing is estimated: the speed is the
  // file's size over the measured time, and while it travels, the program's own percentage.
  const coma1 = (x) => Number(x).toLocaleString(document.documentElement.lang || 'es', { minimumFractionDigits: 1, maximumFractionDigits: 1 });
  const tarjetaEnvio = (nombre, x) => {
    // «Receptor comprobado» only with the safety numbers confirmed (owner, 16:16).
    const verificado = ((E.escudo || {}).identidad || {}).ok === true && ((E.monitor || {}).identidad || {}).estado !== 'clave_cambiada';
    const llego = x && typeof x.llego_en === 'number';
    const viaja = E.envio && E.envio.nombre === nombre;
    const cifras = [x ? tam(x.bytes) : ''];
    if (llego && x.llego_en > 0) { cifras.push(t('sala_mbs', { v: coma1(x.bytes / x.llego_en / 1e6) })); cifras.push(coma1(x.llego_en) + ' s'); }
    const insignia = llego ? `<span class="sala-insignia ok">${esc(t('sala_f_completa'))}</span>` : `<span class="sala-insignia info">${esc(viaja ? t('sala_f_en_camino', { p: n(E.envio.pct || 0) }) : t('sala_f_enviado'))}</span>`;
    const barra = viaja ? `<i class="sala-barra"><b style="width:${Math.min(100, E.envio.pct || 0)}%"></b></i>` : '';
    const pie = `<div class="sala-archivo-pie"><span class="${verificado ? 'ok' : 'aviso'}">${esc(t(verificado ? 'sala_f_receptor' : 'sala_f_receptor_no'))}</span><time>${esc(hora(x ? x.t : 0).slice(0, 5))}</time></div>`;
    return `<div class="sala-archivo-cab"><span class="sala-archivo-ico" aria-hidden="true"><svg class="icono" viewBox="0 0 24 24"><path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5"/></svg></span><span class="sala-archivo-q"><b>${esc(nombre)}</b><small>${esc(cifras.filter(Boolean).join(' · '))}</small></span>${insignia}</div>${barra}${pie}`;
  };
  // 1.0.46 (C2-a): the hour and minute under each bubble; the seconds stay in the tooltip.
  const horaCorta = (s) => (s ? new Date(s * 1000).toLocaleTimeString(document.documentElement.lang || 'es', { hour: '2-digit', minute: '2-digit' }) : RAYA);
  const metaMensaje = (m) => `<time title="${esc(hora(m.t))}">${m.editado ? esc(t('sec_m_editado')) + ' · ' : ''}${esc(horaCorta(m.t))}${m.mio && m.estado ? ' ' + (TICK[m.estado] || '') : ''}</time>`;
  const reacciones = (m) => (m.reacciones || []).length ? `<span class="reac">${m.reacciones.map((r) => `<span title="${esc(r.de || t('sec_m_tu'))}">${esc(r.emoji)}</span>`).join('')}</span>` : '';
  const citaDe = (m, lista) => { if (!m.re) return ''; const o = (lista || []).find((y) => y.id === m.re); return o ? `<span class="cita">${esc((o.texto || (o.archivo && o.archivo.nombre) || '').slice(0, 160))}</span>` : ''; };
  const lineaLlamada = (m) => `<li class="llamada-linea" data-id="${esc(m.id || '')}"><span>${m.llamada.video ? '🎥' : '📞'}</span><span>${esc(llamadaTexto(m.llamada))}</span><time>${esc(hora(m.t))}</time></li>`;
  // Like WhatsApp and Telegram (17 Oct 2026, review with ChatGPT and Grok): a date between
  // days, and lines of the same person within five minutes grouped close together.
  const diaDe = (s) => { const d = new Date((s || 0) * 1000); return d.getFullYear() * 10000 + d.getMonth() * 100 + d.getDate(); };
  const fechaLinea = (s) => {
    const hoy = diaDe(Date.now() / 1000), ayer = diaDe(Date.now() / 1000 - 86400), d = diaDe(s);
    if (d === hoy) return t('sec_fecha_hoy');
    if (d === ayer) return t('sec_fecha_ayer');
    const f = new Date(s * 1000), lang = document.documentElement.lang || 'es';
    return f.toLocaleDateString(lang, f.getFullYear() === new Date().getFullYear() ? { weekday: 'long', day: 'numeric', month: 'long' } : { day: 'numeric', month: 'long', year: 'numeric' });
  };
  const agrupa = (ol, lista) => {
    const lis = [...ol.children].slice(0, (lista || []).length);
    let antes = null;
    lis.forEach((li, i) => {
      const m = lista[i] || {};
      if (!antes || diaDe(m.t) !== diaDe(antes.t)) { const f = document.createElement('li'); f.className = 'fecha'; f.textContent = fechaLinea(m.t); ol.insertBefore(f, li); }
      else if (!m.llamada && !antes.llamada && !!m.mio === !!antes.mio && (m.de || '') === (antes.de || '') && (m.t || 0) - (antes.t || 0) < 300) li.classList.add('sigue');
      antes = m;
    });
  };
  const pintaChat = (conNombre) => {
    const recibidos = E.recibidos || [];
    const clave = JSON.stringify([E.mensajes, recibidos, E.enviados, (E.envio || {}).nombre, (E.envio || {}).pct, ((E.monitor || {}).identidad || {}).estado, conNombre, [...minis.keys()], [...audios.keys()], E.escribiendo]);
    if (clave === chatAntes) return;
    chatAntes = clave;
    const ol = $('c-mensajes'); const abajo = ol.scrollTop + ol.clientHeight >= ol.scrollHeight - 30;
    const usados = new Set();
    ol.innerHTML = (E.mensajes || []).map((m) => {
      const de = !m.mio && conNombre && m.de ? `<i class="de">${esc(m.de)}</i>` : '';
      if (m.llamada) return lineaLlamada(m);
      if (m.borrado) return `<li class="${m.mio ? 'mio' : ''}" data-id="${esc(m.id || '')}">${de}<i class="borrado">${esc(t('sec_m_borrado'))}</i>${metaMensaje(m)}</li>`;
      const nombreArchivo = m.archivo ? m.archivo.nombre : (typeof m.texto === 'string' && m.texto.startsWith('📎 ') ? m.texto.slice(3) : null);
      if (!m.mio && nombreArchivo) {
        const r = recibidos.find((x) => !usados.has(x.id) && x.nombre === nombreArchivo && Math.abs((x.t || 0) - (m.t || 0)) <= 3);
        if (r) {
          usados.add(r.id);
          if ((r.imagen || r.audio) && !r.borrado && !minis.has(r.id) && !audios.has(r.id) && !pedidas.has(r.id)) { pedidas.add(r.id); manda({ tipo: 'secure', accion: 'archivo_vista', id: r.id }); }
          // A voice note plays right in the chat, like WhatsApp.
          if (r.audio && !r.borrado) return `<li class="archivo-li" data-id="${esc(m.id || '')}">${de}${notaVoz(audios.get(r.id), r.nombre)}${reacciones(m)}<time>${esc(hora(m.t))}</time></li>`;
          return `<li class="archivo-li" data-id="${esc(m.id || '')}">${de}${tarjetaArchivo(r)}${reacciones(m)}<time>${esc(hora(m.t))}</time></li>`;
        }
      }
      if (m.mio && nombreArchivo) {
        const x = (E.enviados || []).find((y) => y.nombre === nombreArchivo && Math.abs((y.t || 0) - (m.t || 0)) <= 3);
        const estado = x && typeof x.llego_en === 'number' ? '✓ ' + t('sec_f_entregado_en', { t: duracion(x.llego_en) }) : (E.envio && E.envio.nombre === nombreArchivo ? t('sec_f_en_camino') : t('sec_f_enviado'));
        if (notasPropias.has(nombreArchivo)) return `<li class="mio" data-id="${esc(m.id || '')}">${notaVoz(notasPropias.get(nombreArchivo), nombreArchivo)}<span class="a-entrega">${esc(estado)}</span>${reacciones(m)}<time>${esc(hora(m.t))}</time></li>`;
        // 1.0.46 (C2-a): the delivery as a card, with the real figures: size, speed and time
        // from the first byte to the other side's «it is here»; «Receptor comprobado» only when
        // the person on the other end is the one whose numbers were compared.
        return `<li class="mio sala-archivo" data-id="${esc(m.id || '')}">${tarjetaEnvio(nombreArchivo, x)}${reacciones(m)}</li>`;
      }
      return `<li class="${m.mio ? 'mio' : ''}" data-id="${esc(m.id || '')}">${de}${citaDe(m, E.mensajes)}${esc(m.texto)}${reacciones(m)}${metaMensaje(m)}</li>`;
    }).join('') + (E.escribiendo ? `<li class="escribiendo"><span></span><span></span><span></span></li>` : '');
    agrupa(ol, E.mensajes);
    if (abajo) ol.scrollTop = ol.scrollHeight;
    // Seen: a new message from the other side, on a screen in front (17 Oct 2026 review).
    const suyo = (E.mensajes || []).filter((x) => !x.mio && !x.llamada).slice(-1)[0];
    if (suyo && suyo.id !== ultimoSuyo) { ultimoSuyo = suyo.id; if (!document.hidden && document.hasFocus()) manda({ tipo: 'secure', accion: 'visto' }); }
  };
  // The picture comes once, full size: keep a small copy only.
  en('secure_vista', (m) => {
    // A voice note: kept as it came, to play.
    if (typeof m.url === 'string' && m.url.startsWith('data:audio/')) { audios.set(m.id, m.url); if (typeof m.id === 'string' && m.id.startsWith('sala-')) { if (E && E.sala) pintaSala(E); return; } if (E) pintaChat(((E.grupo || {}).nombres || []).length > 2); return; }
    const img = new Image();
    img.onload = () => {
      const lado = 112, c = document.createElement('canvas'); c.width = lado; c.height = lado;
      const s = Math.min(img.width, img.height), x = (img.width - s) / 2, y = (img.height - s) / 2;
      try { c.getContext('2d').drawImage(img, x, y, s, s, 0, 0, lado, lado); minis.set(m.id, c.toDataURL('image/jpeg', 0.8)); } catch (_) {}
      if (E) pintaChat(((E.grupo || {}).nombres || []).length > 2);
    };
    img.src = m.url;
  });
  // Delete asks on the button itself: a second press within four seconds.
  $('c-mensajes').addEventListener('click', (ev) => {
    const b = ev.target.closest('button[data-archivo]');
    if (!b) return;
    const accion = b.dataset.archivo, id = Number(b.dataset.id);
    if (accion === 'archivo_borrar' && !b.classList.contains('confirmar')) {
      b.classList.add('confirmar'); b.textContent = t('sec_f_eliminar_seguro');
      setTimeout(() => { if (b.isConnected) { b.classList.remove('confirmar'); b.textContent = t('sec_f_eliminar'); } }, 4000);
      return;
    }
    manda({ tipo: 'secure', accion, id });
  });
  // The summary when a conversation ends: on screen and as a file, signed by this computer.
  const descarga = (nombre, tipo, texto) => { const a = document.createElement('a'); a.href = URL.createObjectURL(new Blob([texto], { type: tipo })); a.download = nombre; document.body.appendChild(a); a.click(); setTimeout(() => { URL.revokeObjectURL(a.href); a.remove(); }, 1000); };
  const sello = (s) => { const d = new Date((s || Math.floor(Date.now() / 1000)) * 1000); const p = (n) => String(n).padStart(2, '0'); return `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}-${p(d.getHours())}${p(d.getMinutes())}`; };
  let R = null;
  const pintaResumen = (r) => {
    R = r || null;
    const caja = $('c-resumen');
    if (!r || E.fase === 'lista') { caja.hidden = true; return; }
    caja.hidden = false;
    const filas = [[t('sec_r_clase'), t('sec_r_clase_' + r.clase)], [t('sec_identidad_contacto'), r.contacto], [t('sec_r_duracion'), tiempo(r.duracion_s)], [t('sec_r_mensajes'), n(r.mensajes)], [t('sec_r_archivos'), n(r.archivos)], [t('sec_r_vistas'), n(r.observadas + r.inusuales)]];
    $('c-resumen-datos').innerHTML = filas.map(([a, b]) => `<dt>${esc(a)}</dt><dd>${esc(b)}</dd>`).join('');
  };
  $('c-resumen-json').addEventListener('click', () => { if (R) descarga(`guardiana-secure-resumen-${sello(R.fin)}.json`, 'application/json', R.json); });
  $('c-resumen-csv').addEventListener('click', () => { if (R) descarga(`guardiana-secure-resumen-${sello(R.fin)}.csv`, 'text/csv;charset=utf-8', '\ufeff' + R.csv); });
  // The summary as a PDF too (16 Oct 2026), built here with no library: one page, Helvetica,
  // the same rows as on screen and the signature. The JSON stays the proof that can be checked.
  const pdfDe = (lineas) => {
    const MAPA = { 0x20AC: 0x80, 0x2026: 0x85, 0x2013: 0x96, 0x2014: 0x97, 0x2018: 0x91, 0x2019: 0x92, 0x201C: 0x93, 0x201D: 0x94, 0x2022: 0x95, 0xB7: 0xB7 };
    const win = (s) => { const o = []; for (const ch of String(s)) { const c = ch.codePointAt(0); if (c < 0x80 || (c >= 0xA0 && c <= 0xFF)) o.push(c); else o.push(MAPA[c] || 0x3F); } return o; };
    const cadena = (s) => { const o = [0x28]; for (const b of win(s)) { if (b === 0x28 || b === 0x29 || b === 0x5C) o.push(0x5C); o.push(b); } o.push(0x29); return o; };
    const ascii = (s) => Array.from(s, (c) => c.charCodeAt(0));
    let contenido = [];
    for (const l of lineas) contenido = contenido.concat(ascii(`BT /${l.negrita ? 'F2' : 'F1'} ${l.tam} Tf ${l.x} ${l.y} Td `), cadena(l.texto), ascii(' Tj ET\n'));
    const objetos = [
      ascii('<< /Type /Catalog /Pages 2 0 R >>'),
      ascii('<< /Type /Pages /Kids [3 0 R] /Count 1 >>'),
      ascii('<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 4 0 R /F2 5 0 R >> >> /Contents 6 0 R >>'),
      ascii('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>'),
      ascii('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>'),
      ascii(`<< /Length ${contenido.length} >>\nstream\n`).concat(contenido, ascii('\nendstream')),
    ];
    let bytes = ascii('%PDF-1.4\n%\xE2\xE3\xCF\xD3\n');
    const pos = [];
    objetos.forEach((o, i) => { pos.push(bytes.length); bytes = bytes.concat(ascii(`${i + 1} 0 obj\n`), o, ascii('\nendobj\n')); });
    const xref = bytes.length;
    let tabla = `xref\n0 ${objetos.length + 1}\n0000000000 65535 f \n`;
    pos.forEach((p) => { tabla += String(p).padStart(10, '0') + ' 00000 n \n'; });
    tabla += `trailer\n<< /Size ${objetos.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
    return new Uint8Array(bytes.concat(ascii(tabla)));
  };
  const parte = (s, n) => { const o = []; let x = String(s || ''); while (x.length > n) { o.push(x.slice(0, n)); x = x.slice(n); } if (x) o.push(x); return o; };
  $('c-resumen-pdf').addEventListener('click', () => {
    if (!R) return;
    let j = {}; try { j = JSON.parse(R.json); } catch (_) {}
    const filas = [[t('sec_r_clase'), t('sec_r_clase_' + R.clase)], [t('sec_identidad_contacto'), R.contacto], [t('sec_r_duracion'), tiempo(R.duracion_s)], [t('sec_r_mensajes'), n(R.mensajes)], [t('sec_r_archivos'), n(R.archivos)], [t('sec_r_vistas'), n(R.observadas + R.inusuales)]];
    const L = []; let y = 790;
    L.push({ x: 56, y, tam: 18, negrita: true, texto: 'GUARDIANA SECURE' }); y -= 26;
    L.push({ x: 56, y, tam: 13, negrita: true, texto: t('sec_r_pdf_titulo') }); y -= 18;
    L.push({ x: 56, y, tam: 9.5, texto: t('sec_r_pdf_fecha') + ' ' + new Date().toLocaleString(document.documentElement.lang || 'es') }); y -= 30;
    for (const [a, b] of filas) { L.push({ x: 56, y, tam: 11, negrita: true, texto: a }); L.push({ x: 230, y, tam: 11, texto: String(b) }); y -= 20; }
    y -= 16;
    L.push({ x: 56, y, tam: 11, negrita: true, texto: t('sec_r_pdf_firmado') }); y -= 16;
    for (const s of parte(j.firmante, 80)) { L.push({ x: 56, y, tam: 8.5, texto: s }); y -= 12; }
    y -= 6;
    for (const s of parte(j.firma, 80)) { L.push({ x: 56, y, tam: 8.5, texto: s }); y -= 12; }
    y -= 18;
    for (const s of parte(t('sec_r_pdf_nota'), 95)) { L.push({ x: 56, y, tam: 9.5, texto: s }); y -= 14; }
    const a = document.createElement('a'); a.href = URL.createObjectURL(new Blob([pdfDe(L)], { type: 'application/pdf' })); a.download = `guardiana-secure-resumen-${sello(R.fin)}.pdf`; document.body.appendChild(a); a.click(); setTimeout(() => { URL.revokeObjectURL(a.href); a.remove(); }, 1000);
  });
  $('c-prueba').addEventListener('click', () => manda({ tipo: 'secure', accion: 'prueba' }));
  en('secure_prueba', (m) => descarga(`guardiana-secure-prueba-${sello()}.json`, 'application/json', m.json));
  en('secure_aviso', (m) => {
    // 1.0.46: a relay asked for at the free level is not an error; the Plus line says why.
    if (typeof m.texto === 'string' && m.texto.includes('GUARDIANA Free')) { aviso(t('sala_plus_rele')); return; }
    const extra = m.texto && /^sec_[a-z0-9_]+$/.test(m.texto) ? t(m.texto) : m.texto; const txt = t(m.clave) + (extra ? ' ' + extra : ''); aviso(txt);
  });
  const avisos = [];
  function aviso(txt) {
    // With no conversation on screen the notice goes under the invitation (a wrong code, a
    // failed invitation), where the person is looking.
    if ($('c-chat').hidden) { $('c-aviso').textContent = txt; $('c-aviso').hidden = false; clearTimeout(aviso.r); aviso.r = setTimeout(() => { $('c-aviso').hidden = true; }, 15000); return; }
    avisos.push(txt); const ol = $('c-mensajes'); const li = document.createElement('li'); li.className = 'aviso-li'; li.textContent = txt; ol.appendChild(li); ol.scrollTop = ol.scrollHeight; }
  // Your name, as the others will see it (kept here; sent before inviting).
  try { $('c-yo').value = localStorage.getItem('secure_nombre') || ''; } catch (_) {}
  const mandaNombre = () => { const n = $('c-yo').value.trim().slice(0, 60); try { localStorage.setItem('secure_nombre', n); } catch (_) {} manda({ tipo: 'secure', accion: 'nombre', nombre: n }); };
  $('c-yo').addEventListener('change', mandaNombre);
  $('c-escuchar').addEventListener('click', () => { mandaNombre(); manda({ tipo: 'secure', accion: 'escuchar', puerto: 0 }); });
  $('c-mas-copiar').addEventListener('click', async () => { try { await navigator.clipboard.writeText($('c-mas-codigo').textContent); $('c-mas-copiar').textContent = t('sec_c_copiado'); setTimeout(() => { $('c-mas-copiar').textContent = t('sec_c_copiar'); }, 1500); } catch (_) {} });
  // Sending the code: WhatsApp (a tab with the message ready) or mail; the person picks who.
  const invitacion = () => t('sec_c_mensaje_invitacion', { c: $('c-codigo').textContent });
  $('c-whatsapp').addEventListener('click', () => manda({ tipo: 'navegar', texto: 'https://wa.me/?text=' + encodeURIComponent(invitacion()) }));
  $('c-correo').addEventListener('click', () => manda({ tipo: 'abrir_correo', asunto: t('sec_c_asunto_invitacion'), cuerpo: invitacion() }));
  $('c-copiar').addEventListener('click', async () => { try { await navigator.clipboard.writeText($('c-codigo').textContent); $('c-copiar').textContent = t('sec_c_copiado'); setTimeout(() => { $('c-copiar').textContent = t('sec_c_copiar'); }, 1500); } catch (_) {} });
  $('c-conectar').addEventListener('click', () => { const d = $('c-direccion').value.trim(); if (d) manda({ tipo: 'secure', accion: 'conectar', direccion: d }); });
  $('c-direccion').addEventListener('keydown', (e) => { if (e.key === 'Enter') $('c-conectar').click(); });
  // A code is typed with its dashes put in by themselves: «GR4K2…» becomes «GR-4K2…».
  $('c-direccion').addEventListener('input', (e) => {
    const el = e.target, v = el.value;
    if (/[.:]/.test(v) || (e.inputType || '').startsWith('delete')) return;
    const limpio = v.toUpperCase().replace(/[^A-Z0-9]/g, '');
    if (!/^G([SR][A-Z0-9]*)?$/.test(limpio)) return;
    const r = limpio.slice(2, 13);
    let s = limpio.slice(0, 2);
    if (r.length) s += '-' + r.slice(0, 4);
    if (r.length > 4) s += '-' + r.slice(4, 8);
    if (r.length > 8) s += '-' + r.slice(8, 11);
    if (s !== v) el.value = s;
  });
  $('c-sas-si').addEventListener('click', () => manda({ tipo: 'secure', accion: 'sas', iguales: true, nombre: $('c-nombre').value.trim() }));
  $('c-sas-no').addEventListener('click', () => manda({ tipo: 'secure', accion: 'sas', iguales: false, nombre: '' }));
  $('c-cerrar').addEventListener('click', () => manda({ tipo: 'secure', accion: 'cerrar' }));
  // Reply, edit, delete, react (16 Oct 2026): right click or long press on a message.
  let contexto = { re: null, editar: null, sala: false };
  const limpiaContexto = () => { contexto = { re: null, editar: null, sala: false }; $('c-contexto').hidden = true; $('c-sala-contexto').hidden = true; };
  const mandaTexto = (x, sala) => {
    if (contexto.editar) manda({ tipo: 'secure', accion: sala ? 'sala_editar' : 'editar', id: contexto.editar, texto: x });
    else if (contexto.re) manda({ tipo: 'secure', accion: sala ? 'sala_texto' : 'texto', texto: x, re: contexto.re });
    else manda({ tipo: 'secure', accion: sala ? 'sala_texto' : 'texto', texto: x });
    limpiaContexto();
  };
  $('c-form').addEventListener('submit', (e) => { e.preventDefault(); const x = $('c-texto').value.trim(); if (!x) return; mandaTexto(x, false); $('c-texto').value = ''; });
  let escribiEn = 0;
  const avisaEscribiendo = () => { const ahora = Date.now(); if (ahora - escribiEn > 3000) { escribiEn = ahora; manda({ tipo: 'secure', accion: 'escribiendo' }); } };
  $('c-texto').addEventListener('input', () => { if ($('c-texto').value) avisaEscribiendo(); });
  const menuMsg = $('menu-msg');
  let msgMenu = null;
  const abreMenuMsg = (li, x, y, sala) => {
    const id = li.dataset.id; if (!id) return;
    const lista = sala ? ((E.sala || {}).mensajes || []) : (E.mensajes || []);
    const m = lista.find((z) => z.id === id); if (!m) return;
    msgMenu = { m, sala };
    $('mm-editar').hidden = !m.mio || !!m.archivo || !!m.llamada || !!m.borrado;
    $('mm-emojis').hidden = !!m.llamada;
    $('mm-responder').hidden = !!m.llamada;
    menuMsg.hidden = false;
    const w = menuMsg.offsetWidth || 240, h = menuMsg.offsetHeight || 80;
    menuMsg.style.left = Math.max(8, Math.min(window.innerWidth - w - 8, x)) + 'px';
    menuMsg.style.top = Math.max(8, Math.min(window.innerHeight - h - 8, y)) + 'px';
  };
  const cierraMenuMsg = () => { menuMsg.hidden = true; msgMenu = null; };
  for (const [id, sala] of [['c-mensajes', false], ['c-sala-mensajes', true]]) {
    let pulsando = null;
    $(id).addEventListener('contextmenu', (e) => { const li = e.target.closest('li[data-id]'); if (!li) return; e.preventDefault(); abreMenuMsg(li, e.clientX, e.clientY, sala); });
    $(id).addEventListener('pointerdown', (e) => { const li = e.target.closest('li[data-id]'); if (!li || e.button !== 0) return; clearTimeout(pulsando); pulsando = setTimeout(() => abreMenuMsg(li, e.clientX, e.clientY, sala), 500); });
    for (const ev of ['pointerup', 'pointercancel', 'pointerleave']) $(id).addEventListener(ev, () => clearTimeout(pulsando));
  }
  document.addEventListener('pointerdown', (e) => { if (!e.target.closest('#menu-msg')) cierraMenuMsg(); }, true);
  document.addEventListener('keydown', (e) => { if (e.key === 'Escape') { cierraMenuMsg(); if (contexto.re || contexto.editar) { limpiaContexto(); $('c-texto').value = ''; $('c-sala-texto').value = ''; } } });
  $('mm-emojis').addEventListener('click', (e) => { const b = e.target.closest('button'); if (!b || !msgMenu) return; manda({ tipo: 'secure', accion: msgMenu.sala ? 'sala_reaccion' : 'reaccion', id: msgMenu.m.id, emoji: b.dataset.e }); cierraMenuMsg(); });
  $('mm-responder').addEventListener('click', () => { if (!msgMenu) return; const { m, sala } = msgMenu; contexto = { re: m.id, editar: null, sala }; const caja = $(sala ? 'c-sala-contexto' : 'c-contexto'); caja.hidden = false; caja.querySelector('span').textContent = t('sec_m_respondiendo', { t: (m.texto || (m.archivo && m.archivo.nombre) || '').slice(0, 100) }); $(sala ? 'c-sala-texto' : 'c-texto').focus(); cierraMenuMsg(); });
  $('mm-copiar').addEventListener('click', async () => { if (!msgMenu) return; try { await navigator.clipboard.writeText(msgMenu.m.texto || ''); } catch (_) {} cierraMenuMsg(); });
  $('mm-editar').addEventListener('click', () => { if (!msgMenu) return; const { m, sala } = msgMenu; contexto = { re: null, editar: m.id, sala }; const caja = $(sala ? 'c-sala-contexto' : 'c-contexto'); caja.hidden = false; caja.querySelector('span').textContent = t('sec_m_editando'); const campo = $(sala ? 'c-sala-texto' : 'c-texto'); campo.value = m.texto || ''; campo.focus(); cierraMenuMsg(); });
  $('mm-borrar').addEventListener('click', () => { if (!msgMenu) return; const { m, sala } = msgMenu; cierraMenuMsg(); if (m.mio && !m.borrado) { $('c-borrar-dialogo').hidden = false; $('c-borrar-todos').onclick = () => { manda({ tipo: 'secure', accion: sala ? 'sala_borrar' : 'borrar', id: m.id, para_todos: true }); $('c-borrar-dialogo').hidden = true; }; $('c-borrar-mi').onclick = () => { manda({ tipo: 'secure', accion: sala ? 'sala_borrar' : 'borrar', id: m.id, para_todos: false }); $('c-borrar-dialogo').hidden = true; }; $('c-borrar-no').onclick = () => { $('c-borrar-dialogo').hidden = true; }; } else manda({ tipo: 'secure', accion: sala ? 'sala_borrar' : 'borrar', id: m.id, para_todos: false }); });
  $('c-contexto-no').addEventListener('click', () => { limpiaContexto(); $('c-texto').value = ''; });
  $('c-sala-contexto-no').addEventListener('click', () => { limpiaContexto(); $('c-sala-texto').value = ''; });
  // Files: read here in 64 KB pieces and handed to the program one by one, so a big file never
  // sits in memory; the program encrypts each piece and sends it, and the key goes inside Olm.
  const TROZO = 64 * 1024;
  let cola = [], enviando = false;
  const b64De = (buf) => { let s = ''; const u = new Uint8Array(buf); for (let i = 0; i < u.length; i += 0x8000) s += String.fromCharCode.apply(null, u.subarray(i, i + 0x8000)); return btoa(s); };
  const enviaSiguiente = async () => {
    if (enviando || !cola.length) return;
    enviando = true;
    const f = cola.shift();
    manda({ tipo: 'secure', accion: 'archivo_empieza', nombre: f.name, bytes: f.size });
    $('c-progreso').hidden = false;
    $('c-progreso-texto').textContent = t('sec_f_preparando', { n: f.name });
    const total = Math.max(1, Math.ceil(f.size / TROZO));
    for (let i = 0; i < total; i++) {
      // Hand the pieces over only as fast as the network takes them (at most ~8 MB waiting):
      // a big file never fills the memory.
      // While the link is being recovered (`pausado`), nothing is handed over: what already
      // left waits in the program and goes on from where the other side got to.
      // During a call a file goes slowly (256 KB queued at most), so the call never waits behind it.
      while (E && E.envio && (E.envio.pausado || E.envio.cola > (E.llamada ? 256 * 1024 : 8 * 1024 * 1024))) await new Promise((r) => setTimeout(r, 150));
      const trozo = await f.slice(i * TROZO, (i + 1) * TROZO).arrayBuffer();
      const ultimo = i + 1 === total && trozo.byteLength < TROZO;
      manda({ tipo: 'secure', accion: 'archivo_trozo', b64: b64De(trozo), ultimo });
      if (i + 1 === total && !ultimo) manda({ tipo: 'secure', accion: 'archivo_trozo', b64: '', ultimo: true });
      await new Promise((r) => setTimeout(r, 0));
    }
    // The program says when it has left (E.envio disappears); wait for that before the next.
    const hasta = Date.now() + 5000;
    while (Date.now() < hasta && !(E && E.envio)) await new Promise((r) => setTimeout(r, 100));
    while (E && E.envio && E.envio.nombre) await new Promise((r) => setTimeout(r, 250));
    enviando = false;
    enviaSiguiente();
  };
  // «Compartir sin huellas» (customer request, 18 Oct 2026): a photo or a document is looked at
  // first, its headers only and by the program on this computer; when they say something about
  // the person (where, with which phone, who wrote it, which company), the person chooses: a
  // clean copy, or the file as it is. Anything else, or anything over 25 MB, goes as before.
  const HUELLAS_MAX = 25 * 1024 * 1024;
  const MIRABLE = /\.(jpe?g|png|heic|heif|avif|pdf|docx|xlsx|pptx|docm|xlsm|pptm)$/i;
  let huellasId = 0;
  const huellasEspera = new Map();
  en('secure_huellas', (m) => { const r = huellasEspera.get(m.id); if (r) { huellasEspera.delete(m.id); r(m); } });
  const desdeB64 = (s) => { const b = atob(s); const u = new Uint8Array(b.length); for (let i = 0; i < b.length; i++) u[i] = b.charCodeAt(i); return u; };
  const preguntaHuellas = (f, m) => new Promise((listo) => {
    const inf = m.informe;
    $('c-huellas-nombre').textContent = f.name;
    $('c-huellas-lista').innerHTML = (inf.huellas || []).map((h) => `<li><b>${esc(t('sec_h_' + h.clave))}</b>${h.valor ? ' ' + esc(h.valor) : ''}</li>`).join('');
    $('c-huellas-nota').textContent = inf.nota ? t('sec_h_nota_' + inf.nota) : '';
    $('c-huellas-limpia').hidden = !m.b64;
    $('c-huellas-dialogo').hidden = false;
    const fin = (x) => { $('c-huellas-dialogo').hidden = true; listo(x); };
    $('c-huellas-limpia').onclick = () => fin(new File([desdeB64(m.b64)], m.nombre_limpio, { type: f.type }));
    $('c-huellas-tal').onclick = () => fin(f);
    $('c-huellas-no').onclick = () => fin(null);
  });
  const revisa = async (f) => {
    if (f.size > HUELLAS_MAX || !MIRABLE.test(f.name)) return f;
    const b64 = b64De(await f.arrayBuffer());
    const id = ++huellasId;
    const m = await new Promise((r) => {
      huellasEspera.set(id, r);
      manda({ tipo: 'secure', accion: 'huellas', id, nombre: f.name, b64 });
      // An older program that does not know the action never answers: the file goes as before.
      setTimeout(() => { if (huellasEspera.delete(id)) r(null); }, 15000);
    });
    if (!m || !m.informe || (!(m.informe.huellas || []).length && !m.informe.nota)) return f;
    return preguntaHuellas(f, m);
  };
  const encola = async (files) => {
    const lista = [...files];
    for (const f of lista) { const g = await revisa(f); if (g) { cola.push(g); enviaSiguiente(); } }
  };
  $('c-archivo').addEventListener('change', () => { encola($('c-archivo').files); $('c-archivo').value = ''; });
  // A file dragged anywhere over the conversation: the drop area appears; dropped, it goes.
  const zona = $('c-zona'), chatCaja = $('c-chat');
  ['dragenter', 'dragover'].forEach((ev) => chatCaja.addEventListener(ev, (e) => { if (e.dataTransfer && [...(e.dataTransfer.types || [])].includes('Files')) { e.preventDefault(); zona.classList.add('sobre'); } }));
  chatCaja.addEventListener('dragleave', (e) => { if (!chatCaja.contains(e.relatedTarget)) zona.classList.remove('sobre'); });
  chatCaja.addEventListener('drop', (e) => { e.preventDefault(); zona.classList.remove('sobre'); if (e.dataTransfer && e.dataTransfer.files.length) encola(e.dataTransfer.files); });
  $('p-reanudar').addEventListener('click', () => manda({ tipo: 'secure', accion: 'reanudar' }));

  // ---------- Voice notes (17 Oct 2026, «como WhatsApp, normal y con la voz cambiada») ----------
  // The microphone offers two ways: the voice as it is, or changed (the same «Proteger mi voz»
  // as in calls, voz.js). The note is recorded here, then goes like any file: encrypted end to
  // end, in pieces. Five minutes at most.
  let nota = null;
  const NV_MAX = 5 * 60;
  const mimeNota = () => ['audio/webm;codecs=opus', 'audio/mp4', 'audio/webm'].find((x) => window.MediaRecorder && MediaRecorder.isTypeSupported(x)) || '';
  const pintaNota = () => {
    const g = !!nota;
    $('c-grabando').hidden = !g; $('c-texto').hidden = g;
    $('c-microfono').classList.toggle('activo', g);
    $('c-microfono').title = t(g ? 'sec_nv_enviar' : 'sec_nv_grabar');
    if (g) { const s = Math.floor((Date.now() - nota.desde) / 1000); $('c-grabando-t').textContent = Math.floor(s / 60) + ':' + String(s % 60).padStart(2, '0'); $('c-grabando-modo').textContent = t(nota.cambiada ? 'sec_nv_cambiada_corta' : 'sec_nv_normal_corta'); }
  };
  const empiezaNota = async (cambiada) => {
    $('c-nv-elige').hidden = true;
    if (nota || !window.MediaRecorder) return;
    let s;
    try { s = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true } }); } catch (e) { muestraPermiso(e, 'microfono'); return; }
    let vp = null, fuente = s;
    if (cambiada && window.vozProtegida) { try { vp = window.vozProtegida(s, -4); fuente = vp.stream; } catch (_) { vp = null; } }
    const tipo = mimeNota();
    let r;
    try { r = new MediaRecorder(fuente, tipo ? { mimeType: tipo, audioBitsPerSecond: 32000 } : undefined); } catch (_) { s.getTracks().forEach((x) => x.stop()); if (vp) vp.parar(); aviso(t('sec_l_sin_medios')); return; }
    nota = { s, vp, r, trozos: [], cambiada, desde: Date.now(), tipo: r.mimeType || tipo, reloj: null, enviar: false };
    r.ondataavailable = (e) => { if (e.data && e.data.size) nota && nota.trozos.push(e.data); };
    r.onstop = () => terminaNota();
    r.start(250);
    nota.reloj = setInterval(() => { if (!nota) return; pintaNota(); if (Date.now() - nota.desde > NV_MAX * 1000) paraNota(true); }, 250);
    pintaNota();
  };
  const paraNota = (enviar) => { if (!nota) return; nota.enviar = enviar; try { nota.r.stop(); } catch (_) { terminaNota(); } };
  const terminaNota = () => {
    const n = nota; if (!n) return;
    nota = null; clearInterval(n.reloj);
    n.s.getTracks().forEach((x) => x.stop()); if (n.vp) n.vp.parar();
    pintaNota();
    if (!n.enviar || !n.trozos.length || Date.now() - n.desde < 700) return;
    const tipo = (n.tipo || 'audio/webm').split(';')[0];
    const ext = tipo === 'audio/mp4' ? 'm4a' : 'webm';
    const d = new Date(), p = (x) => String(x).padStart(2, '0');
    const nombre = `nota-de-voz${n.cambiada ? '-cambiada' : ''}-${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}-${p(d.getHours())}${p(d.getMinutes())}${p(d.getSeconds())}.${ext}`;
    const blob = new Blob(n.trozos, { type: tipo });
    notasPropias.set(nombre, URL.createObjectURL(blob));
    encola([new File([blob], nombre, { type: tipo })]);
  };
  $('c-microfono').addEventListener('click', (e) => {
    e.stopPropagation();
    if (nota) { paraNota(true); return; }
    $('c-nv-elige').hidden = !$('c-nv-elige').hidden;
    if (!$('c-nv-elige').hidden) $('c-nv-normal').focus();
  });
  $('c-nv-normal').addEventListener('click', () => empiezaNota(false));
  $('c-nv-cambiada').addEventListener('click', () => empiezaNota(true));
  $('c-nv-cancelar').addEventListener('click', () => paraNota(false));
  document.addEventListener('click', (e) => { if (!e.target.closest('#c-nv-elige') && !e.target.closest('#c-microfono')) $('c-nv-elige').hidden = true; });
  document.addEventListener('keydown', (e) => { if (e.key === 'Escape') { $('c-nv-elige').hidden = true; if (nota) paraNota(false); } });
  // Enter or «send» while recording sends the note.
  $('c-form').addEventListener('submit', (e) => { if (nota) { e.preventDefault(); e.stopImmediatePropagation(); paraNota(true); } }, true);

  // ---------- Ring, voice and video (15 Oct 2026) ----------
  // The ring is made here, by the page itself: two soft notes, no file, no network. It plays
  // because the person pressed a button on this page earlier (the browser's rule for sound).
  let audioCtx = null;
  const timbre = () => {
    try {
      audioCtx = audioCtx || new (window.AudioContext || window.webkitAudioContext)();
      const t0 = audioCtx.currentTime;
      [[880, 0], [1174.7, 0.18], [880, 0.6], [1174.7, 0.78]].forEach(([f, d]) => {
        const o = audioCtx.createOscillator(), g = audioCtx.createGain();
        o.type = 'sine'; o.frequency.value = f; o.connect(g); g.connect(audioCtx.destination);
        g.gain.setValueAtTime(0.0001, t0 + d); g.gain.exponentialRampToValueAtTime(0.25, t0 + d + 0.02); g.gain.exponentialRampToValueAtTime(0.0001, t0 + d + 0.16);
        o.start(t0 + d); o.stop(t0 + d + 0.18);
      });
    } catch (_) {}
  };
  // ---------- The phone (15 Oct 2026) ----------
  // The left column: the conversation this computer is in (one at a time), with the last line
  // and its time, as the chosen design lists them; empty, a quiet sentence.
  const filaSala = (m) => {
    const s = m.sala; if (!s) return '';
    const ultimo = (s.mensajes || []).slice(-1)[0];
    const linea = s.llamada ? t('sec_l_en_curso') : ultimo ? `${ultimo.de}: ${ultimo.texto}` : t('sec_lista_sin_mensajes');
    // 1.0.46 (C2-a): the state under the name, with its dot; the last line as the row's title.
    const dentro = (s.miembros || []).filter((x) => x.aprobado && x.enganchado).length;
    return `<li class="${vista === 'sala' ? 'activa' : ''}" data-vista="sala" title="${esc(linea)}"><span class="avatar casa"><svg class="icono" viewBox="0 0 24 24" aria-hidden="true"><path d="M3 11 12 4l9 7"/><path d="M5 10v10h14V10"/><path d="M10 20v-6h4v6"/></svg></span><b>${esc(t('sec_sala_titulo'))}</b><span class="ultimo sala-punto ${dentro ? 'ok' : ''}">${esc(s.llamada ? t('sala_en_llamada') : dentro ? t('sec_l_sala_conexion', { c: n(dentro), n: n((s.miembros || []).filter((x) => x.aprobado).length) }) : t('sala_sin_conexion'))}</span></li>`;
  };
  const pintaLista = (m, quien, nombres) => {
    const ol = $('c-lista');
    const f = m.fase;
    if (!f || f === 'sola' || (f === 'escuchando' && m.sala)) { ol.innerHTML = filaSala(m) || `<li class="vacia">${esc(t('sec_lista_vacia'))}</li>`; if (!m.sala) $('miga-donde').textContent = t('sec_miga_inicio'); return; }
    const ultimo = (m.mensajes || []).slice(-1)[0];
    const nombre = nombres.length > 2 ? nombres.join(' · ') : (quien || t('sec_lista_nueva'));
    const linea = f !== 'lista' ? t(f === 'comparando' ? 'sec_lista_comparando' : 'sec_lista_conectando') : m.llamada ? t('sec_l_en_curso') : ultimo ? ultimo.texto : t('sec_lista_sin_mensajes');
    // 1.0.46 (C2-a): «Conectado» with a green dot only while the link is up; the rest as it is.
    const estado = f !== 'lista' ? linea : m.reconectando ? t('sala_reconectando') : m.llamada && m.llamada.conectada !== false ? t('sala_en_llamada') : t(m.otro_movil ? 'sala_conectado_movil' : 'sala_conectado');
    const punto = f === 'lista' && !m.reconectando ? 'ok' : f === 'lista' ? 'aviso' : 'info';
    ol.innerHTML = filaSala(m) + `<li class="${vista === 'contacto' || !m.sala ? 'activa' : ''}" data-vista="contacto" title="${esc(linea)}"><span class="avatar">${esc(iniciales(nombre))}</span><b>${esc(nombre)}</b><span class="ultimo sala-punto ${punto}">${esc(estado)}</span></li>`;
    if (vista !== 'sala' || !m.sala) $('miga-donde').textContent = t('sec_miga_conv');
  };
  $('c-lista').addEventListener('click', (e) => { const li = e.target.closest('li[data-vista]'); if (!li || !E.sala) return; vista = li.dataset.vista; modo = vista === 'sala' ? 'sala' : (movilActivo(E) && !hayContacto(E) ? 'movil' : 'equipos'); guardaModo(); fasePinta(); pintaLista(E, E.contacto, ((E.grupo || {}).nombres || [])); pintaVista(E); });
  $('c-nueva').addEventListener('click', () => { if (!hayContacto(E) && !movilActivo(E)) { modo = null; guardaModo(); fasePinta(); pintaModo(E); pintaVista(E); } });

  // «Pasar al móvil»: the program shows a QR code; the phone's camera opens our page, which
  // talks to this computer directly (TLS 1.3 pinned to the certificate in the code). The
  // first time, the person says OK here. While the phone talks, this screen steps back: no
  // microphone or camera here, and the other side's pieces go to the phone.
  let movilHabla = false;
  const idioma = () => (document.documentElement.lang || navigator.language || 'es').slice(0, 2);
  const pintaMovil = (m) => {
    const mv = m.movil;
    const caja = $('c-movil-tarjeta');
    const varios = ((m.grupo || {}).nombres || []).length > 2;
    $('c-movil').hidden = !!mv || varios;
    $('c-movil-antes').hidden = !!mv || varios;
    if (!mv) { caja.hidden = true; movilHabla = false; return; }
    caja.hidden = false;
    if (mv.error) { $('c-mv-estado-t').textContent = t('sec_mv_error', { e: mv.error }); $('c-mv-estado').className = 'm-estado'; return; }
    if (mv.qr && $('c-mv-qr').dataset.qr !== mv.qr) { $('c-mv-qr').innerHTML = mv.qr; $('c-mv-qr').dataset.qr = mv.qr; }
    $('c-mv-qr').hidden = !!mv.aprobado;
    $('c-mv-texto').hidden = !!mv.aprobado;
    $('c-mv-ok').hidden = !mv.pendiente;
    $('c-mv-habla').hidden = !mv.habla;
    const e = $('c-mv-estado');
    e.className = 'm-estado ' + (mv.habla ? 'si' : mv.enganchado ? 'espera' : '');
    const pasoMv = mv.paso || {};
    $('c-mv-estado-t').textContent = t(pasoMv.fallo_paso ? 'sec_mv_no_paso' : pasoMv.relevo === 'pasando' && m.llamada ? 'sec_mv_e_pasando' : mv.habla ? 'sec_mv_e_habla' : mv.pendiente ? 'sec_mv_e_pendiente' : mv.aprobado ? 'sec_mv_e_fuera' : 'sec_mv_e_espera');
    $('c-mv-fuera').textContent = mv.fuera ? t('sec_mv_fuera_si') : mv.puerta_error ? t('sec_mv_fuera_no', { e: mv.puerta_error }) : t('sec_mv_fuera_abriendo');
    movilHabla = !!mv.habla;
  };
  $('c-movil').addEventListener('click', () => manda({ tipo: 'secure', accion: 'movil', idioma: idioma() }));
  $('c-movil-antes').addEventListener('click', () => manda({ tipo: 'secure', accion: 'movil', idioma: idioma() }));
  $('c-mv-si').addEventListener('click', () => manda({ tipo: 'secure', accion: 'movil_ok' }));
  $('c-mv-no').addEventListener('click', () => manda({ tipo: 'secure', accion: 'movil_fin' }));
  $('c-mv-fin').addEventListener('click', () => manda({ tipo: 'secure', accion: 'movil_fin' }));
  en('secure_estado', pintaMovil);
  en('secure_aviso', (m) => { if (m.clave === 'sec_mv_enganchado') timbre(); });

  // «Sala de casa» (16 Oct 2026): one code for the phones of the house, this computer as the
  // exchange for as many hours as chosen; the phones talk to each other through it from
  // anywhere. The person here lets each phone in by the name it typed.
  // The room is a conversation of its own (16 Oct 2026, the owner: «que se vea en el centro»):
  // its chat fills the middle column while no other conversation is open, and the left list
  // offers both when there is one.
  let vista = 'contacto';
  // Someone on the line (17 Oct 2026 review: counting the messages a finished conversation
  // leaves on screen kept the start hidden and the way back gone until the program restarted).
  // 1.0.42: «escuchando» is the invitation waiting for a PC (it travels with the QR of «Nueva
  // conversación»): nobody is on the line yet, so the screen stays on the invitation.
  const hayContacto = (m) => !!m.fase && m.fase !== 'sola' && m.fase !== 'escuchando';
  // Three ways to talk (16 Oct 2026): computer to computer, from the phone, the home room.
  // The start shows the three; once one is chosen (or one is already going) only it is on screen.
  let modo = null;
  try { const nav = performance.getEntriesByType('navigation')[0]; if (nav && nav.type === 'reload') sessionStorage.removeItem('secure-modo'); } catch (_) {}
  try { modo = sessionStorage.getItem('secure-modo') || null; } catch (_) {}
  if (!['equipos', 'movil', 'sala'].includes(modo)) modo = null;
  const guardaModo = () => { try { if (modo) sessionStorage.setItem('secure-modo', modo); else sessionStorage.removeItem('secure-modo'); } catch (_) {} };
  const movilActivo = (m) => !!(m.movil && (m.movil.enganchado || m.movil.pendiente || m.movil.qr));
  const pintaModo = (m) => {
    // The person's choice wins; what is going on only fills the gap (17 Oct 2026: an open room
    // kept the screen in «Modo casa» and the other two cards did nothing). A conversation with
    // someone takes the screen when it starts.
    const enConversacion = hayContacto(m) || movilActivo(m);
    if (enConversacion && modo !== 'equipos' && modo !== 'movil') modo = movilActivo(m) && !hayContacto(m) ? 'movil' : 'equipos';
    else if (!modo && m.sala) modo = 'sala';
    guardaModo();
    const activo = !!m.sala || movilActivo(m) || hayContacto(m);
    $('c-inicio').hidden = !!modo;
    // The way back stays while an invitation waits or the numbers are compared; only an open
    // conversation (which has «Cerrar conversación») or the phone link hide it.
    $('c-modo-cab').hidden = !modo || m.fase === 'lista' || movilActivo(m);
    if (modo) $('c-modo-nombre').textContent = t('sec_modo_' + modo);
    $('c-modo-movil').hidden = modo !== 'movil' && !m.movil;
    $('c-modo-movil-intro').hidden = !!m.movil || hayContacto(m);
    const sala = $('c-sala-tarjeta');
    sala.hidden = modo !== 'sala';
    // The room's code and members: in the middle while it is not open, on the left once it is.
    const colIzq = document.querySelector('.col-izq'), main = document.querySelector('main');
    if (modo === 'sala' && m.sala && sala.parentElement !== colIzq) colIzq.insertBefore(sala, colIzq.querySelector('.pie-izq'));
    if ((modo !== 'sala' || !m.sala) && sala.parentElement !== main) main.insertBefore(sala, $('c-conexion'));
    // The phone card goes to the left while a conversation is open, so the chat keeps the
    // whole middle (17 Oct 2026, the owner: «se ve el margen de abajo cortado»).
    const mvC = $('c-modo-movil'), mvLado = hayContacto(m) && !!m.movil;
    if (mvLado && mvC.parentElement !== colIzq) colIzq.insertBefore(mvC, colIzq.querySelector('.pie-izq'));
    if (!mvLado && mvC.parentElement !== main) main.insertBefore(mvC, $('c-modo-cab').nextSibling);
    const secure = document.querySelector('.secure');
    secure.classList.toggle('sin-lados', !activo);
    secure.classList.toggle('sin-derecha', false);
    $('miga-donde').textContent = modo ? t('sec_modo_' + modo) : t('sec_miga_inicio');
  };
  // 1.0.42: «Nueva conversación» opens the invitation at once: the room for phones and, at the
  // same time, the code for a PC (it travels inside the same link). «Tengo una invitación» and
  // «Usar en mi móvil» are the other two ways in.
  document.querySelectorAll('#c-inicio [data-modo]').forEach((b) => b.addEventListener('click', () => {
    modo = b.dataset.modo; guardaModo(); fasePinta(); pintaModo(E); pintaVista(E);
    if (modo === 'sala') {
      if (!E.sala) manda({ tipo: 'secure', accion: 'sala', horas: 24, idioma: idioma() });
      if (!E.fase || E.fase === 'sola') { mandaNombre(); manda({ tipo: 'secure', accion: 'escuchar', puerto: 0 }); }
    }
    if (modo === 'equipos') { const c = document.querySelector('#c-conexion input:not(#c-yo)'); if (c) c.focus(); }
  }));
  $('c-volver').addEventListener('click', () => {
    // Going back cancels an invitation still waiting, so nothing is left half open.
    if (E.fase && E.fase !== 'sola' && E.fase !== 'lista') { manda({ tipo: 'secure', accion: 'cerrar' }); E = Object.assign({}, E, { fase: 'sola' }); }
    modo = null; guardaModo(); fasePinta(); pintaModo(E); pintaVista(E);
  });
  const pintaVista = (m) => {
    pintaModo(m);
    const s = m.sala;
    if (!s) vista = 'contacto';
    else if (modo === 'sala') vista = 'sala';
    else if (!hayContacto(m) && !movilActivo(m)) vista = 'sala';
    const enSala = !!s && vista === 'sala' && modo === 'sala';
    $('c-sala-chat').hidden = !enSala;
    for (const id of ['c-conexion', 'c-comparar', 'c-espera', 'c-resumen', 'c-chat']) if (enSala) $(id).hidden = true;
    document.querySelector('main').classList.toggle('con-chat', !$('c-chat').hidden || enSala);
    if (!enSala) return;
    const dentro = (s.miembros || []).filter((x) => x.aprobado);
    $('c-sc-sello').textContent = t('sec_sala_sello', { n: n(dentro.length), h: n(s.horas), f: fecha(s.caduca) });
    $('c-sc-llamada').hidden = !s.llamada;
    if (s.llamada) $('c-sc-llamada').textContent = t(s.llamada.a ? 'sec_sala_llamada' : 'sec_sala_llamando', { a: s.llamada.de, b: s.llamada.a || '' });
    $('miga-donde').textContent = t('sec_sala_titulo');
    // The panel on the right tells the room's state in real time (the owner, 16 Oct 2026).
    const caja = $('g'); caja.classList.remove('protegida', 'revisar', 'pausada', 'sin');
    const conectados = dentro.filter((x) => x.enganchado);
    caja.classList.add(dentro.length ? 'protegida' : 'sin');
    $('g-palabra').textContent = t(dentro.length ? 'sec_g_protegida' : 'sec_g_sin');
    $('g-frase').textContent = t(dentro.length ? 'sec_g_sala_frase' : 'sec_g_sala_vacia');
    const chip = $('m-estado-chip'); chip.textContent = t(dentro.length ? 'sec_g_protegida' : 'sec_g_sin'); chip.className = 'chip' + (dentro.length ? ' ok' : '');
    linea($('g-cifrado'), dentro.length ? 'ok' : '', t('sec_l_sala_cifrado'));
    linea($('g-identidad'), dentro.length ? 'ok' : '', dentro.length ? t('sec_l_sala_miembros', { n: n(dentro.length), l: dentro.map((x) => x.nombre).join(', ') }) : t('sec_l_sala_nadie'));
    linea($('g-nivel'), 'ok', t('sec_l_nivel_movil'));
    linea($('g-cuantico'), '', t('sec_l_sala_cuantico'));
    linea($('g-conexion'), conectados.length ? 'ok' : '', conectados.length ? t('sec_l_sala_conexion', { c: n(conectados.length), n: n(dentro.length) }) : t('sec_l_sin_conexion'));
    linea($('g-visto'), s.llamada ? 'ok' : '', s.llamada ? t(s.llamada.a ? 'sec_sala_llamada' : 'sec_sala_llamando', { a: s.llamada.de, b: s.llamada.a || '' }) : t('sec_l_sala_mensajes', { n: n((s.mensajes || []).length) }));
  };
  let salaEnlace = '';
  // A voice note from a phone: the player, asked once from the program (17 Oct 2026).
  const notaSala = (a) => {
    const voz = /^audio\//.test(a.mime || '') || /^nota-de-voz/i.test(a.nombre || '');
    if (!voz) return `<span>📎 ${esc(a.nombre)}</span>`;
    const clave = 'sala-' + a.id;
    if (!audios.has(clave) && !pedidas.has(clave)) { pedidas.add(clave); manda({ tipo: 'secure', accion: 'sala_nota', id: a.id }); }
    return notaVoz(audios.get(clave), a.nombre);
  };
  // A new room starts with nothing of the last one on screen (its voice notes were cached by
  // number, and a new room's numbers could match).
  let salaVista = null;
  const pintaSala = (m) => {
    if (m && m.sala && m.sala.creada !== salaVista) { salaVista = m.sala.creada; for (const k of [...audios.keys()]) if (typeof k === 'string' && k.startsWith('sala-')) audios.delete(k); }
    const s = m.sala;
    $('c-sala-antes').hidden = !!s;
    $('c-sala-abierta').hidden = !s;
    if (!s) return;
    if (s.qr && $('c-sala-qr').dataset.qr !== s.qr) { $('c-sala-qr').innerHTML = s.qr; $('c-sala-qr').dataset.qr = s.qr; }
    salaEnlace = s.enlace || '';
    $('c-sala-pc').hidden = !s.codigo_pc; $('c-sala-pc-codigo').textContent = s.codigo_pc || '';
    $('c-sala-hasta').textContent = t('sec_sala_hasta', { h: n(s.horas), f: fecha(s.caduca) });
    $('c-sala-miembros').innerHTML = (s.miembros || []).filter((x) => x.enganchado || x.aprobado || x.con_nombre).map((x) => {
      const estado = x.pendiente ? t('sec_sala_m_pendiente') : x.aprobado ? (x.enganchado ? t('sec_sala_m_dentro') : t('sec_sala_m_fuera')) : t('sec_sala_m_espera');
      const botones = x.pendiente ? `<button type="button" class="boton chico" data-sala-ok="${x.i}">${esc(t('sec_sala_dejar'))}</button>` : x.aprobado ? `<button type="button" class="boton chico secundario" data-sala-fuera="${x.i}">${esc(t('sec_sala_echar'))}</button>` : '';
      // 1.0.42: the four digits of the link, the same the phone shows; checked before «Aceptar».
      const num = x.numero && (x.pendiente || x.aprobado) ? `<span class="c-sala-num" title="${esc(t('sec_sala_numero_t'))}">${esc(x.numero.split('').join(' '))}</span>` : '';
      return `<li class="${x.enganchado ? 'si' : ''}"><i></i><span class="c-sala-q"><b>${esc(x.nombre)}</b><span class="t">${esc(estado)}</span></span>${num || botones ? `<span class="c-sala-acc">${num}${botones}</span>` : ''}</li>`;
    }).join('');
    const ol = $('c-sala-mensajes'); const abajo = ol.scrollTop + ol.clientHeight >= ol.scrollHeight - 40;
    ol.innerHTML = (s.mensajes || []).map((x) => x.llamada ? lineaLlamada(x) : `<li class="${x.mio ? 'mio' : ''}" data-id="${esc(x.id || '')}"><b>${esc(x.de || t('sec_m_tu'))}</b>${citaDe(x, s.mensajes)}${x.borrado ? `<i class="borrado">${esc(t('sec_m_borrado'))}</i>` : x.archivo ? notaSala(x.archivo) : `<span>${esc(x.texto)}</span>`}${reacciones(x)}${metaMensaje(x)}</li>`).join('') + (s.escribiendo ? `<li class="escribiendo"><span></span><span></span><span></span></li>` : '');
    agrupa(ol, s.mensajes);
    if (abajo) ol.scrollTop = ol.scrollHeight;
  };
  $('c-sc-cerrar').addEventListener('click', () => manda({ tipo: 'secure', accion: 'sala_cerrar' }));
  $('c-sala-abrir').addEventListener('click', () => manda({ tipo: 'secure', accion: 'sala', horas: Number($('c-sala-horas').value), idioma: idioma() }));
  $('c-sala-cerrar').addEventListener('click', () => manda({ tipo: 'secure', accion: 'sala_cerrar' }));
  $('c-sala-copiar').addEventListener('click', async () => { if (!salaEnlace) return; try { await navigator.clipboard.writeText(salaEnlace); $('c-sala-copiar').textContent = t('sec_c_copiado'); setTimeout(() => { $('c-sala-copiar').textContent = t('sec_sala_copiar'); }, 1500); } catch (_) {} });
  $('c-sala-whatsapp').addEventListener('click', () => { if (salaEnlace) manda({ tipo: 'navegar', texto: 'https://wa.me/?text=' + encodeURIComponent(t('sec_sala_invita', { e: salaEnlace })) }); });
  $('c-sala-miembros').addEventListener('click', (e) => {
    const b = e.target.closest('button'); if (!b) return;
    if (b.dataset.salaOk !== undefined) manda({ tipo: 'secure', accion: 'sala_ok', i: Number(b.dataset.salaOk) });
    if (b.dataset.salaFuera !== undefined) manda({ tipo: 'secure', accion: 'sala_fuera', i: Number(b.dataset.salaFuera) });
  });
  $('c-sala-form').addEventListener('submit', (e) => { e.preventDefault(); const x = $('c-sala-texto').value.trim(); if (!x) return; mandaTexto(x, true); $('c-sala-texto').value = ''; });
  en('secure_estado', pintaSala);
  en('secure_estado', pintaVista);
  en('secure_estado', (m) => { const s = m.sala; if (s && (s.miembros || []).some((x) => x.pendiente) && !salaPendienteAntes) timbre(); salaPendienteAntes = !!(s && (s.miembros || []).some((x) => x.pendiente)); });
  let salaPendienteAntes = false;

  let faseAntes = 'sola';
  en('secure_estado', (m) => {
    if (m.fase === 'comparando' && faseAntes !== 'comparando' && faseAntes !== 'saludando') timbre();
    faseAntes = m.fase;
    pintaLlamada(m);
  });
  en('secure_aviso', (m) => { if (m.clave === 'sec_llamada_entrante') timbre(); });
  document.addEventListener('visibilitychange', () => { if (!document.hidden) manda({ tipo: 'secure', accion: 'visto' }); });
  window.addEventListener('focus', () => manda({ tipo: 'secure', accion: 'visto' }));

  // Sending: the camera and microphone recorded in short pieces (Opus, and VP9 for video), each
  // one handed to the program, which encrypts it under the media layer and sends it. Receiving:
  // the pieces the program opened are played as they come. Direct, no media server, ~half a
  // second behind. (The real-time transport comes next; this is today's.)
  let arrancando = false;
  let grabador = null, local = null, fuente = null, buffer = null, pendientes = [], silenciado = false, llamadaVideo = false;
  let mimeRemoto = null, tipoGrabador = null, reproduceOtro = [], reinicioVisto = null;
  // A call has three moments (16 Oct 2026): it rings here (Contestar / Rechazar), we are
  // calling (the button stays pressed, «Llamando a…» with the seconds left), and we talk.
  let sonando = null, tonoLlamando = null;
  const quien = (m) => ((m.grupo || {}).nombres || []).length > 2 ? t('sec_l_grupo') : (m.contacto || '');
  const pintaTimbre = (m) => {
    const e = m.entrante;
    $('c-timbre').hidden = !e;
    if (e) {
      $('c-timbre-t').textContent = t(e.video ? 'sec_l_te_videollama' : 'sec_l_te_llama', { n: e.de || m.contacto || '' });
      $('c-timbre-s').textContent = t('sec_l_cifrada_nota');
      if (!sonando) {
        timbre(); sonando = setInterval(timbre, 2500);
        setTimeout(() => { try { $('c-contestar').focus(); } catch (_) {} }, 50);
        // Before answering: is there a microphone (and a camera, for video) at all?
        try {
          navigator.mediaDevices.enumerateDevices().then((ds) => {
            const mic = ds.some((d) => d.kind === 'audioinput'), cam = ds.some((d) => d.kind === 'videoinput');
            const falta = !mic ? t('sec_p_no_hay_microfono') : (e.video && !cam ? t('sec_p_no_hay_camara') : '');
            let x = $('c-timbre-s').querySelector('.t-aviso');
            if (falta) { if (!x) { x = document.createElement('span'); x.className = 't-aviso'; $('c-timbre-s').appendChild(x); } x.textContent = falta; } else if (x) x.remove();
          }).catch(() => {});
        } catch (_) {}
      }
    } else if (sonando) { clearInterval(sonando); sonando = null; }
  };
  $('c-contestar').addEventListener('click', () => { const e = (E || {}).entrante || {}; manda({ tipo: 'secure', accion: 'contestar', mime: mime(!!e.video) }); });
  $('c-rechazar').addEventListener('click', () => manda({ tipo: 'secure', accion: 'rechazar' }));
  const pintaBotones = (m) => {
    const l = m.llamada;
    const ocupado = !!l || !!m.entrante;
    $('c-voz').disabled = ocupado; $('c-video').disabled = ocupado;
    $('c-voz').classList.toggle('pulsado', !!l && !l.video);
    $('c-video').classList.toggle('pulsado', !!l && !!l.video);
  };
  const textoLlamada = (l) => {
    const ahora = Math.floor(Date.now() / 1000);
    if (!l.conectada) return t('sec_l_llamando_s', { s: Math.max(0, 45 - (ahora - (l.desde || ahora))) });
    // 1.0.46 (C2-b): «En llamada · 08:42» under the name.
    const d = Math.max(0, ahora - (l.desde || ahora));
    return t(l.video ? 'sec_l_en_video' : 'sala_chip_llamada', { t: l.video ? tiempo(d) : (d >= 3600 ? Math.floor(d / 3600) + ':' : '') + String(Math.floor((d % 3600) / 60)).padStart(2, '0') + ':' + String(d % 60).padStart(2, '0') });
  };
  // A call nobody answered stays as a note with «Devolver llamada» (Grok's review, 16 Oct 2026).
  const pintaPerdida = (m) => {
    const p = m.perdida;
    $('c-perdida').hidden = !p || !!m.llamada || !!m.entrante;
    if (p) $('c-perdida-t').textContent = t(p.video ? 'sec_l_perdida_video' : 'sec_l_perdida_voz', { n: p.de || m.contacto || '', h: hora(p.t) });
  };
  $('c-devolver').addEventListener('click', () => { const p = (E || {}).perdida || {}; llama(!!p.video); });
  $('c-perdida-x').addEventListener('click', () => manda({ tipo: 'secure', accion: 'perdida_vista' }));
  const pintaLlamada = (m) => {
    reproduceOtro = m.reproduce_otro || [];
    pintaTimbre(m); pintaBotones(m); pintaPerdida(m);
    const l = m.llamada;
    const caja = $('c-llamada');
    const enLlamada = !!l && l.conectada !== false && !movilHabla && !$('c-chat').hidden;
    document.querySelector('.secure').classList.toggle('en-llamada', enLlamada);
    if (!l) medidorPara();
    if (!l) { $('c-permiso').hidden = true; $('c-permiso').dataset.camara = ''; caja.hidden = true; caja.classList.remove('llamando'); paraMedios(); mimeRemoto = null; if (tonoLlamando) { clearInterval(tonoLlamando); tonoLlamando = null; } return; }
    caja.hidden = false;
    caja.classList.toggle('solo-voz', !l.video);
    $('c-l-letra').textContent = iniciales(m.contacto || quien(m));
    $('vj-otro').textContent = quien(m);
    if (l.conectada !== false && !movilHabla) medidorEmpieza();
    const llamando = l.conectada === false;
    caja.classList.toggle('llamando', llamando);
    caja.querySelector('.pantallas').dataset.llamando = t('sec_l_llamando', { n: quien(m) });
    $('c-l-estado').textContent = movilHabla ? t('sec_mv_habla') : textoLlamada(l);
    if (llamando) { if (!tonoLlamando) tonoLlamando = setInterval(() => { if (E && E.llamada && E.llamada.conectada === false) timbre(); }, 3000); return; }
    if (tonoLlamando) { clearInterval(tonoLlamando); tonoLlamando = null; }
    // With the bridge this page keeps its leg while the phone talks through it (relevo.js).
    if (movilHabla && !puenteAhora(m)) { if (grabador || arrancando) paraMedios(); return; }
    // The other side's player started again (its phone took the call, or gave it back): our
    // recorder starts again too, so its first piece carries the header that player needs.
    if (grabador && typeof l.reinicio === 'number' && reinicioVisto !== null && l.reinicio !== reinicioVisto) cambiaGrabador();
    if (typeof l.reinicio === 'number') reinicioVisto = l.reinicio;
    // WebRTC when both programs and the relay's call service allow it (17 Oct 2026); the
    // recorded pieces otherwise, or when WebRTC did not connect.
    const conRtc = l.rtc && l.rtc.ice && !rtcFallo && window.RTCPeerConnection;
    if (rtc || (conRtc && !grabador)) { if (!rtc && !arrancando) empiezaRtc(l); return; }
    // The relay is still being asked for its call service: a few seconds, then the old way.
    if (l.rtc && l.rtc.pendiente && !rtcFallo && !grabador && !arrancando) { rtcEspera = rtcEspera || Date.now(); if (Date.now() - rtcEspera < 5000) return; rtcManda({ t: 'fallo' }); rtcFallo = true; }
    if (!grabador && !arrancando) empiezaMedios(!!l.video);
    // The other side's player (an iPhone) cannot open what we record: record in what it can.
    else if (grabador && tipoGrabador && mime(llamadaVideo) !== tipoGrabador) cambiaGrabador();
    // The other side said what its pieces are: open the player for exactly that.
    if (l.mime_remoto && l.mime_remoto !== mimeRemoto && buffer !== undefined) { mimeRemoto = l.mime_remoto; preparaReproductor(mimeRemoto); }
  };
  setInterval(() => {
    if (!$('c-llamada').hidden && E.llamada && !movilHabla) $('c-l-estado').textContent = textoLlamada(E.llamada);
    // A connected call with nothing started yet (it was waiting for the relay): start now.
    if (rtcEspera && E && E.llamada && E.llamada.conectada !== false && !rtc && !grabador && !arrancando && (!movilHabla || puenteAhora(E))) pintaLlamada(E);
  }, 1000);
  // The formats, best first; the phone's page knows the same list and tells which it can play.
  const CANDIDATOS = { video: ['video/webm;codecs=vp9,opus', 'video/webm;codecs=vp8,opus', 'video/mp4;codecs=avc1.42E01E,mp4a.40.2', 'video/webm'], voz: ['audio/webm;codecs=opus', 'audio/mp4;codecs=mp4a.40.2', 'audio/webm'] };
  const mime = (video) => {
    const c = CANDIDATOS[video ? 'video' : 'voz'];
    const puede = c.filter((x) => window.MediaRecorder && MediaRecorder.isTypeSupported(x));
    return (reproduceOtro.length && puede.find((x) => reproduceOtro.includes(x))) || puede[0] || c[c.length - 1];
  };
  // «Voz protegida» (16 Oct 2026): the voice leaves transformed (voz.js) so this call is no
  // clean sample for cloning it. The person's choice is kept; the other side is told.
  let vozProt = null;
  let vozActiva = false; try { vozActiva = localStorage.getItem('secure_voz') === '1'; } catch (_) {}
  const fuenteGrabacion = () => {
    if (vozProt) { vozProt.parar(); vozProt = null; }
    if (!vozActiva || !local || !window.vozProtegida || !local.getAudioTracks().length) return local;
    try { vozProt = window.vozProtegida(local, -4); return vozProt.stream; } catch (_) { return local; }
  };
  const pintaVoz = () => { $('c-voz-prot').setAttribute('aria-pressed', String(vozActiva)); $('c-voz-prot').querySelector('span').textContent = t(vozActiva ? 'sec_v_activa' : 'sec_v_boton'); };
  $('c-voz-prot').addEventListener('click', () => {
    vozActiva = !vozActiva; try { localStorage.setItem('secure_voz', vozActiva ? '1' : '0'); } catch (_) {}
    pintaVoz(); manda({ tipo: 'secure', accion: 'voz_protegida', activa: vozActiva });
    if (rtc) rtcVoz(); else if (local) cambiaGrabador();
  });
  const nuevoGrabador = (video) => {
    const tipo = mime(video);
    grabador = new MediaRecorder(fuenteGrabacion(), { mimeType: tipo, audioBitsPerSecond: 64000, videoBitsPerSecond: video ? 1500000 : undefined });
    grabador.ondataavailable = async (e) => { if (e.data && e.data.size) manda({ tipo: 'secure', accion: 'media', b64: b64De(await e.data.arrayBuffer()) }); };
    grabador.start(120);
    tipoGrabador = tipo;
    manda({ tipo: 'secure', accion: 'mime', mime: tipo });
  };
  const cambiaGrabador = () => {
    if (!local) return;
    try { if (grabador && grabador.state !== 'inactive') grabador.stop(); } catch (_) {}
    nuevoGrabador(llamadaVideo);
  };
  // The microphone or the camera could not be used (16 Oct 2026, «que le salga ahí directo»):
  // why, in plain words, with the button to Windows' own setting and one to try again. The
  // call stays up meanwhile.
  const muestraPermiso = (e, que) => {
    const n = (e && e.name) || '';
    const clave = n === 'NotFoundError' || n === 'OverconstrainedError' ? 'sec_p_no_hay' : n === 'NotReadableError' || n === 'AbortError' ? 'sec_p_ocupado' : 'sec_p_bloqueado';
    $('c-permiso-t').textContent = t(clave + '_' + que);
    $('c-permiso-s').textContent = t(clave + '_que');
    $('c-permiso-mic').hidden = clave !== 'sec_p_bloqueado' || que !== 'microfono';
    $('c-permiso-cam').hidden = clave !== 'sec_p_bloqueado' || que !== 'camara';
    $('c-permiso').dataset.camara = que === 'camara' ? '1' : '';
    $('c-permiso').hidden = false;
  };
  $('c-permiso-mic').addEventListener('click', () => manda({ tipo: 'secure', accion: 'ajustes_privacidad', que: 'microfono' }));
  $('c-permiso-cam').addEventListener('click', () => manda({ tipo: 'secure', accion: 'ajustes_privacidad', que: 'camara' }));
  $('c-permiso-otra').addEventListener('click', () => {
    $('c-permiso').hidden = true; $('c-permiso').dataset.camara = '';
    const l = E && E.llamada;
    if (!l) return;
    paraMedios(); empiezaMedios(!!l.video);
  });
  const empiezaMedios = async (video) => {
    arrancando = true;
    llamadaVideo = video;
    preparaReproductor(mimeRemoto || mime(video));
    // 1.0.46: the microphone as chosen before the call («Voz natural» or with noise reduction).
    const audio = window.GuardianaOpus ? window.GuardianaOpus.restricciones() : { echoCancellation: true, noiseSuppression: true, autoGainControl: true, sampleRate: 48000 };
    try {
      local = await navigator.mediaDevices.getUserMedia({ audio, video: video ? { width: { ideal: 1280 }, height: { ideal: 720 }, frameRate: { ideal: 30 } } : false });
    } catch (e) {
      // No camera (or not allowed): the call goes on with the voice instead of ending, and the
      // way to turn it on is right there.
      local = null;
      if (video) { try { local = await navigator.mediaDevices.getUserMedia({ audio, video: false }); aviso(t('sec_l_sin_camara')); muestraPermiso(e, 'camara'); } catch (e2) { e = e2; } }
      if (!local) { arrancando = false; muestraPermiso(e, 'microfono'); return; }
    }
    $('c-permiso').hidden = !$('c-permiso').dataset.camara;
    $('c-local').srcObject = local;
    llamadaVideo = video && local.getVideoTracks().length > 0;
    try { nuevoGrabador(llamadaVideo); } catch (e) { arrancando = false; aviso(t('sec_l_sin_medios')); return; }
    // Our players are new: ask the other side for pieces from a fresh start.
    manda({ tipo: 'secure', accion: 'pide_inicio' });
    arrancando = false;
  };
  // One player per member (a group) or one for the other person (a two-way talk): each
  // sender's pieces go to their own tile, created when the first piece arrives.
  const jugadores = new Map();
  let tipoRemoto = null;
  const jugador = (de) => {
    if (jugadores.has(de)) return jugadores.get(de);
    const tile = document.createElement('div'); tile.className = 'tile';
    const v = document.createElement('video'); v.autoplay = true; v.playsInline = true;
    const n = document.createElement('span'); n.className = 'tile-nombre'; n.textContent = de || (E.contacto || '');
    tile.appendChild(v); tile.appendChild(n);
    $('c-rejilla').appendChild(tile);
    const j = { v, fuente: null, buffer: null, pendientes: [] };
    j.fuente = new MediaSource();
    v.src = URL.createObjectURL(j.fuente);
    j.fuente.addEventListener('sourceopen', () => {
      try { j.buffer = j.fuente.addSourceBuffer(tipoRemoto || mime(llamadaVideo)); j.buffer.mode = 'sequence'; j.buffer.addEventListener('updateend', () => vacia(j)); vacia(j); } catch (e) { aviso(t('sec_l_no_reproduce')); }
    });
    v.play().catch(() => {});
    jugadores.set(de, j);
    $('c-rejilla').dataset.n = String(jugadores.size);
    return j;
  };
  // A new format from the other side (its phone took over, or gave back): new players.
  const preparaReproductor = (tipo) => {
    if (tipoRemoto && tipo !== tipoRemoto && jugadores.size) {
      jugadores.forEach((j) => { try { if (j.fuente && j.fuente.readyState === 'open') j.fuente.endOfStream(); } catch (_) {} j.v.removeAttribute('src'); });
      jugadores.clear();
      $('c-rejilla').innerHTML = ''; $('c-rejilla').dataset.n = '0';
    }
    tipoRemoto = tipo;
  };
  const vacia = (j) => {
    if (!j.buffer || j.buffer.updating || !j.pendientes.length) return;
    try { j.buffer.appendBuffer(j.pendientes.shift()); } catch (_) { j.pendientes = []; }
    // Keep close to live: if the player fell behind, jump.
    try { if (j.v.buffered.length && j.v.buffered.end(j.v.buffered.length - 1) - j.v.currentTime > 0.9) j.v.currentTime = j.v.buffered.end(j.v.buffered.length - 1) - 0.25; } catch (_) {}
  };
  en('secure_media', (m) => {
    (m.trozos || []).forEach((x) => {
      const b = typeof x === 'string' ? x : x.b, de = typeof x === 'string' ? '' : (x.de || '');
      const s = atob(b); const u = new Uint8Array(s.length); for (let i = 0; i < s.length; i++) u[i] = s.charCodeAt(i);
      const j = jugador(de); j.pendientes.push(u.buffer); vacia(j);
    });
  });
  const paraMedios = () => {
    paraRtc(); rtcFallo = false; rtcEspera = 0;
    try { if (grabador && grabador.state !== 'inactive') grabador.stop(); } catch (_) {}
    grabador = null; tipoGrabador = null;
    if (vozProt) { vozProt.parar(); vozProt = null; }
    if (local) { local.getTracks().forEach((x) => x.stop()); local = null; }
    $('c-local').srcObject = null;
    jugadores.forEach((j) => { try { if (j.fuente && j.fuente.readyState === 'open') j.fuente.endOfStream(); } catch (_) {} j.v.removeAttribute('src'); });
    jugadores.clear();
    $('c-rejilla').innerHTML = ''; $('c-rejilla').dataset.n = '0';
    arrancando = false;
  };
  // ---------- WebRTC (17 Oct 2026, «la tecnología de WhatsApp y Google Meet») ----------
  // Voice and video go over UDP through GUARDIANA's relay (TURN, «relay» only: neither side
  // ever sees the other's address), encrypted from this page to the other end (DTLS-SRTP).
  // What sets the call up (offer, answer, candidates) travels inside Olm through the program,
  // so the keys in it are the verified person's. Real-time audio and video, with the jitter
  // buffer, loss recovery and bitrate that adapt to the network: what the recorded pieces
  // could not do. If it does not connect within 15 s, both ends go back to the pieces.
  let rtc = null, rtcFallo = false, rtcEspera = 0;
  // The phone's voice while the phone carries the call through this page's bridge (relevo.js,
  // 18 Oct 2026): it is what this computer sends to the other person then.
  let enMovil = null;
  const puenteAhora = (m) => !!((((m || {}).movil || {}).paso || {}).puente);
  const rtcManda = (d) => manda({ tipo: 'secure', accion: 'rtc', d });
  const obtenLocal = async (video) => {
    // 18 Oct 2026: the microphone as opus.js asks for it (mono, 48 kHz, the browser's echo
    // canceller, noise suppression and gain).
    const audio = window.GuardianaOpus ? window.GuardianaOpus.restricciones() : { echoCancellation: true, noiseSuppression: true, autoGainControl: true };
    try {
      return await navigator.mediaDevices.getUserMedia({ audio, video: video ? { width: { ideal: 1280 }, height: { ideal: 720 }, frameRate: { ideal: 30 } } : false });
    } catch (e) {
      if (video) { try { const s = await navigator.mediaDevices.getUserMedia({ audio, video: false }); aviso(t('sec_l_sin_camara')); muestraPermiso(e, 'camara'); return s; } catch (e2) { e = e2; } }
      muestraPermiso(e, 'microfono');
      return null;
    }
  };
  // What leaves: the voice changed when «Proteger mi voz» is on, the camera as it is.
  const rtcEnvio = () => (enMovil ? new MediaStream([enMovil]) : fuenteGrabacion());
  const rtcTile = (stream) => {
    remoto = stream;
    $('c-rejilla').innerHTML = '';
    const tile = document.createElement('div'); tile.className = 'tile';
    const v = document.createElement('video'); v.autoplay = true; v.playsInline = true; v.srcObject = stream; v.muted = !!enMovil;
    const n = document.createElement('span'); n.className = 'tile-nombre'; n.textContent = (E && E.contacto) || '';
    tile.appendChild(v); tile.appendChild(n); $('c-rejilla').appendChild(tile); $('c-rejilla').dataset.n = '1';
    v.play().catch(() => {});
    window.dispatchEvent(new CustomEvent('guardiana-remoto'));
  };
  const rtcNuevaPc = () => {
    const pc = new RTCPeerConnection({ iceServers: rtc.ice, iceTransportPolicy: 'relay', bundlePolicy: 'max-bundle' });
    const envio = rtcEnvio();
    envio.getTracks().forEach((x) => pc.addTrack(x, envio));
    pc.onicecandidate = (e) => { if (e.candidate && rtc && rtc.pc === pc) rtcManda({ t: 'ice', s: rtc.s, c: e.candidate.toJSON() }); };
    pc.ontrack = (e) => { if (rtc && rtc.pc === pc) rtcTile(e.streams[0] || new MediaStream([e.track])); };
    pc.onconnectionstatechange = () => {
      if (!rtc || rtc.pc !== pc) return;
      if (pc.connectionState === 'connected') { rtc.conectada = true; clearTimeout(rtc.reloj); }
      else if (pc.connectionState === 'failed') rtcFalla(true);
    };
    // Video up to 1.2 Mbps; WebRTC lowers it by itself when the network cannot carry it.
    pc.getSenders().forEach((x) => { if (x.track && x.track.kind === 'video') { try { const p = x.getParameters(); p.encodings = p.encodings && p.encodings.length ? p.encodings : [{}]; p.encodings[0].maxBitrate = 1200000; x.setParameters(p).catch(() => {}); } catch (_) {} } });
    // 18 Oct 2026 (opus.js): RED first, the voice's limits, the quality estimate every 2 s.
    if (window.GuardianaOpus) { const o = window.GuardianaOpus.prepara(pc); pc.addEventListener('connectionstatechange', () => { if (pc.connectionState === 'closed') o.para(); }); }
    return pc;
  };
  const rtcOfrece = async () => {
    if (!rtc) return;
    try { if (rtc.pc) rtc.pc.close(); } catch (_) {}
    rtc.s = Math.random().toString(36).slice(2, 10);
    rtc.pc = rtcNuevaPc(); rtc.pendientes = [];
    const o = await rtc.pc.createOffer();
    if (window.GuardianaOpus) o.sdp = window.GuardianaOpus.sdp(o.sdp);
    await rtc.pc.setLocalDescription(o);
    rtcManda({ t: 'offer', s: rtc.s, sdp: rtc.pc.localDescription.sdp });
  };
  const empiezaRtc = async (l) => {
    arrancando = true;
    llamadaVideo = !!l.video;
    // The phone already carries the voice through the bridge: this computer's microphone stays closed.
    const s = enMovil ? new MediaStream() : await obtenLocal(!!l.video);
    if (!s || !(E && E.llamada)) { if (s) s.getTracks().forEach((x) => x.stop()); arrancando = false; return; }
    local = s;
    local.getAudioTracks().forEach((x) => { x.enabled = !silenciado; });
    if (pausaAplicada) local.getTracks().forEach((x) => { x.enabled = false; });
    $('c-permiso').hidden = !$('c-permiso').dataset.camara;
    $('c-local').srcObject = local;
    llamadaVideo = !!l.video && local.getVideoTracks().length > 0;
    rtc = { ice: l.rtc.ice, ofrece: !!l.rtc.ofrece, pc: null, s: null, pendientes: [], conectada: false, reloj: null };
    rtc.reloj = setTimeout(() => { if (rtc && !rtc.conectada) rtcFalla(true); }, 15000);
    arrancando = false;
    // «I am here»: the side that offers starts again with a fresh offer when it hears it, so
    // the two meet whichever started first (or when a phone takes the call over).
    rtcManda({ t: 'hola' });
    if (rtc.ofrece) rtcOfrece().catch(() => rtcFalla(true));
  };
  const rtcAplicaPendientes = () => { if (!rtc || !rtc.pc || !rtc.pc.remoteDescription) return; const p = rtc.pendientes.filter((x) => x.s === rtc.s); rtc.pendientes = []; p.forEach((x) => rtc.pc.addIceCandidate(x.c).catch(() => {})); };
  en('secure_rtc', async (m) => {
    const d = m.d || {};
    if (d.t === 'fallo') { if (rtc) rtcFalla(false); else rtcFallo = true; return; }
    if (!rtc) return;
    try {
      if (d.t === 'hola') { if (rtc.ofrece) await rtcOfrece(); }
      else if (d.t === 'offer' && !rtc.ofrece && d.s === rtc.s && rtc.pc && rtc.pc.signalingState === 'stable' && d.reinicio) {
        // An ICE restart after a change of network (18 Oct 2026): the same connection, new paths.
        await rtc.pc.setRemoteDescription({ type: 'offer', sdp: d.sdp });
        const a = await rtc.pc.createAnswer();
        if (window.GuardianaOpus) a.sdp = window.GuardianaOpus.sdp(a.sdp);
        await rtc.pc.setLocalDescription(a);
        rtcManda({ t: 'answer', s: rtc.s, sdp: rtc.pc.localDescription.sdp });
      } else if (d.t === 'offer' && !rtc.ofrece) {
        try { if (rtc.pc) rtc.pc.close(); } catch (_) {}
        rtc.s = d.s; rtc.pc = rtcNuevaPc();
        await rtc.pc.setRemoteDescription({ type: 'offer', sdp: d.sdp });
        const a = await rtc.pc.createAnswer();
        if (window.GuardianaOpus) a.sdp = window.GuardianaOpus.sdp(a.sdp);
        await rtc.pc.setLocalDescription(a);
        rtcManda({ t: 'answer', s: rtc.s, sdp: rtc.pc.localDescription.sdp });
        rtcAplicaPendientes();
      } else if (d.t === 'answer' && rtc.ofrece && d.s === rtc.s && rtc.pc && rtc.pc.signalingState === 'have-local-offer') {
        await rtc.pc.setRemoteDescription({ type: 'answer', sdp: d.sdp });
        rtcAplicaPendientes();
      } else if (d.t === 'ice' && d.c) {
        if (rtc.pc && d.s === rtc.s && rtc.pc.remoteDescription) await rtc.pc.addIceCandidate(d.c).catch(() => {});
        else if (rtc.pendientes.length < 200) rtc.pendientes.push({ s: d.s, c: d.c });
      }
    } catch (_) { rtcFalla(true); }
  });
  // Wi-Fi to 4G or back: the side that offers restarts ICE on the same connection, so the
  // voice comes back on the new network without hanging up (18 Oct 2026).
  if (window.GuardianaOpus) window.GuardianaOpus.enCambioDeRed(async () => {
    if (!rtc || !rtc.ofrece || !rtc.pc || rtc.pc.signalingState !== 'stable') return;
    try {
      const o = await rtc.pc.createOffer({ iceRestart: true });
      o.sdp = window.GuardianaOpus.sdp(o.sdp);
      await rtc.pc.setLocalDescription(o);
      rtcManda({ t: 'offer', s: rtc.s, sdp: rtc.pc.localDescription.sdp, reinicio: true });
    } catch (_) {}
  });
  const paraRtc = () => {
    if (!rtc) return;
    clearTimeout(rtc.reloj);
    try { if (rtc.pc) rtc.pc.close(); } catch (_) {}
    rtc = null;
  };
  // WebRTC did not connect: both ends go back to the recorded pieces for this call.
  const rtcFalla = (decirlo) => {
    if (decirlo) rtcManda({ t: 'fallo' });
    rtcFallo = true;
    const l = E && E.llamada;
    // The bridge to the phone rides on this connection: it ends too, and the voice is here again.
    if (window.GuardianaRelevo) window.GuardianaRelevo.rtcCayo();
    if (enMovil) { enMovil = null; $('c-rejilla').querySelectorAll('video').forEach((v) => { v.muted = false; }); }
    paraRtc();
    if (local) { local.getTracks().forEach((x) => x.stop()); local = null; }
    $('c-rejilla').innerHTML = ''; $('c-rejilla').dataset.n = '0';
    if (l && !movilHabla) empiezaMedios(!!l.video);
  };
  // «Proteger mi voz» switched during a WebRTC call: the same sender, the other voice.
  const rtcVoz = () => {
    if (!rtc || !rtc.pc || !local) return;
    const pista = rtcEnvio().getAudioTracks()[0];
    rtc.pc.getSenders().forEach((x) => { if (x.track && x.track.kind === 'audio' && pista) x.replaceTrack(pista).catch(() => {}); });
  };
  // What the bridge to the phone needs from this call (relevo.js, 18 Oct 2026).
  window.GuardianaLlamada = {
    pc: () => (rtc && rtc.pc) || null,
    remoto: () => remoto,
    // The phone carries the voice: its track replaces ours on the same connection, this
    // microphone closes and this speaker goes quiet.
    alMovil: (pista) => {
      enMovil = pista;
      if (rtc && rtc.pc) rtc.pc.getSenders().forEach((x) => { if (x.track && x.track.kind === 'audio') x.replaceTrack(pista).catch(() => {}); });
      if (local) local.getAudioTracks().forEach((x) => { x.stop(); local.removeTrack(x); });
      if (vozProt) { vozProt.parar(); vozProt = null; }
      $('c-rejilla').querySelectorAll('video').forEach((v) => { v.muted = true; });
    },
    // «Volver al PC»: the microphone opens again and its voice replaces the phone's.
    alPc: async () => {
      enMovil = null;
      $('c-rejilla').querySelectorAll('video').forEach((v) => { v.muted = false; v.play().catch(() => {}); });
      if (!rtc || !rtc.pc) return;
      const s = await obtenLocal(false);
      if (!s || !rtc || !rtc.pc || enMovil) { if (s) s.getTracks().forEach((x) => x.stop()); return; }
      if (!local) local = new MediaStream();
      s.getAudioTracks().forEach((x) => { x.enabled = !silenciado && !pausaAplicada; local.addTrack(x); });
      const pista = rtcEnvio().getAudioTracks()[0];
      rtc.pc.getSenders().forEach((x) => { if ((!x.track || x.track.kind === 'audio') && pista) x.replaceTrack(pista).catch(() => {}); });
    },
  };
  const llama = (video) => { if ($('c-voz').disabled) return; $(video ? 'c-video' : 'c-voz').classList.add('pulsado'); $('c-voz').disabled = true; $('c-video').disabled = true; timbre(); manda({ tipo: 'secure', accion: 'llamar', video, mime: mime(video) }); };
  $('c-voz').addEventListener('click', () => llama(false));
  $('c-video').addEventListener('click', () => llama(true));
  $('c-colgar').addEventListener('click', () => manda({ tipo: 'secure', accion: 'colgar' }));
  // Full screen for the video call (the owner, 16 Oct 2026): the pictures fill the screen; Esc
  // or the same button brings them back.
  $('c-pantalla').addEventListener('click', () => { const p = document.querySelector('#c-llamada .pantallas'); if (document.fullscreenElement) document.exitFullscreen().catch(() => {}); else if (p && p.requestFullscreen) p.requestFullscreen().catch(() => {}); });
  en('secure_estado', (m) => { $('c-pantalla').hidden = !(m.llamada && m.llamada.video && m.llamada.conectada !== false); });
  $('c-silencio').addEventListener('click', () => { silenciado = !silenciado; if (local) local.getAudioTracks().forEach((x) => { x.enabled = !silenciado && !pausaAplicada; }); $('c-silencio').setAttribute('aria-pressed', String(silenciado)); $('c-silencio').querySelector('span').textContent = t(silenciado ? 'sec_l_sonar' : 'sec_l_silenciar'); });

  // ---------- The call, live (17 Oct 2026: «que se vea movimiento … datos los más posibles
  // durante una llamada», the design the owner chose: Pulso, with the chat on the left) ----------
  // Every figure here is measured: WebRTC's own statistics once a second (round trip, jitter,
  // loss, bitrate, codecs, the route), and the voices' levels from the sound itself every
  // 120 ms. Where something cannot be measured (the recorded pieces have no round trip) it
  // shows «—», never an invented number.
  let remoto = null;
  let ctxVoz = null;
  const analizadores = new Map();
  const analizador = (stream) => {
    if (!stream || !stream.getAudioTracks || !stream.getAudioTracks().length) return null;
    let a = analizadores.get(stream.id);
    if (a) return a;
    try {
      ctxVoz = ctxVoz || new AudioContext();
      if (ctxVoz.state === 'suspended') ctxVoz.resume().catch(() => {});
      const fuenteVoz = ctxVoz.createMediaStreamSource(stream);
      const an = ctxVoz.createAnalyser(); an.fftSize = 512;
      fuenteVoz.connect(an);
      a = { an, buf: new Float32Array(an.fftSize), fuenteVoz };
      analizadores.set(stream.id, a);
      return a;
    } catch (_) { return null; }
  };
  const nivelDe = (a) => {
    if (!a) return 0;
    a.an.getFloatTimeDomainData(a.buf);
    let suma = 0; for (let i = 0; i < a.buf.length; i++) suma += a.buf[i] * a.buf[i];
    return Math.min(1, Math.sqrt(suma / a.buf.length) * 5);
  };
  // The other voice: the WebRTC stream, or what the player of the recorded pieces is playing.
  const streamRemoto = () => {
    if (remoto && remoto.active) return remoto;
    const v = document.querySelector('#c-rejilla video');
    if (v && v.captureStream) { try { if (!v.dataset.capturado) { v.dataset.capturado = '1'; v._captura = v.captureStream(); } return v._captura; } catch (_) {} }
    return null;
  };
  const BARRAS = 12;
  const barras = { yo: Array(BARRAS).fill(0), el: Array(BARRAS).fill(0) };
  const SVGNS = 'http://www.w3.org/2000/svg';
  ['yo', 'el'].forEach((k) => {
    const g = $('vj-barras-' + k), cx = k === 'yo' ? 96 : 604;
    for (let i = 0; i < BARRAS; i++) { const r = document.createElementNS(SVGNS, 'rect'); r.setAttribute('x', String(cx - 40.5 + i * 7)); r.setAttribute('width', '4'); r.setAttribute('rx', '2'); r.setAttribute('y', '169'); r.setAttribute('height', '2'); g.appendChild(r); }
  });
  const nivel = { yo: 0, el: 0 };
  const pintaVoces = () => {
    const pausa = pausaAplicada;
    const crudoYo = silenciado || pausa ? 0 : nivelDe(analizador(local));
    const crudoEl = pausa ? 0 : nivelDe(analizador(streamRemoto()));
    nivel.yo = nivel.yo * 0.45 + crudoYo * 0.55; nivel.el = nivel.el * 0.45 + crudoEl * 0.55;
    ['yo', 'el'].forEach((k) => {
      const b = barras[k]; b.shift(); b.push(nivel[k]);
      const rects = $('vj-barras-' + k).children;
      for (let i = 0; i < BARRAS; i++) { const h = Math.max(2, Math.round(2 + b[i] * 20)); rects[i].setAttribute('y', String(170 - h / 2)); rects[i].setAttribute('height', String(h)); }
      $('vj-halo-' + k).setAttribute('r', (16 + nivel[k] * 16).toFixed(1));
      const habla = nivel[k] > 0.07;
      const e = $('vj-' + k + '-estado');
      e.textContent = k === 'yo' && silenciado ? t('sec_vj_silenciado') : habla ? t(k === 'yo' && vozActiva ? 'sec_vj_hablando_prot' : 'sec_vj_hablando') : t('sec_vj_silencio');
      e.setAttribute('class', 'vj-t estado ' + k + (habla ? ' habla' : ''));
    });
    $('c-l-letra').style.setProperty('--voz', nivel.el.toFixed(3));
  };
  // Once a second: WebRTC's statistics, as differences from the previous second.
  const MAXS = 60;
  const nuevaMedida = () => ({ rtt: [], jit: [], per: [], kb: [], prev: null, sumaRtt: 0, nRtt: 0, sumaKa: 0, sumaKv: 0, nK: 0, perMax: 0, codecA: '', codecV: '', res: '', fps: 0, proto: '', relay: true, url: '', pps: 0, bytes: 0, enviado: 0 });
  let MED = nuevaMedida();
  let relojVoz = null, relojMide = null;
  const leeRtc = async (pc) => {
    const st = await pc.getStats();
    const por = new Map(); st.forEach((x) => por.set(x.id, x));
    let par = null;
    st.forEach((x) => { if (x.type === 'transport' && x.selectedCandidatePairId) par = por.get(x.selectedCandidatePairId) || par; });
    if (!par) st.forEach((x) => { if (!par && x.type === 'candidate-pair' && x.nominated && x.state === 'succeeded') par = x; });
    const lc = par ? por.get(par.localCandidateId) : null;
    let ai = null, vi = null, ao = null, vo = null;
    st.forEach((x) => {
      if (x.type === 'inbound-rtp' && x.kind === 'audio') ai = x;
      if (x.type === 'inbound-rtp' && x.kind === 'video') vi = x;
      if (x.type === 'outbound-rtp' && x.kind === 'audio') ao = x;
      if (x.type === 'outbound-rtp' && x.kind === 'video') vo = x;
    });
    const codec = (r) => { const c = r && r.codecId ? por.get(r.codecId) : null; return c && c.mimeType ? String(c.mimeType).split('/')[1] || '' : ''; };
    const v = (r, k) => (r && typeof r[k] === 'number' ? r[k] : 0);
    return {
      t: performance.now(),
      rtt: par && typeof par.currentRoundTripTime === 'number' ? par.currentRoundTripTime * 1000 : null,
      jit: ai && typeof ai.jitter === 'number' ? ai.jitter * 1000 : null,
      perdidos: v(ai, 'packetsLost') + v(vi, 'packetsLost'), recibidos: v(ai, 'packetsReceived') + v(vi, 'packetsReceived'),
      bAi: v(ai, 'bytesReceived'), bVi: v(vi, 'bytesReceived'),
      bytes: v(par, 'bytesSent') + v(par, 'bytesReceived'), pk: v(par, 'packetsSent') + v(par, 'packetsReceived'),
      codecA: codec(ai) || codec(ao), codecV: codec(vi) || codec(vo),
      res: vi && vi.frameWidth ? vi.frameWidth + '×' + vi.frameHeight : '', fps: vi && vi.framesPerSecond ? Math.round(vi.framesPerSecond) : 0,
      relay: lc ? lc.candidateType === 'relay' : true, proto: lc ? String(lc.relayProtocol || lc.protocol || '').toUpperCase() : '',
    };
  };
  const ciudadDe = (h) => (/rele-am\./i.test(h || '') ? 'Miami' : /rele\.guardianagroup/i.test(h || '') ? 'Núremberg' : '');
  const empuja = (a, x) => { a.push(x); if (a.length > MAXS) a.shift(); };
  const mide = async () => {
    const l = E && E.llamada;
    if (!l || l.conectada === false || movilHabla) return;
    const tr = (E.monitor || {}).transporte || {};
    let r = null;
    if (rtc && rtc.pc && rtc.conectada) { try { r = await leeRtc(rtc.pc); } catch (_) { r = null; } }
    const p = MED.prev;
    if (r) {
      const dt = p ? Math.max(0.2, (r.t - p.t) / 1000) : 0;
      if (r.rtt != null) { empuja(MED.rtt, r.rtt); MED.sumaRtt += r.rtt; MED.nRtt++; }
      if (r.jit != null) empuja(MED.jit, r.jit);
      if (p && dt) {
        const dL = Math.max(0, r.perdidos - p.perdidos), dR = Math.max(0, r.recibidos - p.recibidos);
        const per = dL + dR ? (100 * dL) / (dL + dR) : 0;
        empuja(MED.per, per); MED.perMax = Math.max(MED.perMax, per);
        const kb = (Math.max(0, r.bytes - p.bytes) * 8) / 1000 / dt;
        empuja(MED.kb, kb);
        MED.sumaKa += (Math.max(0, r.bAi - p.bAi) * 8) / 1000 / dt; MED.sumaKv += (Math.max(0, r.bVi - p.bVi) * 8) / 1000 / dt; MED.nK++;
        MED.pps = Math.max(0, r.pk - p.pk) / dt;
      }
      MED.bytes = r.bytes; MED.codecA = r.codecA || MED.codecA; MED.codecV = r.codecV || MED.codecV; MED.res = r.res || MED.res; MED.fps = r.fps || MED.fps;
      MED.relay = r.relay; MED.proto = r.proto || MED.proto;
      MED.url = ((rtc.ice || [])[0] || {}).urls; MED.url = Array.isArray(MED.url) ? MED.url[0] : MED.url || '';
      MED.prev = r;
    } else {
      // The recorded pieces: only the bitrate the program measured; no round trip to show.
      const kb = (tr.kbps_audio || 0) + (tr.kbps_video || 0);
      if (kb) { empuja(MED.kb, kb); MED.sumaKa += tr.kbps_audio || 0; MED.sumaKv += tr.kbps_video || 0; MED.nK++; }
      MED.pps = kb ? 8 : 0; MED.bytes += (kb * 1000) / 8;
      MED.relay = !!E.rele || tr.directo === false; MED.proto = ''; MED.url = E.rele_usado || '';
      MED.codecA = tr.codec_audio || MED.codecA; MED.codecV = tr.codec_video || MED.codecV; MED.res = tr.resolucion || MED.res;
    }
    pintaMedidas();
    // What the report will say about the connection, kept current in the program.
    if (Date.now() - MED.enviado > 5000) {
      MED.enviado = Date.now();
      const d = { rele: MED.relay, codec_audio: MED.codecA, codec_video: llamadaVideo ? MED.codecV : '', resolucion: llamadaVideo ? MED.res : '' };
      if (MED.nRtt) d.latencia_media_ms = Math.round(MED.sumaRtt / MED.nRtt);
      if (MED.per.length) d.perdida_max_pct = Math.round(MED.perMax * 10) / 10;
      if (MED.nK) { d.kbps_audio_medio = Math.round(MED.sumaKa / MED.nK); if (llamadaVideo) d.kbps_video_medio = Math.round(MED.sumaKv / MED.nK); }
      manda({ tipo: 'secure', accion: 'medidas', d });
    }
    const conVideo = !!(E.llamada && E.llamada.video) && (rtc ? !!(remoto && remoto.getVideoTracks().some((x) => x.readyState === 'live')) : jugadores.size > 0);
    $('c-llamada').classList.toggle('con-video', conVideo);
  };
  const trazo = (a, w, h, lo, hi) => (a.length < 2 ? '' : a.map((v, i) => `${((i * w) / (a.length - 1)).toFixed(1)},${(h - 2 - ((Math.min(hi, Math.max(lo, v)) - lo) / (hi - lo)) * (h - 4)).toFixed(1)}`).join(' '));
  const coma = (x, d) => Number(x).toLocaleString(document.documentElement.lang || 'es', { minimumFractionDigits: d, maximumFractionDigits: d });
  const ultimoDe = (a) => (a.length ? a[a.length - 1] : null);
  const pintaMedidas = () => {
    const rtt = ultimoDe(MED.rtt), jit = ultimoDe(MED.jit), per = ultimoDe(MED.per), kb = ultimoDe(MED.kb);
    const techo = Math.max(150, ...MED.rtt.map((x) => x * 1.15));
    const pts = trazo(MED.rtt, 320, 92, 0, techo);
    $('vp-linea').setAttribute('points', pts);
    $('vp-area').setAttribute('points', pts ? `0,92 ${pts} 320,92` : '');
    if (rtt != null) { const y = 92 - 2 - (rtt / techo) * 88; $('vp-punto').style.left = '100%'; $('vp-punto').style.top = `${(y / 92) * 100}%`; $('vp-punto').hidden = false; } else $('vp-punto').hidden = true;
    $('vp-rtt').textContent = rtt != null ? `${Math.round(rtt)} ms` : RAYA;
    $('vv-lat').textContent = rtt != null ? String(Math.round(rtt)) : RAYA;
    $('vv-lat-l').setAttribute('points', trazo(MED.rtt.slice(-40), 146, 26, 0, techo));
    $('vv-lat-n').textContent = t('sec_vv_latencia_n', { n: quien(E) });
    $('vv-jit').textContent = jit != null ? coma(jit, 1) : RAYA;
    $('vv-jit-l').setAttribute('points', trazo(MED.jit, 146, 26, 0, 30));
    const jitMal = jit != null && jit > 30;
    $('vv-jit-n').textContent = jit == null ? '' : t(jitMal ? 'sec_vv_jitter_mal' : 'sec_vv_jitter_ok');
    $('vv-jit-n').classList.toggle('mal', jitMal);
    $('vv-per').textContent = per != null ? coma(per, 1) : RAYA;
    $('vv-per-l').setAttribute('points', trazo(MED.per, 146, 26, 0, 5));
    const perMal = per != null && per > 2;
    $('vv-per-n').textContent = per == null ? '' : t(perMal ? 'sec_vv_perdida_mal' : 'sec_vv_perdida_ok');
    $('vv-per-n').classList.toggle('mal', perMal);
    $('vv-kb').textContent = kb != null ? coma(kb / 1000, 2) : RAYA;
    $('vv-kb-l').setAttribute('points', trazo(MED.kb, 146, 26, 0, Math.max(200, ...MED.kb.map((x) => x * 1.1))));
    const codecs = [llamadaVideo && MED.codecV ? MED.codecV.toUpperCase() + (MED.res ? ' ' + MED.res : '') + (MED.fps ? ' · ' + MED.fps + ' fps' : '') : '', MED.codecA ? MED.codecA.charAt(0).toUpperCase() + MED.codecA.slice(1) : ''].filter(Boolean).join(' · ');
    $('vv-kb-n').textContent = codecs;
    // The journey's figures and its route.
    const mb = MED.bytes / 1e6;
    $('vj-cifra').textContent = t('sec_vj_cifra', { p: n(Math.round(MED.pps)), mb: coma(mb, 1) });
    $('vj-paquetes').classList.toggle('quieto', !MED.pps);
    const ciudad = ciudadDe(MED.url);
    $('c-viaje').classList.toggle('directo', !MED.relay);
    $('vj-rele').textContent = MED.relay ? t('sec_vj_rele') + (ciudad ? ' · ' + ciudad : '') : t('sec_vj_directo');
    $('vj-rele-s').textContent = t(MED.relay ? 'sec_vj_rele_s' : 'sec_vj_directo_s');
    const pie = [];
    if (rtc && rtc.conectada) { pie.push(esc([MED.proto || 'UDP', MED.relay ? 'TURN' : ''].filter(Boolean).join(' · '))); pie.push(`<span class="ok">${esc(t('sec_vj_dtls'))}</span>`); }
    else pie.push(esc(t('sec_vj_trozos')));
    if (rtt != null) pie.push(esc(t('sec_vj_ida', { ms: n(Math.round(rtt)) })));
    $('vj-pie').innerHTML = pie.map((x) => `<span>${x}</span>`).join('');
    // 1.0.46 (C2-b): the shield's «Calidad de la conexión» reads the same measures.
    window.dispatchEvent(new CustomEvent('guardiana-medidas', { detail: { rele: !!MED.relay, ciudad, rtt: rtt == null ? null : Math.round(rtt), perdida: per, webrtc: !!(rtc && rtc.conectada) } }));
  };
  const medidorEmpieza = () => {
    if (relojMide) return;
    MED = nuevaMedida();
    pintaMedidas();
    relojMide = setInterval(() => { mide().catch(() => {}); }, 1000);
    relojVoz = setInterval(pintaVoces, 120);
  };
  const medidorPara = () => {
    clearInterval(relojMide); clearInterval(relojVoz); relojMide = null; relojVoz = null;
    analizadores.forEach((a) => { try { a.fuenteVoz.disconnect(); } catch (_) {} });
    analizadores.clear(); remoto = null; nivel.yo = 0; nivel.el = 0;
    $('c-llamada').classList.remove('con-video');
  };
  const capasActivas = (m) => [m.fase === 'lista', true, !!m.cuantico, !!(m.llamada && m.llamada.conectada !== false)].filter(Boolean).length;
  const pintaVivo = (m) => {
    const l = m.llamada;
    const caja = $('vivo');
    caja.hidden = !(l && l.conectada !== false && !movilHabla);
    if (caja.hidden) return;
    const pausada = !!(m.monitor && m.monitor.sesion && m.monitor.sesion.estado === 'pausada');
    caja.classList.toggle('pausada', pausada);
    $('vp-palabra').textContent = t(pausada ? 'sec_vv_pausada' : 'sec_vv_protegida');
    $('vp-capas').textContent = t('sec_vv_capas', { n: n(capasActivas(m)) });
  };
  // A stop (something seen, an interference) holds the call's media here as well: WebRTC
  // flows outside the program, so the page itself sends nothing and plays nothing until it ends.
  let pausaAplicada = false;
  const aplicaPausa = (m) => {
    const p = !!(m.monitor && m.monitor.sesion && m.monitor.sesion.estado === 'pausada');
    if (p === pausaAplicada) return;
    pausaAplicada = p;
    if (local) local.getTracks().forEach((x) => { x.enabled = p ? false : (x.kind === 'audio' ? !silenciado : true); });
    document.querySelectorAll('#c-rejilla video').forEach((v) => { v.muted = p; });
  };
  // ---------- This computer, watched (Windows' own record), and the other side's ----------
  const pintaVigia = (m) => {
    const caja = $('equipo'), vg = m.vigia;
    caja.hidden = !(m.fase === 'lista' && vg);
    if (caja.hidden) return;
    const este = vg.este || {};
    const usando = (r) => (vg.en_uso || []).filter((x) => x.recurso === r).map((x) => x.programa);
    const celda = (id, r, lista) => {
      const el = $(id), u = usando(r);
      let c, txt;
      if (u.length) { c = 'alerta'; txt = t('sec_eq_usando', { p: u.join(', ') }); }
      else if (lista.length) { c = 'aviso'; txt = t('sec_eq_uso', { p: lista.join(', ') }); }
      else if (r === 'pantalla' && este.pantalla_protegida) { c = 'ok'; txt = t('sec_eq_protegida_corto'); }
      else if (!este.vigilado) { c = 'nada'; txt = t('sec_eq_no_vigilable_corto'); }
      else if (r === 'pantalla') { c = 'aviso'; txt = t('sec_eq_sin_proteger'); }
      else { c = 'ok'; txt = t('sec_eq_solo'); }
      el.className = 'eq ' + c; el.querySelector('b').textContent = txt;
    };
    celda('eq-mic', 'microfono', este.microfono || []);
    celda('eq-cam', 'camara', este.camara || []);
    celda('eq-pant', 'pantalla', este.pantalla || []);
    const o = vg.otro, nom = m.contacto || '';
    const visto = (x) => [...(x.microfono || []).map((p) => t('sec_eq_visto_mic', { p })), ...(x.camara || []).map((p) => t('sec_eq_visto_cam', { p })), ...(x.pantalla || []).map((p) => t('sec_eq_visto_pant', { p }))];
    let oc = '', ot;
    if (m.otro_movil) ot = t('sec_eq_otro_movil', { n: nom });
    else if (!o) ot = t('sec_eq_otro_nada', { n: nom });
    else if (!o.vigilado) ot = t('sec_eq_otro_no_vigilable', { n: nom });
    else if (!visto(o).length) { oc = 'ok'; ot = t('sec_eq_otro_limpio', { n: nom }); }
    else { oc = 'aviso'; ot = t('sec_eq_otro_visto', { n: nom, l: visto(o).join('; ') }); }
    $('eq-otro').className = 'eq-otro ' + oc; $('eq-otro').textContent = ot;
  };
  // ---------- The call's report (17 Oct 2026: «la valoración final … si ha habido espionaje,
  // si ha habido grabación … ese es el resumen») ----------
  let INF = null;
  const nivelInforme = (r) => {
    const v = r.veredicto || {}, este = v.este || {}, o = v.otro;
    if (!r.sin_espionaje) return 'revisar';
    return !este.vigilado || v.otro_movil || !o || !o.vigilado ? 'incompleto' : 'limpio';
  };
  const filasInforme = (r) => {
    const v = r.veredicto || {}, este = v.este || {}, o = v.otro, nom = r.contacto || '';
    const nivelR = nivelInforme(r);
    const f = [];
    f.push([t('sec_in_espionaje'), t(nivelR === 'revisar' ? 'sec_in_esp_si' : nivelR === 'incompleto' ? 'sec_in_esp_no_visto' : 'sec_in_esp_no'), nivelR === 'revisar' ? 'alerta' : 'ok']);
    const mc = [...(este.microfono || []).map((p) => t('sec_eq_visto_mic', { p })), ...(este.camara || []).map((p) => t('sec_eq_visto_cam', { p }))];
    f.push([t('sec_in_grab_equipo'), !este.vigilado ? t('sec_eq_no_vigilable') : mc.length ? mc.join('; ') : t('sec_in_grab_no'), !este.vigilado ? 'aviso' : mc.length ? 'alerta' : 'ok']);
    const pc = (este.pantalla || []).map((p) => t('sec_eq_visto_pant', { p }));
    f.push([t('sec_in_grab_pantalla'), pc.length ? pc.join('; ') : t(este.pantalla_protegida ? 'sec_in_pant_bloq' : 'sec_in_pant_no'), pc.length ? 'alerta' : este.pantalla_protegida ? 'ok' : 'aviso']);
    const vo = o ? [...(o.microfono || []).map((p) => t('sec_eq_visto_mic', { p })), ...(o.camara || []).map((p) => t('sec_eq_visto_cam', { p })), ...(o.pantalla || []).map((p) => t('sec_eq_visto_pant', { p }))] : [];
    if (r.sala) f.push([t('sec_in_moviles'), t('sec_in_moviles_t'), 'aviso']);
    else f.push([t('sec_in_otro', { n: nom }), v.otro_movil ? t('sec_in_otro_movil') : !o ? t('sec_in_otro_nada') : !o.vigilado ? t('sec_in_otro_no_vigilable') : vo.length ? vo.join('; ') : t('sec_in_otro_limpio'), vo.length ? 'alerta' : o && o.vigilado && !v.otro_movil ? 'ok' : 'aviso']);
    const directa = r.conexion && r.conexion.directa === true;
    f.push([t('sec_in_servidor'), t(directa ? 'sec_in_directo_t' : 'sec_in_rele_t'), 'ok']);
    f.push([t('sec_in_identidad'), t(r.sala ? 'sec_in_identidad_sala' : v.identidad_verificada ? 'sec_in_identidad_ok' : 'sec_in_identidad_mal'), v.identidad_verificada ? 'ok' : 'alerta']);
    f.push([t('sec_in_interf'), v.interferencias ? n(v.interferencias) : t('sec_in_interf_no'), v.interferencias ? 'alerta' : 'ok']);
    const c = r.conexion || {};
    if (c.latencia_media_ms != null || c.kbps_audio_medio != null) {
      const partes = [];
      if (c.latencia_media_ms != null) partes.push(t('sec_in_con_lat', { ms: n(c.latencia_media_ms) }));
      if (c.perdida_max_pct != null) partes.push(t('sec_in_con_per', { p: coma(c.perdida_max_pct, 1) }));
      const cod = [c.codec_video ? String(c.codec_video).toUpperCase() + (c.resolucion ? ' ' + c.resolucion : '') : '', c.codec_audio ? String(c.codec_audio) : ''].filter(Boolean).join(' · ');
      if (cod) partes.push(cod);
      f.push([t('sec_in_conexion'), partes.join(' · '), '']);
    }
    if (r.firmante) f.push([t('sec_in_firmado'), String(r.firmante || '').slice(0, 22) + '…', '']);
    return f;
  };
  // 1.0.42: a call between two phones of the house leaves its summary too (the owner: «cuando se
  // acabe la llamada… un resumen de lo que ha chequeado»), in the same card: this computer was the
  // exchange and was watched; the phones are web pages and cannot be.
  const deSala = (x) => {
    if (!x) return null;
    const e = x.equipo || {};
    const limpio = !(e.microfono || []).length && !(e.camara || []).length && !(e.pantalla || []).length;
    const datos = { tipo: 'guardiana-secure-informe-casa', de: x.de, a: x.a, video: x.video, inicio: x.desde, fin: x.fin, duracion_s: x.duracion_s, camino: x.rtc ? 'webrtc-turn' : 'pc', este_pc: e };
    const csv = ['campo,valor', ...Object.entries({ de: x.de, a: x.a, video: x.video, inicio: x.desde, fin: x.fin, duracion_s: x.duracion_s, microfono: (e.microfono || []).join(' / '), camara: (e.camara || []).join(' / '), pantalla: (e.pantalla || []).join(' / '), pantalla_protegida: !!e.pantalla_protegida, vigilado: !!e.vigilado }).map(([k, v]) => `${k},"${String(v).replace(/"/g, '""')}"`)].join('\n');
    return { sala: true, clase: x.video ? 'videollamada' : 'llamada', contacto: `${x.de} · ${x.a}`, duracion_s: x.duracion_s, inicio: x.desde, fin: x.fin,
      veredicto: { este: e, otro: null, otro_movil: true, identidad_verificada: true, interferencias: 0 }, sin_espionaje: limpio,
      conexion: { directa: false }, firmante: '', firma: '', json: JSON.stringify(datos, null, 2), csv };
  };
  const pintaInforme = (m) => {
    const r = m.informe || deSala((m.sala || {}).informe);
    INF = r || null;
    // 1.0.46 (C2-c): sala.js draws «Resumen de seguridad» from this same report.
    window.dispatchEvent(new CustomEvent('guardiana-informe', { detail: { r: r || null, nivel: r ? nivelInforme(r) : '' } }));
    const velo = $('informe'), caja = $('informe-caja');
    const nuevo = velo.hidden && !!r;
    velo.hidden = !r;
    if (!r) return;
    const nv = nivelInforme(r);
    // No incident is green (the owner, 17 Oct 2026); what could not be checked is said in the rows.
    caja.classList.toggle('revisar', nv === 'revisar'); caja.classList.toggle('incompleto', false);
    $('in-que').textContent = `${t('sec_r_clase_' + r.clase)} · ${r.contacto || ''} · ${tiempo(r.duracion_s || 0)}`;
    $('in-palabra').textContent = t(nv === 'revisar' ? 'sec_in_revisar' : nv === 'incompleto' ? 'sec_in_incompleto' : 'sec_in_limpio');
    $('in-frase').textContent = t(nv === 'revisar' ? 'sec_in_revisar_frase' : nv === 'incompleto' ? 'sec_in_incompleto_frase' : 'sec_in_limpio_frase');
    $('in-datos').innerHTML = filasInforme(r).map(([a, b, c]) => `<div class="in-dato"><dt>${esc(a)}</dt><dd class="${c}">${esc(b)}</dd></div>`).join('');
    if (nuevo) $('in-pdf').focus();
  };
  const cierraInforme = () => manda({ tipo: 'secure', accion: INF && INF.sala ? 'sala_informe_visto' : 'informe_visto' });
  document.addEventListener('keydown', (e) => { if (e.key === 'Escape' && !$('informe').hidden) cierraInforme(); });
  $('in-cerrar').addEventListener('click', cierraInforme);
  $('in-json').addEventListener('click', () => { if (INF) descarga(`guardiana-secure-informe-${sello(INF.fin)}.json`, 'application/json', INF.json); });
  $('in-csv').addEventListener('click', () => { if (INF) descarga(`guardiana-secure-informe-${sello(INF.fin)}.csv`, 'text/csv;charset=utf-8', '﻿' + INF.csv); });
  $('in-pdf').addEventListener('click', () => {
    if (!INF) return;
    const r = INF, L = []; let y = 792;
    const linea2 = (x, tam, texto, negrita, ancho) => { for (const s2 of parte(texto, ancho || 92)) { L.push({ x, y, tam, negrita, texto: s2 }); y -= tam + 4; } };
    L.push({ x: 56, y, tam: 18, negrita: true, texto: 'GUARDIANA SECURE' }); y -= 26;
    L.push({ x: 56, y, tam: 13, negrita: true, texto: `${t('sec_in_titulo')} · ${r.contacto || ''}` }); y -= 18;
    L.push({ x: 56, y, tam: 9.5, texto: `${t('sec_r_clase_' + r.clase)} · ${fechaHora(r.inicio)} – ${hora(r.fin)} · ${tiempo(r.duracion_s || 0)}` }); y -= 30;
    const nv = nivelInforme(r);
    linea2(56, 15, t(nv === 'revisar' ? 'sec_in_revisar' : nv === 'incompleto' ? 'sec_in_incompleto' : 'sec_in_limpio'), true, 60);
    y -= 2;
    linea2(56, 10.5, t(nv === 'revisar' ? 'sec_in_revisar_frase' : nv === 'incompleto' ? 'sec_in_incompleto_frase' : 'sec_in_limpio_frase'), false, 95);
    y -= 14;
    for (const [a, b] of filasInforme(r)) {
      L.push({ x: 56, y, tam: 10.5, negrita: true, texto: a });
      const trozos = parte(b, 62);
      trozos.forEach((s2, i) => { L.push({ x: 220, y: y - i * 14, tam: 10.5, texto: s2 }); });
      y -= Math.max(1, trozos.length) * 14 + 6;
    }
    y -= 10;
    linea2(56, 9.5, t('sec_in_limite'), false, 100);
    y -= 12;
    if (r.firmante) {
      L.push({ x: 56, y, tam: 10.5, negrita: true, texto: t('sec_r_pdf_firmado') }); y -= 15;
      for (const s2 of parte(r.firmante, 80)) { L.push({ x: 56, y, tam: 8.5, texto: s2 }); y -= 12; }
      y -= 4;
      for (const s2 of parte(r.firma, 80)) { L.push({ x: 56, y, tam: 8.5, texto: s2 }); y -= 12; }
      y -= 14;
      linea2(56, 9.5, t('sec_r_pdf_nota'), false, 100);
    }
    const a = document.createElement('a'); a.href = URL.createObjectURL(new Blob([pdfDe(L)], { type: 'application/pdf' })); a.download = `guardiana-secure-informe-${sello(r.fin)}.pdf`; document.body.appendChild(a); a.click(); setTimeout(() => { URL.revokeObjectURL(a.href); a.remove(); }, 1000);
  });

  let ultimo = {};
  en('monitor', (m) => { ultimo = m.monitor || {}; pinta(ultimo); });
  // The texts arrive after the page: draw again so «—» and the chips speak the person's language.
  // The chat too (17 Oct 2026: call lines drawn before the texts arrived stayed as
  // «sec_ll_video · 20 s», because the list is only redrawn when the messages change).
  en('textos', () => { pinta(ultimo); chatAntes = ''; if (E && E.fase) { fasePinta(); pintaSala(E); } });
  // Where this computer is, by its time zone (the same the globe uses): the other side hears it
  // inside the cipher, and each computer draws the line between the two on its own globe.
  // «listo» first, so the texts arrive before any state is drawn (17 Oct 2026 review).
  manda({ tipo: 'listo', vista: 'secure' });
  // Reloading the page starts from nothing: no conversation, no messages, no open invitation
  // (the program keeps a call that is still going).
  try { const nav = performance.getEntriesByType('navigation')[0]; if (nav && nav.type === 'reload') manda({ tipo: 'secure', accion: 'empezar_limpio' }); } catch (_) {}
  mandaNombre();
  try { manda({ tipo: 'secure', accion: 'zona', zona: Intl.DateTimeFormat().resolvedOptions().timeZone || '' }); } catch (_) {}
})();

// 1.0.42 — the GUARDIANA number (17 Oct 2026, the owner: «a cada persona que descarga GUARDIANA se
// le asigne un número… para siempre… sin tener que compartir QR»). The number is shown on the
// start screen; typing someone's number calls their PC; the first time, the three numbers are
// compared as with any invitation, so nobody can pass for someone else. The agenda lists the
// people already verified, each with its number and «Llamar».
'use strict';
(() => {
  const { manda, en, t, esc, $ } = Z;
  const formatea = (v) => { const d = String(v || '').replace(/\D/g, '').slice(0, 12); return [d.slice(0, 4), d.slice(4, 8), d.slice(8, 12)].filter(Boolean).join('-'); };
  const llama = (numero) => manda({ tipo: 'secure', accion: 'conectar', direccion: numero });
  let estado = {};
  const pinta = (m) => {
    estado = m || {};
    const mio = estado.mi_numero;
    $('c-mi-numero-caja').hidden = !mio;
    if (mio) $('c-mi-numero').textContent = mio;
    const cs = (estado.contactos || []).filter((c) => c.estado && c.numero);
    $('c-agenda').hidden = !cs.length;
    const html = cs.map((c) => `<li><span class="avatar">${esc(((c.nombre || '·').trim()[0] || '·').toUpperCase())}</span><span class="ag-quien"><b>${esc(c.nombre || c.numero)}</b><span>${esc(c.numero)}</span></span><button type="button" class="boton chico principal" data-numero="${esc(c.numero)}">${esc(t('sec_num_llamar'))}</button></li>`).join('');
    if ($('c-agenda-lista').dataset.html !== html) { $('c-agenda-lista').innerHTML = html; $('c-agenda-lista').dataset.html = html; }
    const otro = estado.comparando && estado.numero_otro;
    $('c-comparar-num').hidden = !otro;
    if (otro) $('c-comparar-num').textContent = t('sec_num_de_quien', { n: estado.numero_otro });
  };
  en('secure_estado', pinta);
  en('textos', () => { $('c-agenda-lista').dataset.html = ''; pinta(estado); });
  $('c-mi-numero-copiar').addEventListener('click', async () => { try { await navigator.clipboard.writeText($('c-mi-numero').textContent); $('c-mi-numero-copiar').textContent = t('sec_c_copiado'); setTimeout(() => { $('c-mi-numero-copiar').textContent = t('sec_c_copiar'); }, 1500); } catch (_) {} });
  const campo = $('c-llamar-num-campo');
  campo.addEventListener('input', () => { const f = formatea(campo.value); if (f !== campo.value) campo.value = f; $('c-llamar-num-error').hidden = true; });
  $('c-llamar-num').addEventListener('submit', (e) => {
    e.preventDefault();
    const d = campo.value.replace(/\D/g, '');
    if (d.length !== 12) { $('c-llamar-num-error').hidden = false; return; }
    llama(formatea(d));
  });
  $('c-agenda-lista').addEventListener('click', (e) => { const b = e.target.closest('button[data-numero]'); if (b) llama(b.dataset.numero); });
  // Block: this key never gets through again, by number or by invitation; the conversation ends.
  $('c-bloquear').addEventListener('click', () => { manda({ tipo: 'secure', accion: 'bloquear' }); manda({ tipo: 'secure', accion: 'sas', iguales: false, nombre: '' }); });
})();
