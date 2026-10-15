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
  $('p-verificar').addEventListener('click', () => manda({ tipo: 'secure', accion: 'verificar' }));
  $('p-prueba').addEventListener('click', () => manda({ tipo: 'secure', accion: 'prueba' }));
  $('p-terminar').addEventListener('click', () => manda({ tipo: 'secure', accion: 'cerrar' }));
  // The «paused for» clock ticks on its own; the program resends the state every second anyway.
  setInterval(() => { if (!$('p').hidden) pintaPausa(ultimo); }, 1000);

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
    $('c-conexion').hidden = !(f === 'sola' || f === 'escuchando');
    $('c-comparar').hidden = f !== 'comparando' || !E.numeros;
    $('c-espera').hidden = !(f === 'saludando' || f === 'abriendo' || f === 'buscando' || (f === 'comparando' && !E.numeros));
    $('c-chat').hidden = f !== 'lista';
    $('c-huella').textContent = E.huella || RAYA;
    // Waiting: the door in the router gives a GS code; no door, the blind relay gives a GR one.
    const codigo = E.codigo || E.rele_codigo;
    if (f === 'escuchando') { $('c-puerto').hidden = false; $('c-puerto').textContent = E.codigo ? t('sec_c_puerta_abierta') : E.rele_codigo ? t('sec_c_rele_listo') : E.puerta_error ? (E.rele_error ? t('sec_c_puerto', { p: E.puerto }) : t('sec_c_rele_buscando')) : t('sec_c_abriendo'); } else $('c-puerto').hidden = true;
    $('c-codigo-caja').hidden = !(f === 'escuchando' && codigo);
    if (codigo) $('c-codigo').textContent = codigo;
    $('c-codigo-texto').textContent = t(E.rele_codigo && !E.codigo ? 'sec_c_rele_texto' : 'sec_c_codigo_texto');
    $('c-puerta-error').hidden = !(f === 'escuchando' && E.puerta_error && !codigo && E.rele_error);
    if (E.puerta_error) $('c-puerta-error').textContent = t('sec_c_sin_puerta', { p: E.puerto, e: E.puerta_error }) + (E.rele_error ? ' ' + t('sec_rele_fallo', { e: E.rele_error }) : '');
    $('c-espera-texto').textContent = t(f === 'saludando' ? 'sec_e_saludando' : f === 'abriendo' ? 'sec_e_abriendo' : f === 'buscando' ? 'sec_e_buscando' : 'sec_e_esperando');
    if (E.numeros) $('c-numeros').innerHTML = E.numeros.map((x) => `<b>${esc(String(x).padStart(4, '0'))}</b>`).join('');
    if (E.contacto && !$('c-nombre').value) $('c-nombre').value = E.contacto;
    $('c-con').textContent = E.contacto ? t('sec_m_con', { n: E.contacto }) : '';
    const ol = $('c-mensajes'); const abajo = ol.scrollTop + ol.clientHeight >= ol.scrollHeight - 30;
    ol.innerHTML = (E.mensajes || []).map((m) => `<li class="${m.mio ? 'mio' : ''}">${esc(m.texto)}<time>${esc(hora(m.t))}</time></li>`).join('');
    if (abajo) ol.scrollTop = ol.scrollHeight;
  };
  en('secure_estado', (m) => { E = m; ultimo = m.monitor || {}; pinta(ultimo); fasePinta(); pintaResumen(m.resumen); });
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
  $('c-prueba').addEventListener('click', () => manda({ tipo: 'secure', accion: 'prueba' }));
  en('secure_prueba', (m) => descarga(`guardiana-secure-prueba-${sello()}.json`, 'application/json', m.json));
  en('secure_aviso', (m) => { const txt = t(m.clave) + (m.texto ? ' ' + m.texto : ''); aviso(txt); });
  const avisos = [];
  function aviso(txt) { avisos.push(txt); const ol = $('c-mensajes'); const li = document.createElement('li'); li.className = 'aviso-li'; li.textContent = txt; ol.appendChild(li); ol.scrollTop = ol.scrollHeight; }
  $('c-escuchar').addEventListener('click', () => manda({ tipo: 'secure', accion: 'escuchar', puerto: 0 }));
  $('c-copiar').addEventListener('click', async () => { try { await navigator.clipboard.writeText($('c-codigo').textContent); $('c-copiar').textContent = t('sec_c_copiado'); setTimeout(() => { $('c-copiar').textContent = t('sec_c_copiar'); }, 1500); } catch (_) {} });
  $('c-conectar').addEventListener('click', () => { const d = $('c-direccion').value.trim(); if (d) manda({ tipo: 'secure', accion: 'conectar', direccion: d }); });
  $('c-direccion').addEventListener('keydown', (e) => { if (e.key === 'Enter') $('c-conectar').click(); });
  $('c-sas-si').addEventListener('click', () => manda({ tipo: 'secure', accion: 'sas', iguales: true, nombre: $('c-nombre').value.trim() }));
  $('c-sas-no').addEventListener('click', () => manda({ tipo: 'secure', accion: 'sas', iguales: false, nombre: '' }));
  $('c-cerrar').addEventListener('click', () => manda({ tipo: 'secure', accion: 'cerrar' }));
  $('c-form').addEventListener('submit', (e) => { e.preventDefault(); const x = $('c-texto').value.trim(); if (!x) return; manda({ tipo: 'secure', accion: 'texto', texto: x }); $('c-texto').value = ''; });
  // Files: read here in 64 KB pieces and handed to the program one by one, so a big file never
  // sits in memory; the program encrypts each piece and sends it, and the key goes inside Olm.
  const TROZO = 64 * 1024;
  let cola = [], enviando = false;
  const b64De = (buf) => { let s = ''; const u = new Uint8Array(buf); for (let i = 0; i < u.length; i += 0x8000) s += String.fromCharCode.apply(null, u.subarray(i, i + 0x8000)); return btoa(s); };
  const enviaSiguiente = async () => {
    if (enviando || !cola.length) return;
    enviando = true;
    const f = cola.shift();
    manda({ tipo: 'secure', accion: 'archivo_empieza', nombre: f.name });
    $('c-progreso').hidden = false;
    const total = Math.max(1, Math.ceil(f.size / TROZO));
    for (let i = 0; i < total; i++) {
      const trozo = await f.slice(i * TROZO, (i + 1) * TROZO).arrayBuffer();
      const ultimo = i + 1 === total && trozo.byteLength < TROZO;
      manda({ tipo: 'secure', accion: 'archivo_trozo', b64: b64De(trozo), ultimo });
      if (i + 1 === total && !ultimo) manda({ tipo: 'secure', accion: 'archivo_trozo', b64: '', ultimo: true });
      $('c-progreso-texto').textContent = t('sec_f_enviando', { n: f.name, p: Math.round((i + 1) / total * 100) });
      $('c-progreso-barra').style.width = Math.round((i + 1) / total * 100) + '%';
      await new Promise((r) => setTimeout(r, 0));
    }
    setTimeout(() => { if (!cola.length) $('c-progreso').hidden = true; }, 1500);
    enviando = false;
    enviaSiguiente();
  };
  const encola = (files) => { for (const f of files) cola.push(f); enviaSiguiente(); };
  $('c-archivo').addEventListener('change', () => { encola($('c-archivo').files); $('c-archivo').value = ''; });
  const zona = $('c-zona');
  ['dragenter', 'dragover'].forEach((ev) => zona.addEventListener(ev, (e) => { e.preventDefault(); zona.classList.add('sobre'); }));
  ['dragleave', 'drop'].forEach((ev) => zona.addEventListener(ev, (e) => { e.preventDefault(); zona.classList.remove('sobre'); }));
  zona.addEventListener('drop', (e) => { if (e.dataTransfer && e.dataTransfer.files.length) encola(e.dataTransfer.files); });
  $('p-reanudar').addEventListener('click', () => manda({ tipo: 'secure', accion: 'reanudar' }));

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
  let faseAntes = 'sola';
  en('secure_estado', (m) => {
    if (m.fase === 'comparando' && faseAntes !== 'comparando' && faseAntes !== 'saludando') timbre();
    if (m.fase === 'comparando' && faseAntes === 'escuchando') timbre();
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
  const pintaLlamada = (m) => {
    const l = m.llamada;
    const caja = $('c-llamada');
    if (!l) { caja.hidden = true; paraMedios(); return; }
    caja.hidden = false;
    caja.classList.toggle('solo-voz', !l.video);
    $('c-l-estado').textContent = t(l.video ? 'sec_l_en_video' : 'sec_l_en_voz', { t: tiempo(Math.max(0, Math.floor(Date.now() / 1000) - (l.desde || 0))) });
    if (!grabador && !arrancando) empiezaMedios(!!l.video);
  };
  setInterval(() => { if (!$('c-llamada').hidden && E.llamada) $('c-l-estado').textContent = t(E.llamada.video ? 'sec_l_en_video' : 'sec_l_en_voz', { t: tiempo(Math.max(0, Math.floor(Date.now() / 1000) - (E.llamada.desde || 0))) }); }, 1000);
  const mime = (video) => {
    const c = video ? ['video/webm;codecs=vp9,opus', 'video/webm;codecs=vp8,opus', 'video/webm'] : ['audio/webm;codecs=opus', 'audio/webm'];
    return c.find((x) => window.MediaRecorder && MediaRecorder.isTypeSupported(x)) || c[c.length - 1];
  };
  const empiezaMedios = async (video) => {
    arrancando = true;
    llamadaVideo = video;
    preparaReproductor(mime(video));
    try {
      local = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true, sampleRate: 48000 }, video: video ? { width: { ideal: 1280 }, height: { ideal: 720 }, frameRate: { ideal: 30 } } : false });
    } catch (e) { arrancando = false; aviso(t('sec_l_sin_permiso')); manda({ tipo: 'secure', accion: 'colgar' }); return; }
    $('c-local').srcObject = local;
    const tipo = mime(video);
    grabador = new MediaRecorder(local, { mimeType: tipo, audioBitsPerSecond: 64000, videoBitsPerSecond: video ? 1500000 : undefined });
    grabador.ondataavailable = async (e) => { if (e.data && e.data.size) manda({ tipo: 'secure', accion: 'media', b64: b64De(await e.data.arrayBuffer()) }); };
    grabador.start(120);
    arrancando = false;
  };
  const preparaReproductor = (tipo) => {
    const v = $('c-remoto');
    fuente = new MediaSource();
    v.src = URL.createObjectURL(fuente);
    buffer = null;
    fuente.addEventListener('sourceopen', () => {
      try { buffer = fuente.addSourceBuffer(tipo); buffer.mode = 'sequence'; buffer.addEventListener('updateend', vacia); vacia(); } catch (e) { aviso(t('sec_l_no_reproduce')); }
    });
    v.play().catch(() => {});
  };
  const vacia = () => {
    if (!buffer || buffer.updating || !pendientes.length) return;
    try { buffer.appendBuffer(pendientes.shift()); } catch (_) { pendientes = []; }
    // Keep close to live: if the player fell more than 1.5 s behind, jump.
    const v = $('c-remoto');
    try { if (v.buffered.length && v.buffered.end(v.buffered.length - 1) - v.currentTime > 0.9) v.currentTime = v.buffered.end(v.buffered.length - 1) - 0.25; } catch (_) {}
  };
  en('secure_media', (m) => { (m.trozos || []).forEach((b) => { const s = atob(b); const u = new Uint8Array(s.length); for (let i = 0; i < s.length; i++) u[i] = s.charCodeAt(i); pendientes.push(u.buffer); }); vacia(); });
  const paraMedios = () => {
    try { if (grabador && grabador.state !== 'inactive') grabador.stop(); } catch (_) {}
    grabador = null;
    if (local) { local.getTracks().forEach((x) => x.stop()); local = null; }
    $('c-local').srcObject = null;
    try { if (fuente && fuente.readyState === 'open') fuente.endOfStream(); } catch (_) {}
    fuente = null; buffer = null; pendientes = [];
    arrancando = false;
    $('c-remoto').removeAttribute('src');
  };
  $('c-voz').addEventListener('click', () => { timbre(); manda({ tipo: 'secure', accion: 'llamar', video: false }); });
  $('c-video').addEventListener('click', () => { timbre(); manda({ tipo: 'secure', accion: 'llamar', video: true }); });
  $('c-colgar').addEventListener('click', () => manda({ tipo: 'secure', accion: 'colgar' }));
  $('c-silencio').addEventListener('click', () => { silenciado = !silenciado; if (local) local.getAudioTracks().forEach((x) => { x.enabled = !silenciado; }); $('c-silencio').setAttribute('aria-pressed', String(silenciado)); $('c-silencio').textContent = t(silenciado ? 'sec_l_sonar' : 'sec_l_silenciar'); });

  let ultimo = {};
  en('monitor', (m) => { ultimo = m.monitor || {}; pinta(ultimo); });
  // The texts arrive after the page: draw again so «—» and the chips speak the person's language.
  en('textos', () => pinta(ultimo));
  manda({ tipo: 'listo', vista: 'secure' });
})();
