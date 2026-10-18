// «Quién vende tus datos dentro de tu PC» (1.0.46, Diagnóstico). Everything it shows comes from
// /api/quien-vende, read on this computer: the programs that talk to a registered data broker or
// to a company of a documented case, the browser extensions and what they may read, the
// system's own advertising settings, the AI services in use, the maker's programs, the antivirus
// and the TVs of the house. The switches are the shared component (interruptor.js): a name is
// cut with the same rule as anywhere else (/api/reglas), a system setting through
// /api/quien-vende/ajuste. app.js hands over the session, the texts and the language (`iniciar`).
(() => {
  'use strict';
  window.GuardianaQuienVende = { iniciar };

  async function iniciar(ctx) {
    const { api, t, esc, LOC } = ctx;
    const $ = (id) => document.getElementById(id);
    const G = window.GuardianaInterruptor;
    const num = (n) => Number(n || 0).toLocaleString(LOC);
    // «1 consulta» / «1,234 queries»: the singular when there is one, the locale's separators always.
    const tn = (k, n) => t(n === 1 && t(k + '_uno') !== k + '_uno' ? k + '_uno' : k).replace('{n}', num(n));
    const hora = (ms) => new Date(ms).toLocaleTimeString(LOC, { hour: '2-digit', minute: '2-digit' });
    const fecha = (ms) => new Date(ms).toLocaleDateString(LOC, { day: 'numeric', month: 'short' });
    const dia = (s) => { const d = new Date(s + 'T12:00:00Z'); return isNaN(d) ? s : d.toLocaleDateString(LOC, { day: 'numeric', month: 'long', year: 'numeric' }); };
    // A cut of a name: Permitido / Cortado. A setting of the system: On / Off (grey when off:
    // turning advertising off is not a cut, so it is never red).
    const TEXTO_CORTE = () => ({ permitido: t('interruptor_permitido'), cortado: t('interruptor_cortado'), cortando: t('cortando'), permitiendo: t('permitiendo') });
    const TEXTO_AJUSTE = () => ({ permitido: t('qv_aj_activado'), cortado: t('qv_aj_desactivado'), cortando: t('qv_aj_desactivando'), permitiendo: t('qv_aj_activando') });
    const corte = (permitido, datos, etiqueta, cuando) => {
      if (!G) return '';
      const tx = TEXTO_CORTE();
      if (!permitido && cuando) tx.cortado = t('interruptor_cortado') + ' ' + hora(cuando);
      return G.html({ cortado: !permitido, texto: tx, etiqueta, datos, chico: true });
    };
    const ajuste = (activo, datos, etiqueta) => (G ? G.html({ cortado: !activo, texto: TEXTO_AJUSTE(), etiqueta, datos, chico: true }).replace('interruptor-caja', 'interruptor-caja ajuste') : '');
    let D = null;
    const caso = (id) => (D && D.casos || []).find((c) => c.id === id);
    const pillCaso = (id) => {
      const c = caso(id);
      if (!c) return '';
      return `<a class="lente-pill aviso" href="${esc(c.url)}" target="_blank" rel="noopener" title="${esc(t('qv_caso_' + c.id))}">${esc(c.medio)} · ${esc(c.fecha.slice(0, 4))}</a>`;
    };
    const bien = (texto, neutro) => `<p class="qv-bien${neutro ? ' neutro' : ''}">${esc(texto)}</p>`;
    const inicial = (s) => esc((String(s || '?').replace(/[^\p{L}\p{N}]/gu, '') || '?').charAt(0).toUpperCase());
    const ini = (s) => `<div class="lente-ini">${inicial(s)}</div>`;
    const cifra = (id, n) => { const el = $(id); el.textContent = num(n); el.className = 'n' + (n > 0 ? ' aviso' : ''); };

    function pintaProgramas(d) {
      const ps = d.programas || [];
      $('qv-programas').innerHTML = ps.length ? ps.map((p) => `
        <div class="qv-fila">
          ${ini(p.programa || 'PC')}
          <div class="qv-fila-cuerpo">
            <div class="qv-fila-tit"><b>${esc(p.programa || t('qv_sin_programa'))}</b>${p.firmante ? `<span class="qv-t3">${esc(t('qv_firmado_por').replace('{f}', p.firmante))}</span>` : ''}<span class="qv-sp"></span><span class="qv-t2">${esc(tn('qv_consultas', p.consultas))}</span></div>
            ${p.empresas.map((e) => `
              <div class="qv-fila-tit"><span>${esc(e.empresa)}</span>${e.origen === 'registro' ? `<span class="lente-pill aviso">${esc(t('qv_registro_ca'))}</span>` : pillCaso(e.caso)}${e.declara && e.declara.includes('g') ? `<span class="lente-pill">${esc(t('qv_declara_ubicacion'))}</span>` : ''}<span class="qv-sp"></span>${e.derechos ? `<a href="${esc(e.derechos)}" target="_blank" rel="noopener">${esc(t('qv_pedir_borrado'))}</a>` : ''}</div>
              <ul class="qv-sub">${e.dominios.map((x) => `<li><span class="qv-mono">${esc(x.dominio)}</span><span class="qv-sp"></span><span class="qv-t3">${esc(tn('qv_veces', x.consultas))} · ${esc(fecha(x.ultima))}</span>${corte(!x.corte, { dominio: x.dominio, regla: x.corte ? x.corte.id : '' }, x.dominio, x.corte && x.corte.desde)}</li>`).join('')}</ul>`).join('')}
          </div>
        </div>`).join('') : bien(t('qv_prog_vacio'));
    }

    function pintaExtensiones(d) {
      const es = d.extensiones || [];
      const marcadas = es.filter((e) => e.caso || e.todas_las_webs || e.chats_ia);
      const otras = es.filter((e) => !marcadas.includes(e));
      $('qv-ext-cuentas').textContent = d.cuentas_miradas ? tn('qv_ext_total', es.length) : '';
      if (!d.cuentas_miradas) { $('qv-extensiones').innerHTML = bien(t('qv_ext_sin_cuentas'), true); return; }
      const fila = (e) => `
        <div class="qv-fila">
          ${ini(e.nombre)}
          <div class="qv-fila-cuerpo">
            <div class="qv-fila-tit"><b>${esc(e.nombre || e.id)}</b><span class="qv-t3">${esc(e.navegador)}${e.version ? ' · ' + esc(e.version) : ''}</span><span class="qv-sp"></span>${e.caso ? pillCaso(e.caso) : ''}${e.todas_las_webs ? `<span class="lente-pill aviso">${esc(t('qv_ext_todas'))}</span>` : (e.chats_ia ? `<span class="lente-pill aviso">${esc(t('qv_ext_chats'))}</span>` : '')}</div>
            ${e.caso ? `<p class="qv-nota">${esc(t('qv_caso_' + e.caso))}</p>` : ''}
            ${e.todas_las_webs || e.chats_ia ? `<p class="qv-nota">${esc(t('qv_ext_lee_ia'))} ${esc(t('qv_ext_quitar_' + String(e.navegador).toLowerCase()))}</p>` : ''}
          </div>
        </div>`;
      $('qv-extensiones').innerHTML = (marcadas.length ? marcadas.map(fila).join('') : bien(es.length ? t('qv_ext_vacio') : t('qv_ext_ninguna')))
        + (otras.length ? `<details class="qv-mas"><summary>${esc(tn('qv_ext_otras', otras.length))}</summary>${otras.map(fila).join('')}</details>` : '');
    }

    function pintaAjustes(d) {
      const as = d.ajustes || [];
      if (d.sistema === 'otro' || !as.length) { $('qv-ajustes').innerHTML = bien(t('qv_aj_otro'), true); return; }
      $('qv-ajustes').innerHTML = as.map((a) => {
        let control = '';
        if (a.por_politica) control = `<span class="lente-pill">${esc(t('qv_aj_politica'))}</span>`;
        else if (a.activo === null || a.activo === undefined) control = `<span class="qv-estado">${esc(t('qv_aj_desconocido'))}</span>`;
        else if (a.cambiable) control = ajuste(a.activo, { ajuste: a.clave }, t('qv_aj_' + a.clave));
        else control = `<span class="qv-estado ${a.activo ? 'aviso' : 'ok'}">${esc(t(a.activo ? 'qv_aj_activado' : 'qv_aj_desactivado'))}</span>`;
        return `<div class="qv-fila"><div class="qv-fila-cuerpo">
          <div class="qv-fila-tit"><b>${esc(t('qv_aj_' + a.clave))}</b><span class="qv-sp"></span>${control}</div>
          <p class="qv-nota">${esc(t('qv_aj_' + a.clave + '_que'))}${a.cuentas > 1 ? ' ' + esc(tn('qv_aj_cuentas', a.cuentas)) : ''}</p>
        </div></div>`;
      }).join('');
    }

    function pintaIa(d) {
      const ls = d.ia || [];
      const leen = (d.extensiones || []).filter((e) => e.chats_ia).length;
      const aviso = leen ? `<div class="qv-fila"><div class="qv-fila-cuerpo"><div class="qv-fila-tit"><span class="qv-estado aviso">${esc(tn('qv_ia_ext', leen))}</span></div></div></div>` : '';
      $('qv-ia').innerHTML = aviso + (ls.length ? ls.map((s) => `
        <div class="qv-fila">
          ${ini(s.servicio)}
          <div class="qv-fila-cuerpo">
            <div class="qv-fila-tit"><b>${esc(s.servicio)}</b>${s.caso ? pillCaso(s.caso) : ''}<span class="qv-sp"></span><span class="qv-t2">${esc(tn('qv_consultas', s.consultas))}</span></div>
            ${s.programas.length ? `<p class="qv-nota">${esc(t('qv_ia_desde'))} ${s.programas.map(esc).join(', ')}</p>` : ''}
            <p class="qv-nota">${esc(t(s.guia ? 'qv_ia_guia_' + s.guia : 'qv_ia_guia_otra'))}</p>
          </div>
        </div>`).join('') : bien(t('qv_ia_vacio'), true));
    }

    function pintaFabricante(d) {
      const fs = d.fabricante || [];
      $('qv-fabricante').innerHTML = fs.length ? fs.map((f) => `
        <div class="qv-fila">
          ${ini(f.marca)}
          <div class="qv-fila-cuerpo">
            <div class="qv-fila-tit"><b>${esc(f.programa)}</b><span class="qv-t3">${esc(f.marca)} · ${esc(f.firmante)}</span><span class="qv-sp"></span><span class="qv-t2">${esc(tn('qv_consultas', f.consultas))}</span></div>
            <p class="qv-nota">${esc(t('qv_fab_habla'))} ${f.destinos.map((x) => `${esc(x[0])} (${num(x[1])})`).join(', ')}</p>
          </div>
        </div>`).join('') : bien(t(d.sistema === 'windows' ? 'qv_fab_vacio' : 'qv_fab_no_windows'), d.sistema !== 'windows');
    }

    function pintaAntivirus(d) {
      if (!d.antivirus_leido) { $('qv-antivirus').innerHTML = bien(t('qv_av_no_leido'), true); return; }
      const as = d.antivirus || [];
      $('qv-antivirus').innerHTML = as.length ? as.map((a) => `
        <div class="qv-fila">${ini(a.nombre)}<div class="qv-fila-cuerpo">
          <div class="qv-fila-tit"><b>${esc(a.nombre)}</b><span class="qv-sp"></span>${a.caso ? pillCaso(a.caso) : `<span class="qv-t3">${esc(t('qv_av_sin_caso'))}</span>`}</div>
          ${a.caso ? `<p class="qv-nota">${esc(t('qv_caso_' + a.caso))}</p>` : ''}
        </div></div>`).join('') : bien(t('qv_av_vacio'), true);
    }

    function pintaTv(d) {
      if (!d.hogar_activo) { $('qv-tv').innerHTML = `<p class="qv-bien neutro">${esc(t('qv_tv_sin_hogar'))} <a href="/hogar">${esc(t('nav_hogar'))}</a></p>`; return; }
      const ts = d.tv || [];
      $('qv-tv').innerHTML = ts.length ? ts.map((x) => `
        <div class="qv-fila"><div class="lente-ini">TV</div><div class="qv-fila-cuerpo">
          <div class="qv-fila-tit"><b>${esc(x.empresa)}</b><span class="qv-mono qv-t3">${esc(x.dominio)}</span><span class="qv-sp"></span>${pillCaso(x.caso)}</div>
          <p class="qv-nota">${esc(tn('qv_tv_aparatos', x.aparatos))}${x.nombres.length ? ': ' + x.nombres.map(esc).join(', ') : ''} · ${esc(tn('qv_consultas', x.consultas))}. ${esc(t('qv_tv_que'))}</p>
        </div></div>`).join('') : bien(t('qv_tv_vacio'));
    }

    function pintaLado(d) {
      const r = d.resumen || {};
      const pend = (r.programas || 0) + (r.extensiones || 0) + (r.ajustes_activos || 0);
      const p = $('qv-pendiente');
      p.hidden = false;
      p.className = 'lente-estado ' + (pend ? 'aviso' : 'ok');
      p.textContent = pend ? tn('qv_pend', pend) : t('qv_pend_nada');
      const apagables = (d.ajustes || []).filter((a) => a.activo === true && a.cambiable && !a.por_politica);
      const b = $('qv-apagar');
      b.hidden = !apagables.length;
      b.textContent = tn('qv_apagar', apagables.length);
      b.onclick = async () => {
        b.disabled = true;
        try {
          for (const a of apagables) await api('/api/quien-vende/ajuste', { method: 'POST', body: { clave: a.clave, activo: false } });
        } catch (e) { p.textContent = motivo(e); }
        b.disabled = false;
        await cargar();
      };
      $('qv-pie').textContent = t('qv_ultima').replace('{h}', hora(d.hasta));
      $('qv-casos').innerHTML = (d.casos || []).slice().sort((x, y) => y.fecha.localeCompare(x.fecha)).map((c) => `<li><time datetime="${esc(c.fecha)}">${esc(dia(c.fecha))} · ${esc(c.medio)}</time>${esc(t('qv_caso_' + c.id))} <a href="${esc(c.url)}" target="_blank" rel="noopener">${esc(t('qv_fuente'))}</a></li>`).join('');
    }

    function pinta(d) {
      D = d;
      const r = d.resumen || {};
      cifra('qv-n-programas', r.programas || 0);
      cifra('qv-n-extensiones', r.extensiones || 0);
      cifra('qv-n-ajustes', r.ajustes_activos || 0);
      const ia = $('qv-n-ia');
      ia.textContent = num(r.ia || 0);
      ia.className = 'n';
      const dias = (d.hasta - d.desde) / 86400000;
      $('qv-ventana').textContent = (dias <= 1.01 ? t('qv_ventana_24h') : t('qv_ventana_7d')) + (d.detalle_desde ? ' · ' + t('qv_desde').replace('{f}', fecha(d.detalle_desde)) : '');
      pintaProgramas(d);
      pintaExtensiones(d);
      pintaAjustes(d);
      pintaIa(d);
      pintaFabricante(d);
      pintaAntivirus(d);
      pintaTv(d);
      pintaLado(d);
    }

    async function cargar() {
      try { pinta(await api('/api/quien-vende')); } catch (e) {
        if (e && e.plus) return;
        $('qv-programas').innerHTML = `<p class="qv-mirando">${esc(String(e.message || e))}</p>`;
      }
    }

    function motivo(e) {
      const m = String((e && e.message) || e);
      if (m === 'politica') return t('qv_aj_politica');
      if (m === 'no_soportado') return t('qv_aj_otro');
      return m;
    }

    if (G) {
      G.engancha($('qv-programas'), async (boton, quiereCortar) => {
        const dom = boton.dataset.dominio;
        if (!dom) return;
        if (quiereCortar) {
          // The server still asks before cutting what looks necessary; a «no» puts the switch back.
          const cuerpo = { scope: 'device', device_id: 'self', match_kind: 'suffix', pattern: dom, action: 'cortar' };
          let r = await api('/api/reglas', { method: 'POST', body: cuerpo });
          if (r && r.necesita === 'confirmar') {
            if (!window.confirm(r.mensaje || t('qv_no_se_pudo'))) throw new Error(t('qv_no_cortado'));
            r = await api('/api/reglas', { method: 'POST', body: Object.assign({}, cuerpo, { confirmed: true }) });
          }
          if (!r || !r.creada) throw new Error((r && r.mensaje) || t('qv_no_se_pudo'));
        } else if (boton.dataset.regla) {
          await api('/api/reglas/' + encodeURIComponent(boton.dataset.regla) + '/deshacer', { method: 'POST' });
        }
        await cargar();
      }, TEXTO_CORTE());
      G.engancha($('qv-ajustes'), async (boton, quiereApagar) => {
        try {
          await api('/api/quien-vende/ajuste', { method: 'POST', body: { clave: boton.dataset.ajuste, activo: !quiereApagar } });
        } catch (e) { throw new Error(motivo(e)); }
        await cargar();
      }, TEXTO_AJUSTE());
    }
    await cargar();
  }
})();
