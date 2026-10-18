// «Contratos» (1.0.45, «contrato vivo»; page redrawn in 1.0.46 as the owner chose: R1-d's
// «Antes y después de cada actualización» plus R2-d's program cards with search, filter chips and
// a 400 px inspector). Which companies each program on this computer normally contacts, learnt
// from what it asked for; a calm notice, with the before and the after, when one starts contacting
// new ones; and the chained register of every change. The sentences come from the panel already in
// the person's language: this script only lays them out and sends the person's decision back. It
// says «empezó a consultar», never more: a name asked for is not data sent (the owner's principle,
// 18 Oct 2026). Day one is quiet: every program is learning and nothing is flagged until there is a
// week to compare against; GUARDIANA's own programs never raise a notice.
(() => {
  'use strict';
  window.GuardianaContratos = { iniciar };

  const DIA = 86400000;
  const VENTANA_DIAS = 45;
  const FILAS_LINEA = 6;
  const TARJETAS = 6;

  async function iniciar(ctx) {
    const { api, t, tn, esc, LOC, cadaTanto } = ctx;
    const $ = (id) => document.getElementById(id);
    const L = window.GuardianaLente || null;
    document.title = 'GUARDIANA · ' + t('nav_contratos');
    const num = (n) => (L ? L.num(n) : String(n));
    const fecha = (ms) => (ms ? new Date(ms).toLocaleDateString(LOC, { day: 'numeric', month: 'short' }) : '');
    const fechaHora = (ms) => (ms ? new Date(ms).toLocaleString(LOC, { dateStyle: 'medium', timeStyle: 'short' }) : '');
    const hora = (ms) => new Date(ms).toLocaleTimeString(LOC, { hour: '2-digit', minute: '2-digit' });
    const rel = (() => {
      let f = null;
      try { f = new Intl.RelativeTimeFormat(LOC, { numeric: 'auto' }); } catch (_) {}
      return (ms) => {
        if (!ms) return '';
        const dias = Math.round((ms - Date.now()) / DIA);
        if (!f) return fecha(ms);
        if (Math.abs(dias) < 1) return f.format(Math.round((ms - Date.now()) / 3600000), 'hour');
        if (Math.abs(dias) < 45) return f.format(dias, 'day');
        return fecha(ms);
      };
    })();
    const ini = (s) => esc(String(s || '?').trim().charAt(0).toUpperCase() || '?');

    let ultimo = null;
    let filtro = 'todos';
    let busca = '';
    let todos = false;
    let elegido = null;

    const msg = (texto, mal) => { const m = $('ct-msg'); m.textContent = texto || ''; m.className = 'ct-msg' + (mal ? ' bad' : ''); };

    // The cut is a switch, green «Permitido» ↔ red «Cortado», with the word beside it (the owner,
    // 18 Oct 2026). Off cuts only what is new; on again undoes that cut.
    const TEXTO_INT = () => ({ permitido: t('interruptor_permitido'), cortado: t('interruptor_cortado'), cortando: t('cortando'), permitiendo: t('permitiendo') });
    const interruptor = (a, cortado) => (window.GuardianaInterruptor
      ? window.GuardianaInterruptor.html({ cortado, texto: TEXTO_INT(), etiqueta: t('ct_interruptor_nuevo').replace('{programa}', a.programa), chico: true })
      : '');

    // ---- what each program is, in one word and one sentence ---------------------------------
    const avisosDe = (clave) => (ultimo ? ultimo.avisos.filter((a) => a.clave === clave) : []);
    const abiertoDe = (clave) => avisosDe(clave).find((a) => a.estado === 'abierto') || null;
    const conReglas = (c) => c.estado === 'fijado' || avisosDe(c.clave).some((a) => a.reglas > 0);
    const nombresNuevos = (a) => {
      const out = [];
      (a.nuevas || []).forEach((e) => { (e.nombres && e.nombres.length ? e.nombres : [e.empresa]).forEach((n) => { if (!out.includes(n)) out.push(n); }); });
      return out;
    };
    const habituales = (c) => (c.empresas || []).filter((e) => !(c.cortadas || []).includes(e.empresa));
    const clase = (c) => {
      if (c.propio) return 'propio';
      if (abiertoDe(c.clave)) return 'cambios';
      if (c.estado === 'aprendiendo') return 'aprendiendo';
      if (c.estado === 'fijado') return 'reglas';
      if (c.estado === 'no_aplica') return 'no_aplica';
      return 'igual';
    };
    const frase = (c) => {
      const a = abiertoDe(c.clave);
      switch (clase(c)) {
        case 'propio': return t((c.historial || []).length > 1 ? 'lente_ct_e_propio' : 'lente_ct_e_propio_doc');
        case 'cambios': return tn('lente_ct_e_nuevos', nombresNuevos(a).length);
        case 'aprendiendo': return t('lente_ct_e_aprende');
        case 'reglas': return t('lente_ct_e_fijado');
        case 'no_aplica': return c.motivo_texto || c.estado_texto;
        default: return t('lente_ct_e_igual');
      }
    };
    const pildora = (c) => {
      switch (clase(c)) {
        case 'cambios': return `<span class="lente-pill aviso">${esc(t('lente_ct_p_cambios'))}</span>`;
        case 'aprendiendo': return `<span class="lente-pill">${esc(t('lente_ct_f_aprendiendo'))}</span>`;
        case 'reglas': return `<span class="lente-pill">${esc(t('lente_ct_f_reglas'))}</span>`;
        case 'no_aplica': return `<span class="lente-pill">${esc(c.estado_texto)}</span>`;
        default: return `<span class="lente-pill">${esc(t('lente_ct_p_igual'))}</span>`;
      }
    };
    const meta = (c) => {
      const h = c.historial || [];
      const firma = c.firmado_por ? ' · ' + t('lente_ct_firma_ok') : '';
      if (h.length > 1) return t('lente_ct_actualizado').replace('{v}', h[h.length - 1].etiqueta).replace('{cuando}', rel(h[h.length - 1].desde)) + firma;
      if (c.version) return t('lente_ct_version').replace('{v}', c.version) + firma;
      return t('lente_ct_visto').replace('{cuando}', rel(c.desde));
    };
    const pie = (c) => {
      const a = abiertoDe(c.clave);
      switch (clase(c)) {
        case 'cambios': {
          const n = nombresNuevos(a);
          return n.slice(0, 2).map((x) => `<span class="lente-pill aviso">${esc(x)}</span>`).join('') + (n.length > 2 ? `<span class="lente-pill">+${num(n.length - 2)}</span>` : '');
        }
        case 'aprendiendo': {
          const total = (ultimo && ultimo.aprende_dias) || 7;
          const d = Math.min(total, Math.max(1, Math.floor((Date.now() - c.desde) / DIA) + 1));
          return `<span class="lente-pill">${esc(t('lente_ct_dia').replace('{d}', num(d)).replace('{t}', num(total)))}</span>`;
        }
        case 'propio': {
          const sitios = [];
          (c.empresas || []).forEach((e) => (e.sitios || []).forEach((s) => { if (!sitios.includes(s)) sitios.push(s); }));
          return sitios.slice(0, 2).map((s) => `<span class="lente-pill">${esc(s)}</span>`).join('');
        }
        case 'no_aplica': return '';
        default: return `<span class="lente-pill">${esc(tn('lente_ct_habituales', habituales(c).length))}</span>`;
      }
    };

    // ---- the four tiles, the side card --------------------------------------------------------
    const pintaCifras = (r) => {
      const ahora = Date.now();
      const vigilados = r.contratos.filter((c) => c.estado !== 'no_aplica').length;
      let actualizaciones = 0;
      r.contratos.forEach((c) => (c.historial || []).forEach((v, i) => { if (i > 0 && v.desde >= ahora - 30 * DIA) actualizaciones += 1; }));
      const cambiaron = new Set(r.avisos.filter((a) => a.estado === 'abierto').map((a) => a.clave)).size;
      const cortadas = r.contratos.reduce((s, c) => s + (c.cortadas || []).length, 0);
      if (L) {
        L.cifras([{ n: vigilados }, { n: actualizaciones }, { n: cambiaron, tono: cambiaron ? 'aviso' : '' }, { n: cortadas, tono: cortadas ? 'alerta' : '' }]);
        const esperan = r.avisos.filter((a) => a.estado === 'abierto').reduce((s, a) => s + (a.nuevas || []).length, 0);
        const aprendiendo = r.contratos.some((c) => c.estado === 'aprendiendo') && !r.contratos.some((c) => c.estado === 'vigilando' || c.estado === 'fijado');
        if (esperan) L.estado(tn('lente_ct_esperan', esperan), 'aviso');
        else if (aprendiendo) L.estado(t('lente_ct_silencio'), '');
        else if (r.contratos.length) L.estado(t('lente_ct_sin_cambios'), 'ok');
        else L.estado('', '');
        L.pie(t('lente_ct_ultima').replace('{h}', hora(ahora)));
      }
      const acc = $('ct-accion');
      if (acc) {
        const n = r.avisos.filter((a) => a.estado === 'abierto').reduce((s, a) => s + (a.nuevas || []).length, 0);
        acc.textContent = n ? tn('lente_ct_revisar', n) : t('lente_ct_ver_programas');
        acc.dataset.revisar = n ? '1' : '';
      }
    };

    // ---- «Antes y después de cada actualización» ------------------------------------------------
    const pintaLinea = (r) => {
      const ahora = Date.now();
      const desde = ahora - VENTANA_DIAS * DIA;
      const filas = r.contratos
        .filter((c) => c.estado !== 'no_aplica' || c.propio)
        .map((c) => ({ c, vs: (c.historial || []).filter((v) => v.desde >= desde), a: abiertoDe(c.clave) }))
        // Updated inside the window (a version after the first one began in it), or waiting.
        .filter((x) => (x.c.historial || []).slice(1).some((v) => v.desde >= desde) || x.a)
        .sort((x, y) => (!!y.a - !!x.a) || ((y.vs.length ? y.vs[y.vs.length - 1].desde : 0) - (x.vs.length ? x.vs[x.vs.length - 1].desde : 0)))
        .slice(0, FILAS_LINEA);
      const ol = $('ct-linea');
      if (!r.contratos.length) { ol.innerHTML = `<li class="lente-vacio-li"><div class="lente-vacio"><p>${esc(t('ct_vacio'))}</p></div></li>`; return; }
      if (!filas.length) { ol.innerHTML = `<li class="lente-vacio-li"><div class="lente-vacio"><p>${esc(t('lente_ct_linea_vacia'))}</p><a class="boton" href="#ct-programas-sec">${esc(t('lente_ct_ver_programas'))}</a></div></li>`; return; }
      ol.innerHTML = filas.map(({ c, vs, a }) => {
        let der;
        if (a) {
          const antes = (a.antes || []).length;
          const nuevas = (a.nuevas || []).map((e) => e.nombres && e.nombres[0] ? `${e.nombres[0]} (${e.empresa})` : e.empresa);
          der = `<span>${esc(t('lente_ct_antes_despues').replace('{a}', num(antes)).replace('{d}', num(antes + (a.nuevas || []).length)))}</span>`
            + (nuevas.length ? `<span class="lente-pill aviso">${esc(t('lente_ct_nueva').replace('{x}', nuevas[0]))}</span>` : '');
        } else if (c.propio) {
          der = `<span>${esc(t('lente_ct_e_propio'))}</span>`;
        } else {
          der = `<span>${esc(tn('lente_ct_sin_cambios_n', habituales(c).length))}</span>`;
        }
        // The versions of the window and the one before it (drawn at the window's start).
        const h = c.historial || [];
        let k = h.findIndex((v) => v.desde >= desde);
        if (k < 0) k = h.length - 1;
        const puntos = h.slice(Math.max(0, k - 1)).slice(-4);
        const ejes = puntos.map((v, i) => {
          const pct = Math.max(0, Math.min(100, ((v.desde - desde) / (ahora - desde)) * 100));
          const nueva = a && i === puntos.length - 1 && v.etiqueta === a.version_ahora;
          return `<span class="${nueva ? 'nueva' : ''}" data-p="${pct.toFixed(1)}">${esc(v.etiqueta)} · ${esc(fecha(v.desde))}</span>`;
        }).join('');
        return `<li tabindex="0" role="button" data-clave="${esc(c.clave)}" aria-selected="${elegido === c.clave}">
          <span class="lente-ini">${ini(c.programa)}</span>
          <div><div class="ct-l-cab"><b>${esc(c.programa)}</b><span class="ct-l-der">${der}</span></div>
          <div class="ct-eje">${ejes}</div></div></li>`;
      }).join('');
      // Where each version sits in the 45 days, through the CSSOM (no style attributes: CSP).
      ol.querySelectorAll('.ct-eje > span[data-p]').forEach((s) => {
        const p = Number(s.dataset.p);
        s.style.left = p + '%';
        if (p > 70) s.classList.add('der');
      });
    };

    // ---- the program cards ------------------------------------------------------------------
    const pasaFiltro = (c) => {
      const k = clase(c);
      if (filtro === 'cambios' && k !== 'cambios') return false;
      if (filtro === 'aprendiendo' && k !== 'aprendiendo') return false;
      if (filtro === 'reglas' && !conReglas(c)) return false;
      if (busca && !(`${c.programa} ${c.nombre}`.toLowerCase().includes(busca))) return false;
      return true;
    };
    const pintaTarjetas = (r) => {
      $('ct-f-todos').textContent = t('lente_ct_f_todos').replace('{n}', num(r.contratos.length));
      document.querySelectorAll('#ct-filtros [data-filtro]').forEach((b) => b.setAttribute('aria-pressed', String(b.dataset.filtro === filtro)));
      const lista = r.contratos.filter(pasaFiltro);
      const vistas = todos ? lista : lista.slice(0, TARJETAS);
      const caja = $('ct-programas');
      if (!r.contratos.length) caja.innerHTML = `<div class="lente-tarjeta ct-vacia"><div class="lente-vacio"><p>${esc(t('ct_vacio'))}</p></div></div>`;
      else if (!lista.length) caja.innerHTML = `<div class="lente-tarjeta ct-vacia"><div class="lente-vacio"><p>${esc(t('lente_ct_nada'))}</p><button type="button" class="boton" data-limpia="1">${esc(t('lente_ct_ver_todos'))}</button></div></div>`;
      else caja.innerHTML = vistas.map((c) => `<article class="ct-prog" tabindex="0" role="button" data-clave="${esc(c.clave)}" aria-selected="${elegido === c.clave}">
          <div class="ct-p-cab"><span class="lente-ini grande">${ini(c.programa)}</span><div><b>${esc(c.programa)}</b><small>${esc(meta(c))}</small></div>${pildora(c)}</div>
          <p class="ct-p-frase">${esc(frase(c))}</p>
          <div class="ct-p-pie">${pie(c)}<span class="boton texto sigue">${esc(t('lente_ct_ver'))}</span></div>
        </article>`).join('');
      const mas = $('ct-mas');
      mas.hidden = lista.length <= TARJETAS;
      $('ct-mostrando').textContent = t('lente_ct_mostrando').replace('{n}', num(vistas.length)).replace('{t}', num(lista.length));
      $('ct-ver-todos').hidden = todos;
    };

    // ---- the inspector ----------------------------------------------------------------------
    const pintaInspector = () => {
      const ins = $('ct-inspector');
      const c = ultimo && elegido ? ultimo.contratos.find((x) => x.clave === elegido) : null;
      if (!c) { ins.hidden = true; return; }
      $('ct-i-ini').textContent = String(c.programa || '?').charAt(0).toUpperCase();
      $('ct-i-nombre').textContent = c.programa;
      $('ct-i-meta').textContent = meta(c);
      const a = abiertoDe(c.clave);
      const resueltos = avisosDe(c.clave).filter((x) => x.estado === 'cortado' || x.estado === 'cortado_auto');
      const conInterruptor = a || resueltos[0] || null;
      const deshacible = !a && !!resueltos[0];
      const secciones = [];
      secciones.push(`<section class="li-sec"><h3>${esc(t('lente_ct_i_pasa'))}</h3><p class="li-frase">${esc(frase(c))}</p>
        ${conInterruptor ? `<p>${esc(conInterruptor.nota)}</p>${conInterruptor.reglas_texto ? `<p>${esc(conInterruptor.reglas_texto)} <a class="enlace" href="/reglas">${esc(t('nav_reglas'))}</a></p>` : ''}
          <div class="li-botones" data-id="${conInterruptor.id}">${interruptor(conInterruptor, deshacible)}${a ? `<button type="button" class="boton chico" data-accion="aceptar">${esc(t('ct_accion_aceptar'))}</button>` : ''}</div>` : ''}
        ${clase(c) === 'aprendiendo' && c.aprende_hasta ? `<p>${esc(t('ct_aprende_hasta').replace('{fecha}', fecha(c.aprende_hasta)))}</p>` : ''}</section>`);
      if (a && (a.nuevas || []).length) {
        secciones.push(`<section class="li-sec"><h3>${esc(t('lente_ct_i_nuevas'))}</h3><ul class="li-lista">${a.nuevas.map((e) => `<li class="nueva"><span>${esc(e.empresa)}${e.nombres && e.nombres.length ? ` · ${esc(e.nombres.join(', '))}` : ''}</span><span>${esc(tn('lente_ct_i_veces', e.veces))}</span></li>`).join('')}</ul></section>`);
      }
      const hab = habituales(c);
      if (hab.length) {
        secciones.push(`<section class="li-sec"><h3>${esc(t('lente_ct_i_habituales'))}</h3><ul class="li-lista">${hab.slice(0, 40).map((e) => `<li><span>${esc(e.empresa)}</span><span>${esc(tn('lente_ct_i_veces', e.veces))}</span></li>`).join('')}</ul></section>`);
      }
      if ((c.cortadas || []).length) {
        secciones.push(`<section class="li-sec"><h3>${esc(t('ct_cortadas'))}</h3><ul class="li-lista">${c.cortadas.map((x) => `<li><span>${esc(x)}</span><span></span></li>`).join('')}</ul></section>`);
      }
      if ((c.historial || []).length) {
        secciones.push(`<section class="li-sec"><h3>${esc(t('lente_ct_i_versiones'))}</h3><ul class="li-lista">${c.historial.slice().reverse().slice(0, 12).map((v) => `<li><span>${esc(v.etiqueta)}</span><span>${esc(fecha(v.desde))}</span></li>`).join('')}</ul></section>`);
      }
      const puedeFijar = (c.estado === 'aprendiendo' || c.estado === 'vigilando') && !c.propio;
      secciones.push(`<section class="li-sec"><h3>${esc(t('lente_ct_i_archivo'))}</h3><p class="li-mono">${esc(c.ruta)}</p>
        <p>${esc(c.firmado_por ? t('ct_firmado').replace('{x}', c.firmado_por) : t('ct_sin_firma'))} · ${esc(tn('ct_lanzamientos', c.lanzamientos))}</p>
        ${puedeFijar || c.estado === 'fijado' ? `<div class="li-botones">${puedeFijar ? `<button type="button" class="boton chico" data-fijar="1">${esc(t('ct_accion_fijar'))}</button>` : `<button type="button" class="boton chico" data-fijar="0">${esc(t('ct_accion_soltar'))}</button>`}</div><p>${esc(t('ct_fijar_explica'))}</p>` : ''}</section>`);
      $('ct-i-cuerpo').innerHTML = secciones.join('');
      ins.hidden = false;
    };
    const elige = (clave, foco) => {
      elegido = clave;
      document.querySelectorAll('[data-clave][aria-selected]').forEach((e) => e.setAttribute('aria-selected', String(e.dataset.clave === clave)));
      pintaInspector();
      if (foco) $('ct-inspector').focus({ preventScroll: true });
    };
    const cierra = () => {
      if (!elegido) return;
      const antes = elegido;
      elegido = null;
      $('ct-inspector').hidden = true;
      document.querySelectorAll('[data-clave][aria-selected="true"]').forEach((e) => e.setAttribute('aria-selected', 'false'));
      const vuelve = document.querySelector(`.ct-prog[data-clave="${CSS.escape(antes)}"]`);
      if (vuelve) vuelve.focus({ preventScroll: true });
    };

    const pinta = (r) => {
      ultimo = r;
      $('ct-como').textContent = t('ct_como').replace('{dias}', r.aprende_dias).replace('{n}', r.aprende_lanzamientos);
      $('ct-solo-windows').classList.toggle('hidden', !!r.disponible);
      pintaCifras(r);
      pintaLinea(r);
      pintaTarjetas(r);
      if (elegido && !r.contratos.some((c) => c.clave === elegido)) elegido = null;
      pintaInspector();
    };

    const pintaRegistro = (filas) => {
      $('ct-registro').innerHTML = filas.length
        ? filas.map((f) => `<li title="${esc(f.hash)}"><time datetime="${new Date(f.ts).toISOString()}">${esc(fechaHora(f.ts))}</time><span>${esc(f.frase)}</span></li>`).join('')
        : '';
    };

    const carga = async () => {
      const [r, reg] = await Promise.all([api('/api/contratos'), api('/api/contratos/registro')]);
      pinta(r);
      pintaRegistro(reg);
    };

    // ---- events -----------------------------------------------------------------------------
    const abreDesde = (ev) => {
      const el = ev.target.closest('[data-clave]');
      if (!el) return;
      ev.stopPropagation();
      elige(el.dataset.clave, true);
    };
    ['ct-linea', 'ct-programas'].forEach((id) => {
      $(id).addEventListener('click', (ev) => {
        if (ev.target.closest('[data-limpia]')) { filtro = 'todos'; busca = ''; $('ct-buscar').value = ''; pintaTarjetas(ultimo); return; }
        abreDesde(ev);
      });
      $(id).addEventListener('keydown', (ev) => { if ((ev.key === 'Enter' || ev.key === ' ') && ev.target.matches('[data-clave]')) { ev.preventDefault(); abreDesde(ev); } });
    });
    $('ct-filtros').addEventListener('click', (ev) => {
      const b = ev.target.closest('[data-filtro]');
      if (!b || !ultimo) return;
      filtro = b.dataset.filtro; todos = false;
      pintaTarjetas(ultimo);
    });
    $('ct-buscar').addEventListener('input', (ev) => { busca = ev.target.value.trim().toLowerCase(); todos = false; if (ultimo) pintaTarjetas(ultimo); });
    $('ct-ver-todos').addEventListener('click', () => { todos = true; if (ultimo) pintaTarjetas(ultimo); });
    $('ct-i-cerrar').addEventListener('click', (ev) => { ev.stopPropagation(); cierra(); });
    document.addEventListener('keydown', (ev) => { if (ev.key === 'Escape' && elegido) cierra(); });
    document.addEventListener('click', (ev) => {
      if (!elegido) return;
      if (ev.target.closest('#ct-inspector, [data-clave], .interruptor-caja')) return;
      cierra();
    });
    // «Revisar N cambios»: the programs with changes, the first one open.
    const acc = $('ct-accion');
    if (acc) acc.addEventListener('click', (ev) => {
      if (!acc.dataset.revisar || !ultimo) return;
      ev.preventDefault();
      ev.stopPropagation();
      filtro = 'cambios'; todos = false; busca = ''; $('ct-buscar').value = '';
      pintaTarjetas(ultimo);
      const a = ultimo.avisos.find((x) => x.estado === 'abierto');
      $('ct-programas-sec').scrollIntoView({ behavior: 'smooth', block: 'start' });
      if (a) elige(a.clave, true);
    });
    // «Exportar contratos»: what this page already has, as a JSON file the person keeps.
    const exp = $('ct-exportar');
    if (exp) exp.addEventListener('click', (ev) => {
      ev.preventDefault();
      if (!ultimo) return;
      const datos = { generado: new Date().toISOString(), contratos: ultimo.contratos, avisos: ultimo.avisos };
      const url = URL.createObjectURL(new Blob([JSON.stringify(datos, null, 2)], { type: 'application/json' }));
      const a = document.createElement('a');
      a.href = url; a.download = 'guardiana-contratos.json';
      document.body.appendChild(a); a.click(); a.remove();
      setTimeout(() => URL.revokeObjectURL(url), 2000);
    });

    // The switch in the inspector: off asks to cut what is new, on undoes it.
    if (window.GuardianaInterruptor) {
      window.GuardianaInterruptor.engancha($('ct-i-cuerpo'), async (b, quiereCortar) => {
        const caja = b.closest('[data-id]');
        const id = Number(caja.getAttribute('data-id'));
        const r = await api(`/api/contratos/avisos/${id}/${quiereCortar ? 'cortar' : 'deshacer'}`, { method: 'POST' });
        msg(r.texto);
        await carga();
        return true;
      }, TEXTO_INT());
    }
    $('ct-i-cuerpo').addEventListener('click', async (ev) => {
      const b = ev.target.closest('button[data-accion], button[data-fijar]');
      if (!b) return;
      b.disabled = true;
      try {
        if (b.dataset.accion) {
          const id = Number(b.closest('[data-id]').getAttribute('data-id'));
          const r = await api(`/api/contratos/avisos/${id}/${b.dataset.accion}`, { method: 'POST' });
          msg(r.texto);
        } else {
          await api('/api/contratos/fijar', { method: 'POST', body: { clave: elegido, fijar: b.dataset.fijar === '1' } });
        }
        await carga();
      } catch (e) {
        msg(t('ct_error').replace('{x}', String(e.message || e)), true);
        b.disabled = false;
      }
    });

    $('ct-comprobar').addEventListener('click', async () => {
      const el = $('ct-comprobado');
      try {
        const r = await api('/api/contratos/comprobar');
        el.textContent = r.texto;
        el.className = 'ct-msg' + (r.ok ? '' : ' bad');
      } catch (e) {
        el.textContent = t('ct_error').replace('{x}', String(e.message || e));
      }
    });

    await carga();
    // Kept up to date while the page is open; nothing is redrawn under a person who is reading
    // the inspector or pressing a button.
    if (cadaTanto) {
      cadaTanto(async () => {
        if (document.activeElement && document.activeElement.closest && document.activeElement.closest('#ct-inspector, .lente-buscar')) return;
        const r = await api('/api/contratos');
        if (JSON.stringify(r) !== JSON.stringify(ultimo)) await carga();
      }, 20000);
    }
  }
})();
