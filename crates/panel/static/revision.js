// «Revisión del ordenador» (1.0.24). What Windows says about this computer, as checks with a
// traffic light, a sentence and the list behind it; whatever appeared since the first review is
// marked «nuevo». Read-only: the page asks GUARDIANA, GUARDIANA asks Windows, nothing changes.
// It can be kept as a PDF, exported as data and shared, like everything in GUARDIANA.
(() => {
  'use strict';
  window.GuardianaRevision = { iniciar };
  const FUERTES = new Set(['nuevo', 'sin_firma', 'firma_mala', 'no_existe', 'sin_clave', 'invitado', 'delicado', 'redirigido', 'apagado', 'sin_actualizar', 'en_uso', 'detectada', 'actuo']);

  async function iniciar(ctx) {
    const { api, t, esc, LOC } = ctx;
    const $ = (id) => document.getElementById(id);
    let ultimo = null;
    const fecha = (ms) => (ms ? new Date(ms).toLocaleString(LOC, { dateStyle: 'medium', timeStyle: 'short' }) : '');
    const rellena = (k, datos) => Object.entries(datos || {}).reduce((s, [a, b]) => s.split('{' + a + '}').join(b), t(k));
    const frase = (c) => rellena(c.frase, c.datos);
    const marca = (m) => `<span class="rv-marca${FUERTES.has(m) ? ' fuerte' : ''}">${esc(t('rv_marca_' + m))}</span>`;

    const pinta = (r) => {
      ultimo = r;
      if (!r.disponible) { $('rv-semaforo').textContent = t('rv_no_windows'); $('rv-ahora').disabled = true; return; }
      const s = r.resumen;
      $('rv-semaforo').innerHTML = [['alerta', s.alerta], ['aviso', s.aviso], ['ok', s.ok], ['info', s.info]]
        .map(([k, n]) => `<span><span class="rv-punto ${k}"></span> <b>${n}</b>${esc(t('rv_resumen_' + k))}</span>`).join('')
        + (s.nuevos ? `<span><b>${s.nuevos}</b>${esc(t('rv_resumen_nuevos'))}</span>` : '');
      $('rv-cuando').textContent = r.corriendo ? t('rv_revisando') : r.cuando ? t('rv_cuando').replace('{fecha}', fecha(r.cuando)) + (r.desde ? ' · ' + t('rv_desde').replace('{fecha}', fecha(r.desde)) : '') : t('rv_aun_no');
      $('rv-msg').textContent = r.error ? t('rv_error').replace('{e}', r.error === 'no_windows' ? t('rv_no_windows') : r.error) : '';
      $('rv-ahora').disabled = !!r.corriendo;
      const e = r.equipo;
      $('rv-equipo').textContent = e && e.sistema ? [e.fabricante, e.modelo, e.sistema + (e.display ? ' ' + e.display : ''), e.compilacion ? 'build ' + e.compilacion + (e.ubr ? '.' + e.ubr : '') : ''].filter(Boolean).join(' · ') : '';
      $('rv-grupos').innerHTML = r.grupos.map((g) => `<section class="rv-grupo"><h2>${esc(t('rv_g_' + g.id))}</h2>${g.checks.map((c) => {
        const lista = c.lista.length ? `<details${c.lista.some((i) => i.marcas.includes('nuevo')) ? ' open' : ''}><summary>${esc(t('rv_ver_lista').replace('{n}', c.lista.length))}</summary><ul>${c.lista.map((i) =>
          `<li>${esc(i.t || '—')}${i.marcas.map(marca).join('')}${i.d || i.cuando ? `<div class="muted">${esc([i.d, i.cuando ? fecha(i.cuando) : ''].filter(Boolean).join(' · '))}</div>` : ''}</li>`).join('')}</ul></details>` : '';
        return `<div class="card rv-check ${c.estado}"><div class="rv-t"><span class="rv-punto ${c.estado}"></span>${esc(t('rv_t_' + c.id))}</div><div>${esc(frase(c))}</div>${lista}</div>`;
      }).join('')}</section>`).join('');
    };

    const carga = async () => { const r = await api('/api/revision'); pinta(r); return r; };
    // While Windows is answering (tens of seconds), ask again every 4 s; then every 5 minutes.
    let espera = null;
    const sigue = (r) => { clearTimeout(espera); espera = setTimeout(async () => { try { sigue(await carga()); } catch (_) {} }, r && r.corriendo ? 4000 : 300000); };
    sigue(await carga());
    $('rv-ahora').addEventListener('click', async () => { $('rv-ahora').disabled = true; try { const r = await api('/api/revision/ahora', { method: 'POST' }); pinta(r); sigue(r); } catch (e) { $('rv-msg').textContent = String(e.message || e); } });

    // ---- Keep it and share it
    const nombre = (ext) => `guardiana-revision-${new Date().toISOString().slice(0, 10)}.${ext}`;
    const texto = () => {
      const r = ultimo; if (!r) return '';
      const l = [t('rv_titulo') + ' · ' + fecha(r.cuando), $('rv-equipo').textContent, ''];
      r.grupos.forEach((g) => { l.push('== ' + t('rv_g_' + g.id)); g.checks.forEach((c) => { l.push(`[${t('rv_resumen_' + (c.estado === 'nd' ? 'info' : c.estado)).trim()}] ${t('rv_t_' + c.id)}: ${frase(c)}`); c.lista.forEach((i) => l.push('   · ' + i.t + (i.marcas.length ? ' (' + i.marcas.map((m) => t('rv_marca_' + m)).join(', ') + ')' : ''))); }); l.push(''); });
      return l.join('\n');
    };
    $('rv-json').addEventListener('click', () => {
      if (!ultimo) return;
      const a = document.createElement('a');
      a.href = URL.createObjectURL(new Blob([JSON.stringify({ guardiana: 'revision', version: 1, cuando: ultimo.cuando, equipo: ultimo.equipo, resumen: ultimo.resumen, grupos: ultimo.grupos }, null, 2)], { type: 'application/json' }));
      a.download = nombre('json'); document.body.appendChild(a); a.click(); a.remove();
    });
    $('rv-compartir').addEventListener('click', async () => {
      const x = texto(); if (!x) return;
      try { if (navigator.share) { await navigator.share({ title: t('rv_titulo'), text: x }); return; } } catch (e) { if (e && e.name === 'AbortError') return; }
      try { await navigator.clipboard.writeText(x); $('rv-msg').textContent = t('rv_copiado'); } catch (_) { $('rv-msg').textContent = t('rv_no_copiado'); }
    });
    $('rv-pdf').addEventListener('click', async () => {
      const r = ultimo; if (!r || !ctx.pdf) return;
      const doc = ctx.pdf.pdfDocument(t('rv_titulo'));
      doc.line('GUARDIANA · ' + t('rv_titulo'), 16, true).gap(3).line(t('rv_cuando').replace('{fecha}', fecha(r.cuando)), 9, false, 0.45);
      if ($('rv-equipo').textContent) doc.line($('rv-equipo').textContent, 9, false, 0.45);
      const s = r.resumen;
      doc.gap(4).line(['alerta', 'aviso', 'ok', 'info'].map((k) => s[k] + t('rv_resumen_' + k)).join('   ') + (s.nuevos ? '   ' + s.nuevos + t('rv_resumen_nuevos') : ''), 10, true).regla();
      r.grupos.forEach((g) => {
        doc.seccion(t('rv_g_' + g.id));
        g.checks.forEach((c) => {
          doc.juntos(40).line(`${t('rv_resumen_' + (c.estado === 'nd' ? 'info' : c.estado)).trim().toUpperCase()} · ${t('rv_t_' + c.id)}`, 10, true).line(frase(c), 9, false, 0.25);
          c.lista.slice(0, 60).forEach((i) => doc.line('   · ' + i.t + (i.marcas.length ? ' (' + i.marcas.map((m) => t('rv_marca_' + m)).join(', ') + ')' : '') + (i.cuando ? ' · ' + fecha(i.cuando) : ''), 8, false, 0.35));
          if (c.lista.length > 60) doc.line('   … ' + (c.lista.length - 60), 8, false, 0.35);
          doc.gap(5);
        });
      });
      doc.gap(8).line(t('rv_pie'), 8, false, 0.45);
      const res = await ctx.pdf.savePdfFile(nombre('pdf'), doc.bytes(t('rv_pdf_pie')));
      $('rv-msg').textContent = res === 'cancelado' ? '' : t('rv_pdf_hecho');
    });
  }
})();
