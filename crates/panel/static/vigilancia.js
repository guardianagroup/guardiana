// «Vigilancia» (1.0.45): the constant, read-only watch. The owner, 18 Oct 2026: «la experiencia
// del cliente debe ser sentir el monitoreo constante… para saber que todo está bien y nada está
// vigilando o extrayendo sus datos». Each watch with the time of its last look, what it sees now
// and the history, chained in the ledger. Nothing here changes the computer; the only button asks
// for a look now.
(() => {
  'use strict';
  window.GuardianaVigilancia = { iniciar };
  const ORDEN = ['camara_microfono', 'arranque', 'privacidad', 'proteccion'];
  const CHIP = { ok: 'ok', en_uso: 'info', aviso: 'aviso', atencion: 'aviso', esperando: '', nd: '' };

  async function iniciar(ctx) {
    const { api, t, esc, LOC, cadaTanto } = ctx;
    const $ = (id) => document.getElementById(id);
    // A text, or the fallback when the files do not have it (t() gives the key back).
    const tt = (k, otro) => { const r = t(k); return r === k ? otro : r; };
    const rellena = (k, datos) => Object.entries(datos || {}).reduce((s, [a, b]) => s.split('{' + a + '}').join(b), t(k));
    const hora = (ms) => (ms ? new Date(ms).toLocaleTimeString(LOC, { hour: '2-digit', minute: '2-digit' }) : '');
    const mismoDia = (a, b) => a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
    const cuando = (ms) => {
      if (!ms) return '';
      const d = new Date(ms), hoy = new Date(), ayer = new Date(Date.now() - 864e5);
      if (mismoDia(d, hoy)) return t('vg_hoy_a').replace('{h}', hora(ms));
      if (mismoDia(d, ayer)) return t('vg_ayer_a').replace('{h}', hora(ms));
      return d.toLocaleDateString(LOC, { day: 'numeric', month: 'short' }) + ' ' + hora(ms);
    };
    const valor = (v) => (v === 'sin_fijar' ? t('vg_valor_sin_fijar') : tt('vg_valor_' + String(v).toLowerCase(), String(v)));
    const perfil = (p) => tt('vg_perfil_' + String(p).toLowerCase(), p);
    // A finding's sentence, from its key and its data, in the person's language.
    const frase = (s) => {
      const f = (s.datos && s.datos.frase) || '';
      const d = Object.assign({}, (s.datos && s.datos.datos) || {});
      if (d.recurso) d.recurso = t('vg_recurso_' + d.recurso);
      if (d.ajuste) d.ajuste = tt('vg_aj_' + d.ajuste, d.ajuste);
      d.cuenta = d.cuenta ? t('vg_de_cuenta').replace('{x}', d.cuenta) : '';
      if ('antes' in d) d.antes = valor(d.antes);
      if ('despues' in d) d.despues = valor(d.despues);
      if ('activo' in d) d.activo = d.activo === 'true' ? t('vg_pt_activo') : t('vg_pt_inactivo');
      if (d.tipo) d.tipo = tt('vg_tipo_' + d.tipo, d.tipo);
      if (f.startsWith('vg_ev_cortafuegos') && d.x) d.x = perfil(d.x);
      // A use says since when Windows says it started; anything else, when it was seen.
      d.cuando = cuando(f.startsWith('vg_ev_uso') && d.desde ? Number(d.desde) : s.ts);
      return f ? rellena(f, d) : s.texto;
    };
    const linea = (s) => `<li class="${s.importancia === 'aviso' ? 'aviso' : ''}"><time datetime="${new Date(s.ts).toISOString()}">${esc(cuando(s.ts))}</time><span>${esc(frase(s))}</span><small title="${esc(s.hash)}">${esc((s.hash || '').slice(0, 8))}</small></li>`;
    let ultimo = null, visto = 0;

    function pintaCabeza(r) {
      const p = $('vg-pastilla');
      p.className = 'vg-pastilla ' + r.estado;
      $('vg-pastilla-texto').textContent = t('vg_estado_' + r.estado);
      $('vg-pastilla-senal').textContent = r.ultima_senal ? '· ' + t('vg_ultima_senal').replace('{h}', hora(r.ultima_senal)) : '';
      const ritmo = [];
      if (r.vigila_desde) ritmo.push(t('vg_vigila_desde').replace('{h}', cuando(r.vigila_desde)));
      ritmo.push(t('vg_ritmo').replace('{r}', r.cada_rapida_s).replace('{l}', Math.round(r.cada_lenta_s / 60)));
      $('vg-ritmo').textContent = ritmo.join(' · ');
      const b = $('vg-ahora');
      b.disabled = !!r.comprobando;
      b.textContent = r.comprobando ? t('vg_comprobando') : t('vg_comprobar_ahora');
      const pu = r.pulso || { horas: [] };
      $('vg-hoy').textContent = t('vg_hoy').replace('{c}', Number(pu.comprobaciones || 0).toLocaleString(LOC)).replace('{s}', pu.sucesos || 0).replace('{a}', pu.avisos || 0);
      const max = Math.max(1, ...pu.horas.map((h) => h[0]));
      const ahoraH = Math.floor((Date.now() - pu.desde) / 36e5);
      $('vg-horas').innerHTML = pu.horas.map((h, i) => {
        const cls = [i > ahoraH ? 'futuro' : '', i === ahoraH ? 'ahora' : '', h[2] ? 'aviso' : ''].filter(Boolean).join(' ');
        const alto = i > ahoraH ? 4 : Math.max(6, Math.round((h[0] / max) * 100));
        return `<li class="${cls}" title="${esc(String(i).padStart(2, '0') + ':00 · ' + t('vg_hora_titulo').replace('{c}', h[0]).replace('{s}', h[1]))}"><b data-alto="${alto}"></b></li>`;
      }).join('');
      // Heights from data attributes: the page's policy forbids inline styles in the markup.
      $('vg-horas').querySelectorAll('b').forEach((el) => { el.style.height = el.dataset.alto + '%'; });
    }

    function pintaAtencion(r) {
      const a = r.atencion || [];
      $('vg-atencion').hidden = !a.length;
      $('vg-atencion').innerHTML = a.map((x) => `<p>${esc(t(x.frase).replace('{x}', perfil(x.x)))}</p>`).join('');
      const av = r.avisos || [];
      $('vg-avisos').hidden = !av.length;
      $('vg-avisos-lista').innerHTML = av.map(linea).join('');
    }

    const item = (izq, der) => `<li><span>${esc(izq)}</span>${der ? `<em>${esc(der)}</em>` : ''}</li>`;
    function lista(v) {
      const l = v.lista || [];
      if (v.id === 'camara_microfono') {
        const filas = l.map((u) => {
          const quien = u.propio ? t('vg_cm_propio') : u.programa + (u.motor_web ? ' (' + t('vg_cm_motor_web') + ')' : '');
          const que = t('vg_rec_' + u.recurso);
          const cuandoTxt = u.ahora ? t('vg_cm_ahora_desde').replace('{h}', hora(u.desde)) : t('vg_cm_de_a').replace('{a}', cuando(u.desde)).replace('{b}', hora(u.hasta));
          return item(quien + ' · ' + que, cuandoTxt);
        });
        return (filas.length ? `<ul>${filas.join('')}</ul>` : '') + `<p class="vg-nota">${esc(t('vg_cm_limite'))}</p>`;
      }
      if (v.id === 'arranque') {
        const filas = l.map((a) => item(a.titulo + ' · ' + tt('vg_tipo_' + a.tipo, a.tipo), (a.presente ? '' : t('vg_marca_ido') + ' · ') + (a.sistema ? t('vg_marca_sistema') + ' · ' : '') + t('vg_desde').replace('{h}', cuando(a.visto))));
        const desde = v.datos.desde ? `<p class="vg-nota">${esc(t('vg_ar_desde').replace('{f}', cuando(Number(v.datos.desde))))}</p>` : '';
        return (filas.length ? `<ul>${filas.join('')}</ul>` : '') + desde;
      }
      if (v.id === 'privacidad') {
        const filas = l.map((p) => item(tt('vg_aj_' + p.ajuste, p.ajuste) + (p.cuenta ? t('vg_de_cuenta').replace('{x}', p.cuenta) : ''), valor(p.valor)));
        return filas.length ? `<ul>${filas.join('')}</ul>` : '';
      }
      if (v.id === 'proteccion') {
        const filas = l.map((p) => {
          const nombre = p.clase === 'perfil' ? t('vg_pt_clase_perfil').replace('{x}', perfil(p.nombre)) : t('vg_pt_clase_' + p.clase).replace('{x}', p.nombre);
          let estado = p.activo ? t('vg_pt_activo') : t('vg_pt_inactivo');
          if ((p.clase === 'tiempo_real' || p.clase === 'manipulacion') && p.al_mando === false) estado = t('vg_pt_segundo_plano');
          return item(nombre, estado);
        });
        const mac = ultimo && ultimo.sistema === 'macos' ? `<p class="vg-nota">${esc(t('vg_pt_mac'))}</p>` : '';
        return (filas.length ? `<ul>${filas.join('')}</ul>` : '') + mac;
      }
      return '';
    }

    function pintaVigilancias(r) {
      const vs = (r.vigilancias || []).slice().sort((a, b) => ORDEN.indexOf(a.id) - ORDEN.indexOf(b.id));
      $('vg-rejilla').innerHTML = vs.map((v) => {
        const datos = Object.assign({}, v.datos);
        const chip = CHIP[v.estado] !== undefined ? CHIP[v.estado] : '';
        const ultima = v.ultima ? t('vg_ultima_comprobacion').replace('{h}', hora(v.ultima)) : (v.estado === 'nd' ? '' : t('vg_sin_comprobar'));
        const error = v.error ? `<p class="vg-nota">${esc(t('vg_error').replace('{x}', v.error))}</p>` : '';
        return `<article class="tarjeta vg-vigia ${v.estado}">
          <div class="vg-vigia-cab"><h3>${esc(t('vg_check_' + v.id))}</h3><span class="chip ${chip}">${esc(t('vg_estado_' + v.estado))}</span></div>
          <p class="vg-ultima">${esc(ultima)}</p>
          <p class="vg-frase">${esc(rellena(v.frase, datos))}</p>
          ${error}
          ${v.estado === 'nd' ? '' : lista(v)}
        </article>`;
      }).join('');
    }

    function pintaHistorial(r) {
      const h = r.historial || [];
      $('vg-historial').innerHTML = h.length ? h.map(linea).join('') : `<li class="vacio">${esc(t('vg_historial_vacio'))}</li>`;
      const c = $('vg-cadena');
      if (r.cadena_rota === null || r.cadena_rota === undefined) {
        c.className = 'chip ok';
        c.textContent = t('vg_cadena_ok');
        c.title = t('vg_cadena_ok_titulo');
      } else {
        c.className = 'chip aviso';
        c.textContent = t('vg_cadena_rota').replace('{x}', r.cadena_rota || t('vg_cadena_final'));
        c.title = '';
      }
    }

    async function carga() {
      const r = await api('/api/vigilancia');
      ultimo = r;
      pintaCabeza(r);
      pintaAtencion(r);
      pintaVigilancias(r);
      pintaHistorial(r);
      // Seen here: the pill stops saying «Atención» for these (what is still off keeps saying it).
      const max = Math.max(0, ...(r.avisos || []).map((s) => s.id));
      if (max > visto) {
        visto = max;
        api('/api/vigilancia/visto', { method: 'POST', body: { hasta: max } }).catch(() => {});
      }
    }

    $('vg-ahora').addEventListener('click', async () => {
      $('vg-ahora').disabled = true;
      $('vg-ahora').textContent = t('vg_comprobando');
      try { await api('/api/vigilancia/ahora', { method: 'POST', body: {} }); } catch (_) {}
      setTimeout(() => { carga().catch(() => {}); }, 1500);
    });
    await carga();
    cadaTanto(carga, 10000);
  }
})();
