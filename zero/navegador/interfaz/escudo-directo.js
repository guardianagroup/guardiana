// «Escudo en directo» (18 Oct 2026, the owner: «somos como un escudo… sentir el monitoreo
// constante»). Every line is a check the program really made, drawn from `secure_estado.escudo`
// (crates/zero/src/secure.rs, `escudo_json`) with the second it was last true; nothing here is
// estimated or animated for show. A line without a measure says so. Green only for what is
// verified; amber when something needs a look; red only for a confirmed problem.
// Also here: the calm notice when a conversation was destroyed for an anomaly, the switch for
// that destruction and the disappearing-messages timer, with the honest limit of both.
'use strict';
(() => {
  const { manda, en, t, esc, $ } = Z;
  const caja = $('escudo-directo');
  if (!caja) return;
  const RAYA = '—';
  const hora = (s) => (s ? new Date(s * 1000).toLocaleTimeString(document.documentElement.lang || 'es', { hour: '2-digit', minute: '2-digit', second: '2-digit' }) : RAYA);
  const hace = (s) => {
    if (!s) return RAYA;
    const d = Math.max(0, Math.round(Date.now() / 1000 - s));
    return d < 60 ? t('esc_hace_s', { n: d }) : t('esc_hace_min', { n: Math.floor(d / 60) });
  };
  // Where GUARDIANA's relays are (crates/zero/src/rele.rs): the name, the city, the country.
  const RELES = [['rele-am.guardianagroup.com', 'esc_rele_miami'], ['rele.guardianagroup.com', 'esc_rele_nuremberg']];
  const dondeRele = (n) => { const r = RELES.find(([h]) => String(n || '').startsWith(h)); return r ? t(r[1]) : ''; };
  let ultimo = null, calidad = null;
  // The voice call's quality, from the call's own getStats (opus.js): a word and a dot.
  window.addEventListener('guardiana-calidad', (e) => { calidad = e.detail; if (ultimo) pinta(ultimo); });

  const fila = (estado, titulo, texto, cuando) => `<li class="ed-fila ${estado}"><i class="ed-punto" aria-hidden="true"></i>`
    + `<div><b>${esc(titulo)}</b><span>${esc(texto)}</span></div><time>${esc(cuando || '')}</time></li>`;

  function pinta(m) {
    ultimo = m;
    // Not while the person is choosing something here (a redraw would close the list).
    if (caja.contains(document.activeElement) && document.activeElement.tagName === 'SELECT') return;
    const e = m.escudo;
    const abierta = e && m.fase && m.fase !== 'sola' && m.fase !== 'escuchando' && m.fase !== 'buscando';
    caja.hidden = !abierta;
    if (!abierta) return;
    const filas = [];
    // 1. Identity.
    filas.push(e.identidad.ok
      ? fila('ok', t('esc_identidad'), t('esc_identidad_ok', { n: e.identidad.contacto || '' }), hora(e.identidad.desde))
      : fila('aviso', t('esc_identidad'), t('esc_identidad_pendiente'), ''));
    // 2. Encryption, as it is right now.
    const c = e.cifrado;
    if (c.olm && c.cuantico) {
      filas.push(fila('ok', t('esc_cifrado'), t('esc_cifrado_pq', { hace: hace(c.renovado), n: c.renueva_cada, min: Math.round(c.renueva_s / 60) }), hora(c.renovado)));
    } else if (c.olm) {
      filas.push(fila('aviso', t('esc_cifrado'), t('esc_cifrado_olm'), ''));
    } else {
      filas.push(fila('aviso', t('esc_cifrado'), t('esc_cifrado_abriendo'), ''));
    }
    // 3. The road.
    const r = e.ruta;
    if (r.rele) filas.push(fila('info', t('esc_ruta'), t('esc_ruta_rele', { donde: dondeRele(r.rele_nombre) || t('esc_rele_guardiana') }), ''));
    else if (r.directa === true) filas.push(fila('info', t('esc_ruta'), t('esc_ruta_directa'), ''));
    else filas.push(fila('', t('esc_ruta'), RAYA, ''));
    // 4. Microphone, camera, screen capture: only what Windows recorded.
    const v = e.vigia;
    if (!v.activa) filas.push(fila('', t('esc_equipo'), t('esc_equipo_sin_medida'), ''));
    else if ((v.en_uso || []).length) {
      const quien = v.en_uso.map((x) => `${x.programa} (${t('esc_recurso_' + x.recurso)})`).join(', ');
      filas.push(fila('alerta', t('esc_equipo'), t('esc_equipo_uso', { quien }), hora(v.mirado)));
    } else filas.push(fila('ok', t('esc_equipo'), t('esc_equipo_nadie'), hora(v.mirado)));
    // 5. Integrity: what opened and what did not.
    const g = e.integridad;
    const mal = (g.mensajes_mal || 0) + (g.trozos_mal || 0);
    filas.push(fila(mal ? 'alerta' : 'ok', t('esc_integridad'), t('esc_integridad_texto', { m: g.mensajes_ok || 0, b: g.trozos_ok || 0, mal }), ''));
    // 6. Devices in the session.
    const d = e.dispositivos || [];
    const nombres = d.map((x) => t('esc_disp_' + x)).join(' · ');
    filas.push(fila(d.length === 2 ? 'ok' : 'info', t('esc_dispositivos', { n: d.length }), nombres, ''));
    if (m.otro_movil) filas.push(fila('info', t('esc_movil_otro_t'), t('esc_movil_otro', { n: m.contacto || '' }), ''));
    // 7. The call's quality, when there is one.
    if (m.llamada && calidad) filas.push(fila(calidad.nivel === 'debil' ? 'aviso' : 'ok', t('esc_calidad'), t('esc_calidad_' + calidad.nivel), ''));
    const temporizadores = m.temporizadores || [0];
    caja.innerHTML = `<div class="ed-cab"><span class="rotulo">${esc(t('esc_titulo'))}</span><span class="ed-vivo">${esc(t('esc_vivo'))}</span></div>`
      + `<ul class="ed-lista">${filas.join('')}</ul>`
      + `<div class="ed-ajustes">`
      + `<label class="ed-ajuste"><input type="checkbox" id="ed-auto" ${e.auto_destruccion ? 'checked' : ''}><span>${esc(t('esc_auto'))}</span></label>`
      + `<label class="ed-ajuste"><span>${esc(t('esc_temporizador'))}</span><select class="campo" id="ed-temp">${temporizadores.map((s) => `<option value="${s}" ${s === m.temporizador ? 'selected' : ''}>${esc(t('esc_temp_' + s))}</option>`).join('')}</select></label>`
      + `<p class="ed-limite">${esc(t('esc_limite'))}</p></div>`;
    $('ed-auto').addEventListener('change', (x) => {
      try { localStorage.setItem('secure_auto_destruccion', x.target.checked ? '1' : '0'); } catch (_) {}
      manda({ tipo: 'secure', accion: 'auto_destruccion', si: x.target.checked });
    });
    $('ed-temp').addEventListener('change', (x) => manda({ tipo: 'secure', accion: 'temporizador', s: Number(x.target.value) || 0 }));
  }

  // The notice: calm, the reason in plain words, and what could not be undone.
  let aviso = null;
  function pintaDestruida(d) {
    if (!d) { if (aviso) { aviso.remove(); aviso = null; } return; }
    if (!aviso) {
      aviso = document.createElement('div');
      aviso.className = 'ed-destruida';
      aviso.setAttribute('role', 'alertdialog');
      document.body.appendChild(aviso);
    }
    const motivo = d.otro_lado ? t('esc_d_otro_lado', { n: d.contacto || '' }) : t('esc_d_' + d.motivo, { p: d.programa || '' });
    aviso.innerHTML = `<section class="tarjeta"><h2>${esc(t('esc_d_titulo', { motivo }))}</h2>`
      + `<p>${esc(t('esc_d_hecho'))}</p><p class="ed-limite">${esc(t('esc_limite'))}</p>`
      + `<p class="ed-cuando">${esc(hora(d.cuando))}</p>`
      + `<div class="c-fila"><button type="button" class="boton principal" id="ed-entendido">${esc(t('esc_d_entendido'))}</button></div></section>`;
    $('ed-entendido').addEventListener('click', () => manda({ tipo: 'secure', accion: 'destruida_vista' }));
  }

  en('secure_estado', (m) => { pinta(m); pintaDestruida(m.destruida); });
  // Redraw every 15 s so «hace 3 min» stays true without a new state.
  setInterval(() => { if (ultimo && !caja.hidden) pinta(ultimo); }, 15000);
  // The person's choice about destruction survives a reload of the page.
  en('textos', () => {
    let si = true; try { si = localStorage.getItem('secure_auto_destruccion') !== '0'; } catch (_) {}
    if (!si) manda({ tipo: 'secure', accion: 'auto_destruccion', si: false });
  });
})();
