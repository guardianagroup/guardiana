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
    const comparando = E.comparando || (f === 'comparando' ? E.numeros : null);
    $('c-comparar').hidden = !comparando;
    $('c-comparar-quien').textContent = E.comparando && E.contacto_nuevo ? t('sec_s_nuevo', { n: E.contacto_nuevo }) : '';
    $('c-espera').hidden = !(f === 'saludando' || f === 'abriendo' || f === 'buscando' || (f === 'comparando' && !E.numeros));
    // A conversation that was cut keeps its messages on screen (16 Oct 2026: «se ha eliminado
    // el chat»): read only, with a line saying so, until the person starts another one.
    const cortada = f !== 'lista' && (E.mensajes || []).length > 0 && (f === 'sola' || f === 'escuchando');
    $('c-chat').hidden = f !== 'lista' && !cortada;
    $('c-chat').classList.toggle('cortada', cortada);
    $('c-cortada').hidden = !cortada;
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
    if (E.puerta_error) $('c-puerta-error').textContent = t('sec_c_sin_puerta', { p: E.puerto, e: E.puerta_error }) + (E.rele_error ? ' ' + t('sec_rele_fallo', { e: E.rele_error }) : '');
    $('c-espera-texto').textContent = t(f === 'saludando' ? 'sec_e_saludando' : f === 'abriendo' ? 'sec_e_abriendo' : f === 'buscando' ? 'sec_e_buscando' : 'sec_e_esperando');
    if (comparando) $('c-numeros').innerHTML = comparando.map((x) => `<b>${esc(String(x).padStart(4, '0'))}</b>`).join('');
    if (E.comparando) { if (!$('c-nombre').dataset.nuevo) { $('c-nombre').value = ''; $('c-nombre').dataset.nuevo = '1'; } }
    else { delete $('c-nombre').dataset.nuevo; if (E.contacto && !$('c-nombre').value && f !== 'lista') $('c-nombre').value = E.contacto; }
    const g = E.grupo || {};
    const nombres = g.nombres || [];
    $('c-con').textContent = nombres.length > 2 ? t('sec_m_grupo', { n: nombres.join(' · ') }) : E.contacto ? t('sec_m_con', { n: E.contacto }) : '';
    // The host, once someone is in: the code stays on screen to invite more.
    const mas = $('c-mas');
    if (g.anfitrion && E.codigo_grupo && nombres.length < (g.max || 6)) { mas.hidden = false; $('c-mas-codigo').textContent = E.codigo_grupo; } else mas.hidden = true;
    pintaChat(nombres.length > 2);
  };
  en('secure_estado', (m) => { E = m; ultimo = m.monitor || {}; pinta(ultimo); fasePinta(); pintaResumen(m.resumen); pintaProgreso(m); });
  // How far a file is, both ways, and how long it has left (16 Oct 2026).
  const duracion = (s) => { s = Math.max(0, Math.round(s)); if (s < 60) return s + ' s'; if (s < 3600) return Math.floor(s / 60) + ' min' + (s % 60 ? ' ' + (s % 60) + ' s' : ''); const h = Math.floor(s / 3600), mi = Math.floor((s % 3600) / 60); return h + ' h' + (mi ? ' ' + mi + ' min' : ''); };
  const pintaProgreso = (m) => {
    const v = m.envio, r = m.recibiendo;
    if (v && v.nombre) {
      $('c-progreso').hidden = false;
      const queda = typeof v.restante_s === 'number' ? t('sec_f_quedan', { t: duracion(v.restante_s) }) : t('sec_f_calculando');
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
  let chatAntes = '';
  const tam = (b) => (b >= 1e9 ? (b / 1e9).toFixed(1) + ' GB' : b >= 1e6 ? (b / 1e6).toFixed(1) + ' MB' : b >= 1e3 ? Math.round(b / 1e3) + ' KB' : b + ' B');
  const tarjetaArchivo = (r) => {
    const ext = (r.nombre.split('.').pop() || '').slice(0, 4).toUpperCase();
    const mini = r.imagen && !r.borrado && minis.has(r.id) ? `<img src="${minis.get(r.id)}" alt="">` : esc(ext || '·');
    const botones = r.borrado ? '' : `<div class="a-botones">${r.abrible ? `<button type="button" class="boton chico" data-archivo="archivo_abrir" data-id="${r.id}">${esc(t('sec_f_abrir'))}</button>` : ''}<button type="button" class="boton chico secundario" data-archivo="archivo_carpeta" data-id="${r.id}">${esc(t('sec_f_carpeta'))}</button><button type="button" class="boton chico secundario" data-archivo="archivo_borrar" data-id="${r.id}">${esc(t('sec_f_eliminar'))}</button></div>`;
    return `<div class="archivo${r.borrado ? ' borrado' : ''}"><div class="a-mini">${mini}</div><div class="a-info"><b>${esc(r.nombre)}</b><span>${esc(tam(r.bytes))} · ${esc(t(r.borrado ? 'sec_f_eliminado' : 'sec_f_en_descargas'))}</span></div>${botones}</div>`;
  };
  const pintaChat = (conNombre) => {
    const recibidos = E.recibidos || [];
    const clave = JSON.stringify([E.mensajes, recibidos, E.enviados, (E.envio || {}).nombre, conNombre, [...minis.keys()]]);
    if (clave === chatAntes) return;
    chatAntes = clave;
    const ol = $('c-mensajes'); const abajo = ol.scrollTop + ol.clientHeight >= ol.scrollHeight - 30;
    const usados = new Set();
    ol.innerHTML = (E.mensajes || []).map((m) => {
      const de = !m.mio && conNombre && m.de ? `<i class="de">${esc(m.de)}</i>` : '';
      if (!m.mio && typeof m.texto === 'string' && m.texto.startsWith('📎 ')) {
        const nombre = m.texto.slice(3);
        const r = recibidos.find((x) => !usados.has(x.id) && x.nombre === nombre && Math.abs((x.t || 0) - (m.t || 0)) <= 3);
        if (r) {
          usados.add(r.id);
          if (r.imagen && !r.borrado && !minis.has(r.id) && !pedidas.has(r.id)) { pedidas.add(r.id); manda({ tipo: 'secure', accion: 'archivo_vista', id: r.id }); }
          return `<li class="archivo-li">${de}${tarjetaArchivo(r)}<time>${esc(hora(m.t))}</time></li>`;
        }
      }
      if (m.mio && typeof m.texto === 'string' && m.texto.startsWith('📎 ')) {
        const nombre = m.texto.slice(3);
        const x = (E.enviados || []).find((y) => y.nombre === nombre && Math.abs((y.t || 0) - (m.t || 0)) <= 3);
        const estado = x && typeof x.llego_en === 'number' ? '✓ ' + t('sec_f_entregado_en', { t: duracion(x.llego_en) }) : (E.envio && E.envio.nombre === nombre ? t('sec_f_en_camino') : t('sec_f_enviado'));
        return `<li class="mio">${esc(m.texto)}${x ? ` <small class="a-tam">${esc(tam(x.bytes))}</small>` : ''}<span class="a-entrega">${esc(estado)}</span><time>${esc(hora(m.t))}</time></li>`;
      }
      return `<li class="${m.mio ? 'mio' : ''}">${de}${esc(m.texto)}<time>${esc(hora(m.t))}</time></li>`;
    }).join('');
    if (abajo) ol.scrollTop = ol.scrollHeight;
  };
  // The picture comes once, full size: keep a small copy only.
  en('secure_vista', (m) => {
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
  en('secure_aviso', (m) => { const extra = m.texto && /^sec_[a-z0-9_]+$/.test(m.texto) ? t(m.texto) : m.texto; const txt = t(m.clave) + (extra ? ' ' + extra : ''); aviso(txt); });
  const avisos = [];
  function aviso(txt) { avisos.push(txt); const ol = $('c-mensajes'); const li = document.createElement('li'); li.className = 'aviso-li'; li.textContent = txt; ol.appendChild(li); ol.scrollTop = ol.scrollHeight; }
  // Your name, as the others will see it (kept here; sent before inviting).
  try { $('c-yo').value = localStorage.getItem('secure_nombre') || ''; } catch (_) {}
  const mandaNombre = () => { const n = $('c-yo').value.trim().slice(0, 60); try { localStorage.setItem('secure_nombre', n); } catch (_) {} manda({ tipo: 'secure', accion: 'nombre', nombre: n }); };
  $('c-yo').addEventListener('change', mandaNombre);
  mandaNombre();
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
    manda({ tipo: 'secure', accion: 'archivo_empieza', nombre: f.name, bytes: f.size });
    $('c-progreso').hidden = false;
    $('c-progreso-texto').textContent = t('sec_f_preparando', { n: f.name });
    const total = Math.max(1, Math.ceil(f.size / TROZO));
    for (let i = 0; i < total; i++) {
      // Hand the pieces over only as fast as the network takes them (at most ~8 MB waiting):
      // a big file never fills the memory.
      while (E && E.envio && E.envio.cola > 8 * 1024 * 1024) await new Promise((r) => setTimeout(r, 150));
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
  // ---------- The phone (15 Oct 2026) ----------
  // «Pasar al móvil»: the program shows a QR code; the phone's camera opens our page, which
  // talks to this computer directly (TLS 1.3 pinned to the certificate in the code). The
  // first time, the person says OK here. While the phone talks, this screen steps back: no
  // microphone or camera here, and the other side's pieces go to the phone.
  let movilHabla = false;
  const idioma = () => (document.documentElement.lang || navigator.language || 'es').slice(0, 2);
  const pintaMovil = (m) => {
    const mv = m.movil;
    const caja = $('c-movil-caja');
    const varios = ((m.grupo || {}).nombres || []).length > 2;
    $('c-movil').hidden = !!mv || varios;
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
    $('c-mv-estado-t').textContent = t(mv.habla ? 'sec_mv_e_habla' : mv.pendiente ? 'sec_mv_e_pendiente' : mv.aprobado ? 'sec_mv_e_fuera' : 'sec_mv_e_espera');
    $('c-mv-fuera').textContent = mv.fuera ? t('sec_mv_fuera_si') : mv.puerta_error ? t('sec_mv_fuera_no', { e: mv.puerta_error }) : t('sec_mv_fuera_abriendo');
    movilHabla = !!mv.habla;
  };
  $('c-movil').addEventListener('click', () => manda({ tipo: 'secure', accion: 'movil', idioma: idioma() }));
  $('c-mv-si').addEventListener('click', () => manda({ tipo: 'secure', accion: 'movil_ok' }));
  $('c-mv-no').addEventListener('click', () => manda({ tipo: 'secure', accion: 'movil_fin' }));
  $('c-mv-fin').addEventListener('click', () => manda({ tipo: 'secure', accion: 'movil_fin' }));
  en('secure_estado', pintaMovil);
  en('secure_aviso', (m) => { if (m.clave === 'sec_mv_enganchado') timbre(); });

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
  let mimeRemoto = null, tipoGrabador = null, reproduceOtro = [];
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
      if (!sonando) { timbre(); sonando = setInterval(timbre, 2500); }
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
    return t(l.video ? 'sec_l_en_video' : 'sec_l_en_voz', { t: tiempo(Math.max(0, ahora - (l.desde || ahora))) });
  };
  const pintaLlamada = (m) => {
    reproduceOtro = m.reproduce_otro || [];
    pintaTimbre(m); pintaBotones(m);
    const l = m.llamada;
    const caja = $('c-llamada');
    if (!l) { caja.hidden = true; caja.classList.remove('llamando'); paraMedios(); mimeRemoto = null; if (tonoLlamando) { clearInterval(tonoLlamando); tonoLlamando = null; } return; }
    caja.hidden = false;
    caja.classList.toggle('solo-voz', !l.video);
    const llamando = l.conectada === false;
    caja.classList.toggle('llamando', llamando);
    caja.querySelector('.pantallas').dataset.llamando = t('sec_l_llamando', { n: quien(m) });
    $('c-l-estado').textContent = movilHabla ? t('sec_mv_habla') : textoLlamada(l);
    if (llamando) { if (!tonoLlamando) tonoLlamando = setInterval(() => { if (E && E.llamada && E.llamada.conectada === false) timbre(); }, 3000); return; }
    if (tonoLlamando) { clearInterval(tonoLlamando); tonoLlamando = null; }
    if (movilHabla) { if (grabador || arrancando) paraMedios(); return; }
    if (!grabador && !arrancando) empiezaMedios(!!l.video);
    // The other side's player (an iPhone) cannot open what we record: record in what it can.
    else if (grabador && tipoGrabador && mime(llamadaVideo) !== tipoGrabador) cambiaGrabador();
    // The other side said what its pieces are: open the player for exactly that.
    if (l.mime_remoto && l.mime_remoto !== mimeRemoto && buffer !== undefined) { mimeRemoto = l.mime_remoto; preparaReproductor(mimeRemoto); }
  };
  setInterval(() => { if (!$('c-llamada').hidden && E.llamada && !movilHabla) $('c-l-estado').textContent = textoLlamada(E.llamada); }, 1000);
  // The formats, best first; the phone's page knows the same list and tells which it can play.
  const CANDIDATOS = { video: ['video/webm;codecs=vp9,opus', 'video/webm;codecs=vp8,opus', 'video/mp4;codecs=avc1.42E01E,mp4a.40.2', 'video/webm'], voz: ['audio/webm;codecs=opus', 'audio/mp4;codecs=mp4a.40.2', 'audio/webm'] };
  const mime = (video) => {
    const c = CANDIDATOS[video ? 'video' : 'voz'];
    const puede = c.filter((x) => window.MediaRecorder && MediaRecorder.isTypeSupported(x));
    return (reproduceOtro.length && puede.find((x) => reproduceOtro.includes(x))) || puede[0] || c[c.length - 1];
  };
  const nuevoGrabador = (video) => {
    const tipo = mime(video);
    grabador = new MediaRecorder(local, { mimeType: tipo, audioBitsPerSecond: 64000, videoBitsPerSecond: video ? 1500000 : undefined });
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
  const empiezaMedios = async (video) => {
    arrancando = true;
    llamadaVideo = video;
    preparaReproductor(mimeRemoto || mime(video));
    const audio = { echoCancellation: true, noiseSuppression: true, autoGainControl: true, sampleRate: 48000 };
    try {
      local = await navigator.mediaDevices.getUserMedia({ audio, video: video ? { width: { ideal: 1280 }, height: { ideal: 720 }, frameRate: { ideal: 30 } } : false });
    } catch (e) {
      // No camera (or not allowed): the call goes on with the voice instead of ending.
      local = null;
      if (video) { try { local = await navigator.mediaDevices.getUserMedia({ audio, video: false }); aviso(t('sec_l_sin_camara')); } catch (_) {} }
      if (!local) { arrancando = false; aviso(t('sec_l_sin_permiso')); manda({ tipo: 'secure', accion: 'colgar' }); return; }
    }
    $('c-local').srcObject = local;
    llamadaVideo = video && local.getVideoTracks().length > 0;
    nuevoGrabador(llamadaVideo);
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
    try { if (grabador && grabador.state !== 'inactive') grabador.stop(); } catch (_) {}
    grabador = null; tipoGrabador = null;
    if (local) { local.getTracks().forEach((x) => x.stop()); local = null; }
    $('c-local').srcObject = null;
    jugadores.forEach((j) => { try { if (j.fuente && j.fuente.readyState === 'open') j.fuente.endOfStream(); } catch (_) {} j.v.removeAttribute('src'); });
    jugadores.clear();
    $('c-rejilla').innerHTML = ''; $('c-rejilla').dataset.n = '0';
    arrancando = false;
  };
  const llama = (video) => { if ($('c-voz').disabled) return; $(video ? 'c-video' : 'c-voz').classList.add('pulsado'); $('c-voz').disabled = true; $('c-video').disabled = true; timbre(); manda({ tipo: 'secure', accion: 'llamar', video, mime: mime(video) }); };
  $('c-voz').addEventListener('click', () => llama(false));
  $('c-video').addEventListener('click', () => llama(true));
  $('c-colgar').addEventListener('click', () => manda({ tipo: 'secure', accion: 'colgar' }));
  $('c-silencio').addEventListener('click', () => { silenciado = !silenciado; if (local) local.getAudioTracks().forEach((x) => { x.enabled = !silenciado; }); $('c-silencio').setAttribute('aria-pressed', String(silenciado)); $('c-silencio').textContent = t(silenciado ? 'sec_l_sonar' : 'sec_l_silenciar'); });

  let ultimo = {};
  en('monitor', (m) => { ultimo = m.monitor || {}; pinta(ultimo); });
  // The texts arrive after the page: draw again so «—» and the chips speak the person's language.
  en('textos', () => pinta(ultimo));
  // Where this computer is, by its time zone (the same the globe uses): the other side hears it
  // inside the cipher, and each computer draws the line between the two on its own globe.
  try { manda({ tipo: 'secure', accion: 'zona', zona: Intl.DateTimeFormat().resolvedOptions().timeZone || '' }); } catch (_) {}
  manda({ tipo: 'listo', vista: 'secure' });
})();
