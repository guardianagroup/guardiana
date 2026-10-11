// The side panel: the shield in real time, the mandate, the redaction, my data and settings.
'use strict';
(() => {
  const { manda, en, t, tn, esc, $, n } = Z;
  let vista = null;
  let pais = (c) => c || '';
  const fecha = (ms) => new Date(ms).toLocaleDateString(document.documentElement.lang, { day: 'numeric', month: 'long', year: 'numeric' });
  const hora = (ms) => new Date(ms).toLocaleTimeString(document.documentElement.lang, { hour: '2-digit', minute: '2-digit', second: '2-digit' });

  function muestra(v) {
    vista = v;
    document.querySelectorAll('section.vista').forEach((s) => s.classList.toggle('vista-activa', s.id === 'v-' + v));
    const s = $('v-' + v);
    $('titulo').textContent = s ? t(s.dataset.titulo) : '';
  }
  $('cerrar').addEventListener('click', () => manda({ tipo: 'panel', vista: null }));

  // --- shield -------------------------------------------------------------------------------
  const COLOR_CAT = { rastreador: 'rojo', publicidad: 'ambar', telemetria: 'ambar', esperado: 'verde', desconocido: '' };
  // One row per company, in the order it first showed up on this page, and it never moves: not
  // when the person presses a button, not when requests arrive (the owner, 10 Oct 2026: rows
  // jumping between lists lost the person's place). Each row is drawn once and then updated in
  // place, so its buttons, its height and the focus stay where they are; only the colour bar,
  // the chip and the button's words change. The order starts again only with a new page.
  const filas = new Map();
  let cargaFilas = null;
  let pendiente = null;
  function nuevaFila(sitio) {
    const li = document.createElement('li');
    li.className = 'tercero';
    li.dataset.sitio = sitio;
    li.innerHTML = '<span class="quien"></span><span class="botones"><button class="boton chico" type="button"></button><button class="boton chico" type="button"></button></span>'
      + '<span class="linea"></span><span class="chips"></span><span class="nota-dato oculto"></span>';
    li.querySelectorAll('button').forEach((b) => b.addEventListener('click', () => pulsa(li, b)));
    return li;
  }
  // The button answers at once (it waits, marked, until the program says what holds now), and
  // the row is lit for a moment once it changed. The view is never scrolled.
  function pulsa(li, b) {
    if (b.classList.contains('vacio') || b.getAttribute('aria-busy') === 'true') return;
    const x = filas.get(li.dataset.sitio);
    if (!x) return;
    pendiente = { sitio: x.datos.sitio, antes: clave(x.datos), hasta: Date.now() + 3000 };
    b.setAttribute('aria-busy', 'true');
    li.classList.add('enviando');
    manda({ tipo: b.dataset.accion, sitio: x.datos.sitio });
    $('e-recarga').classList.remove('oculto');
    // If no answer changes it, the button is free again (never stuck waiting).
    setTimeout(() => {
      if (pendiente && pendiente.sitio === x.datos.sitio && Date.now() > pendiente.hasta) {
        pendiente = null;
        li.classList.remove('enviando');
        li.querySelectorAll('button').forEach((y) => y.removeAttribute('aria-busy'));
      }
    }, 3100);
  }
  const clave = (x) => x.ahora + '|' + (x.regla || '');
  function botonesDe(x, corta, parcial) {
    // Every button has its way back: «Volver a bloquear» on what the person unblocked (even if
    // something of it is still cut: their marked data never passes by a rule), «Desbloquear»
    // on what a rule or the lists cut, «Bloquear» on what passes; a row cut only in part also
    // offers «Bloquear todo». What the task's limits cut, or a row cut only for carrying marked
    // data, has no «Desbloquear»: it would do nothing.
    const b = [];
    if (x.fijo) return b;
    if (x.regla === 'permitido') b.push(['bloquear_sitio', 'accion_rebloquear', 'corta']);
    else if (corta) {
      if (x.deshace) b.push(['desbloquear_sitio', 'accion_desbloquear', 'deja']);
      if (parcial) b.push(['bloquear_todo', 'accion_bloquear_todo', 'corta']);
    } else b.push(['bloquear_sitio', 'accion_bloquear', 'corta']);
    return b;
  }
  function pintaFila(li, x) {
    const corta = x.ahora === 'cortado' || x.ahora === 'parcial';
    const parcial = x.ahora === 'parcial';
    const sigue = !corta && (x.categoria === 'rastreador' || x.categoria === 'publicidad' || x.corredor);
    for (const c of ['cortado', 'parcial', 'sigue']) li.classList.remove(c);
    if (parcial) li.classList.add('parcial');
    else if (corta) li.classList.add('cortado');
    else if (sigue) li.classList.add('sigue');
    li.querySelector('.quien').innerHTML = `${esc(x.quien)}${x.pais ? `<small>${esc(pais(x.pais))}</small>` : ''}`;
    // The site may be shortened; its counts never are.
    li.querySelector('.linea').innerHTML = `<span class="mono">${esc(x.sitio)}</span><span class="cuenta">${esc(x.cortadas > 0 ? t('escudo_cortadas_de', { c: n(x.cortadas), n: n(x.vistas) }) : tn('escudo_peticiones', x.vistas, { n: n(x.vistas) }))}</span>`;
    // What holds now, in one chip: the person's rule when there is one, else cut, partly or not.
    const [estado, color] = x.regla === 'tuya' || x.regla === 'permitido' ? [t('regla_' + x.regla), 'tuya']
      : parcial ? [t('escudo_parcial'), 'ambar'] : corta ? [t('escudo_estado_cortado'), 'rojo'] : [t('escudo_estado_pasa'), ''];
    // Why it is cut now: printed only when the chips do not say it already.
    const por = corta ? (x.por || x.motivo) : null;
    li.querySelector('.chips').innerHTML = [
      `<span class="chip estado ${color}">${esc(estado)}</span>`,
      `<span class="chip ${COLOR_CAT[x.categoria] || ''}">${esc(t('cat_' + x.categoria))}</span>`,
      x.corredor ? `<span class="chip rojo" title="${esc(x.corredor)}">${esc(t('corredor_etiqueta'))}</span>` : '',
      por && !['rastreador', 'publicidad', 'corredor', 'corte_tuyo', 'telemetria'].includes(por) ? `<span class="motivo">${esc(t('motivo_' + por))}</span>` : '',
    ].join('');
    // Marked data is cut whatever the buttons say: how it goes out, said where it was cut.
    const nota = li.querySelector('.nota-dato');
    nota.classList.toggle('oculto', !x.dato);
    nota.textContent = x.dato ? t('escudo_dato_cortado') : '';
    // Two places for buttons in every row, always: the column keeps its width and the row its
    // height whatever the words, and the same button stays under the finger.
    const lista = botonesDe(x, corta, parcial);
    const huecos = [...li.querySelectorAll('.botones button')];
    // Taken before anything changes: a button about to be hidden loses the focus at once.
    const enFoco = huecos.indexOf(document.activeElement);
    const cambio = pendiente && pendiente.sitio === x.sitio && (clave(x) !== pendiente.antes || Date.now() > pendiente.hasta);
    huecos.forEach((b, i) => {
      const d = lista[i];
      b.classList.remove('corta', 'deja', 'vacio');
      if (d) {
        const [accion, k, clase] = d;
        b.dataset.accion = accion;
        b.textContent = t(k);
        b.title = t(k + '_titulo');
        b.classList.add(clase);
        b.tabIndex = 0;
        b.removeAttribute('aria-hidden');
      } else {
        b.dataset.accion = '';
        b.textContent = '';
        b.title = '';
        b.classList.add('vacio');
        b.tabIndex = -1;
        b.setAttribute('aria-hidden', 'true');
      }
      if (cambio) b.removeAttribute('aria-busy');
    });
    if (cambio) {
      pendiente = null;
      li.classList.remove('enviando');
      li.classList.add('recien');
      setTimeout(() => li.classList.remove('recien'), 1200);
    }
    // A button that had the focus and is no longer there (the row's second one): the focus goes
    // to the row's first, never away from the row.
    if (enFoco > 0 && huecos[enFoco].classList.contains('vacio') && !huecos[0].classList.contains('vacio')) huecos[0].focus({ preventScroll: true });
  }
  function pintaFilas(m) {
    const lista = m.terceros || [];
    const ul = $('e-terceros');
    const carga = m.pestana + ':' + (m.carga || 0);
    if (carga !== cargaFilas) {
      cargaFilas = carga;
      filas.clear();
      pendiente = null;
      ul.replaceChildren();
    }
    for (const x of lista) {
      let f = filas.get(x.sitio);
      if (!f) {
        // New companies go at the end: nothing above them moves.
        f = { li: nuevaFila(x.sitio) };
        filas.set(x.sitio, f);
        ul.appendChild(f.li);
      }
      f.datos = x;
      pintaFila(f.li, x);
    }
    // Now: how many companies this page has, and to how many something is cut at this moment
    // (a figure of now, said as such: it follows the buttons, unlike the counts of what happened).
    const empresas = new Set(lista.map((x) => x.quien));
    const cortadas = new Set(lista.filter((x) => x.ahora !== 'pasa').map((x) => x.quien));
    $('e-lista-resumen').textContent = `${tn('escudo_lista_empresas', empresas.size, { n: n(empresas.size) })} · ${tn('escudo_lista_cortadas', cortadas.size, { n: n(cortadas.size) })}`;
    $('e-lista-caja').classList.toggle('oculto', !lista.length);
    $('e-vacio').classList.toggle('oculto', lista.length > 0);
  }
  let sitioEscudo = null;
  // Today, across the whole browser, at the top of the shield: refreshed while it is open.
  en('hoy', (m) => {
    const h = m.hoy || {};
    $('e-dia').classList.toggle('apagada', !m.cortando);
    $('e-dia-frase').innerHTML = h.empresas
      ? `${esc(tn('escudo_dia_frase', h.empresas, { empresas: n(h.empresas) }))} <b>${esc(tn('escudo_dia_cortadas', h.empresas_cortadas, { cortadas: n(h.empresas_cortadas) }))}</b>`
      : esc(t('escudo_dia_cero'));
    $('e-dia-barra').style.width = h.empresas ? `${Math.round((100 * (h.empresas_cortadas || 0)) / h.empresas)}%` : '0';
    $('e-dia-cortadas').textContent = n(h.cortadas);
    $('e-dia-datos').textContent = n(h.datos_salvados);
    $('e-dia-parametros').textContent = n(h.parametros_quitados);
    $('e-dia-corredores').textContent = n(h.corredores);
    $('e-rastro-caja').classList.toggle('oculto', !Z.rastro($('e-rastro'), { ...(m.rastro_hoy || {}), seguidores: ((m.rastro_hoy || {}).seguidores || []).slice(0, 3) }));
  });
  $('e-dia-ver').addEventListener('click', () => manda({ tipo: 'abrir_cortes' }));
  en('escudo', (m) => {
    // On the browser's own pages there is no page to speak of: only the day.
    $('e-pagina-cab').classList.toggle('oculto', !m.sitio);
    if (m.sitio !== sitioEscudo) { sitioEscudo = m.sitio; $('e-recarga').classList.add('oculto'); }
    $('e-sitio').textContent = m.sitio || '';
    const r = m.resumen || {};
    $('e-resumen').textContent = r.empresas ? `${tn('escudo_resumen', r.empresas, { empresas: n(r.empresas) })} ${tn('escudo_resumen_cortadas', r.cortadas, { cortadas: n(r.cortadas) })}` : '';
    $('e-proteccion').checked = !!m.cortando;
    $('e-maxima').classList.toggle('encendida', !!m.maxima);
    $('e-maxima').classList.toggle('oculto', !m.protege);
    $('e-maxima-activa').classList.toggle('oculto', !m.maxima);
    pintaFilas(m);
    const hechos = [];
    if (m.parametros_quitados) hechos.push(tn('parametros_quitados', m.parametros_quitados, { n: n(m.parametros_quitados) }));
    if (m.datos_salvados) hechos.push(tn('datos_salvados', m.datos_salvados, { n: n(m.datos_salvados) }));
    if (m.cookies) hechos.push(t(m.cookies.accion === 'rechazado' ? 'cookies_rechazado' : 'cookies_escondido', { gestor: m.cookies.gestor }));
    $('e-hechos').innerHTML = hechos.map((h) => `<span>${esc(h)}</span>`).join('');
    pintaChivatos(m);
  });

  // «Los chivatos»: what this page's pixels tried to tell an advertising network about the
  // person, one line each, only what the request said; and, when something was cut, a card to
  // share without amounts, products or email.
  const lang = () => document.documentElement.lang || 'es';
  // An amount as it travelled, written the person's way when it is a plain number.
  function importe(v) {
    const s = String(v);
    if (!/^-?\d+(\.\d+)?$/.test(s)) return s;
    const dec = (s.split('.')[1] || '').length;
    return Number(s).toLocaleString(lang(), { minimumFractionDigits: dec, maximumFractionDigits: dec });
  }
  function dato(tipo, d) {
    if (!d) return '';
    return t(`chivato_${tipo}_${d.tuyo ? 'tuyo' : 'otro'}${d.cifrado ? '' : '_claro'}`);
  }
  function lineaChivato(c) {
    const red = t('chivato_red_' + c.red);
    const que = c.evento === 'otro' ? t('chivato_ev_otro', { nombre: c.nombre }) : t('chivato_ev_' + c.evento);
    const partes = [c.servidor ? t('chivato_a_servidor', { sitio: c.servidor, red, que }) : t('chivato_a', { red, que })];
    if (c.importe) partes.push(importe(c.importe) + (c.moneda ? ' ' + c.moneda : ''));
    const correo = dato('correo', c.correo);
    if (correo) partes.push(correo);
    const tel = dato('telefono', c.telefono);
    if (tel) partes.push(tel);
    if (c.veces > 1) partes.push(t('chivato_veces', { n: n(c.veces) }));
    return partes.join(' · ');
  }
  let fraseTarjeta = '';
  function fraseDe(m) {
    const k = m.tarjeta;
    if (!k || !m.sitio) return '';
    return tn('chivatos_tarjeta', k.empresas, { web: m.sitio, n: n(k.empresas), que: t('chivatos_yo_' + k.evento), correo: k.correo ? t('chivatos_tarjeta_correo') : '' });
  }
  // The card as an image, the same look as «your month in data»: only the sentence.
  async function dibujaTarjeta(frase) {
    const c = $('e-tarjeta-lienzo');
    const x = c.getContext('2d');
    try { await document.fonts.ready; } catch (_) {}
    if (frase !== fraseTarjeta) return;
    x.clearRect(0, 0, 1200, 630);
    x.fillStyle = '#F4F6FA'; x.fillRect(0, 0, 1200, 630);
    x.strokeStyle = 'rgba(11,16,32,.05)';
    for (let i = 0; i < 1200; i += 44) { x.beginPath(); x.moveTo(i, 0); x.lineTo(i, 630); x.stroke(); }
    for (let j = 0; j < 630; j += 44) { x.beginPath(); x.moveTo(0, j); x.lineTo(1200, j); x.stroke(); }
    x.fillStyle = '#C0301A'; x.beginPath(); x.arc(84, 84, 9, 0, Math.PI * 2); x.fill();
    x.fillStyle = '#0B1020'; x.font = '600 24px Unbounded'; x.fillText('GUARDIANA ZERO', 108, 93);
    x.fillStyle = '#5B6275'; x.font = '500 22px Plex'; x.fillText(t('chivatos_titulo'), 72, 170);
    x.fillStyle = '#0B1020'; x.font = '600 40px Unbounded';
    let linea = ''; let y = 240;
    for (const p of frase.split(' ')) {
      const prueba = linea ? linea + ' ' + p : p;
      if (x.measureText(prueba).width > 1056 && linea) { x.fillText(linea, 72, y); y += 54; linea = p; } else linea = prueba;
    }
    x.fillText(linea, 72, y);
    x.fillStyle = '#5B6275'; x.font = '400 18px Plex'; x.fillText(t('lema'), 72, 540);
    x.fillStyle = '#1F4BFF'; x.font = '500 20px PlexMono'; x.fillText('guardianagroup.com', 72, 570);
  }
  function pintaChivatos(m) {
    const lista = m.chivatos || [];
    $('e-chivatos-caja').classList.toggle('oculto', !lista.length);
    const paso = lista.some((c) => !c.cortado);
    $('e-chivatos-titulo').textContent = t(paso ? 'chivatos_titulo_paso' : 'chivatos_titulo');
    $('e-chivatos-nota').textContent = t(paso ? 'chivatos_nota_paso' : 'chivatos_nota');
    $('e-chivatos').innerHTML = lista.map((c) => `<li class="${c.cortado ? 'cortado' : 'paso'}" title="${esc(c.nombre)}"><span>${esc(lineaChivato(c))}</span><span class="estado">— ${esc(t(c.cortado ? 'chivato_cortado' : 'chivato_paso'))}</span></li>`).join('');
    const frase = fraseDe(m);
    $('e-tarjeta').classList.toggle('oculto', !frase);
    if (frase !== fraseTarjeta) {
      fraseTarjeta = frase;
      $('e-tarjeta-frase').textContent = frase;
      $('e-tarjeta-guardada').textContent = '';
      if (frase) dibujaTarjeta(frase);
    }
  }
  $('e-tarjeta-guardar').addEventListener('click', async () => {
    if (!fraseTarjeta) return;
    await dibujaTarjeta(fraseTarjeta);
    manda({ tipo: 'guardar_imagen', que: 'chivatos', datos: $('e-tarjeta-lienzo').toDataURL('image/png') });
  });
  $('e-tarjeta-copiar').addEventListener('click', () => {
    if (!fraseTarjeta) return;
    navigator.clipboard.writeText(fraseTarjeta).then(() => { $('e-tarjeta-copiar').textContent = t('copiado'); setTimeout(() => { $('e-tarjeta-copiar').textContent = t('chivatos_copiar'); }, 1500); });
  });
  $('e-recargar').addEventListener('click', () => { manda({ tipo: 'recargar' }); $('e-recarga').classList.add('oculto'); });
  $('e-ver-todo').addEventListener('click', () => manda({ tipo: 'abrir_cortes' }));
  $('e-proteccion').addEventListener('change', (e) => manda({ tipo: 'ajuste', clave: 'cortar_seguimiento', valor: e.target.checked }));
  $('e-maxima-activar').addEventListener('click', () => { manda({ tipo: 'ajuste', clave: 'maxima', valor: true }); $('e-recarga').classList.remove('oculto'); });
  $('e-maxima-quitar').addEventListener('click', () => manda({ tipo: 'ajuste', clave: 'maxima', valor: false }));

  // --- mandate ---------------------------------------------------------------------------------
  let sitioActual = '';
  // Ideas to start from: they fill the task and the websites, and the person changes what they
  // want. The websites are the same everywhere; the task is written in the person's language.
  const IDEAS = {
    vuelo: ['google.com', 'skyscanner.com', 'kayak.com'],
    hotel: ['booking.com', 'airbnb.com', 'google.com'],
    precios: ['amazon.com', 'ebay.com', 'google.com'],
    investigar: ['wikipedia.org', 'google.com', 'britannica.com'],
  };
  // What the websites box will allow, as the person types: the program answers with the exact
  // list the limit will use (each website's registrable site, entries that are not websites left
  // out), and that same list is what starts the mandate.
  const trozos = () => $('m-webs').value.split(/[\s,;]+/).map((w) => w.trim()).filter(Boolean);
  let permitidos = [];
  let previaTimer = 0;
  function vistaWebs() {
    clearTimeout(previaTimer);
    previaTimer = setTimeout(() => manda({ tipo: 'mandato_previa', webs: trozos() }), 200);
  }
  en('mandato_previa', (m) => {
    permitidos = m.permitidos || [];
    $('m-webs-vista').innerHTML = permitidos.length
      ? `<span class="pequeno gris">${esc(t('mandato_podra'))}</span>` + permitidos.map((w) => `<span class="chip azul mono">${esc(w)}</span>`).join('')
      : '';
  });
  $('m-webs').addEventListener('input', () => { vistaWebs(); $('m-error').classList.add('oculto'); });
  document.querySelectorAll('#m-ideas .idea').forEach((b) => b.addEventListener('click', () => {
    document.querySelectorAll('#m-ideas .idea').forEach((x) => x.classList.toggle('elegida', x === b));
    const idea = b.dataset.idea;
    if (idea === 'esta') {
      $('m-tarea').value = t('idea_esta_tarea', { sitio: sitioActual });
      $('m-webs').value = sitioActual;
    } else {
      $('m-tarea').value = t('idea_' + idea + '_tarea');
      $('m-webs').value = IDEAS[idea].join(', ');
    }
    vistaWebs();
    $('m-tarea').focus();
  }));
  $('m-empezar').addEventListener('click', () => {
    const webs = trozos();
    $('m-error').classList.toggle('oculto', webs.length > 0);
    if (!webs.length) return;
    manda({ tipo: 'mandato_empezar', tarea: $('m-tarea').value, webs, estricto: $('m-estricto').checked, minutos: Number($('m-duracion').value) });
  });
  $('m-terminar').addEventListener('click', () => manda({ tipo: 'mandato_terminar' }));
  $('m-copiar').addEventListener('click', () => { navigator.clipboard.writeText($('m-senuelo').textContent).then(() => { $('m-copiar').textContent = t('copiado'); setTimeout(() => { $('m-copiar').textContent = t('copiar'); }, 1500); }); });
  $('r-guardar').addEventListener('click', () => manda({ tipo: 'recibo_guardar' }));
  $('r-nuevo').addEventListener('click', () => manda({ tipo: 'mandato_nuevo' }));
  en('mandato', (m) => {
    sitioActual = m.sitio_actual || '';
    const estado = m.estado || 'ninguno';
    $('m-nuevo').classList.toggle('oculto', estado !== 'ninguno');
    $('m-activo').classList.toggle('oculto', estado !== 'activo');
    $('m-recibo').classList.toggle('oculto', estado !== 'terminado');
    const esta = $('m-idea-esta');
    esta.classList.toggle('oculto', !sitioActual);
    esta.textContent = sitioActual ? t('idea_esta', { sitio: sitioActual }) : '';
    vistaWebs();
    const md = m.mandato;
    const cuentas = m.cuentas ? t('mandato_cuentas', { visitas: n(m.cuentas.visitas), sitios: n(m.cuentas.sitios), cortes: n(m.cuentas.cortes), datos: n(m.cuentas.datos) }) : '';
    if (estado === 'activo' && md) {
      $('m-tarea-txt').textContent = md.tarea || '';
      $('m-sitios').innerHTML = md.permitidos.map((s) => `<span class="chip azul mono">${esc(s)}</span>`).join('');
      $('m-cuentas').textContent = cuentas;
      $('m-senuelo').textContent = md.senuelo;
      $('m-caduca').textContent = md.caduca ? t('mandato_termina', { hora: new Date(md.caduca).toLocaleTimeString(document.documentElement.lang, { hour: '2-digit', minute: '2-digit' }) }) : '';
      const pasos = (md.pasos || []).slice(-40).reverse();
      $('m-pasos-vacio').classList.toggle('oculto', pasos.length > 0);
      $('m-pasos').innerHTML = pasos.map((p) => `<li><time>${esc(hora(p.ts))}</time><span class="que ${esc(p.que)}">${esc(t('mandato_paso_' + p.que))}</span><span class="mono">${esc(p.sitio)}</span>`
        + (p.que === 'cortado' && !md.permitidos.includes(p.sitio) ? `<button class="boton chico deja" data-sitio="${esc(p.sitio)}">${esc(t('mandato_anadir_corto'))}</button>` : '<span></span>') + '</li>').join('');
      $('m-pasos').querySelectorAll('button[data-sitio]').forEach((b) => {
        b.title = t('mandato_anadir_web', { sitio: b.dataset.sitio });
        b.addEventListener('click', () => manda({ tipo: 'mandato_anadir', sitio: b.dataset.sitio }));
      });
    }
    if (estado === 'terminado') {
      $('r-cuentas').textContent = cuentas;
      $('r-json').textContent = m.recibo ? JSON.stringify(m.recibo, null, 1) : '';
      $('r-ruta').textContent = '';
      $('r-huella').textContent = m.huella ? t('recibo_huella', { huella: m.huella }) : '';
    }
  });
  en('guardado', (m) => {
    if (m.que === 'chivatos') $('e-tarjeta-guardada').textContent = m.texto || m.ruta || '';
    else $('r-ruta').textContent = m.texto || m.ruta || '';
  });

  // --- redaction --------------------------------------------------------------------------------
  let tachado = null;
  $('t-tachar').addEventListener('click', () => manda({ tipo: 'tachar', texto: $('t-texto').value }));
  // What the AI will receive appears as the person writes: no button to find first.
  let tacharTimer = 0;
  $('t-texto').addEventListener('input', () => {
    clearTimeout(tacharTimer);
    tacharTimer = setTimeout(() => manda({ tipo: 'tachar', texto: $('t-texto').value }), 350);
  });
  en('tachado', (m) => {
    tachado = m;
    const listo = !!(m.texto && m.texto.trim());
    $('t-paso-2').classList.toggle('apagado', !listo);
    $('t-pegar').disabled = !listo;
    $('t-copiar').disabled = !listo;
    const hay = (m.sustituciones || []).length > 0;
    $('t-resultado').classList.toggle('oculto', !hay && !m.texto);
    $('t-nada').classList.toggle('oculto', hay || !m.texto);
    let html = esc(m.texto || '');
    for (const s of m.sustituciones || []) html = html.split(esc(s.marca)).join(`<mark>${esc(s.marca)}</mark>`);
    $('t-salida').innerHTML = html;
    $('t-tabla').innerHTML = (m.sustituciones || []).map((s) => `<tr><td>${esc(s.marca)}</td><td>${esc(t(s.clave))}</td></tr>`).join('');
  });
  $('t-pegar').addEventListener('click', () => { if (tachado) manda({ tipo: 'tachon_pegar', texto: tachado.texto }); });
  $('t-copiar').addEventListener('click', () => { if (tachado) navigator.clipboard.writeText(tachado.texto).then(() => { $('t-copiar').textContent = t('copiado'); setTimeout(() => { $('t-copiar').textContent = t('copiar'); }, 1500); }); });
  $('t-respuesta').addEventListener('click', () => manda({ tipo: 'tachon_respuesta' }));
  en('restaurado', (m) => {
    const caja = $('t-restaurada');
    caja.classList.remove('oculto');
    caja.textContent = m.texto || t('tachon_seleccion_vacia');
  });

  // --- my data ----------------------------------------------------------------------------------
  const TIPOS = ['correo', 'telefono', 'documento', 'nombre', 'otro'];
  const LEYES = ['co', 'ue', 'ca', 'br', 'otro'];
  function pintaTipos() { $('d-tipo').innerHTML = TIPOS.map((x) => `<option value="${x}">${esc(t('dato_' + x))}</option>`).join(''); }
  $('d-nuevo').addEventListener('submit', (e) => {
    e.preventDefault();
    const valor = $('d-valor').value.trim();
    if (!valor) return;
    manda({ tipo: 'tinta_anadir', dato: $('d-tipo').value, valor });
    $('d-valor').value = '';
  });
  en('tinta', (m) => {
    const lista = m.lista || [];
    $('d-marcados').innerHTML = lista.map((x, i) => `<li><span class="chip">${esc(t('dato_' + x.tipo))}</span><span class="mono">${esc(x.visible)}</span><button class="boton chico" data-i="${i}">${esc(t('tinta_quitar'))}</button></li>`).join('');
    $('d-marcados').querySelectorAll('button').forEach((b) => b.addEventListener('click', () => manda({ tipo: 'tinta_quitar', indice: Number(b.dataset.i) })));
    $('d-marcados-vacio').classList.toggle('oculto', lista.length > 0);
  });
  en('libro', (m) => {
    const entradas = m.entradas || [];
    $('d-libro-vacio').classList.toggle('oculto', entradas.length > 0);
    $('d-libro').replaceChildren(...entradas.map((e) => {
      const d = document.createElement('div');
      d.className = 'entrada';
      const datos = (e.clases || []).map((c) => t('dato_' + c)).join(', ');
      d.innerHTML = `<div class="cab"><b>${esc(e.empresa || e.sitio)}</b>${e.empresa ? `<span class="mono gris">${esc(e.sitio)}</span>` : ''}${e.pais ? `<span class="gris pequeno">${esc(pais(e.pais))}</span>` : ''}${e.corredor ? `<span class="chip rojo">${esc(t('corredor_etiqueta'))}</span>` : ''}</div>`
        + `<div class="datos">${esc(t('libro_fila', { datos, veces: n(e.veces), fecha: fecha(e.ultima) }))}</div>`
        + `<button class="boton chico">${esc(t('libro_carta'))}</button><div class="carta oculto"></div>`;
      const caja = d.querySelector('.carta');
      d.querySelector('button').addEventListener('click', () => {
        caja.classList.toggle('oculto');
        if (!caja.classList.contains('oculto') && !caja.dataset.listo) {
          caja.dataset.listo = '1';
          caja.innerHTML = `<label class="etiqueta">${esc(t('ley'))}</label><select class="campo">${LEYES.map((l) => `<option value="${l}">${esc(t('ley_' + l))}</option>`).join('')}</select>`
            + `<label class="etiqueta">${esc(t('libro_carta'))}</label><input class="campo asunto" readonly><textarea class="campo" readonly style="margin-top:6px"></textarea>`
            + `<div class="fila-acciones"><button class="boton principal copiar">${esc(t('carta_copiar'))}</button><button class="boton correo">${esc(t('carta_correo'))}</button></div><p class="explica pequeno" style="margin-top:8px">${esc(t('carta_nota'))}</p>`;
          const sel = caja.querySelector('select');
          const pide = () => manda({ tipo: 'carta', sitio: e.sitio, ley: sel.value });
          sel.addEventListener('change', pide);
          caja.querySelector('.copiar').addEventListener('click', () => {
            navigator.clipboard.writeText(caja.querySelector('.asunto').value + '\n\n' + caja.querySelector('textarea').value);
            manda({ tipo: 'carta_hecha', sitio: e.sitio });
          });
          caja.querySelector('.correo').addEventListener('click', () => { manda({ tipo: 'abrir_correo', asunto: caja.querySelector('.asunto').value, cuerpo: caja.querySelector('textarea').value }); manda({ tipo: 'carta_hecha', sitio: e.sitio }); });
          d.dataset.sitio = e.sitio;
          pide();
        }
      });
      d.dataset.sitio = e.sitio;
      return d;
    }));
  });
  en('carta', (m) => {
    const d = [...document.querySelectorAll('.entrada')].find((x) => x.dataset.sitio === m.sitio);
    if (!d) return;
    d.querySelector('.carta .asunto').value = m.asunto;
    d.querySelector('.carta textarea').value = m.cuerpo;
  });

  // --- settings ---------------------------------------------------------------------------------
  en('ajustes', (m) => {
    $('a-proteccion').checked = !!m.cortar_seguimiento;
    $('a-cookies').checked = m.rechazar_cookies !== false;
    const imp = m.importar || [];
    $('a-importar-caja').classList.toggle('oculto', !imp.length);
    $('a-importar').innerHTML = imp.map((x) => `<button class="boton chico" data-de="${esc(x)}">${esc(x)}</button>`).join(' ');
    $('e-proteccion').checked = !!m.cortar_seguimiento;
    // One plain list, with no headings: the engines that say they keep no record of who searches
    // what go first (the owner, 10 Oct 2026).
    const opcion = (b) => `<option value="${esc(b.id)}"${b.id === m.buscador ? ' selected' : ''}>${esc(b.nombre)}</option>`;
    const todos = m.buscadores || [];
    $('a-buscador').innerHTML = todos.filter((b) => b.privado).map(opcion).join('')
      + todos.filter((b) => !b.privado).map(opcion).join('');
    $('a-idioma').value = m.idioma || '';
    document.querySelectorAll('.temas button').forEach((b) => b.setAttribute('aria-pressed', String(b.dataset.tema === (m.tema || ''))));
    $('a-borrar-palabra').placeholder = t('borrar_confirma', { palabra: t('borrar_palabra') });
    $('a-huella').textContent = m.huella_clave ? t('ajustes_huella', { huella: m.huella_clave }) : '';
    // The sites the person told not to ask about again, each with its way back.
    const sin = m.sin_preguntar || [];
    $('a-sin-preguntar-caja').classList.toggle('oculto', !sin.length);
    $('a-sin-preguntar').innerHTML = sin.map((s) => `<li><span class="mono">${esc(s)}</span><button class="boton chico" data-sitio="${esc(s)}">${esc(t('preguntar_otra_vez'))}</button></li>`).join('');
    $('a-sin-preguntar').querySelectorAll('button').forEach((b) => b.addEventListener('click', () => manda({ tipo: 'preguntar_otra_vez', sitio: b.dataset.sitio })));
  });
  en('acerca', (m) => {
    $('a-version').textContent = t('version', { v: m.version });
    $('a-motor').textContent = t('motor_web', { v: m.motor });
  });
  $('a-proteccion').addEventListener('change', (e) => manda({ tipo: 'ajuste', clave: 'cortar_seguimiento', valor: e.target.checked }));
  // «Buscar actualización»: one read of the public ledger, only when pressed.
  $('a-buscar-version').addEventListener('click', () => manda({ tipo: 'buscar_version' }));
  $('a-descargar-version').addEventListener('click', () => manda({ tipo: 'descargar_version' }));
  en('version', (m) => {
    const r = $('a-version-res');
    $('a-buscar-version').disabled = m.estado === 'buscando';
    $('a-descargar-version').classList.toggle('oculto', m.estado !== 'nueva');
    r.textContent = m.estado === 'buscando' ? t('version_buscando')
      : m.estado === 'nueva' ? t('version_nueva', { v: m.version })
        : m.estado === 'al_dia' ? t('version_al_dia', { v: m.version })
          : t('version_error', { error: m.error || '' });
  });
  $('a-importar').addEventListener('click', (e) => {
    const b = e.target.closest('[data-de]');
    if (b) manda({ tipo: 'importar_favoritos', de: b.dataset.de });
  });
  $('a-cookies').addEventListener('change', (e) => manda({ tipo: 'ajuste', clave: 'rechazar_cookies', valor: e.target.checked }));
  $('a-buscador').addEventListener('change', (e) => manda({ tipo: 'ajuste', clave: 'buscador', valor: e.target.value }));
  document.querySelector('.temas [data-tema="dia"]').innerHTML = Z.ICONO.sol;
  document.querySelector('.temas [data-tema="noche"]').innerHTML = Z.ICONO.luna;
  document.querySelectorAll('.temas button').forEach((b) => b.addEventListener('click', () => manda({ tipo: 'ajuste', clave: 'tema', valor: b.dataset.tema })));
  $('a-idioma').addEventListener('change', (e) => manda({ tipo: 'ajuste', clave: 'idioma', valor: e.target.value }));
  $('a-aislada').addEventListener('click', () => manda({ tipo: 'pestana_aislada' }));
  $('a-borrar-palabra').addEventListener('input', (e) => { $('a-borrar').disabled = e.target.value.trim().toUpperCase() !== t('borrar_palabra'); });
  $('a-borrar').addEventListener('click', () => { manda({ tipo: 'borrar_todo' }); $('a-borrar-palabra').value = ''; $('a-borrar').disabled = true; });

  // --- a question before data leaves --------------------------------------------------------------
  let preguntaId = null;
  en('pregunta_formulario', (m) => {
    preguntaId = m.id;
    $('f-sitio').textContent = m.sitio;
    $('f-desde').textContent = m.pestana ? t('formulario_desde', { titulo: m.pestana }) : '';
    $('f-texto').textContent = t('formulario_texto', { sitio: m.sitio, datos: (m.datos || []).map((c) => t(c)).join(', ') });
    $('f-datos').innerHTML = (m.datos || []).map((c) => `<span class="chip ambar">${esc(t(c))}</span>`).join('');
    $('f-corredor').textContent = m.corredor ? t('formulario_corredor', { sitio: m.sitio }) : '';
    $('f-mandato').classList.toggle('oculto', !m.mandato);
    $('f-recordar-caja').classList.toggle('oculto', !!m.mandato);
    $('f-recordar').checked = false;
  });
  const responde = (enviar) => { if (preguntaId != null) manda({ tipo: 'formulario_respuesta', id: preguntaId, enviar, recordar: $('f-recordar').checked }); preguntaId = null; };
  $('f-enviar').addEventListener('click', () => responde(true));
  $('f-no').addEventListener('click', () => responde(false));

  // --- first run ----------------------------------------------------------------------------------
  $('b-si').addEventListener('click', () => manda({ tipo: 'bienvenida', cortar: true }));
  $('b-no').addEventListener('click', () => manda({ tipo: 'bienvenida', cortar: false }));

  en('textos', () => {
    try { const dn = new Intl.DisplayNames([document.documentElement.lang], { type: 'region' }); pais = (c) => { try { return dn.of(c); } catch (_) { return c; } }; } catch (_) {}
    pintaTipos();
    if (vista) muestra(vista);
    if (lic) pintaLicencia(lic);
  });
  en('vista', (m) => muestra(m.vista));

  // --- subscription -------------------------------------------------------------------------------
  // Every line is drawn from the state the program sends; the dates in the person's language.
  let lic = null;
  function lineaLicencia(m) {
    switch (m.estado) {
      case 'prueba': return tn('licencia_prueba', m.dias, { d: n(m.dias), fecha: fecha(m.termina) });
      case 'suscrita': return t('licencia_suscrita', { fecha: fecha(m.desde) });
      case 'prueba_terminada': return t('licencia_prueba_terminada', { fecha: fecha(m.desde) });
      case 'suscripcion_terminada': return t('licencia_terminada_' + (m.motivo || 'cancelada'), { fecha: fecha(m.desde) });
      case 'ilegible': return t('licencia_ilegible_fin', { datos: m.donde_datos || '' });
      default: return '';
    }
  }
  // The short form, where there is room for one line only.
  function pastillaLicencia(m) {
    if (m.estado === 'prueba_terminada') return t('licencia_pastilla_terminada');
    if (m.estado === 'ilegible') return t('licencia_pastilla_ilegible_fin');
    return t('licencia_pastilla_sin_suscripcion');
  }
  function pintaLicencia(m) {
    lic = m;
    const terminada = m.protege === false;
    const suscrita = m.estado === 'suscrita';
    // The licence file cannot be read right now: said first, above what was last known.
    const sinLeer = !terminada && m.ilegible ? t('licencia_ilegible', { datos: m.donde_datos || '' }) : '';
    const caja = $('l-caja');
    caja.classList.toggle('sin-proteccion', terminada);
    caja.classList.toggle('prueba', m.estado === 'prueba' || !!sinLeer);
    caja.classList.toggle('oculto', m.estado === 'desconocido' && !sinLeer);
    // The end-of-trial text says everything itself; the others get their detail below.
    $('l-estado').textContent = terminada ? '' : (sinLeer || lineaLicencia(m));
    $('l-estado').classList.toggle('oculto', terminada);
    let detalle = '';
    if (terminada) detalle = lineaLicencia(m);
    else if (sinLeer) detalle = lineaLicencia(m);
    else if (m.estado === 'prueba') detalle = t('licencia_al_terminar');
    else if (suscrita) {
      const partes = [];
      if (m.periodo === 0 || m.periodo === 30 || m.periodo === 365) partes.push(t('licencia_periodo_' + m.periodo));
      if (m.fallida && m.caduca) partes.push(t('licencia_fallida', { fecha: fecha(m.caduca) }));
      else if (m.proxima && m.periodo !== 0) partes.push(t('licencia_proxima', { fecha: fecha(m.proxima) }));
      detalle = partes.join(' ');
    }
    $('l-detalle').textContent = detalle;
    $('l-detalle').style.color = terminada ? 'var(--tinta)' : '';
    $('l-comprar-caja').classList.toggle('oculto', suscrita);
    $('l-clave-caja').classList.toggle('oculto', suscrita && !m.fallida);
    $('l-activar').classList.toggle('principal', terminada);
    $('l-activar').disabled = !!m.ocupada;
    $('l-ocupada').classList.toggle('oculto', !m.ocupada);
    $('l-error').classList.toggle('oculto', !m.error);
    $('l-error').textContent = m.error || '';
    const con = m.conexiones || [];
    $('l-conexiones').innerHTML = con.map((c) => `<li><time>${esc(fecha(c.ms))} ${esc(hora(c.ms))}</time><span><span class="mono">${esc(c.host)}</span> · ${esc(t(c.version ? 'conexion_version' : (c.pedida ? 'licencia_conexion_pedida' : 'licencia_conexion_periodica')))}</span></li>`).join('');
    $('l-conexiones-vacio').classList.toggle('oculto', con.length > 0);
    $('l-donde').textContent = m.donde_datos ? t('licencia_donde', { datos: m.donde_datos, ancla: m.donde_ancla }) : '';
    // Where the shield is, the same news; in the settings, one line.
    $('e-licencia').classList.toggle('oculto', !terminada);
    $('e-licencia-txt').textContent = terminada ? lineaLicencia(m) : '';
    $('e-proteccion').disabled = terminada;
    $('a-proteccion').disabled = terminada;
    $('a-cookies').disabled = terminada;
    $('a-licencia').textContent = terminada ? pastillaLicencia(m) : (m.ilegible ? t('licencia_pastilla_ilegible') : lineaLicencia(m));
  }
  en('licencia', pintaLicencia);
  $('l-comprar').addEventListener('click', () => manda({ tipo: 'licencia_comprar' }));
  $('l-form').addEventListener('submit', (e) => {
    e.preventDefault();
    manda({ tipo: 'licencia_activar', clave: $('l-clave').value });
  });
  $('e-licencia-ver').addEventListener('click', () => manda({ tipo: 'panel', vista: 'licencia' }));
  $('a-licencia-ver').addEventListener('click', () => manda({ tipo: 'panel', vista: 'licencia' }));
  let avisoTimer = 0;
  en('aviso', (m) => {
    const a = $('aviso');
    a.textContent = m.texto || '';
    a.classList.add('visto');
    clearTimeout(avisoTimer);
    avisoTimer = setTimeout(() => a.classList.remove('visto'), 4200);
  });
  manda({ tipo: 'listo', vista: 'panel' });
})();
