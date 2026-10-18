// «Contratos» (1.0.45, «contrato vivo»). Which companies each program on this computer normally
// contacts, learnt from what it asked for; a calm notice, with the before and the after, when one
// starts contacting new ones; and the chained register of every change. The sentences come from
// the panel already in the person's language: this script only lays them out and sends the
// person's decision back. It says «empezó a contactar», never more: a name asked for is not data
// sent (the owner's principle, 18 Oct 2026).
(() => {
  'use strict';
  window.GuardianaContratos = { iniciar };

  async function iniciar(ctx) {
    const { api, t, tn, esc, LOC, cadaTanto } = ctx;
    const $ = (id) => document.getElementById(id);
    document.title = 'GUARDIANA · ' + t('nav_contratos');
    const fecha = (ms) => (ms ? new Date(ms).toLocaleDateString(LOC, { dateStyle: 'medium' }) : '');
    const fechaHora = (ms) => (ms ? new Date(ms).toLocaleString(LOC, { dateStyle: 'medium', timeStyle: 'short' }) : '');
    const CHIP = { aprendiendo: 'info', vigilando: 'ok', fijado: 'azul', no_aplica: '' };
    const CHIP_AVISO = { abierto: 'aviso', aceptado: 'ok', cortado: 'rojo', cortado_auto: 'rojo', deshecho: '' };
    const abiertos = new Set();
    let ultimo = null;

    const msg = (texto, mal) => { const m = $('ct-msg'); m.textContent = texto || ''; m.className = 'ct-msg' + (mal ? ' bad' : ''); };

    const empresaLi = (e) => {
      const nombres = (e.nombres || []).join(', ');
      return `<li><b>${esc(e.empresa)}</b> <span class="ct-meta">${esc(tn('ct_veces', e.veces))} · ${esc(t('ct_desde').replace('{fecha}', fecha(e.desde)))}</span>${nombres ? `<div class="ct-mono">${esc(nombres)}</div>` : ''}</li>`;
    };

    const pintaAviso = (a) => {
      const abierto = a.estado === 'abierto';
      const deshacible = a.estado === 'cortado' || a.estado === 'cortado_auto';
      const antes = (a.antes || []).map((x) => `<li>${esc(x)}</li>`).join('');
      const nuevas = (a.nuevas || []).map((e) => `<li><b>${esc(e.empresa)}</b></li>`).join('');
      const detalle = (a.nuevas || []).map(empresaLi).join('');
      const versionAntes = a.version_antes ? ' · ' + esc(t('ct_version').replace('{v}', a.version_antes)) : '';
      const versionAhora = a.version_ahora ? ' · ' + esc(t('ct_version').replace('{v}', a.version_ahora)) : '';
      const sinCortar = (a.sin_cortar || []).map((s) => `<li>${esc(s)}</li>`).join('');
      const visto = abiertos.has(a.id);
      return `<article class="tarjeta ct-aviso" data-id="${a.id}">
        <div class="ct-cabeza"><h3>${esc(a.titulo)}</h3><span class="chip ${CHIP_AVISO[a.estado] || ''}">${esc(a.estado_texto)}</span></div>
        <div class="ct-meta">${esc(fechaHora(a.desde))}${a.resuelto ? ' → ' + esc(fechaHora(a.resuelto)) : ''}</div>
        <p class="ct-nota">${esc(a.nota)}</p>
        <div class="ct-antes-despues">
          <div class="ct-col"><div class="rotulo">${esc(t('ct_antes'))}${versionAntes}</div><ul>${antes || `<li class="ct-meta">—</li>`}</ul></div>
          <div class="ct-col nuevo"><div class="rotulo">${esc(t('ct_despues'))}${versionAhora}</div><ul>${nuevas}</ul></div>
        </div>
        ${a.reglas_texto ? `<p class="ct-nota">${esc(a.reglas_texto)} <a class="enlace" href="/reglas">→</a></p>` : ''}
        ${sinCortar ? `<ul class="ct-nota">${sinCortar}</ul>` : ''}
        <div class="ct-botones">
          ${abierto ? `<button type="button" class="btn cut chico" data-accion="cortar">${esc(t('ct_accion_cortar'))}</button>
          <button type="button" class="btn secondary chico" data-accion="aceptar">${esc(t('ct_accion_aceptar'))}</button>` : ''}
          ${deshacible ? `<button type="button" class="btn secondary chico" data-accion="deshacer">${esc(t('ct_accion_deshacer'))}</button>` : ''}
          <button type="button" class="enlace" data-accion="detalle" aria-expanded="${visto}">${esc(t('ct_accion_detalle'))}</button>
        </div>
        <div class="ct-detalle"${visto ? '' : ' hidden'}><div class="rotulo">${esc(t('ct_nombres'))}</div><div class="ct-col"><ul>${detalle}</ul></div></div>
      </article>`;
    };

    const pintaPrograma = (c) => {
      const cuando = c.estado === 'aprendiendo' && c.aprende_hasta
        ? t('ct_aprende_hasta').replace('{fecha}', fecha(c.aprende_hasta))
        : c.aprendido ? t('ct_aprendido_el').replace('{fecha}', fecha(c.aprendido)) : '';
      const meta = [
        c.version ? t('ct_version').replace('{v}', c.version) : t('ct_version_desconocida'),
        c.firmado_por ? t('ct_firmado').replace('{x}', c.firmado_por) : t('ct_sin_firma'),
        tn('ct_lanzamientos', c.lanzamientos),
        cuando,
      ].filter(Boolean).map(esc).join(' · ');
      const empresas = (c.empresas || []).map(empresaLi).join('');
      const cortadas = (c.cortadas || []).map((x) => `<li>${esc(x)}</li>`).join('');
      const versiones = (c.versiones || []).join(' → ');
      const puedeFijar = c.estado === 'aprendiendo' || c.estado === 'vigilando';
      return `<article class="tarjeta ct-prog" data-clave="${esc(c.clave)}">
        <div class="ct-cabeza"><h3>${esc(c.programa)}</h3><span class="chip ${CHIP[c.estado] || ''}">${esc(c.estado_texto)}</span></div>
        <div class="ct-meta">${meta}</div>
        <div class="ct-mono">${esc(c.ruta)}</div>
        ${c.motivo_texto ? `<p class="ct-nota">${esc(c.motivo_texto)}</p>` : ''}
        ${c.estado !== 'no_aplica' ? `<details class="ct-plegable"><summary>${esc(t('ct_ver_contrato'))} · ${esc(tn('ct_empresas_n', (c.empresas || []).length))}</summary>
          <div class="ct-col"><ul>${empresas || '<li class="ct-meta">—</li>'}</ul></div>
          ${cortadas ? `<div class="ct-col"><div class="rotulo">${esc(t('ct_cortadas'))}</div><ul>${cortadas}</ul></div>` : ''}
          ${versiones ? `<p class="ct-meta">${esc(t('ct_versiones'))}: ${esc(versiones)}</p>` : ''}
        </details>
        <div class="ct-botones">
          ${puedeFijar ? `<button type="button" class="btn secondary chico" data-fijar="1">${esc(t('ct_accion_fijar'))}</button>` : ''}
          ${c.estado === 'fijado' ? `<button type="button" class="btn secondary chico" data-fijar="0">${esc(t('ct_accion_soltar'))}</button>` : ''}
          <span class="ct-meta">${esc(t('ct_fijar_explica'))}</span>
        </div>` : ''}
      </article>`;
    };

    const pinta = (r) => {
      ultimo = r;
      $('ct-como').textContent = t('ct_como').replace('{dias}', r.aprende_dias).replace('{n}', r.aprende_lanzamientos);
      $('ct-solo-windows').classList.toggle('hidden', !!r.disponible);
      const cuenta = (e) => r.contratos.filter((c) => c.estado === e).length;
      $('ct-n-aprendiendo').textContent = cuenta('aprendiendo');
      $('ct-n-vigilando').textContent = cuenta('vigilando');
      $('ct-n-fijado').textContent = cuenta('fijado');
      $('ct-n-no-aplica').textContent = cuenta('no_aplica');
      $('ct-avisos').innerHTML = r.avisos.length
        ? r.avisos.map(pintaAviso).join('')
        : `<p class="ct-nota">${esc(t('ct_avisos_ninguno'))}</p>`;
      $('ct-programas').innerHTML = r.contratos.length
        ? r.contratos.map(pintaPrograma).join('')
        : `<p class="ct-nota">${esc(t('ct_vacio'))}</p>`;
    };

    const pintaRegistro = (filas) => {
      $('ct-registro').innerHTML = filas.map((f) => `<li><time datetime="${new Date(f.ts).toISOString()}">${esc(fechaHora(f.ts))}</time><span>${esc(f.frase)}</span><span class="ct-mono" title="${esc(f.cabeza_extracto)}">${esc(f.hash.slice(0, 16))}…</span></li>`).join('');
    };

    const carga = async () => {
      const [r, reg] = await Promise.all([api('/api/contratos'), api('/api/contratos/registro')]);
      pinta(r);
      pintaRegistro(reg);
    };

    $('ct-avisos').addEventListener('click', async (ev) => {
      const b = ev.target.closest('button[data-accion]');
      if (!b) return;
      const art = b.closest('[data-id]');
      const id = Number(art.getAttribute('data-id'));
      const accion = b.getAttribute('data-accion');
      if (accion === 'detalle') {
        if (abiertos.has(id)) abiertos.delete(id); else abiertos.add(id);
        const d = art.querySelector('.ct-detalle');
        d.hidden = !abiertos.has(id);
        b.setAttribute('aria-expanded', String(abiertos.has(id)));
        return;
      }
      b.disabled = true;
      try {
        const r = await api(`/api/contratos/avisos/${id}/${accion}`, { method: 'POST' });
        msg(r.texto);
        await carga();
      } catch (e) {
        msg(t('ct_error').replace('{x}', String(e.message || e)), true);
        b.disabled = false;
      }
    });

    $('ct-programas').addEventListener('click', async (ev) => {
      const b = ev.target.closest('button[data-fijar]');
      if (!b) return;
      const clave = b.closest('[data-clave]').getAttribute('data-clave');
      b.disabled = true;
      try {
        await api('/api/contratos/fijar', { method: 'POST', body: { clave, fijar: b.getAttribute('data-fijar') === '1' } });
        await carga();
      } catch (e) {
        msg(t('ct_error').replace('{x}', String(e.message || e)), true);
        b.disabled = false;
      }
    });

    $('ct-comprobar').addEventListener('click', async () => {
      try {
        const r = await api('/api/contratos/comprobar');
        const el = $('ct-comprobado');
        el.textContent = r.texto;
        el.className = 'ct-msg' + (r.ok ? '' : ' bad');
      } catch (e) {
        $('ct-comprobado').textContent = t('ct_error').replace('{x}', String(e.message || e));
      }
    });

    await carga();
    // Kept up to date while the page is open; a pass costs little when nothing is new. Nothing
    // is redrawn under a person who is reading a detail or pressing a button.
    if (cadaTanto) {
      cadaTanto(async () => {
        if (document.activeElement && document.activeElement.closest && document.activeElement.closest('.ct-aviso, .ct-prog')) return;
        const r = await api('/api/contratos');
        if (JSON.stringify(r) !== JSON.stringify(ultimo)) await carga();
      }, 20000);
    }
  }
})();
