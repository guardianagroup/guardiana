// «Lo que se cortó»: every request cut, one by one, with what is known about it. The data comes
// from the program (the browser's own log on this computer); nothing here talks to the network.
'use strict';
(() => {
  const { manda, en, t, esc, $, n } = Z;
  const COLOR_CAT = { rastreador: 'rojo', publicidad: 'ambar', telemetria: 'ambar', esperado: 'verde', desconocido: '' };
  let periodo = 'hoy';
  let datos = null;
  let abierta = -1;
  let pais = (c) => c || '';
  // The reasons that mean «it carried your data»: the same ones the new tab counts.
  const DE_DATO = ['tinta', 'senuelo'];
  let filtroInicial = location.hash === '#dato' ? '@dato' : '';

  const lang = () => document.documentElement.lang || 'es';
  const nombrePais = (c) => (c ? pais(c) : t('cortes_desconocido'));
  function hora(ts) {
    const d = new Date(ts);
    const h = d.toLocaleTimeString(lang(), { hour: '2-digit', minute: '2-digit', second: '2-digit' });
    return periodo === 'hoy' ? h : `${d.toLocaleDateString(lang(), { day: 'numeric', month: 'short' })} ${h}`;
  }

  function pide() {
    manda({ tipo: 'cortes', periodo });
  }
  document.querySelectorAll('.periodos button').forEach((b) => b.addEventListener('click', () => {
    periodo = b.dataset.periodo;
    document.querySelectorAll('.periodos button').forEach((x) => x.setAttribute('aria-pressed', String(x === b)));
    abierta = -1;
    pide();
  }));
  // Back to the page the person was on: this list opens in its own tab, with nothing behind it.
  $('volver').addEventListener('click', () => manda({ tipo: 'volver' }));
  $('pdf').addEventListener('click', () => manda({ tipo: 'exportar_cortes', periodo, formato: 'pdf' }));
  $('csv').addEventListener('click', () => manda({ tipo: 'exportar_cortes', periodo, formato: 'csv' }));
  $('f-motivo').addEventListener('change', pintaFilas);
  $('f-texto').addEventListener('input', pintaFilas);

  function barras(lista) {
    const max = Math.max(1, ...lista.map((x) => x.n));
    const top = lista.slice(0, 12);
    const resto = lista.slice(12).reduce((a, x) => a + x.n, 0);
    const li = top.map((x) => `<li><span class="nombre">${esc(x.quien)}${x.pais ? `<small>${esc(nombrePais(x.pais))}</small>` : ''}</span><span class="n">${n(x.n)}</span><span class="barra"><i style="width:${Math.max(2, Math.round(100 * x.n / max))}%"></i></span></li>`);
    if (resto) li.push(`<li><span class="nombre"><small>${esc(t('cortes_otras', { n: n(lista.length - 12) }))}</small></span><span class="n">${n(resto)}</span></li>`);
    return li.join('');
  }
  const simple = (lista, nombre) => lista.slice(0, 10).map((x) => `<li><span>${esc(nombre(x))}</span><span class="n">${n(x.n)}</span></li>`).join('');

  function pinta() {
    const d = datos;
    $('c-total').textContent = n(d.total);
    $('c-empresas').textContent = n(d.empresas.length);
    $('c-paises').textContent = n(d.paises.filter((x) => x.pais).length);
    $('c-webs').textContent = n(d.paginas.filter((x) => x.pagina).length);
    $('r-empresas').innerHTML = barras(d.empresas);
    $('r-paises').innerHTML = simple(d.paises, (x) => nombrePais(x.pais));
    $('r-paginas').innerHTML = simple(d.paginas, (x) => x.pagina || t('pestana_aislada'));
    $('r-motivos').innerHTML = d.motivos.map((x) => `<span class="chip rojo">${esc(t('motivo_' + x.motivo))}<b>${n(x.n)}</b></span>`).join('');
    $('r-tipos').innerHTML = d.tipos.map((x) => `<span class="chip">${esc(t('recurso_' + x.recurso))}<b>${n(x.n)}</b></span>`).join('');
    $('resumen').classList.toggle('oculto', !d.total);
    // Keep the chosen filter across refreshes; «@dato» is what the new tab's figure opens.
    const elegido = $('f-motivo').value || filtroInicial;
    filtroInicial = '';
    const conDato = d.motivos.some((x) => DE_DATO.includes(x.motivo));
    $('f-motivo').innerHTML = `<option value="">${esc(t('cortes_filtro_motivo'))}</option>`
      + (conDato ? `<option value="@dato">${esc(t('cortes_filtro_dato'))}</option>` : '')
      + d.motivos.map((x) => `<option value="${esc(x.motivo)}">${esc(t('motivo_' + x.motivo))}</option>`).join('');
    if ([...$('f-motivo').options].some((o) => o.value === elegido)) $('f-motivo').value = elegido;
    $('recortada').classList.toggle('oculto', !d.recortada);
    $('recortada').textContent = d.recortada ? t('cortes_recortada', { n: n(d.lista.length) }) : '';
    const hoy = new Date().toLocaleString(lang(), { dateStyle: 'long', timeStyle: 'short' });
    const f = (s) => new Date(s + 'T12:00:00').toLocaleDateString(lang(), { day: 'numeric', month: 'long', year: 'numeric' });
    $('informe').textContent = t('cortes_informe', { fecha: hoy, desde: f(d.desde), hasta: f(d.hasta) });
    pintaFilas();
  }

  function detalle(c) {
    const partes = [];
    partes.push(`<p><b>${esc(c.quien)}</b> · ${esc(nombrePais(c.pais))}</p>`);
    partes.push(`<p>${esc(t('cortes_det_destino'))}: <span class="mono">${esc(c.host)}${esc(c.ruta)}</span></p>`);
    partes.push(`<p>${esc(t('recurso_' + c.recurso + '_explica'))}</p>`);
    partes.push(`<p>${esc(c.motivo ? t('motivo_' + c.motivo) : '')} · ${esc(c.metodo)}</p>`);
    partes.push(`<p>${esc(c.aislada ? t('cortes_det_aislada') : t('cortes_det_web', { pagina: c.pagina }))}</p>`);
    if (c.mandato) partes.push(`<p>${esc(t('cortes_det_mandato'))}</p>`);
    if (c.parametros) partes.push(`<p>${esc(t('cortes_det_parametros', { n: n(c.parametros) }))}</p>`);
    if (c.dato) partes.push(`<p>${esc(t('cortes_det_dato', { dato: t(c.dato).toLowerCase(), como: t('como_' + (c.como || 'tal_cual')) }))}</p>`);
    if (c.corredor) {
      partes.push(`<p>${esc(t('cortes_det_corredor', { empresa: c.corredor }))}</p>`);
      if (c.corredor_url && /^https:\/\//.test(c.corredor_url)) partes.push(`<p><a href="${esc(c.corredor_url)}" target="_blank" rel="noopener noreferrer">${esc(t('cortes_det_corredor_url'))}</a></p>`);
    }
    return `<div class="det">${partes.join('')}</div>`;
  }

  function pintaFilas() {
    if (!datos) return;
    const motivo = $('f-motivo').value;
    const q = $('f-texto').value.trim().toLowerCase();
    const vale = (c) => !motivo || (motivo === '@dato' ? DE_DATO.includes(c.motivo) : c.motivo === motivo);
    const lista = datos.lista.map((c, i) => [c, i]).filter(([c]) => vale(c)
      && (!q || [c.quien, c.host, c.ruta, c.pagina, nombrePais(c.pais)].some((x) => String(x || '').toLowerCase().includes(q))));
    const filas = [];
    for (const [c, i] of lista) {
      filas.push(`<tr class="fila" data-i="${i}" aria-expanded="${abierta === i}">`
        + `<td class="hora">${esc(hora(c.ts))}</td>`
        + `<td class="quien"><b>${esc(c.quien)}</b><small>${esc(nombrePais(c.pais))}</small></td>`
        + `<td class="destino">${esc(c.host)}<span>${esc(c.ruta)}</span></td>`
        + `<td class="que"><span class="chip ${COLOR_CAT[c.categoria] || ''}">${esc(t('cat_' + c.categoria))}</span>${c.corredor ? ` <span class="chip rojo">${esc(t('corredor_etiqueta'))}</span>` : ''}</td>`
        + `<td class="tipo">${esc(t('recurso_' + c.recurso))}</td>`
        + `<td class="motivo">${esc(c.motivo ? t('motivo_' + c.motivo) : '')}</td>`
        + `<td class="pagina">${esc(c.aislada ? t('pestana_aislada') : c.pagina)}</td></tr>`);
      if (abierta === i) filas.push(`<tr class="detalle"><td colspan="7">${detalle(c)}</td></tr>`);
    }
    $('filas').innerHTML = filas.join('');
    // The PDF says when it lists only part of the period.
    const filtro = [motivo ? $('f-motivo').selectedOptions[0].textContent : '', q ? `«${$('f-texto').value.trim()}»` : ''].filter(Boolean).join(', ');
    $('informe-filtro').textContent = filtro ? t('cortes_informe_filtro', { filtro, n: n(lista.length) }) : '';
    const vacio = $('vacio');
    vacio.classList.toggle('oculto', lista.length > 0);
    vacio.textContent = datos.total ? t('cortes_sin_coincidencias') : t('cortes_vacio');
    $('filas').querySelectorAll('tr.fila').forEach((tr) => tr.addEventListener('click', () => {
      const i = Number(tr.dataset.i);
      abierta = abierta === i ? -1 : i;
      pintaFilas();
    }));
  }

  en('cortes', (m) => { datos = m; pinta(); });
  let avisoTimer = 0;
  en('guardado', (m) => {
    const a = $('aviso');
    a.textContent = m.texto || '';
    a.classList.add('visto');
    clearTimeout(avisoTimer);
    avisoTimer = setTimeout(() => a.classList.remove('visto'), 4500);
  });
  en('textos', () => {
    try { const dn = new Intl.DisplayNames([lang()], { type: 'region' }); pais = (c) => { try { return dn.of(c); } catch (_) { return c; } }; } catch (_) {}
    if (datos) pinta(); else pide();
  });
  // Coming back to the tab brings what was cut meanwhile.
  document.addEventListener('visibilitychange', () => { if (document.visibilityState === 'visible' && datos) pide(); });
  manda({ tipo: 'listo', vista: 'cortes' });
})();
