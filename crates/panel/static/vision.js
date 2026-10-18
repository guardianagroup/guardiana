// «Visión profunda» (1.0.45). Per program: the companies it talks to, the bytes up and down,
// what went to addresses no name was seen for, the retransmissions, and the «posible
// transferencia» warnings; the antivirus and the AI apps first. Read-only: the page asks
// GUARDIANA, GUARDIANA reads what it already measured. Nothing here is decrypted or captured.
(() => {
  'use strict';
  window.GuardianaVision = { iniciar };

  async function iniciar(ctx) {
    const { api, t, esc, LOC, cadaTanto } = ctx;
    const $ = (id) => document.getElementById(id);
    let dias = 1;
    let ultimo = null;
    let pais = (c) => c;
    try {
      const dn = new Intl.DisplayNames([LOC], { type: 'region' });
      pais = (c) => { try { return dn.of(c) || c; } catch (_) { return c; } };
    } catch (_) {}
    const bytes = (b) => {
      const n = Number(b) || 0;
      const u = ['B', 'KB', 'MB', 'GB', 'TB'];
      let i = 0, v = n;
      while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
      return v.toLocaleString(LOC, { maximumFractionDigits: v >= 10 || i === 0 ? 0 : 1 }) + ' ' + u[i];
    };
    const hora = (ms) => new Date(ms).toLocaleTimeString(LOC, { hour: '2-digit', minute: '2-digit' });
    const dia = (ms) => new Date(ms).toLocaleDateString(LOC, { weekday: 'short', day: 'numeric', month: 'short' });
    const empresa = (e) => {
      if (!e) return t('vp_empresa_desconocida');
      if (e === '@local') return t('vp_empresa_local');
      if (e === '@anycast') return t('vp_empresa_anycast');
      if (e === '@otras') return t('vp_empresa_otras');
      return e;
    };
    const lugar = (p) => (p ? pais(p) : '');
    $('vp-dias').setAttribute('aria-label', t('vp_periodo'));

    const pintaDefender = (d) => {
      const caja = $('vp-defender');
      if (!d || (d.muestras == null && d.maps == null)) { caja.hidden = true; return; }
      caja.hidden = false;
      const fila = (clave, v, directiva, prefijo) => {
        // A value Microsoft does not document is shown as the number Windows gave.
        const texto = v == null ? '' : t(prefijo + v);
        const valor = v == null ? t('vp_defender_sin_dato') : (texto === prefijo + v ? String(v) : texto);
        const marca = directiva ? ` <span class="chip">${esc(t('vp_defender_directiva'))}</span>` : '';
        return `<div><dt>${esc(t(clave))}</dt><dd>${esc(valor)}${marca}</dd></div>`;
      };
      $('vp-defender-valores').innerHTML =
        fila('vp_defender_muestras', d.muestras, d.muestras_directiva, 'vp_defender_muestras_') +
        fila('vp_defender_maps', d.maps, d.maps_directiva, 'vp_defender_maps_');
    };

    const pintaTransferencias = (r) => {
      const boton = $('vp-archivos-boton');
      boton.textContent = r.archivos ? t('vp_archivos_apagar') : t('vp_archivos_encender');
      boton.className = r.archivos ? 'boton' : 'boton principal';
      $('vp-archivos-estado').textContent = r.archivos ? t('vp_archivos_encendido') : t('vp_archivos_apagado');
      const lista = r.transferencias || [];
      $('vp-transferencias').innerHTML = lista.length
        ? lista.map((x) => `<li class="vp-aviso"><span class="chip aviso">${esc(t('vp_posible'))}</span> <span class="vp-cuando">${esc(hora(x.ts))}</span> <span>${esc(x.frase)}</span></li>`).join('')
        : (r.archivos ? `<li class="muted">${esc(t('vp_transferencias_ninguna'))}</li>` : '');
    };

    const barras = (p) => {
      if (!p.por_dia || !p.por_dia.length) return '';
      const max = Math.max(...p.por_dia.map((d) => d.enviados), 1);
      const filas = p.por_dia.map((d) => {
        const alto = Math.max(2, Math.round((d.enviados / max) * 100));
        const fab = d.enviados ? Math.round((d.al_fabricante / d.enviados) * 100) : 0;
        const titulo = `${dia(d.dia)} · ${t('vp_subido')} ${bytes(d.enviados)} · ${t('vp_al_fabricante')} ${bytes(d.al_fabricante)}`;
        return `<li title="${esc(titulo)}"><span class="vp-col vp-h${Math.round(alto / 5) * 5}"><i class="vp-f${Math.round(fab / 5) * 5}"></i></span><small>${esc(dia(d.dia))}</small><small class="muted">${esc(bytes(d.enviados))}</small></li>`;
      }).join('');
      return `<div class="vp-dia"><span class="rotulo">${esc(t('vp_por_dia'))}</span><ol class="vp-columnas">${filas}</ol><p class="muted vp-leyenda"><i class="vp-ley-fab"></i>${esc(t('vp_al_fabricante'))} <i class="vp-ley-resto"></i>${esc(t('vp_a_otros'))}</p></div>`;
    };

    const tipoChip = (p) => {
      if (p.tipo === 'antivirus') return `<span class="chip info">${esc(t('vp_tipo_antivirus'))}${p.producto ? ' · ' + esc(p.producto) : ''}</span>`;
      if (p.tipo === 'ia') return `<span class="chip info">${esc(t('vp_tipo_ia'))}${p.producto ? ' · ' + esc(p.producto) : ''}</span>`;
      if (p.tipo === 'navegador') return `<span class="chip">${esc(t('vp_tipo_navegador'))}</span>`;
      return '';
    };

    const tarjeta = (p) => {
      const empresas = p.empresas.map((e) => `<tr>
        <td>${esc(empresa(e.empresa))}${e.fabricante ? ` <span class="chip ok">${esc(t('vp_su_fabricante'))}</span>` : ''}${e.ia ? ` <span class="chip info">${esc(t('vp_tipo_ia'))}</span>` : ''}</td>
        <td>${esc(lugar(e.pais))}</td>
        <td class="num">${esc(bytes(e.enviados))}</td>
        <td class="num">${esc(bytes(e.recibidos))}</td>
        <td class="num">${e.sin_dns ? `<span class="chip aviso">${esc(bytes(e.sin_dns))}</span>` : '<span class="muted">—</span>'}</td>
      </tr>`).join('');
      const mas = p.n_empresas > p.empresas.length ? `<p class="muted">${esc(t('vp_mas_empresas').replace('{n}', p.n_empresas - p.empresas.length))}</p>` : '';
      const ia = p.ia && p.ia.length ? `<p class="vp-ia"><span class="rotulo">${esc(t('vp_ia_web'))}</span> ${p.ia.map((e) => `<span class="chip info">${esc(e.empresa)} · ↑ ${esc(bytes(e.enviados))}</span>`).join(' ')}</p>` : '';
      const fab = p.tipo === 'antivirus' || p.tipo === 'ia'
        ? `<div><dt>${esc(t('vp_al_fabricante'))}</dt><dd>${esc(bytes(p.al_fabricante))}</dd></div>` : '';
      const aviso = p.transferencias ? ` <span class="chip aviso">${esc(t('vp_posibles_n').replace('{n}', p.transferencias))}</span>` : '';
      return `<article class="tarjeta vp-programa">
        <header><h3>${esc(p.programa)}</h3> ${tipoChip(p)}${aviso}</header>
        <dl class="vp-mini">
          <div><dt>${esc(t('vp_subido'))}</dt><dd>${esc(bytes(p.enviados))}</dd></div>
          <div><dt>${esc(t('vp_bajado'))}</dt><dd>${esc(bytes(p.recibidos))}</dd></div>
          ${fab}
          <div><dt>${esc(t('vp_sin_dns'))}</dt><dd>${esc(bytes(p.sin_dns))}</dd></div>
          <div><dt>${esc(t('vp_reenviado'))}</dt><dd>${esc(bytes(p.reenviados))}</dd></div>
          <div><dt>${esc(t('vp_conexiones'))}</dt><dd>${esc(Number(p.conexiones).toLocaleString(LOC))}</dd></div>
        </dl>
        ${barras(p)}
        ${ia}
        <div class="vp-tabla"><table>
          <thead><tr><th>${esc(t('vp_col_empresa'))}</th><th>${esc(t('vp_col_pais'))}</th><th class="num">↑ ${esc(t('vp_subido'))}</th><th class="num">↓ ${esc(t('vp_bajado'))}</th><th class="num">${esc(t('vp_sin_dns'))}</th></tr></thead>
          <tbody>${empresas}</tbody>
        </table></div>
        ${mas}
      </article>`;
    };

    const pinta = (r) => {
      ultimo = r;
      if (!r.disponible) {
        $('vp-no').hidden = false;
        $('vp-todo').hidden = true;
        return;
      }
      $('vp-no').hidden = true;
      $('vp-todo').hidden = false;
      document.querySelectorAll('#vp-dias [data-dias]').forEach((b) => {
        const actual = Number(b.dataset.dias) === dias;
        b.className = 'boton chico' + (actual ? ' principal' : '');
        b.setAttribute('aria-pressed', actual ? 'true' : 'false');
      });
      $('vp-subido').textContent = bytes(r.enviados);
      $('vp-bajado').textContent = bytes(r.recibidos);
      $('vp-sin-dns').textContent = bytes(r.sin_dns);
      $('vp-reenviado').textContent = bytes(r.reenviados);
      $('vp-vivo').textContent = t('vp_actualizado').replace('{hora}', hora(Date.now())).replace('{h}', r.horas);
      const c = r.cadena || {};
      $('vp-cadena').innerHTML = c.filas
        ? (c.ok
          ? `<span class="chip ok">${esc(t('vp_cadena_ok'))}</span> <span class="muted">${esc(t('vp_cadena_filas').replace('{n}', c.filas))}</span>`
          : `<span class="chip alerta">${esc(t('vp_cadena_rota'))}</span>`)
        : `<span class="muted">${esc(t('vp_cadena_vacia'))}</span>`;
      pintaTransferencias(r);
      pintaDefender(r.defender);
      $('vp-programas').innerHTML = r.programas.length
        ? r.programas.map(tarjeta).join('')
        : `<p class="muted">${esc(t('vp_sin_programas'))}</p>`;
    };

    const cargar = async () => pinta(await api('/api/vision?dias=' + dias));
    document.querySelectorAll('#vp-dias [data-dias]').forEach((b) => b.addEventListener('click', () => {
      dias = Number(b.dataset.dias) || 1;
      cargar().catch(() => {});
    }));
    $('vp-archivos-boton').addEventListener('click', async () => {
      const activa = !(ultimo && ultimo.archivos);
      if (!activa && !window.confirm(t('vp_archivos_confirma_apagar'))) return;
      await api('/api/vision/archivos', { method: 'POST', body: { activa } });
      await cargar();
    });
    await cargar();
    cadaTanto(cargar, 10000);
  }
})();
