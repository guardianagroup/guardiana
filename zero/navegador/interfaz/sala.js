// GUARDIANA 1.0.46 — «C2 · SALA», the Comunicaciones the owner chose on 18 Oct 2026:
//   C2-a the conversation: rooms on the left, the person on top with «Cifrado» and, apart,
//        «Identidad verificada» (the safety numbers), the messages in the middle, «+» for a
//        file, a voice note or the delivery room.
//   C2-b the call: the person in the middle with the real level of their voice, four controls,
//        and on the right «Escudo en directo» with five checks and the code for the phone.
//   C2-c after hanging up: «Resumen de seguridad», from the signed report.
// Everything here is drawn from what the program sends (`secure_estado`) and what the call
// itself measures (opus.js, secure.js). Where something cannot be measured it says so —
// «No se puede comprobar completamente» — and never that nobody else is listening.
'use strict';
(() => {
  const { manda, en, t, esc, $, n } = Z;
  if (!$('sala-escudo')) return;
  // Inside GUARDIANA ZERO (the only place with `chrome.webview`) the frame draws the top row.
  if (window.chrome && window.chrome.webview)document.body.classList.add('en-zero');
  const lang = () => document.documentElement.lang || 'es';
  const iniciales = (x) => { const p = String(x || '').trim().split(/\s+/).filter(Boolean); return p.length ? (Array.from(p[0])[0] + (p.length > 1 ? Array.from(p[p.length - 1])[0] : '')).toUpperCase() : '·'; };
  const fechaCorta = (s) => (s ? new Date(s * 1000).toLocaleDateString(lang(), { day: 'numeric', month: 'short' }) : '');
  const reloj = (s) => { s = Math.max(0, Math.round(s || 0)); const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), x = s % 60; return (h ? h + ':' + String(m).padStart(2, '0') : String(m).padStart(2, '0')) + ':' + String(x).padStart(2, '0'); };
  const coma = (x, d) => Number(x).toLocaleString(lang(), { minimumFractionDigits: d, maximumFractionDigits: d });
  const hayContacto = (m) => !!m.fase && m.fase !== 'sola' && m.fase !== 'escuchando';
  let E = {}, medidas = null, calidad = null, recibidoEn = 0;

  // ---------- Small menus («…», «+», «Salida de audio») ----------
  const menus = [];
  const cierraMenus = (salvo) => menus.forEach(([b, c]) => { if (c !== salvo) { c.hidden = true; b.setAttribute('aria-expanded', 'false'); } });
  const menu = (boton, caja, antes) => {
    if (!boton || !caja) return;
    menus.push([boton, caja]);
    boton.addEventListener('click', async (e) => {
      e.stopPropagation();
      const abrir = caja.hidden;
      cierraMenus(caja);
      if (abrir && antes) await antes();
      caja.hidden = !abrir;
      boton.setAttribute('aria-expanded', String(abrir));
      if (abrir) { const f = caja.querySelector('button:not([disabled]), label, [tabindex]'); if (f) f.focus(); }
    });
    caja.addEventListener('click', (e) => { if (e.target.closest('button, label')) { e.stopPropagation(); setTimeout(() => cierraMenus(), 0); } });
  };
  document.addEventListener('click', (e) => { if (!e.target.closest('.sala-menu')) cierraMenus(); });
  document.addEventListener('keydown', (e) => { if (e.key === 'Escape') cierraMenus(); });

  // «…» on the conversation: continue on the phone, the conversation's security, close it.
  menu($('sala-opciones'), $('sala-opciones-menu'));
  const secure = document.querySelector('.secure');
  const inspector = (si) => { secure.classList.toggle('sala-inspector', si); };
  $('sala-ver-seguridad').addEventListener('click', () => inspector(!secure.classList.contains('sala-inspector')));
  $('sala-aside-cerrar').addEventListener('click', () => inspector(false));

  // «+»: a file (the picker of secure.js), a voice note (its microphone menu), the delivery room.
  menu($('sala-mas'), $('sala-mas-menu'));
  $('sala-mas-nota').addEventListener('click', (e) => { e.stopPropagation(); cierraMenus(); $('c-microfono').click(); });
  $('sala-mas-entrega').addEventListener('click', (e) => { e.stopPropagation(); cierraMenus(); const b = $('ent-abrir'); if (b) b.click(); });
  $('sala-mas-menu').querySelector('label').addEventListener('keydown', (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); $('c-archivo').click(); } });
  // The delivery room is a panel on the right (entrega.js draws it; sala.css places it).

  // ---------- C2-a: the person on top, the rooms on the left ----------
  const pintaCabecera = (m) => {
    const f = m.fase;
    const idn = (m.monitor || {}).identidad;
    const verificado = !!idn && idn.estado !== 'clave_cambiada';
    const cortada = f !== 'lista';
    let clase = 'info', texto = t('sala_conectando');
    if (f === 'lista' && m.reconectando) { clase = 'aviso'; texto = t('sala_reconectando'); }
    else if (f === 'lista') { clase = 'ok'; texto = t(m.otro_movil ? 'sala_conectado_movil' : 'sala_conectado'); }
    else if (f === 'comparando') texto = t('sala_comparando');
    else if (cortada && (m.mensajes || []).length) { clase = ''; texto = t('sala_sin_conexion'); }
    const e = $('sala-estado'); e.className = 'sala-estado ' + clase; e.querySelector('span').textContent = texto;
    // «Cifrado» is not «Identidad verificada»: two marks, two meanings (the owner's spec).
    const ins = [];
    const esc2 = (m.escudo || {}).cifrado || {};
    if (f === 'lista' && esc2.olm) ins.push(`<span class="sala-insignia info" title="${esc(t(esc2.cuantico ? 'sala_cifrado_t_pq' : 'sala_cifrado_t'))}"><svg class="icono" viewBox="0 0 24 24" aria-hidden="true"><rect x="5" y="10.5" width="14" height="10" rx="2.2"/><path d="M8 10.5V8a4 4 0 0 1 8 0v2.5"/></svg>${esc(t('sala_cifrado'))}</span>`);
    if (f === 'lista') ins.push(verificado
      ? `<span class="sala-insignia ok" title="${esc(t('sala_identidad_ok_t', { f: fechaCorta(idn.verificado_en) }))}"><svg class="icono" viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3l7.5 2.8v5.6c0 4.6-3.1 8.2-7.5 9.6-4.4-1.4-7.5-5-7.5-9.6V5.8z"/><path d="M8.8 12.2l2.2 2.2 4.3-4.6"/></svg>${esc(t('sala_identidad_ok'))}</span>`
      : `<span class="sala-insignia aviso" title="${esc(t('sala_identidad_no_t'))}">${esc(t('sala_identidad_no'))}</span>`);
    const html = ins.join('');
    if ($('sala-insignias').dataset.html !== html) { $('sala-insignias').innerHTML = html; $('sala-insignias').dataset.html = html; }
  };
  // The people already verified (the book): not connected now, a call by their number away.
  const pintaContactos = (m) => {
    const actual = hayContacto(m) ? (m.contacto || '') : null;
    const cs = (m.contactos || []).filter((c) => c.estado && c.numero && c.nombre !== actual);
    const ol = $('sala-contactos');
    $('sala-cap-contactos').hidden = !cs.length; ol.hidden = !cs.length;
    const ocupado = hayContacto(m);
    const html = cs.map((c) => `<li data-numero="${esc(c.numero)}" class="${ocupado ? 'apagada' : ''}" title="${esc(ocupado ? t('sala_contacto_ocupado') : t('sala_contacto_llamar', { n: c.numero }))}" tabindex="${ocupado ? -1 : 0}" role="button" aria-disabled="${ocupado}"><span class="avatar">${esc(iniciales(c.nombre || c.numero))}</span><b>${esc(c.nombre || c.numero)}</b><span class="ultimo sala-punto">${esc(t('sala_sin_conexion'))}</span></li>`).join('');
    if (ol.dataset.html !== html) { ol.innerHTML = html; ol.dataset.html = html; }
  };
  const llamaContacto = (li) => {
    if (!li || li.classList.contains('apagada')) return;
    if (E.reles === false) { avisoPlus(); return; }
    manda({ tipo: 'secure', accion: 'conectar', direccion: li.dataset.numero });
  };
  $('sala-contactos').addEventListener('click', (e) => llamaContacto(e.target.closest('li[data-numero]')));
  $('sala-contactos').addEventListener('keydown', (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); llamaContacto(e.target.closest('li[data-numero]')); } });
  // What waits for an answer: our invitation, someone at the door of the home room, a delivery.
  let entregas = [];
  en('entregas', (m) => { entregas = (m && m.entregas) || []; pintaInvitaciones(E); });
  const pintaInvitaciones = (m) => {
    const filas = [];
    if (m.fase === 'escuchando') {
      const codigo = m.codigo || m.rele_codigo || '';
      filas.push(`<li data-ir="invitacion"><span class="avatar suave">${esc(iniciales(t('sala_inv_tuya')))}</span><b>${esc(t('sala_inv_tuya'))}</b><span class="ultimo">${esc(codigo || t('sec_c_preparando'))}</span><span class="sala-insignia aviso">${esc(t('sala_pendiente'))}</span></li>`);
    }
    for (const x of ((m.sala || {}).miembros || []).filter((y) => y.pendiente)) {
      filas.push(`<li data-ir="sala"><span class="avatar suave">${esc(iniciales(x.nombre))}</span><b>${esc(x.nombre || '·')}</b><span class="ultimo">${esc(t('sala_inv_sala_quiere'))}</span><span class="sala-insignia aviso">${esc(t('sala_pendiente'))}</span></li>`);
    }
    for (const x of entregas.filter((y) => y.estado === 'esperando' || y.estado === 'abierta')) {
      const vence = x.vence ? new Date(x.vence * 1000).toLocaleString(lang(), { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' }) : '';
      filas.push(`<li data-ir="entrega"><span class="avatar suave"><svg class="icono" viewBox="0 0 24 24" aria-hidden="true"><path d="M3.5 13.5l2.5-8h12l2.5 8v5a1.5 1.5 0 0 1-1.5 1.5h-14a1.5 1.5 0 0 1-1.5-1.5z"/><path d="M3.5 13.5h5l1 2h5l1-2h5"/></svg></span><b>${esc(x.titulo || t('ent_sin_titulo'))}</b><span class="ultimo">${esc(t('sala_inv_entrega', { f: vence }))}</span><span class="sala-insignia ${x.estado === 'abierta' ? 'info' : 'aviso'}">${esc(t(x.estado === 'abierta' ? 'sala_en_curso' : 'sala_pendiente'))}</span></li>`);
    }
    const ol = $('sala-invitaciones');
    $('sala-cap-inv').hidden = !filas.length; ol.hidden = !filas.length;
    const html = filas.join('');
    if (ol.dataset.html !== html) { ol.innerHTML = html; ol.dataset.html = html; }
  };
  $('sala-invitaciones').addEventListener('click', (e) => {
    const li = e.target.closest('li[data-ir]'); if (!li) return;
    if (li.dataset.ir === 'sala') { const s = document.querySelector('#c-lista li[data-vista="sala"]'); if (s) s.click(); }
    if (li.dataset.ir === 'entrega') { const b = $('ent-abrir'); if (b) b.click(); }
  });
  // «Nueva conexión»: one conversation at a time; with one open, the button says so.
  const pintaNueva = (m) => {
    const b = $('c-nueva'), ocupado = hayContacto(m);
    b.setAttribute('aria-disabled', String(ocupado));
    b.classList.toggle('apagada', ocupado);
    b.title = ocupado ? t('sala_nueva_ocupado') : '';
  };
  $('c-nueva').addEventListener('click', (e) => { if ($('c-nueva').classList.contains('apagada')) { e.stopImmediatePropagation(); avisoBreve(t('sala_nueva_ocupado')); } }, true);
  // The free level: no relay, said once in plain words (and the way to Plus is in the panel).
  const avisoPlus = () => avisoBreve(t('sala_plus_rele'));
  let breve = null;
  const avisoBreve = (txt) => {
    if (!breve) { breve = document.createElement('div'); breve.className = 'aviso-breve'; breve.setAttribute('role', 'status'); document.body.appendChild(breve); }
    breve.textContent = txt; breve.classList.add('visto');
    clearTimeout(avisoBreve.r); avisoBreve.r = setTimeout(() => breve.classList.remove('visto'), 6000);
  };

  // The status at the top right: «Sala cifrada», «En llamada · 08:42», or nothing open.
  const pintaChip = () => {
    const m = E, chip = $('m-estado-chip');
    // A stop, or something to look at, keeps the panel's own word (secure.js).
    if (chip.classList.contains('alerta') || chip.classList.contains('aviso') || m.incidente) return;
    const l = m.llamada;
    let txt, clase = 'chip sala-chip';
    if (l && l.conectada !== false) { txt = t('sala_chip_llamada', { t: reloj(Date.now() / 1000 - (l.desde || Date.now() / 1000)) }); clase += ' ok'; }
    else if (m.fase === 'lista') { txt = t('sala_chip_cifrada'); clase += ' ok'; }
    else if (m.sala && document.querySelector('#c-lista li.activa[data-vista="sala"]')) return; // the room's own word
    else txt = t('sala_chip_sin');
    if (chip.textContent !== txt) chip.textContent = txt;
    chip.className = clase;
  };

  // ---------- C2-b: the call ----------
  const BARRAS = 40;
  const onda = $('sala-onda');
  onda.innerHTML = Array.from({ length: BARRAS }, () => '<i></i>').join('');
  const historia = Array(BARRAS).fill(0);
  let relojOnda = null;
  const modoVoz = () => {
    if ($('c-voz-prot').getAttribute('aria-pressed') === 'true') return 'protegida';
    return (window.GuardianaVoz && window.GuardianaVoz.modo === 'natural') ? 'natural' : 'ruido';
  };
  const pasoOnda = () => {
    // The level secure.js measured from the other voice itself (an AnalyserNode, every 120 ms).
    const v = parseFloat(getComputedStyle($('c-l-letra')).getPropertyValue('--voz')) || 0;
    historia.shift(); historia.push(v);
    const barras = onda.children;
    for (let i = 0; i < BARRAS; i++) {
      // Newest in the middle, older to both sides: a calm, symmetric picture of the real level.
      const k = Math.abs(i - (BARRAS - 1) / 2);
      const x = historia[BARRAS - 1 - Math.min(BARRAS - 1, Math.round(k * 2))];
      barras[i].style.height = Math.max(4, Math.round(4 + x * 60)) + 'px';
    }
    const habla = v > 0.07;
    const silenciado = $('c-silencio').getAttribute('aria-pressed') === 'true';
    const quien = E.contacto || '';
    const partes = [t('sala_l_' + modoVoz())];
    partes.push(silenciado ? t('sala_l_silenciado') : t(habla ? 'sala_l_habla' : 'sala_l_calla', { n: quien }));
    const txt = partes.join(' · ');
    if ($('sala-onda-t').textContent !== txt) $('sala-onda-t').textContent = txt;
    onda.classList.toggle('habla', habla);
  };
  const pintaLlamada = (m) => {
    const l = m.llamada;
    const activa = !!l && l.conectada !== false;
    $('sala-l-nombre').textContent = m.contacto || '';
    if (activa && !relojOnda) relojOnda = setInterval(pasoOnda, 100);
    if (!activa && relojOnda) { clearInterval(relojOnda); relojOnda = null; historia.fill(0); }
    const varios = ((m.grupo || {}).nombres || []).length > 2;
    $('sala-l-movil').hidden = varios || !!(m.movil && m.movil.habla);
  };
  $('sala-l-movil').addEventListener('click', () => {
    // The code appears in the shield's panel; the person scans it there.
    if (!(E.movil && E.movil.qr)) manda({ tipo: 'secure', accion: 'movil', idioma: lang().slice(0, 2) });
    const q = $('sala-escudo').querySelector('.sala-qr'); if (q) q.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
  });
  // «Salida de audio»: the outputs this computer has, as the browser lists them.
  let salida = '';
  const aplicaSalida = () => {
    if (!salida) return;
    document.querySelectorAll('#c-rejilla video, #c-rejilla audio').forEach((v) => { if (v.setSinkId && v.sinkId !== salida) v.setSinkId(salida).catch(() => {}); });
  };
  new MutationObserver(aplicaSalida).observe($('c-rejilla'), { childList: true, subtree: true });
  const puedeSalida = () => typeof HTMLMediaElement !== 'undefined' && 'setSinkId' in HTMLMediaElement.prototype;
  menu($('sala-salida'), $('sala-salida-menu'), async () => {
    const caja = $('sala-salida-menu');
    if (!puedeSalida()) { caja.innerHTML = `<p class="sala-menu-nota">${esc(t('sala_l_salida_no'))}</p>`; return; }
    let ds = [];
    try { ds = (await navigator.mediaDevices.enumerateDevices()).filter((d) => d.kind === 'audiooutput'); } catch (_) {}
    if (ds.length <= 1) { caja.innerHTML = `<p class="sala-menu-nota">${esc(t('sala_l_salida_una'))}</p>`; return; }
    const actual = salida || 'default';
    caja.innerHTML = ds.map((d, i) => `<button type="button" role="menuitemradio" aria-checked="${d.deviceId === actual}" data-id="${esc(d.deviceId)}"><span>${esc(d.label || (d.deviceId === 'default' ? t('sala_l_salida_defecto') : t('sala_l_salida_n', { n: i + 1 })))}</span></button>`).join('');
  });
  $('sala-salida-menu').addEventListener('click', (e) => { const b = e.target.closest('button[data-id]'); if (!b) return; salida = b.dataset.id; aplicaSalida(); });

  // «Escudo en directo»: five checks, each from a real source.
  window.addEventListener('guardiana-calidad', (e) => { calidad = e.detail; pintaEscudo(E); });
  window.addEventListener('guardiana-medidas', (e) => { medidas = e.detail; });
  const ICONOS = {
    cifrado: '<rect x="5" y="10.5" width="14" height="10" rx="2.2"/><path d="M8 10.5V8a4 4 0 0 1 8 0v2.5"/>',
    identidad: '<path d="M12 3l7.5 2.8v5.6c0 4.6-3.1 8.2-7.5 9.6-4.4-1.4-7.5-5-7.5-9.6V5.8z"/><path d="M8.8 12.2l2.2 2.2 4.3-4.6"/>',
    mic: '<rect x="9" y="3" width="6" height="11" rx="3"/><path d="M5.5 11a6.5 6.5 0 0 0 13 0M12 17.5V21"/>',
    cam: '<rect x="3" y="6.5" width="13" height="11" rx="2"/><path d="M16 10.5l5-3v9l-5-3"/>',
    red: '<path d="M3 9a13 13 0 0 1 18 0M6 12.5a8.5 8.5 0 0 1 12 0M9 16a4 4 0 0 1 6 0"/><circle cx="12" cy="19" r=".8" fill="currentColor"/>',
  };
  const fila = (icono, titulo, texto, estado, insignia, ayuda) => `<li class="sala-e-fila ${estado}"${ayuda ? ` title="${esc(ayuda)}"` : ''}><span class="sala-e-ico" aria-hidden="true"><svg class="icono" viewBox="0 0 24 24">${ICONOS[icono]}</svg></span><span class="sala-e-q"><b>${esc(titulo)}</b><small>${esc(texto)}</small></span>${insignia ? `<span class="sala-insignia ${estado}">${esc(insignia)}</span>` : ''}</li>`;
  const hace = (s) => { const d = Math.max(0, Math.round(Date.now() / 1000 - s)); return d < 60 ? t('esc_hace_s', { n: d }) : t('esc_hace_min', { n: Math.floor(d / 60) }); };
  const pintaEscudo = (m) => {
    const caja = $('sala-escudo');
    const l = m.llamada;
    const activa = !!l && l.conectada !== false && !!m.escudo;
    caja.hidden = !activa;
    if (!activa) return;
    const e = m.escudo, filas = [];
    const c = e.cifrado || {};
    if (c.olm && c.cuantico) filas.push(fila('cifrado', t('sala_e_cifrado'), t('sala_e_cifrado_pq', { hace: c.renovado ? hace(c.renovado) : '—' }), 'ok', t('sala_e_activo')));
    else if (c.olm) filas.push(fila('cifrado', t('sala_e_cifrado'), t('sala_e_cifrado_e2e'), 'ok', t('sala_e_activo')));
    else filas.push(fila('cifrado', t('sala_e_cifrado'), t('sala_e_cifrado_abriendo'), 'aviso', ''));
    const id = e.identidad || {};
    filas.push(id.ok
      ? fila('identidad', t('sala_e_identidad'), t('sala_e_identidad_ok', { f: fechaCorta(id.desde) }), 'ok', t('sala_e_verificada'))
      : fila('identidad', t('sala_e_identidad'), t('sala_e_identidad_no'), 'aviso', t('sala_e_sin_verificar'), t('sala_identidad_no_t')));
    const v = e.vigia || {}, uso = (r) => (v.en_uso || []).filter((x) => x.recurso === r).map((x) => x.programa);
    if (!v.activa) {
      filas.push(fila('mic', t('sala_e_mic'), t('sala_e_no_comprobable'), 'aviso', t('sala_e_sin_comprobar'), t('sala_e_no_comprobable_t')));
      filas.push(fila('cam', t('sala_e_cam'), t('sala_e_no_comprobable'), 'aviso', t('sala_e_sin_comprobar'), t('sala_e_no_comprobable_t')));
    } else {
      const mic = uso('microfono');
      filas.push(mic.length ? fila('mic', t('sala_e_mic'), t('sala_e_mic_uso', { p: mic.join(', ') }), 'alerta', t('sala_e_en_uso')) : fila('mic', t('sala_e_mic'), t('sala_e_mic_ok'), 'ok', t('sala_e_correcto')));
      const cam = [...uso('camara'), ...uso('pantalla')];
      filas.push(cam.length ? fila('cam', t('sala_e_cam'), t('sala_e_cam_uso', { p: cam.join(', ') }), 'alerta', t('sala_e_en_uso')) : fila('cam', t('sala_e_cam'), t(v.pantalla_protegida ? 'sala_e_cam_ok_prot' : 'sala_e_cam_ok'), 'ok', t('sala_e_sin_uso')));
    }
    // The connection: the route the call takes and its round trip, from the call's own statistics.
    const rele = medidas ? medidas.rele : !!(e.ruta || {}).rele;
    const ciudad = medidas && medidas.ciudad ? medidas.ciudad : (e.ruta || {}).ciudad || '';
    const rtt = calidad && calidad.rtt_ms != null ? calidad.rtt_ms : medidas ? medidas.rtt : null;
    const ruta = rele ? t('sala_e_cal_rele', { c: ciudad ? ' · ' + ciudad : '' }) : t('sala_e_cal_directa');
    const texto = rtt != null ? ruta + ' · ' + t('sala_e_cal_ms', { ms: n(rtt) }) : ruta;
    if (calidad) filas.push(fila('red', t('sala_e_calidad'), texto, calidad.nivel === 'debil' ? 'aviso' : 'ok', t('sala_e_' + calidad.nivel), t('sala_e_calidad_t')));
    else filas.push(fila('red', t('sala_e_calidad'), texto, '', t('sala_e_sin_medida')));
    // The code to move this call to the phone (one use, a minute).
    const mv = m.movil || {};
    let qr = '';
    const varios = ((m.grupo || {}).nombres || []).length > 2;
    if (!varios) {
      if (mv.habla) qr = `<p class="sala-qr-t">${esc(t('sala_e_movil_habla'))}</p>`;
      else if (mv.pendiente) qr = `<p class="sala-qr-t">${esc(t('sec_mv_e_pendiente'))}</p>`;
      else if (mv.qr && !mv.aprobado) qr = `<div class="sala-qr-caja">${mv.qr}</div><p class="sala-qr-t">${esc(t('sala_e_movil_texto'))}</p>`;
      else qr = `<div class="sala-qr-caja vacia"><button type="button" class="boton" id="sala-qr-crear">${esc(t('sala_e_movil_crear'))}</button></div><p class="sala-qr-t">${esc(t('sala_e_movil_crear_t'))}</p>`;
    }
    const caduca = typeof mv.qr_caduca_s === 'number' && mv.qr && !mv.aprobado ? Math.max(0, mv.qr_caduca_s - Math.floor((Date.now() - recibidoEn) / 1000)) : null;
    const html = `<div class="sala-e-cab"><h2>${esc(t('esc_titulo'))}</h2><span class="sala-estado ok"><i></i><span>${esc(t('sala_e_comprobando'))}</span></span></div>`
      + `<ul class="sala-e-lista">${filas.join('')}</ul>`
      + (qr ? `<section class="sala-qr"><div class="sala-qr-cab"><b>${esc(t('sala_e_movil'))}</b>${caduca != null ? `<span>${esc(t('sala_e_caduca', { s: n(caduca) }))}</span>` : ''}</div>${qr}</section>` : '');
    if (caja.dataset.html !== html) {
      caja.innerHTML = html; caja.dataset.html = html;
      const b = $('sala-qr-crear');
      if (b) b.addEventListener('click', () => manda({ tipo: 'secure', accion: 'movil', idioma: lang().slice(0, 2) }));
    }
  };

  // ---------- C2-c: «Resumen de seguridad» ----------
  let INF = null;
  const kv = (k, v, clase) => `<div class="sala-kv"><dt>${esc(k)}</dt><dd class="${clase || ''}">${esc(v)}</dd></div>`;
  const seccion = (titulo, filas) => `<section class="sala-in-sec"><h3>${esc(titulo)}</h3><dl>${filas.join('')}</dl></section>`;
  const huella = (h) => { const x = String(h || ''); return /^[0-9a-f]{24,}$/i.test(x) ? 'sha256 · ' + x.slice(0, 24).match(/.{8}/g).join(' ') : x.slice(0, 40); };
  const pintaResumen = (r) => {
    INF = r;
    if (!r) return;
    const fin = r.fin || Math.floor(Date.now() / 1000);
    const d = new Date(fin * 1000);
    const dia = d.toLocaleDateString(lang(), { weekday: 'long', day: 'numeric', month: 'long' });
    $('sala-in-titulo').textContent = t(r.sala ? 'sala_in_titulo_casa' : 'sala_in_titulo');
    $('in-que').textContent = t('sala_in_cuando', { f: dia.charAt(0).toUpperCase() + dia.slice(1), h: d.toLocaleTimeString(lang(), { hour: '2-digit', minute: '2-digit' }), d: reloj(r.duracion_s) });
    const v = r.veredicto || {}, este = v.este || {}, c = r.conexion || {};
    const directa = c.directa === true, sinDato = c.directa == null;
    const seg = [
      kv(t('sala_e_cifrado'), t('sala_in_e2e'), 'ok'),
      kv(t('sala_e_identidad'), t(r.sala ? 'sec_in_identidad_sala' : v.identidad_verificada ? 'sala_in_id_ok' : 'sala_in_id_no'), v.identidad_verificada ? 'ok' : 'alerta'),
      kv(t('sala_in_servidores'), sinDato ? '—' : t(directa ? 'sala_in_ninguno' : 'sala_in_rele'), ''),
    ];
    let calidadTxt = t('sala_in_sin_medida');
    if (c.latencia_media_ms != null) {
      const per = c.perdida_max_pct != null ? c.perdida_max_pct : 0;
      const O = window.GuardianaOpus;
      const palabra = O ? t('sala_e_' + O.nivel(O.mos(c.latencia_media_ms / 2 + 20, per), 0)) : '';
      calidadTxt = t('sala_in_calidad_v', { q: palabra, ms: n(c.latencia_media_ms), p: coma(per, 1) }).replace(/^ · /, '');
    }
    const con = [kv(t('sala_in_ruta'), sinDato ? '—' : t(directa ? 'sala_in_ruta_directa' : 'sala_in_ruta_rele'), ''), kv(t('sala_in_calidad'), calidadTxt, '')];
    const mic = este.microfono || [], cam = [...(este.camara || []), ...(este.pantalla || [])];
    const sen = !este.vigilado
      ? [kv(t('sala_in_mic'), t('sala_e_no_comprobable'), 'aviso'), kv(t('sala_e_cam'), t('sala_e_no_comprobable'), 'aviso')]
      : [kv(t('sala_in_mic'), mic.length ? t('sala_e_mic_uso', { p: mic.join(', ') }) : t('sala_in_mic_ok'), mic.length ? 'alerta' : 'ok'),
        kv(t('sala_e_cam'), cam.length ? t('sala_e_cam_uso', { p: cam.join(', ') }) : t('sala_in_cam_ok'), cam.length ? 'alerta' : 'ok')];
    const integ = r.firmante
      ? `<section class="sala-in-sec integridad"><h3>${esc(t('sala_in_integridad'))}</h3><dl>`
        + `<div class="sala-kv mono"><dt>${esc(t('sala_in_huella'))}</dt><dd>${esc(huella(r.hash_cadena))}</dd></div>`
        + `<div class="sala-kv mono"><dt>${esc(t('sala_in_firma'))}</dt><dd>${esc(t('sala_in_firma_v'))}</dd>${r.firma_ok === true ? `<span class="sala-insignia ok">${esc(t('sala_in_firma_ok'))}</span>` : r.firma_ok === false ? `<span class="sala-insignia alerta">${esc(t('sala_in_firma_no'))}</span>` : ''}</div>`
        + `</dl></section>`
      : '';
    $('sala-in-secciones').innerHTML = seccion(t('sala_in_seguridad'), seg) + seccion(t('sala_in_conexion'), con) + seccion(t('sala_in_sensores'), sen) + integ;
  };
  // After secure.js has drawn its part of the same report (it dispatches this in the middle).
  window.addEventListener('guardiana-informe', (e) => queueMicrotask(() => {
    const nuevo = !INF && e.detail.r;
    pintaResumen(e.detail.r);
    if (nuevo) { $('sala-in-detalles').hidden = true; $('sala-in-ver').setAttribute('aria-expanded', 'false'); $('sala-in-ver').textContent = t('sala_in_detalles'); }
  }));
  $('sala-in-ver').addEventListener('click', () => {
    const abrir = $('sala-in-detalles').hidden;
    $('sala-in-detalles').hidden = !abrir;
    $('sala-in-ver').setAttribute('aria-expanded', String(abrir));
    $('sala-in-ver').textContent = t(abrir ? 'sala_in_ocultar' : 'sala_in_detalles');
  });
  // «Guardar informe»: the PDF to read and the signed JSON that can be checked, together.
  $('in-pdf').addEventListener('click', () => { setTimeout(() => $('in-json').click(), 300); });

  // ---------- Everything, on every state ----------
  // This file loads before secure.js (so no state is missed) and paints after it: a microtask
  // runs once every handler of the same message has run.
  const despues = (f) => (m) => queueMicrotask(() => { try { f(m); } catch (e) { console.error(e); } });
  en('secure_estado', despues((m) => {
    E = m; recibidoEn = Date.now();
    pintaCabecera(m); pintaContactos(m); pintaInvitaciones(m); pintaNueva(m); pintaChip(); pintaLlamada(m); pintaEscudo(m);
    // The inspector closes by itself when there is nothing to inspect.
    if (!hayContacto(m)) inspector(false);
  }));
  en('monitor', despues(() => pintaChip()));
  en('textos', despues(() => { if (E && E.fase) { $('sala-insignias').dataset.html = ''; $('sala-contactos').dataset.html = ''; $('sala-invitaciones').dataset.html = ''; pintaCabecera(E); pintaContactos(E); pintaInvitaciones(E); pintaChip(); } if (INF) pintaResumen(INF); }));
  setInterval(() => { if (E.llamada) { pintaChip(); pintaEscudo(E); } }, 1000);
})();
