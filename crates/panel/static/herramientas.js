// «Herramientas» (1.0.45, customer requests of 18 Oct 2026): «Modo apagón», «Limpiar archivo»
// and «Antes de instalar». Everything happens on this computer: a file picked here goes to
// GUARDIANA on 127.0.0.1 and nowhere else, is looked at in memory (or, for an installer, in
// GUARDIANA's own folder) and is not kept. app.js hands over the session, the texts and the
// language (`iniciar`).
(() => {
  'use strict';
  window.GuardianaHerramientas = { iniciar };
  const MAX_HUELLAS = 64 * 1024 * 1024;

  // `foto.jpg` → `foto-sin-huellas.jpg`, the same rule as guardiana_core::huellas::nombre_limpio.
  const nombreLimpio = (n) => { const i = n.lastIndexOf('.'); return i > 0 ? n.slice(0, i) + '-sin-huellas' + n.slice(i) : n + '-sin-huellas'; };

  async function iniciar(ctx) {
    const { api, t, tn, esc, LOC, cabeceras } = ctx;
    const $ = (id) => document.getElementById(id);
    const fecha = (ms) => (ms ? new Date(ms).toLocaleString(LOC, { dateStyle: 'medium', timeStyle: 'short' }) : '');
    const tam = (b) => b >= 1048576 ? (b / 1048576).toLocaleString(LOC, { maximumFractionDigits: 1 }) + ' MB' : Math.max(1, Math.round(b / 1024)).toLocaleString(LOC) + ' KB';
    // A file to GUARDIANA as it is (not JSON): the session goes in a header, as everywhere.
    const sube = async (ruta, cuerpo, extra) => {
      const r = await fetch(ruta, { method: 'POST', body: cuerpo, headers: Object.assign({ 'Content-Type': 'application/octet-stream' }, cabeceras(), extra || {}) });
      if (r.status === 401) { const s = $('no-session'); if (s) s.classList.remove('hidden'); throw new Error('sin sesión'); }
      return r;
    };
    const zona = (labelId, input, alSoltar) => {
      const z = $(labelId);
      ['dragenter', 'dragover'].forEach((ev) => z.addEventListener(ev, (e) => { e.preventDefault(); z.classList.add('sobre'); }));
      z.addEventListener('dragleave', () => z.classList.remove('sobre'));
      z.addEventListener('drop', (e) => { e.preventDefault(); z.classList.remove('sobre'); const f = e.dataTransfer && e.dataTransfer.files[0]; if (f) alSoltar(f); });
      $(input).addEventListener('change', () => { const f = $(input).files[0]; $(input).value = ''; if (f) alSoltar(f); });
    };

    // ---------- Modo apagón ----------
    let A = null, confirmando = false;
    // The owner, 18 Oct 2026: every cut or let-through is a switch, green «Permitido» ↔ red
    // «Cortado», with the word beside it; never a «Cortar» text button. aria-checked="true" is
    // «Permitido». The class `interruptor` is the shared component of diseno.css.
    const interruptor = (permitido, datos, etiqueta) => `<span class="hx-interruptor"><button type="button" class="interruptor" role="switch" aria-checked="${permitido ? 'true' : 'false'}" data-estado="${permitido ? 'permitido' : 'cortado'}" aria-label="${esc(etiqueta)}" ${datos}></button><span class="interruptor-estado">${esc(t(permitido ? 'hx_permitido' : 'hx_cortado'))}</span></span>`;
    const pintaApagon = (a) => {
      A = a;
      // The main switch: «Permitido» while the blackout is off, «Cortado» while it is on.
      const b = $('hx-ap-boton');
      b.setAttribute('aria-checked', a.activo ? 'false' : 'true');
      b.dataset.estado = a.activo ? 'cortado' : 'permitido';
      $('hx-ap-estado').textContent = t(a.activo ? 'hx_ap_estado_on' : 'hx_ap_estado_off');
      $('hx-ap-confirmar').hidden = !confirmando || a.activo;
      $('hx-ap-desde').textContent = a.activo && a.desde ? t('hx_ap_desde').replace('{fecha}', fecha(a.desde)) : '';
      const cortados = a.cortados || [];
      $('hx-ap-cortados').innerHTML = cortados.length
        ? cortados.map((c) => `<li><span><b>${esc(c.nombre)}</b><small class="muted">${esc(tn('hx_ap_veces', c.veces))}${c.programa ? ' · ' + esc(c.programa) : ''} · ${esc(fecha(c.ultima))}</small></span>${interruptor(false, `data-aprobar="${esc(c.nombre)}"`, c.nombre)}</li>`).join('')
        : `<li class="muted">${esc(t(a.activo ? 'hx_ap_nada_cortado' : 'hx_ap_apagado_nada'))}</li>`;
      const ap = a.aprobados || [];
      $('hx-ap-aprobados').innerHTML = ap.length
        ? ap.map((n) => `<li><span><b>${esc(n)}</b><small class="muted">${esc(t('hx_ap_y_debajo'))}</small></span>${interruptor(true, `data-quitar="${esc(n)}"`, n)}</li>`).join('')
        : `<li class="muted">${esc(t('hx_ap_sin_aprobados'))}</li>`;
      $('hx-ap-esenciales').innerHTML = (a.esenciales || []).map((e) => `<li><span><b>${esc(e.dominio)}</b><small class="muted">${esc(t('hx_para_' + e.para))}</small></span></li>`).join('');
    };
    const cargaApagon = async () => { try { pintaApagon(await api('/api/apagon')); } catch (_) {} };
    $('hx-ap-boton').addEventListener('click', async () => {
      if (!A) return;
      // Turning it off is always one click; turning it on asks once, because it cuts a lot.
      if (A.activo) { confirmando = false; pintaApagon(await api('/api/apagon', { method: 'POST', body: { activo: false } })); return; }
      confirmando = true; pintaApagon(A);
    });
    $('hx-ap-si').addEventListener('click', async () => { confirmando = false; pintaApagon(await api('/api/apagon', { method: 'POST', body: { activo: true } })); });
    $('hx-ap-no').addEventListener('click', () => { confirmando = false; pintaApagon(A); });
    const aprueba = async (nombre) => {
      $('hx-ap-msg').textContent = '';
      try { pintaApagon(await api('/api/apagon/aprobar', { method: 'POST', body: { nombre } })); $('hx-ap-nombre').value = ''; } catch (_) { $('hx-ap-msg').textContent = t('hx_ap_no_nombre'); }
    };
    $('hx-apagon').addEventListener('click', async (e) => {
      const ok = e.target.closest('button[data-aprobar]');
      if (ok) { aprueba(ok.dataset.aprobar); return; }
      const fuera = e.target.closest('button[data-quitar]');
      if (fuera) pintaApagon(await api('/api/apagon/quitar', { method: 'POST', body: { nombre: fuera.dataset.quitar } }));
    });
    $('hx-ap-form').addEventListener('submit', (e) => { e.preventDefault(); const n = $('hx-ap-nombre').value.trim(); if (n) aprueba(n); });
    await cargaApagon();
    // While it is on, what it cuts appears as it happens (every five seconds, not while hidden).
    setInterval(() => { if (A && A.activo && !document.hidden) cargaApagon(); }, 5000);

    // ---------- Limpiar archivo ----------
    const listaHuellas = (inf) => (inf.huellas || []).map((h) => `<li><b>${esc(t('hx_h_' + h.clave))}</b>${h.valor ? ' <span>' + esc(h.valor) + '</span>' : ''}</li>`).join('');
    const guarda = async (blob, nombre) => {
      // Where the person chooses (next to the original, if they want), when the browser can ask;
      // otherwise it goes to the browser's downloads folder.
      if (window.showSaveFilePicker) {
        try {
          const h = await window.showSaveFilePicker({ suggestedName: nombre });
          const w = await h.createWritable(); await w.write(blob); await w.close();
          return 'elegido';
        } catch (e) { if (e && e.name === 'AbortError') return 'cancelado'; }
      }
      const a = document.createElement('a');
      a.href = URL.createObjectURL(blob); a.download = nombre;
      document.body.appendChild(a); a.click(); a.remove();
      setTimeout(() => URL.revokeObjectURL(a.href), 30000);
      return 'descargas';
    };
    const mira = async (f) => {
      const el = $('hx-li-resultado');
      if (f.size > MAX_HUELLAS) { el.innerHTML = `<p class="muted">${esc(t('hx_li_grande'))}</p>`; return; }
      el.innerHTML = `<p class="muted">${esc(t('hx_li_mirando').replace('{n}', f.name))}</p>`;
      let inf;
      try { const r = await sube('/api/huellas/mirar', f); if (!r.ok) throw new Error(await r.text()); inf = await r.json(); } catch (_) { el.innerHTML = `<p class="muted">${esc(t('hx_error'))}</p>`; return; }
      const cab = `<p class="hx-archivo"><b>${esc(f.name)}</b> <span class="muted">${esc(tam(f.size))}</span></p>`;
      if (inf.formato === 'otro') { el.innerHTML = cab + `<p class="muted">${esc(t('hx_li_otro'))}</p>`; return; }
      const nota = inf.nota ? `<p class="muted small">${esc(t('hx_nota_' + inf.nota))}</p>` : '';
      if (!inf.huellas.length && !inf.nota) { el.innerHTML = cab + `<p><span class="chip ok">${esc(t('hx_li_limpio_chip'))}</span> ${esc(t('hx_li_limpio'))}</p>`; return; }
      el.innerHTML = cab + `<p><span class="chip aviso">${esc(tn('hx_li_dice', inf.huellas.length))}</span></p><ul class="hx-huellas">${listaHuellas(inf)}</ul>${nota}`
        + (inf.limpiable ? `<div class="hx-botones"><button type="button" class="boton principal" id="hx-li-guardar">${esc(t('hx_li_guardar'))}</button></div><p class="muted small">${esc(t('hx_li_original'))}</p>` : '');
      const b = $('hx-li-guardar');
      if (!b) return;
      b.addEventListener('click', async () => {
        b.disabled = true;
        try {
          const r = await sube('/api/huellas/limpiar', f);
          if (!r.ok) throw new Error(await r.text());
          const limpio = await r.blob();
          // Checked again, the clean copy itself: what is said is what was measured.
          const r2 = await sube('/api/huellas/mirar', limpio);
          const otra = r2.ok ? await r2.json() : null;
          const donde = await guarda(limpio, nombreLimpio(f.name));
          if (donde === 'cancelado') { b.disabled = false; return; }
          const quedan = otra && otra.huellas && otra.huellas.length;
          const msg = quedan ? `<span class="chip aviso">${esc(t('hx_li_quedan'))}</span>` : `<span class="chip ok">${esc(t('hx_li_comprobado'))}</span>`;
          b.insertAdjacentHTML('afterend', ` <span class="hx-hecho">${msg} ${esc(t(donde === 'elegido' ? 'hx_li_guardado' : 'hx_li_en_descargas').replace('{n}', nombreLimpio(f.name)))}</span>${otra && otra.nota ? `<p class="muted small">${esc(t('hx_nota_' + otra.nota))}</p>` : ''}`);
          b.remove();
        } catch (_) { b.disabled = false; b.insertAdjacentHTML('afterend', ` <span class="muted">${esc(t('hx_error'))}</span>`); }
      });
    };
    zona('hx-li-zona', 'hx-li-archivo', mira);

    // ---------- Antes de instalar ----------
    const firma = async (f) => {
      const el = $('hx-in-resultado');
      el.innerHTML = `<p class="muted">${esc(t('hx_in_mirando').replace('{n}', f.name))}</p>`;
      const ext = (f.name.match(/\.([A-Za-z0-9]{1,12})$/) || [])[1] || 'exe';
      let r;
      try { const x = await sube('/api/instalador/firma', f, { 'X-Guardiana-Extension': ext }); if (!x.ok) throw new Error(await x.text()); r = await x.json(); } catch (_) { el.innerHTML = `<p class="muted">${esc(t('hx_error'))}</p>`; return; }
      const cab = `<p class="hx-archivo"><b>${esc(f.name)}</b> <span class="muted">${esc(tam(f.size))}</span></p>`;
      const fila = (k, v) => v ? `<div><dt>${esc(t(k))}</dt><dd>${esc(v)}</dd></div>` : '';
      let cuerpo;
      if (r.estado === 'verificada') {
        cuerpo = `<p><span class="chip ok">${esc(t('hx_in_verificado').replace('{x}', r.editor || '—'))}</span></p><dl class="hx-datos">${fila('hx_in_emisor', r.emisor)}${fila('hx_in_hasta', r.valido_hasta)}${fila('hx_in_sello', r.sello_tiempo || t('hx_in_sin_sello'))}${fila('hx_in_tipo', r.tipo)}</dl><p class="muted small">${esc(t('hx_in_cadena'))}</p>`;
      } else if (r.estado === 'sin_firma') {
        cuerpo = `<p><span class="chip aviso">${esc(t('hx_in_sin_firma'))}</span></p><p class="muted">${esc(t('hx_in_sin_firma_texto'))}</p>`;
      } else if (r.estado === 'no_valida') {
        cuerpo = `<p><span class="chip aviso">${esc(t('hx_in_no_valida'))}</span></p><p class="muted">${esc(t('hx_in_no_valida_texto'))}</p><dl class="hx-datos">${fila('hx_in_windows', r.windows)}${fila('hx_in_mensaje', r.mensaje)}</dl>`;
      } else if (r.estado === 'no_admitido') {
        cuerpo = `<p class="muted">${esc(t('hx_in_no_admitido'))}</p>`;
      } else {
        cuerpo = `<p class="muted">${esc(t('hx_in_no_disponible'))}</p>`;
      }
      el.innerHTML = cab + cuerpo;
    };
    zona('hx-in-zona', 'hx-in-archivo', firma);
  }
})();
