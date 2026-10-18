// «Qué se rompe si corto» (1.0.45, customer request of 18 Oct 2026): before the person cuts a
// company or a site, which programs of this computer used it in the last seven days and how
// often, from the per-program attribution already in the ledger (Windows says which program
// asked, on this computer only). For the «Cortar» component; it brings no style of its own:
// the markup uses the shared classes and the component places it where it wants.
//
//   const r = await GuardianaQueSeRompe.consultar(api, { empresa: 'Microsoft' });   // or { dominio: 'teams.microsoft.com' }
//   GuardianaQueSeRompe.pinta(elemento, r, { t, tn, esc, LOC });
//   GuardianaQueSeRompe.resumen(r, { t, tn })  // one sentence, for a confirmation dialog
//
// It says what was measured and no more: a program that did not ask in the window, or asked
// through a resolver of its own, is not in it, and the free plan keeps a day of detail (the
// answer's `detalle_desde` says from when, and the sentence says it too).
(() => {
  'use strict';
  async function consultar(api, { empresa, dominio } = {}) {
    const q = dominio ? 'dominio=' + encodeURIComponent(dominio) : 'empresa=' + encodeURIComponent(empresa || '');
    return api('/api/que-se-rompe?' + q);
  }
  const ventana = (r, { t }, LOC) => {
    if (r.detalle_desde && r.detalle_desde > r.desde) {
      return t('qsr_desde').replace('{fecha}', new Date(r.detalle_desde).toLocaleString(LOC || undefined, { dateStyle: 'medium', timeStyle: 'short' }));
    }
    return t('qsr_siete_dias');
  };
  function resumen(r, { t, tn }) {
    const p = r.programas || [];
    if (!p.length) return t(r.sin_programa ? 'qsr_resumen_sin_programa' : 'qsr_resumen_nada');
    const nombres = p.slice(0, 3).map((x) => x.programa).join(', ');
    return tn('qsr_resumen', p.length).replace('{programas}', nombres + (p.length > 3 ? '…' : ''));
  }
  function pinta(el, r, { t, tn, esc, LOC }) {
    const p = r.programas || [];
    const fecha = (ms) => new Date(ms).toLocaleString(LOC || undefined, { dateStyle: 'medium', timeStyle: 'short' });
    let html = `<p class="qsr-ventana muted small">${esc(ventana(r, { t }, LOC))}</p>`;
    if (!p.length && !r.sin_programa && !r.otros_dispositivos) {
      html += `<p class="qsr-nada">${esc(t('qsr_nada'))}</p>`;
    } else {
      if (p.length) {
        html += `<p class="qsr-titulo">${esc(t('qsr_puede_parar'))}</p><ul class="qsr-programas">`
          + p.map((x) => `<li><b>${esc(x.programa)}</b> <span class="muted">${esc(tn('qsr_veces', x.consultas))} · ${esc(t('qsr_ultima').replace('{fecha}', fecha(x.ultima)))}</span>${x.nombres && x.nombres.length ? `<br><small class="muted">${esc(x.nombres.join(', '))}</small>` : ''}</li>`).join('')
          + '</ul>';
      }
      if (r.sin_programa) html += `<p class="muted small">${esc(tn('qsr_sin_programa', r.sin_programa))}</p>`;
      if (r.otros_dispositivos) html += `<p class="muted small">${esc(tn('qsr_otros', r.otros_dispositivos))}</p>`;
    }
    html += `<p class="muted small">${esc(t('qsr_limite'))}</p>`;
    el.innerHTML = html;
  }
  window.GuardianaQueSeRompe = { consultar, pinta, resumen };
})();
