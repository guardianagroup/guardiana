// «Revisión del ordenador» (1.0.24). What Windows says about this computer, as checks with a
// traffic light, a sentence and the list behind it; whatever appeared since the first review is
// marked «nuevo». Read-only: the page asks GUARDIANA, GUARDIANA asks Windows, nothing changes.
// It can be kept as a PDF, exported as data and shared, like everything in GUARDIANA.
(() => {
  'use strict';
  window.GuardianaRevision = { iniciar };
  const FUERTES = new Set(['nuevo', 'sin_firma', 'firma_mala', 'no_existe', 'sin_clave', 'invitado', 'delicado', 'redirigido', 'apagado', 'sin_actualizar', 'en_uso', 'detectada', 'actuo']);
  const GRUPOS = ['proteccion', 'acceso', 'arranque', 'salida', 'privacidad', 'antivirus'];
  // Which sections of revision.ps1 feed each group (the script's own order).
  const SECCIONES = { proteccion: ['equipo', 'actualizaciones', 'defensa', 'sistema'], acceso: ['acceso'], arranque: ['arranque'], salida: ['salida', 'certificados'], privacidad: ['privacidad', 'programas'], antivirus: ['defender_sucesos'] };
  const TOTAL_SECCIONES = 11;

  async function iniciar(ctx) {
    const { api, t, esc, LOC } = ctx;
    const $ = (id) => document.getElementById(id);
    let ultimo = null;
    const fecha = (ms) => (ms ? new Date(ms).toLocaleString(LOC, { dateStyle: 'medium', timeStyle: 'short' }) : '');
    const rellena = (k, datos) => Object.entries(datos || {}).reduce((s, [a, b]) => s.split('{' + a + '}').join(b), t(k));
    const frase = (c) => rellena(c.frase, c.datos);
    const conclusion = (r) => {
      const s = r.resumen || {};
      const clave = s.alerta ? 'rv_conclusion_alerta' : s.aviso ? 'rv_conclusion_aviso' : 'rv_conclusion_ok';
      return rellena(clave, { alerta: s.alerta || 0, aviso: s.aviso || 0, ok: s.ok || 0, info: s.info || 0 });
    };
    const marca = (m) => `<span class="rv-marca${FUERTES.has(m) ? ' fuerte' : ''}">${esc(t('rv_marca_' + m))}</span>`;
    let empezo = 0, reloj = null;
    const ordenEstado = { alerta: 4, aviso: 3, info: 2, nd: 2, ok: 1 };
    const progreso = (activo) => {
      $('rv-progreso').hidden = !activo;
      $('rv-escena').setAttribute('aria-busy', activo ? 'true' : 'false');
      clearInterval(reloj); reloj = null;
      if (!activo) { empezo = 0; return; }
      if (!empezo) empezo = Date.now();
      const actualiza = () => { $('rv-tiempo').textContent = t('rv_progreso_tiempo').replace('{n}', Math.max(1, Math.round((Date.now() - empezo) / 1000))); };
      actualiza(); reloj = setInterval(actualiza, 1000);
    };
    const pintaEscena = (r) => {
      const recibidos = Array.isArray(r.grupos) ? r.grupos : [];
      const grupos = recibidos.length ? recibidos : GRUPOS.map((id) => ({ id, checks: [] }));
      $('rv-escena').classList.toggle('trabajando', !!r.corriendo);
      $('rv-pasos').innerHTML = grupos.map((g) => {
        const checks = Array.isArray(g.checks) ? g.checks : [];
        const peor = checks.reduce((a, c) => (ordenEstado[c.estado] || 0) > (ordenEstado[a] || 0) ? c.estado : a, 'ok');
        // While it runs, each group follows the script's own sections (1.0.31): done, now, waiting.
        const v = r.vivo, secs = SECCIONES[g.id] || [];
        const hechas = v ? secs.filter((x) => v.hechas.some((h) => h[0] === x)) : [];
        const ahora = v && secs.includes(v.seccion) && hechas.length < secs.length;
        const listo = v && secs.length && hechas.length === secs.length;
        const ms = v ? v.hechas.filter((h) => secs.includes(h[0])).reduce((a, h) => a + h[1], 0) : 0;
        const estado = r.corriendo ? (listo ? 'listo' : ahora ? 'revisando' : v ? 'espera' : 'revisando') : (peor === 'nd' ? 'info' : peor);
        const detalle = r.corriendo ? (listo ? t('rv_paso_listo').replace('{s}', (ms / 1000).toLocaleString(LOC, { maximumFractionDigits: 1 })) : ahora ? t('rv_paso_revisando') : v ? t('rv_paso_espera') : t('rv_paso_revisando')) : t('rv_paso_comprobaciones').replace('{n}', checks.length);
        const nombre = t('rv_g_' + g.id);
        return `<div class="rv-paso ${estado}" title="${esc(nombre + ' · ' + detalle)}"><i aria-hidden="true"></i><span><b>${esc(nombre)}</b><small>${esc(detalle)}</small></span></div>`;
      }).join('');
      progreso(!!r.corriendo);
      pintaVivo(r);
    };
    // The live feed: the script's real steps, newest at the bottom (1.0.31). Nothing invented.
    const lineaVivo = ([tipo, texto]) => {
      if (tipo === 'seccion') return t('rv_vivo_seccion').replace('{x}', t('rv_vivo_s_' + texto));
      if (tipo === 'firma') return t('rv_vivo_firma').replace('{x}', texto);
      if (tipo === 'dns') return t('rv_vivo_dns').replace('{x}', texto);
      if (tipo === 'cuenta') { const [k, n] = texto.split('\t'); return t('rv_vivo_cuenta_' + k).replace('{n}', n); }
      return texto;
    };
    const pintaVivo = (r) => {
      const v = r.corriendo ? r.vivo : null;
      $('rv-vivo').hidden = !v;
      if (!v) return;
      $('rv-vivo-cifras').textContent = t('rv_vivo_cifras').replace('{s}', v.hechas.length).replace('{t}', TOTAL_SECCIONES).replace('{f}', v.firmas);
      $('rv-vivo-lista').innerHTML = v.pasos.map((p, i) => `<li class="${esc(p[0])}${i === v.pasos.length - 1 ? ' ultimo' : ''}">${esc(lineaVivo(p))}</li>`).join('');
    };

    const pinta = (r) => {
      ultimo = r;
      if (!r.disponible) { $('rv-semaforo').textContent = t('rv_no_windows'); $('rv-ahora').disabled = true; return; }
      pintaEscena(r);
      const s = r.resumen;
      const deEstado = (k) => (r.grupos || []).flatMap((g) => (g.checks || []).filter((c) => (c.estado === 'nd' ? 'info' : c.estado) === k));
      const pista = (cs) => {
        if (!cs.length) return '';
        const nombres = cs.slice(0, 8).map((c) => t('rv_t_' + c.id));
        return nombres.join(' · ') + (cs.length > nombres.length ? ' · +' + (cs.length - nombres.length) : '');
      };
      const conPista = (html, texto) => texto ? html.replace('<span>', `<span title="${esc(texto)}">`) : html;
      $('rv-semaforo').innerHTML = [['alerta', s.alerta], ['aviso', s.aviso], ['ok', s.ok], ['info', s.info]]
        .map(([k, n]) => conPista(`<span><span class="rv-punto ${k}"></span> <b>${n}</b>${esc(t('rv_resumen_' + k))}</span>`, pista(deEstado(k)))).join('')
        + (s.nuevos ? conPista(`<span><b>${s.nuevos}</b>${esc(t('rv_resumen_nuevos'))}</span>`, pista((r.grupos || []).flatMap((g) => (g.checks || []).filter((c) => (c.lista || []).some((i) => i.marcas.includes('nuevo')))))) : '');
      $('rv-cuando').textContent = r.corriendo ? t('rv_revisando') : r.cuando ? t('rv_cuando').replace('{fecha}', fecha(r.cuando)) + (r.desde ? ' · ' + t('rv_desde').replace('{fecha}', fecha(r.desde)) : '') : t('rv_aun_no');
      $('rv-msg').textContent = r.error ? t('rv_error').replace('{e}', r.error === 'no_windows' ? t('rv_no_windows') : r.error) : '';
      $('rv-ahora').disabled = !!r.corriendo;
      $('rv-ahora').textContent = t(r.corriendo ? 'rv_boton_revisando' : 'rv_ahora');
      const e = r.equipo;
      $('rv-equipo').textContent = e && e.sistema ? [e.fabricante, e.modelo, e.sistema + (e.display ? ' ' + e.display : ''), e.compilacion ? 'build ' + e.compilacion + (e.ubr ? '.' + e.ubr : '') : ''].filter(Boolean).join(' · ') : '';
      $('rv-grupos').innerHTML = r.grupos.map((g) => `<section class="rv-grupo"><h2>${esc(t('rv_g_' + g.id))}</h2>${g.checks.map((c) => {
        const fila = (i) => {
          const texto = [i.t, i.d, i.marcas.map((m) => t('rv_marca_' + m)).join(', '), i.cuando ? fecha(i.cuando) : ''].filter(Boolean).join(' · ');
          return `<li title="${esc(texto)}">${esc(i.t || '—')}${i.marcas.map(marca).join('')}${i.d || i.cuando ? `<div class="muted">${esc([i.d, i.cuando ? fecha(i.cuando) : ''].filter(Boolean).join(' · '))}</div>` : ''}</li>`;
        };
        const lista = !c.lista.length ? '' : c.lista.length <= 2
          ? `<ul>${c.lista.map(fila).join('')}</ul>`
          : `<details${c.lista.some((i) => i.marcas.includes('nuevo')) ? ' open' : ''}><summary title="${esc(c.lista.slice(0, 8).map((i) => i.t || '—').join(' · '))}">${esc(t('rv_ver_lista').replace('{n}', c.lista.length))}</summary><ul>${c.lista.map(fila).join('')}</ul></details>`;
        return `<div class="card rv-check ${c.estado}"><div class="rv-t"><span class="rv-punto ${c.estado}"></span>${esc(t('rv_t_' + c.id))}</div><div>${esc(frase(c))}</div>${lista}</div>`;
      }).join('')}</section>`).join('');
      $('rv-conclusion').hidden = !!r.corriendo || !r.cuando;
      $('rv-conclusion-texto').textContent = r.corriendo || !r.cuando ? '' : conclusion(r);
    };

    const carga = async () => { const r = await api('/api/revision'); pinta(r); return r; };
    // While Windows is answering (tens of seconds), ask again every 4 s; then every 5 minutes.
    let espera = null;
    const sigue = (r) => { clearTimeout(espera); espera = setTimeout(async () => { try { sigue(await carga()); } catch (_) {} }, r && r.corriendo ? 1500 : 300000); };
    sigue(await carga());
    $('rv-ahora').addEventListener('click', async () => {
      $('rv-ahora').disabled = true; $('rv-ahora').textContent = t('rv_boton_revisando'); empezo = Date.now(); progreso(true);
      try { const r = await api('/api/revision/ahora', { method: 'POST' }); pinta(r); sigue(r); }
      catch (e) { progreso(false); $('rv-ahora').disabled = false; $('rv-msg').textContent = String(e.message || e); }
    });

    // ---- Keep it and share it
    const nombre = (ext) => `guardiana-revision-${new Date().toISOString().slice(0, 10)}.${ext}`;
    const texto = () => {
      const r = ultimo; if (!r) return '';
      const l = [t('rv_titulo') + ' · ' + fecha(r.cuando), $('rv-equipo').textContent, ''];
      r.grupos.forEach((g) => { l.push('== ' + t('rv_g_' + g.id)); g.checks.forEach((c) => { l.push(`[${t('rv_resumen_' + (c.estado === 'nd' ? 'info' : c.estado)).trim()}] ${t('rv_t_' + c.id)}: ${frase(c)}`); c.lista.forEach((i) => l.push('   · ' + i.t + (i.marcas.length ? ' (' + i.marcas.map((m) => t('rv_marca_' + m)).join(', ') + ')' : ''))); }); l.push(''); });
      l.push('== ' + t('rv_conclusion_titulo'), conclusion(r), '');
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
      doc.seccion(t('rv_conclusion_titulo')).line(conclusion(r), 10, false, 0.45);
      doc.gap(8).line(t('rv_pie'), 8, false, 0.45);
      const res = await ctx.pdf.savePdfFile(nombre('pdf'), doc.bytes(t('rv_pdf_pie')));
      $('rv-msg').textContent = res === 'cancelado' ? '' : t('rv_pdf_hecho');
    });
  }
})();
