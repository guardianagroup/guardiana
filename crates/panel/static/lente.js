// GUARDIANA 1.0.46 — «Lente», the interior page pattern (mockup R1-d, chosen by the owner on
// 18 Oct 2026). The markup lives in each page (see the pattern note at the top of lente.css);
// this script only fills what the pattern adds on top of each page's own script:
//   - the four tiles, from the same answers the page already asks for (it listens to the page's
//     own requests, so nothing is asked twice and app.js is not touched), with the thousands
//     separator of the panel's language, and never a zero before the data is in;
//   - the line of state and the actions of the «Qué significa» card;
//   - the on/off settings drawn as the standard switch (Protección máxima, Modo Hogar, «vigila a
//     mi antivirus»), driving the page's own controls, which stay in the page, hidden.
// Loaded before app.js. Nothing here is sent anywhere; it only reads what the page received.
(() => {
  'use strict';
  // The panel's language: app.js sets it on <html lang> before it asks for anything.
  const lang = () => {
    const l = (document.documentElement.lang || 'en').slice(0, 2);
    return l === 'es' || l === 'pt' ? l : 'en';
  };
  const loc = () => ({ en: 'en-US', es: 'es-ES', pt: 'pt-BR' }[lang()]);
  let T = null;
  const textos = () => {
    if (T) return T;
    try { const raw = localStorage.getItem('guardiana_textos_' + lang()); if (raw) T = JSON.parse(raw); } catch (_) {}
    return T;
  };
  const t = (k) => { const x = textos(); return (x && x.panel && x.panel[k]) || ''; };
  // Spanish groups thousands from 1.000 on, not only from 10.000 (the browser's default for «es»
  // leaves «1234»): the owner reads «1.234».
  const num = (n) => (typeof n === 'number' && isFinite(n) ? n.toLocaleString(loc(), { useGrouping: 'always' }) : String(n ?? ''));
  const tn = (k, n) => (n === 1 && t(k + '_uno') ? t(k + '_uno') : t(k)).replace('{n}', num(n));
  const $ = (id) => document.getElementById(id);
  const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
  const bytes = (b) => {
    const u = ['B', 'KB', 'MB', 'GB', 'TB'];
    let i = 0; let v = Number(b) || 0;
    while (v >= 1000 && i < u.length - 1) { v /= 1000; i += 1; }
    return v.toLocaleString(loc(), { maximumFractionDigits: v < 10 && i ? 1 : 0 }) + ' ' + u[i];
  };
  const pagina = () => document.body && document.body.getAttribute('data-page');
  const hora = (ms) => new Date(ms).toLocaleTimeString(loc(), { hour: '2-digit', minute: '2-digit' });
  const fecha = (ms) => new Date(ms).toLocaleDateString(loc(), { day: 'numeric', month: 'short', year: 'numeric' });
  const DIA = 86400000;

  // ---- tiles ------------------------------------------------------------------------------
  // cifra(el | id, value, tono): a number is written with the locale's grouping; text as it is;
  // null leaves the quiet bar (no data yet). tono: '' | 'aviso' | 'alerta' | 'ok' | 'chico'.
  function cifra(el, valor, tono) {
    const e = typeof el === 'string' ? $(el) : el;
    if (!e) return;
    e.className = 'n' + (tono ? ' ' + tono : '');
    if (valor === null || valor === undefined) { e.textContent = ''; return; }
    e.textContent = typeof valor === 'number' ? num(valor) : String(valor);
  }
  // The tiles by their position (1..4) inside the page's .lente-cifras.
  function cifras(lista) {
    const cajas = document.querySelectorAll('.lente-cifras .lente-cifra');
    lista.forEach((c, i) => {
      const caja = cajas[i];
      if (!caja || !c) return;
      cifra(caja.querySelector('.n'), c.n, c.tono || '');
      if (c.l !== undefined) { const l = caja.querySelector('.l'); if (l) l.textContent = c.l; }
    });
  }
  // A page's own counters (app.js and the page scripts write them): written again with the
  // grouping; a dash or a lone zero written before the data stays as the quiet bar.
  function agrupa(raiz) {
    (raiz || document).querySelectorAll('.lente-cifra .n[id]').forEach((n) => {
      if (n.dataset.lenteAgrupa) return;
      n.dataset.lenteAgrupa = '1';
      const arregla = () => {
        const s = n.textContent.trim();
        if (/^\d{4,}$/.test(s)) n.textContent = num(Number(s));
        else if (s === '–' || s === '-') n.textContent = '';
      };
      new MutationObserver(arregla).observe(n, { childList: true, characterData: true, subtree: true });
      arregla();
    });
  }
  const sinDatos = () => [{ n: '—', tono: 'chico' }, { n: '—', tono: 'chico' }, { n: '—', tono: 'chico' }, { n: '—', tono: 'chico' }];
  function estado(texto, tono) {
    const e = document.querySelector('.lente-estado');
    if (!e) return;
    e.hidden = !texto;
    e.className = 'lente-estado' + (tono ? ' ' + tono : '');
    e.textContent = texto || '';
  }
  function pie(texto) { const p = document.querySelector('.lente-pie'); if (p) p.textContent = texto || ''; }

  // ---- the page's own answers ---------------------------------------------------------------
  const oyentes = [];
  function escucha(ruta, fn) { oyentes.push({ ruta, fn }); }
  const fetchOriginal = window.fetch;
  window.fetch = function (entrada, opciones) {
    const p = fetchOriginal.apply(this, arguments);
    let ruta = '';
    try { ruta = new URL(typeof entrada === 'string' ? entrada : entrada.url, location.href).pathname; } catch (_) {}
    const metodo = String((opciones && opciones.method) || 'GET').toUpperCase();
    const quien = oyentes.filter((o) => o.ruta === ruta);
    if (ruta === '/api/textos' || quien.length) {
      p.then((r) => {
        if (!r) return;
        if (!r.ok) {
          if (r.status === 402) quien.forEach((o) => { try { o.fn(null, { plus: true }); } catch (_) {} });
          return;
        }
        r.clone().json().then((j) => {
          if (ruta === '/api/textos') { T = j; return; }
          quien.forEach((o) => { if (metodo === 'GET' || o.fn.tambienPost) { try { o.fn(j, { metodo }); } catch (e) { console.error(e); } } });
        }).catch(() => {});
      }).catch(() => {});
    }
    return p;
  };

  // ---- the actions of the «Qué significa» card ----------------------------------------------
  // data-lente-pulsa="id": presses that control of the page (kept, hidden, where it was).
  // data-lente-envia="form": submits that form. data-lente-abre="id": opens and shows that part.
  document.addEventListener('click', (ev) => {
    const a = ev.target.closest && ev.target.closest('[data-lente-pulsa], [data-lente-envia], [data-lente-abre]');
    if (!a) return;
    if (a.hasAttribute('data-lente-pulsa')) {
      const d = $(a.getAttribute('data-lente-pulsa'));
      if (!d) return;
      ev.preventDefault();
      d.click();
    } else if (a.hasAttribute('data-lente-envia')) {
      const f = $(a.getAttribute('data-lente-envia'));
      if (!f) return;
      ev.preventDefault();
      if (f.requestSubmit) f.requestSubmit(); else f.submit();
    } else {
      const d = $(a.getAttribute('data-lente-abre'));
      if (!d) return;
      ev.preventDefault();
      if (d.tagName === 'DETAILS') d.open = true;
      d.scrollIntoView({ behavior: 'smooth', block: 'start' });
      const campo = d.querySelector('input:not([type=hidden]):not([type=checkbox]):not([type=radio]), select, textarea, button');
      if (campo) setTimeout(() => campo.focus({ preventScroll: true }), 250);
    }
  });

  // ---- an on/off setting drawn as the standard switch, driving the page's own control --------
  // o: { caja, encendido(): bool | null, cambia(quiere), fallo(), observa: [el], on, off, etiqueta }
  function interruptorDe(o) {
    // interruptor.js comes after this script: wait for it.
    if (!window.GuardianaInterruptor && document.readyState === 'loading') {
      let pinta = null;
      document.addEventListener('DOMContentLoaded', () => { pinta = interruptorDe(o); }, { once: true });
      return () => { if (pinta) pinta(); };
    }
    const caja = typeof o.caja === 'string' ? $(o.caja) : o.caja;
    if (!caja || !window.GuardianaInterruptor) return null;
    let ocupado = false;
    const pinta = () => {
      if (ocupado) return;
      const on = o.encendido();
      if (on === null) { caja.innerHTML = ''; return; }
      const b = caja.querySelector('button.interruptor');
      if (b && b.getAttribute('aria-checked') === String(on) && b.dataset.lang === lang()) return;
      caja.innerHTML = window.GuardianaInterruptor.html({ cortado: !on, texto: { permitido: t(o.on) || 'On', cortado: t(o.off) || 'Off', cortando: t(o.off) || 'Off', permitiendo: t(o.on) || 'On' }, etiqueta: t(o.etiqueta) });
      const c = caja.querySelector('.interruptor-caja');
      if (c) c.classList.add('ajuste');
      const nb = caja.querySelector('button.interruptor');
      if (nb) nb.dataset.lang = lang();
    };
    caja.addEventListener('click', (ev) => {
      const b = ev.target.closest('button.interruptor');
      if (!b || b.disabled) return;
      const antes = o.encendido();
      if (antes === null) return;
      ocupado = true;
      b.disabled = true; b.setAttribute('aria-busy', 'true');
      b.setAttribute('aria-checked', String(!antes));
      const fin = Date.now() + 15000;
      const espera = () => {
        if (o.encendido() !== antes || Date.now() > fin || (o.fallo && o.fallo())) { ocupado = false; pinta(); return; }
        setTimeout(espera, 150);
      };
      o.cambia(!antes);
      setTimeout(espera, 150);
    });
    (o.observa || []).forEach((el) => { if (el) new MutationObserver(pinta).observe(el, { attributes: true, childList: true, subtree: true, characterData: true }); });
    pinta();
    return pinta;
  }

  // Protección máxima: the standard switch over the page's own two buttons (app.js keeps them).
  function maxima() {
    interruptorDe({
      caja: 'lente-maxima-sw', on: 'lente_activada', off: 'lente_desactivada', etiqueta: 'maxima_titulo',
      encendido: () => {
        const m = $('maxima'); const on = $('maxima-activa');
        if (!m || m.classList.contains('hidden') || !on) return null;
        const act = $('maxima-activar'); const quit = $('maxima-quitar');
        // Neither button offered yet: app.js has not painted the state.
        if (act && quit && act.classList.contains('hidden') && quit.classList.contains('hidden') && on.classList.contains('hidden')) return null;
        return !on.classList.contains('hidden');
      },
      cambia: (quiere) => $(quiere ? 'maxima-activar' : 'maxima-quitar').click(),
      fallo: () => { const m = $('maxima-msg'); return !!(m && m.classList.contains('bad') && m.textContent); },
      observa: [$('maxima'), $('maxima-activa'), $('maxima-activar'), $('maxima-quitar')],
    });
  }

  // ---- per page ---------------------------------------------------------------------------
  const PAGINAS = {
    radiografia() {
      escucha('/api/radiografia', (r) => {
        if (!r) return;
        const rp = (r.rastreadores || 0) + (r.publicidad || 0);
        cifras([{ n: r.servicios }, { n: rp }, { n: r.destinos_nuevos, tono: r.destinos_nuevos ? 'aviso' : '' }, { n: r.cortados, tono: r.cortados ? 'alerta' : '' }]);
        // The figures add up (R7): every name of the minute counts in one row, and «Sin
        // clasificar» is the rest. A name the lists moved in the window counts in both; the
        // total is the distinct names.
        const filas = [['lente_rastreadores', r.rastreadores], ['lente_publicidad', r.publicidad], ['lente_telemetria', r.telemetria || 0], ['lente_normales', r.esperados], ['lente_sin_clasificar', r.sin_clasificar || 0]];
        const d = $('lente-desglose');
        if (d) {
          const total = r.servicios || 0;
          d.innerHTML = total
            ? `<table class="lente-desglose"><tbody>${filas.map(([k, v]) => `<tr><td>${esc(t(k))}</td><td class="lente-num">${num(v || 0)}</td><td class="lente-barra-c"><span class="lente-barra"><i data-p="${Math.round(100 * (v || 0) / total)}"></i></span></td></tr>`).join('')}<tr class="lente-total"><td>${esc(t('lente_total_nombres'))}</td><td class="lente-num">${num(total)}</td><td></td></tr></tbody></table>`
            : `<div class="lente-vacio"><p>${esc(t('lente_foto_vacio'))}</p><a class="boton" href="/por-dentro">${esc(t('lente_foto_ir_globo'))}</a></div>`;
          // Widths through the CSSOM: the panel's policy forbids style attributes in markup.
          d.querySelectorAll('i[data-p]').forEach((i) => { i.style.width = i.dataset.p + '%'; });
        }
        pie(t('lente_ultima_lectura').replace('{h}', hora(r.hasta || Date.now())));
      });
      maxima();
    },
    extracto() {
      escucha('/api/extracto', (r) => {
        if (!r) return;
        const ev = r.eventos || [];
        const empresas = new Set(ev.map((e) => e.empresa).filter(Boolean));
        const cortadas = ev.filter((e) => e.verdict === 'cortado').length;
        const huecos = (r.huecos || []).length;
        cifras([{ n: ev.length }, { n: empresas.size }, { n: cortadas, tono: cortadas ? 'alerta' : '' }, { n: huecos, tono: huecos ? 'aviso' : '' }]);
      });
    },
    informe() {
      escucha('/api/informe', (r, o) => {
        if (!r || !r.plus) { cifras(sinDatos()); estado(t('lente_informe_ejemplo'), 'aviso'); return; }
        const f = r.total || {};
        const nuevos = (r.novedades || []).length;
        cifras([{ n: f.consultas }, { n: (f.rastreadores || 0) + (f.publicidad || 0) }, { n: nuevos, tono: nuevos ? 'aviso' : '' }, { n: f.cortados, tono: f.cortados ? 'alerta' : '' }]);
        if (r.desde && r.hasta) pie(t('lente_informe_periodo').replace('{desde}', fecha(r.desde)).replace('{hasta}', fecha(r.hasta)));
      });
    },
    revision() {
      escucha('/api/revision', (r) => {
        if (!r) return;
        if (!r.cuando) { cifras(sinDatos()); estado(t('lente_revision_nunca'), ''); return; }
        const s = r.resumen || {};
        cifras([{ n: s.ok }, { n: s.aviso, tono: s.aviso ? 'aviso' : '' }, { n: s.alerta, tono: s.alerta ? 'aviso' : '' }, { n: s.nuevos }]);
        const mirar = (s.aviso || 0) + (s.alerta || 0);
        estado(mirar ? tn('lente_revision_mirar', mirar) : t('lente_revision_bien'), mirar ? 'aviso' : 'ok');
        pie(t('lente_ultima_revision').replace('{h}', fecha(r.cuando) + ' · ' + hora(r.cuando)));
      });
    },
    ia() {
      escucha('/api/ia', (r) => {
        if (!r) { cifras(sinDatos()); return; }
        const s = r.servicios || [];
        const serv = new Set(s.map((x) => x.servicio));
        const disp = new Set(s.map((x) => x.device_id));
        const consultas = s.reduce((a, x) => a + (x.consultas || 0), 0);
        cifras([{ n: serv.size }, { n: consultas }, { n: disp.size }, { n: (r.alcances || []).length }]);
      });
    },
    antivirus() {
      escucha('/api/antivirus', (r) => {
        if (!r) { cifras(sinDatos()); return; }
        const s = r.resumen || {};
        cifras([{ n: (s.productos || []).length || (r.vistos || []).length }, { n: bytes(s.subidos), tono: 'chico' }, { n: s.paises || 0 }, { n: s.destinos || 0 }]);
        if (repintaAv) setTimeout(repintaAv, 0);
      });
      const cb = () => $('av-vigilar');
      const repintaAv = interruptorDe({
        caja: 'lente-av-sw', on: 'lente_activada', off: 'lente_desactivada', etiqueta: 'av_vigilar',
        encendido: () => (cb() ? cb().checked : null),
        cambia: (quiere) => { const c = cb(); if (!c) return; c.checked = quiere; c.dispatchEvent(new Event('change', { bubbles: true })); },
        observa: [$('av-vigilar-txt')],
      });
    },
    dispositivos() {
      escucha('/api/dispositivos', (d) => {
        if (!Array.isArray(d)) return;
        const suma = (k) => d.reduce((a, x) => a + ((x.totales && x.totales[k]) || 0), 0);
        const cort = suma('cortados');
        cifras([{ n: d.length }, { n: suma('consultas') }, { n: suma('rastreadores') + suma('publicidad') }, { n: cort, tono: cort ? 'alerta' : '' }]);
      });
    },
    vigilancia() {
      escucha('/api/vigilancia', (r) => {
        if (!r) { cifras(sinDatos()); return; }
        const p = r.pulso || {};
        const activas = (r.vigilancias || []).filter((v) => v.disponible).length;
        const avisos = (r.avisos || []).length;
        cifras([{ n: activas }, { n: p.comprobaciones || 0 }, { n: p.sucesos || 0 }, { n: avisos, tono: avisos ? 'aviso' : '' }]);
        estado(avisos ? tn('lente_vigilancia_avisos', avisos) : t('lente_vigilancia_bien'), avisos ? 'aviso' : 'ok');
        if (r.ultima_senal) pie(t('lente_ultima_lectura').replace('{h}', hora(r.ultima_senal)));
      });
    },
    vision() {
      agrupa();
      escucha('/api/vision', (r) => {
        if (!r || !r.disponible) { setTimeout(() => cifras(sinDatos()), 0); }
      });
    },
    sabeDeTi() { agrupa(); },
    identificacion() {
      escucha('/api/identificacion', (r) => {
        if (!r || !r.cobertura) return;
        const c = r.cobertura;
        cifras([{ n: c.consultas }, { n: c.identificada, tono: '' }, { n: c.probable, tono: c.probable ? 'aviso' : '' }, { n: c.sin_identificar }]);
        if (r.pct_texto && c.consultas) estado(r.pct_texto, c.pct_identificada >= 80 ? 'ok' : '');
      });
    },
    certificado() {
      cifras(sinDatos());
      estado(t('lente_cert_pendiente'), '');
      const lee = (r) => {
        if (!r || !r.resumen) return;
        const s = r.resumen;
        cifras([{ n: s.consultas }, { n: s.empresas }, { n: s.cortadas, tono: s.cortadas ? 'alerta' : '' }, { n: s.dias }]);
        estado('', '');
      };
      lee.tambienPost = true;
      escucha('/api/certificado', lee);
    },
    borrado() {
      escucha('/api/borrado', (r) => {
        if (!r) return;
        const env = r.envios || [];
        const venc = env.filter((e) => e.vencido).length;
        const vistos = (r.corredores || []).filter((c) => c.evidencia && c.evidencia.consultas > 0).length;
        cifras([{ n: vistos, tono: vistos ? 'aviso' : '' }, { n: (r.corredores || []).length }, { n: env.length }, { n: venc, tono: venc ? 'aviso' : '' }]);
      });
    },
    expedientes() {
      agrupa();
      escucha('/api/expedientes', (r) => {
        if (!r) return;
        cifra('x-pronto', r.pronto || 0, r.pronto ? 'aviso' : '');
        const v = $('x-vencidos');
        if (v && r.vencidos) v.classList.add('aviso');
      });
    },
    reglas() {
      escucha('/api/reglas', (r) => {
        if (!r || !r.reglas) return;
        const act = r.reglas.filter((x) => x.activa);
        const cort = act.filter((x) => x.action === 'cortar').length;
        const hoy = new Date(); hoy.setHours(0, 0, 0, 0);
        cifras([{ n: act.length }, { n: cort }, { n: act.length - cort }, { n: r.reglas.filter((x) => x.created_at >= hoy.getTime()).length }]);
      });
      maxima();
    },
    herramientas() {
      const lee = (r) => {
        if (!r) return;
        cifras([{ n: t(r.activo ? 'lente_activada' : 'lente_desactivada'), tono: 'chico' + (r.activo ? ' aviso' : '') }, { n: (r.cortados || []).length }, { n: (r.aprobados || []).length }, { n: (r.esenciales || []).length }]);
        if (r.activo && r.desde) pie(t('lente_ultima_lectura').replace('{h}', hora(Date.now())));
      };
      lee.tambienPost = true;
      escucha('/api/apagon', lee);
      escucha('/api/apagon/aprobar', lee);
      escucha('/api/apagon/quitar', lee);
    },
    empresa() {
      agrupa();
      escucha('/api/empresa', (r) => {
        if (!r) return;
        const c = r.casa || {};
        cifra(document.querySelectorAll('.lente-cifras .lente-cifra .n')[3], c.n_nombres || 0);
        const m = $('lente-miga-empresa');
        if (m && r.empresa) m.textContent = r.empresa;
        const cort = $('f-cortadas');
        if (cort && c.cortadas) cort.classList.add('alerta');
      });
    },
    hogar() {
      escucha('/api/hogar', (r) => {
        if (!r) return;
        cifras([
          { n: t(r.encendido ? 'lente_encendido' : 'lente_apagado'), tono: 'chico' + (r.encendido ? ' ok' : '') },
          { n: r.dispositivos || 0 },
          { n: r.encendido && r.desde ? fecha(r.desde) : '—', tono: 'chico' },
          { n: t(r.escuchando_en_lan ? 'lente_si' : 'lente_no') || (r.escuchando_en_lan ? '✓' : '—'), tono: 'chico' + (r.escuchando_en_lan ? ' ok' : '') },
        ]);
      });
      interruptorDe({
        caja: 'lente-hogar-sw', on: 'lente_encendido', off: 'lente_apagado', etiqueta: 'nav_hogar',
        encendido: () => { const b = $('h-toggle'); return b && b.dataset.on ? b.dataset.on === '1' : null; },
        cambia: () => $('h-toggle').click(),
        fallo: () => !!($('h-aviso') && $('h-aviso').textContent),
        observa: [$('h-toggle')],
      });
    },
    licencia() {
      escucha('/api/licencia', (r) => {
        if (!r || !r.estado) return;
        const plan = (r.estado.plan || {}).plan;
        const nivel = plan === 'prueba' ? t('lente_lic_prueba') : (r.estado.nivel === 'plus' ? 'Plus' : t('lente_gratis'));
        const dias = plan === 'prueba' ? r.estado.plan.dias_restantes : null;
        const con = (r.conexiones || []).length;
        cifras([{ n: nivel, tono: 'chico' }, { n: dias === null || dias === undefined ? '—' : dias, tono: dias === null || dias === undefined ? 'chico' : (dias <= 2 ? 'aviso' : '') }, { n: r.horas_observadas || 0 }, { n: con }]);
      });
    },
    verify() {
      escucha('/api/verify', (r) => {
        if (!r || !r.informe) return;
        const i = r.informe;
        const firma = i.signature === 'valid';
        const libro = i.ledger && typeof i.ledger === 'object' && i.ledger.found;
        const FIRMA = { valid: 'lente_firma_valida', dev_key: 'lente_firma_dev', no_signature_file: 'lente_firma_sin_archivo', invalid: 'lente_firma_no' };
        const sinLibro = i.ledger === 'no_ledger_file';
        cifras([
          { n: i.version || '—', tono: 'chico' },
          { n: t(FIRMA[i.signature] || 'lente_firma_no'), tono: 'chico' + (firma ? ' ok' : (i.signature === 'invalid' ? ' alerta' : '')) },
          { n: t(libro ? 'lente_en_registro' : (sinLibro ? 'lente_registro_sin_archivo' : 'lente_no_en_registro')), tono: 'chico' + (libro ? ' ok' : (sinLibro ? '' : ' aviso')) },
          { n: libro && i.ledger.found.date ? i.ledger.found.date : '—', tono: 'chico' },
        ]);
      });
    },
    estado() {
      escucha('/api/estado', (e) => {
        if (!e) return;
        const listas = e.listas || [];
        cifras([{ n: e.version || '—', tono: 'chico' }, { n: t(e.dns_aplicado ? 'lente_aplicado' : 'lente_sin_aplicar'), tono: 'chico' + (e.dns_aplicado ? ' ok' : ' aviso') }, { n: listas.length }, { n: listas.reduce((a, l) => a + (l.entradas || 0), 0) }]);
        estado(e.caducado ? t('pulso_apartado') : (e.dns_aplicado ? t('lente_estado_bien') : t('lente_estado_dns')), e.caducado || !e.dns_aplicado ? 'aviso' : 'ok');
      });
    },
  };

  window.GuardianaLente = { cifra, cifras, estado, pie, escucha, num, bytes, t, tn, interruptorDe, get LOC() { return loc(); } };
  // Run at once: this script sits after the page's markup and before app.js, so the page's first
  // requests (sent as soon as app.js runs) are already heard.
  const arranca = () => {
    const p = pagina();
    if (PAGINAS[p]) PAGINAS[p]();
  };
  if (document.body && document.querySelector('main')) arranca();
  else document.addEventListener('DOMContentLoaded', arranca, { once: true });
})();
