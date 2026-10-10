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
  // One row per company: what holds now (cut or passing) decides the colour and the button;
  // the counts below say what happened on this page.
  function fila(x) {
    const cortado = x.ahora === 'cortado';
    const sigue = !cortado && (x.categoria === 'rastreador' || x.categoria === 'publicidad' || x.corredor);
    const li = document.createElement('li');
    li.className = 'tercero' + (cortado ? ' cortado' : sigue ? ' sigue' : '');
    const paisTxt = x.pais ? `<small>${esc(pais(x.pais))}</small>` : '';
    const detalle = [
      `<span class="mono">${esc(x.sitio)}</span>`,
      `<span>${esc(x.cortadas > 0 ? t('escudo_cortadas_de', { c: n(x.cortadas), n: n(x.vistas) }) : tn('escudo_peticiones', x.vistas, { n: n(x.vistas) }))}</span>`,
      `<span class="chip ${COLOR_CAT[x.categoria] || ''}">${esc(t('cat_' + x.categoria))}</span>`,
      x.corredor ? `<span class="chip rojo" title="${esc(x.corredor)}">${esc(t('corredor_etiqueta'))}</span>` : '',
      // The reason is printed only when the chips do not say it already.
      x.cortadas > 0 && x.motivo && !['rastreador', 'publicidad', 'corredor'].includes(x.motivo) ? `<span class="motivo">${esc(t('motivo_' + x.motivo))}</span>` : '',
    ].join('');
    const boton = cortado
      ? `<button class="boton chico deja" data-accion="${x.regla === 'tuya' ? 'deshacer_sitio' : 'permitir_sitio'}" title="${esc(t('accion_permitir_titulo'))}">${esc(t('accion_permitir'))}</button>`
      : `<button class="boton chico corta" data-accion="cortar_sitio" title="${esc(t('accion_cortar_titulo'))}">${esc(t('accion_cortar'))}</button>`;
    li.innerHTML = `<span class="quien">${esc(x.quien)}${paisTxt}</span>${boton}<span class="detalle">${detalle}</span>`;
    li.querySelector('button').addEventListener('click', (e) => {
      manda({ tipo: e.currentTarget.dataset.accion, sitio: x.sitio });
      $('e-recarga').classList.remove('oculto');
    });
    return li;
  }
  let sitioEscudo = null;
  en('escudo', (m) => {
    if (m.sitio !== sitioEscudo) { sitioEscudo = m.sitio; $('e-recarga').classList.add('oculto'); }
    $('e-sitio').textContent = m.sitio || '';
    const r = m.resumen || {};
    $('e-resumen').textContent = r.empresas ? `${tn('escudo_resumen', r.empresas, { empresas: n(r.empresas) })} ${tn('escudo_resumen_cortadas', r.cortadas, { cortadas: n(r.cortadas) })}` : '';
    $('e-proteccion').checked = !!m.cortando;
    const lista = (m.terceros || []).slice();
    lista.sort((a, b) => (b.cortadas - a.cortadas) || (b.vistas - a.vistas));
    const cortados = lista.filter((x) => x.ahora === 'cortado');
    const vistos = lista.filter((x) => x.ahora !== 'cortado');
    $('e-cortados').replaceChildren(...cortados.map(fila));
    $('e-vistos').replaceChildren(...vistos.map(fila));
    $('e-cortados-n').textContent = n(cortados.length);
    $('e-vistos-n').textContent = n(vistos.length);
    $('e-cortados-caja').classList.toggle('oculto', !cortados.length);
    $('e-vistos-caja').classList.toggle('oculto', !vistos.length);
    $('e-vacio').classList.toggle('oculto', lista.length > 0);
    const hechos = [];
    if (m.parametros_quitados) hechos.push(tn('parametros_quitados', m.parametros_quitados, { n: n(m.parametros_quitados) }));
    if (m.datos_salvados) hechos.push(tn('datos_salvados', m.datos_salvados, { n: n(m.datos_salvados) }));
    if (m.cookies) hechos.push(t(m.cookies.accion === 'rechazado' ? 'cookies_rechazado' : 'cookies_escondido', { gestor: m.cookies.gestor }));
    $('e-hechos').innerHTML = hechos.map((h) => `<span>${esc(h)}</span>`).join('');
  });
  $('e-recargar').addEventListener('click', () => { manda({ tipo: 'recargar' }); $('e-recarga').classList.add('oculto'); });
  $('e-ver-todo').addEventListener('click', () => manda({ tipo: 'abrir_cortes' }));
  $('e-proteccion').addEventListener('change', (e) => manda({ tipo: 'ajuste', clave: 'cortar_seguimiento', valor: e.target.checked }));

  // --- mandate ---------------------------------------------------------------------------------
  let sitioActual = '';
  $('m-empezar').addEventListener('click', () => {
    const webs = $('m-webs').value.split(/[\s,]+/).map((s) => s.trim()).filter(Boolean);
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
    if (estado === 'ninguno' && !$('m-webs').value && sitioActual) $('m-webs').value = sitioActual;
    const md = m.mandato;
    const cuentas = m.cuentas ? t('mandato_cuentas', { visitas: n(m.cuentas.visitas), sitios: n(m.cuentas.sitios), cortes: n(m.cuentas.cortes), datos: n(m.cuentas.datos) }) : '';
    if (estado === 'activo' && md) {
      $('m-tarea-txt').textContent = md.tarea || '';
      $('m-sitios').innerHTML = md.permitidos.map((s) => `<span class="chip azul mono">${esc(s)}</span>`).join('');
      $('m-cuentas').textContent = cuentas;
      $('m-senuelo').textContent = md.senuelo;
      $('m-caduca').textContent = md.caduca ? t('mandato_termina', { hora: new Date(md.caduca).toLocaleTimeString(document.documentElement.lang, { hour: '2-digit', minute: '2-digit' }) }) : '';
      const pasos = (md.pasos || []).slice(-40).reverse();
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
  en('guardado', (m) => { $('r-ruta').textContent = m.texto || m.ruta || ''; });

  // --- redaction --------------------------------------------------------------------------------
  let tachado = null;
  $('t-tachar').addEventListener('click', () => manda({ tipo: 'tachar', texto: $('t-texto').value }));
  en('tachado', (m) => {
    tachado = m;
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
    // Two groups: the engines that say they keep no record of who searches what, and the rest.
    const opcion = (b) => `<option value="${esc(b.id)}"${b.id === m.buscador ? ' selected' : ''}>${esc(b.nombre)}</option>`;
    const grupo = (clave, lista) => (lista.length ? `<optgroup label="${esc(t(clave))}">${lista.map(opcion).join('')}</optgroup>` : '');
    const todos = m.buscadores || [];
    $('a-buscador').innerHTML = grupo('buscador_privados', todos.filter((b) => b.privado))
      + grupo('buscador_otros', todos.filter((b) => !b.privado));
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
      default: return '';
    }
  }
  function pintaLicencia(m) {
    lic = m;
    const terminada = m.protege === false;
    const suscrita = m.estado === 'suscrita';
    const caja = $('l-caja');
    caja.classList.toggle('sin-proteccion', terminada);
    caja.classList.toggle('prueba', m.estado === 'prueba');
    caja.classList.toggle('oculto', m.estado === 'desconocido');
    // The end-of-trial text says everything itself; the others get their detail below.
    $('l-estado').textContent = terminada ? '' : lineaLicencia(m);
    $('l-estado').classList.toggle('oculto', terminada);
    let detalle = '';
    if (terminada) detalle = lineaLicencia(m);
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
    $('a-licencia').textContent = terminada ? t(m.estado === 'prueba_terminada' ? 'licencia_pastilla_terminada' : 'licencia_pastilla_sin_suscripcion') : lineaLicencia(m);
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
