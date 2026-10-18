// «Quién es cada destino» (1.0.45): how much of what left the house GUARDIANA can say who is
// behind, at three levels, each with its source and date; and, for what it cannot, the network
// that served it. Read-only. The owner, 18 Oct 2026: «precisión máxima de todos los datos hacia
// dónde van». Every figure comes from /api/identificacion; nothing here is estimated.
(() => {
  'use strict';
  window.GuardianaIdentificacion = { iniciar };

  async function iniciar(ctx) {
    const { api, t, esc, LOC } = ctx;
    const $ = (id) => document.getElementById(id);
    const num = (n) => Number(n || 0).toLocaleString(LOC);
    let dias = 7;

    const fila = (n) => {
      const id = n.identidad || {};
      const pruebas = (id.pruebas_texto || []).map(esc).join(' · ');
      const infra = id.infraestructura_texto ? `<span class="id-pruebas">${esc(id.infraestructura_texto)}</span>` : '';
      return `<li><span class="id-nombre">${esc(n.qname || t('id_sin_nombre'))}</span>`
        + `<span class="id-frase">${esc(id.frase || '')} · ${esc(t('id_consultas').replace('{n}', num(n.consultas)))}</span>`
        + (pruebas ? `<span class="id-pruebas">${pruebas}</span>` : '') + infra + '</li>';
    };
    const lista = (el, v) => {
      el.innerHTML = v && v.length ? v.map(fila).join('') : `<li class="id-vacio">${esc(t('id_lista_vacia'))}</li>`;
    };

    async function carga() {
      const r = await api('/api/identificacion?dias=' + dias);
      const c = r.cobertura || {};
      $('id-pct').textContent = c.consultas ? `${c.pct_identificada} %` : '—';
      const barra = $('id-barra').children;
      barra[0].style.width = (c.pct_identificada || 0) + '%';
      barra[1].style.width = (c.pct_probable || 0) + '%';
      barra[2].style.width = (c.pct_sin_identificar || 0) + '%';
      $('id-barra').setAttribute('aria-label', r.pct_texto || '');
      const cuenta = (n, p) => esc(t('id_consultas_pct').replace('{n}', num(n)).replace('{pct}', p || 0));
      $('id-n-ok').innerHTML = cuenta(c.identificada, c.pct_identificada);
      $('id-n-prob').innerHTML = cuenta(c.probable, c.pct_probable);
      $('id-n-sin').innerHTML = cuenta(c.sin_identificar, c.pct_sin_identificar);
      lista($('id-probables'), r.probables);
      lista($('id-sin'), r.sin_identificar);
      const e = r.ediciones || {};
      $('id-ediciones').textContent = t('id_ediciones')
        .replace('{mundo}', e.empresas_mundo || '').replace('{redes}', e.redes || '')
        .replace('{listas}', e.listas || '').replace('{base}', e.base_red_fuente || '')
        .replace('{base_fecha}', e.base_red || '');
      document.querySelectorAll('.id-ventana [data-dias]').forEach((b) => b.setAttribute('aria-pressed', String(Number(b.dataset.dias) === dias)));
    }

    document.querySelectorAll('.id-ventana [data-dias]').forEach((b) => b.addEventListener('click', () => {
      dias = Number(b.dataset.dias) || 7;
      carga().catch((e) => console.error(e));
    }));
    await carga();
  }
})();
