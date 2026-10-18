// «Certificado de flujos» and «Tu derecho con un clic» (1.0.45). The certificate of a month (or
// of any period) is made by GUARDIANA from this computer's ledger and handed to this page, which
// saves it where the person says: the sealed .json, its .sha256 and the .html to read and print.
// From the same certificate, the companies this computer contacted, each with its letter of
// access or deletion under the person's law. GUARDIANA never sends anything: the letter opens in
// the person's own mail, or goes to the clipboard for the company's form. The name and e-mail
// typed here stay in this page; they are never sent to GUARDIANA's server.
(() => {
  'use strict';
  window.GuardianaDerechos = { iniciar };
  // Mail clients cut or refuse longer mailto: links (as on «Haz que te borren»).
  const MAILTO_MAX = 2000;
  const RECUERDO_PAIS = 'guardiana_derechos_pais';

  async function iniciar(ctx) {
    const { api, t, tn, esc, LOC } = ctx;
    const $ = (id) => document.getElementById(id);
    const fx = (k, d) => Object.entries(d || {}).reduce((s, [a, b]) => s.split('{' + a + '}').join(b), t(k));
    const num = (n) => Number(n || 0).toLocaleString(LOC);
    const dia = (iso) => {
      if (!iso) return '';
      const [y, m, d] = iso.split('-').map(Number);
      return new Date(y, m - 1, d).toLocaleDateString(LOC, { day: 'numeric', month: 'long', year: 'numeric' });
    };
    const mesNombre = (m) => {
      const [y, mm] = m.split('-').map(Number);
      const s = new Date(y, mm - 1, 1).toLocaleDateString(LOC, { month: 'long', year: 'numeric' });
      return s.charAt(0).toUpperCase() + s.slice(1);
    };
    const qs = (o) => new URLSearchParams(Object.entries(o).filter(([, v]) => v !== undefined && v !== null && v !== '')).toString();
    const guardaLocal = (k, v) => { try { localStorage.setItem(k, v); } catch (_) {} };
    const leeLocal = (k) => { try { return localStorage.getItem(k) || ''; } catch (_) { return ''; } };

    // A file saved the way the panel saves its PDFs: the system's dialog when the browser has
    // one, a download otherwise. The bytes are exactly the text GUARDIANA made: the seal of the
    // .json is the SHA-256 of these bytes.
    async function guarda(nombre, texto, tipo) {
      const blob = new Blob([texto], { type: tipo });
      if (typeof window.showSaveFilePicker === 'function') {
        try {
          const ext = '.' + nombre.split('.').pop();
          const h = await window.showSaveFilePicker({ suggestedName: nombre, types: [{ description: ext, accept: { [tipo.split(';')[0]]: [ext] } }] });
          const w = await h.createWritable(); await w.write(blob); await w.close();
          return 'guardado';
        } catch (e) { if (e && e.name === 'AbortError') return 'cancelado'; }
      }
      const a = document.createElement('a');
      a.href = URL.createObjectURL(blob); a.download = nombre;
      document.body.appendChild(a); a.click(); a.remove();
      setTimeout(() => URL.revokeObjectURL(a.href), 10000);
      return 'descargado';
    }
    const avisaGuardado = (el, r, nombre) => { if (el && r !== 'cancelado') el.textContent = t('pdf_' + r).replace('{nombre}', nombre); };

    // The certificate in a window of its own, with the print dialog open: «Guardar como PDF»
    // is the browser's own. The window is blank (no address, no key in it), so the PDF the
    // browser writes carries no link to this panel.
    function verImprimir(html) {
      const w = window.open('', '_blank');
      if (!w) { $('c-guardado').textContent = t('cert_ventana_bloqueada'); return; }
      w.document.open();
      w.document.write(html.split('href="/static/certificado.css"').join('href="' + location.origin + '/static/certificado.css"'));
      w.document.close();
      let hecho = false;
      const imprime = () => { if (hecho) return; hecho = true; try { w.focus(); w.print(); } catch (_) {} };
      w.addEventListener('load', imprime);
      setTimeout(imprime, 800);
    }

    // ----- the certificate ---------------------------------------------------------------
    let C = null;
    const periodo = () => ($('c-mes').value === 'otro'
      ? { desde: $('c-desde').value, hasta: $('c-hasta').value }
      : { mes: $('c-mes').value });

    function pintaCertificado(r) {
      const s = r.resumen;
      $('c-resultado').classList.remove('hidden');
      $('c-periodo').textContent = fx('cert_periodo', { desde: dia(s.desde_dia), hasta: dia(s.hasta_dia) });
      $('c-n-consultas').textContent = num(s.consultas);
      $('c-n-empresas').textContent = num(s.empresas);
      $('c-n-cortadas').textContent = num(s.cortadas);
      $('c-n-no').textContent = num(s.no_contactadas) + (s.referencia ? ' / ' + num(s.referencia) : '');
      $('c-cadena').innerHTML = s.comprobada
        ? `<span class="chip ok">${esc(t('cert_chip_cadena_ok'))}</span>`
        : `<span class="chip alerta">${esc(t('cert_chip_cadena_mal'))}</span>`;
      const avisos = [];
      if (s.registro_desde === null || s.registro_desde === undefined) avisos.push(t('cert_cobertura_vacia'));
      else if (!s.cobertura_completa) avisos.push(t('cert_cobertura_parcial').replace('{fecha}', new Date(s.registro_desde).toLocaleString(LOC)));
      else avisos.push(t('cert_cobertura_completa'));
      if (s.sin_vigilar_ms > 0) avisos.push(t('cert_sin_vigilar_corto').replace('{horas}', (s.sin_vigilar_ms / 3600000).toLocaleString(LOC, { maximumFractionDigits: 1 })));
      if (!s.medido) avisos.push(t('cert_medido_no'));
      $('c-cobertura').textContent = avisos.join(' ');
      $('c-principales').innerHTML = s.principales.map(([e, n, c]) => `<li><a class="enlace-empresa" href="/empresa?e=${encodeURIComponent(e)}">${esc(e)}</a> <span class="muted">${esc(tn('der_carta_veces', n))}${c ? ' · ' + esc(fx('cert_cortadas_n', { n: num(c) })) : ''}</span></li>`).join('');
      $('c-sello').textContent = r.sello;
      $('c-guardado').textContent = '';
    }

    async function hacerCertificado() {
      $('c-msg').textContent = '';
      const p = periodo();
      if (p.desde !== undefined && (!p.desde || !p.hasta)) { $('c-msg').textContent = t('cert_falta_periodo'); return; }
      try {
        C = await api('/api/certificado?' + qs({ ...p, dominios: $('c-dominios').checked ? '1' : '' }));
        pintaCertificado(C);
        await cargaDerechos();
      } catch (e) { $('c-msg').textContent = t('cert_error') + ' ' + String(e.message || e); }
    }

    $('c-form').addEventListener('submit', (ev) => { ev.preventDefault(); hacerCertificado(); });
    $('c-mes').addEventListener('change', () => {
      const otro = $('c-mes').value === 'otro';
      $('c-desde-l').classList.toggle('hidden', !otro);
      $('c-hasta-l').classList.toggle('hidden', !otro);
    });
    $('c-ver').addEventListener('click', () => { if (C) verImprimir(C.html); });
    $('c-html').addEventListener('click', async () => { if (C) avisaGuardado($('c-guardado'), await guarda(C.archivo_html, C.html, 'text/html;charset=utf-8'), C.archivo_html); });
    $('c-json').addEventListener('click', async () => { if (C) avisaGuardado($('c-guardado'), await guarda(C.archivo_json, C.json, 'application/json'), C.archivo_json); });
    $('c-sha').addEventListener('click', async () => { if (C) avisaGuardado($('c-guardado'), await guarda(C.archivo_sha256, C.linea_sha256, 'text/plain;charset=utf-8'), C.archivo_sha256); });
    $('c-archivo').addEventListener('change', async () => {
      const f = $('c-archivo').files && $('c-archivo').files[0];
      const out = $('c-comprobado');
      if (!f) return;
      out.textContent = '';
      try {
        const r = await api('/api/certificado/verificar', { method: 'POST', body: await f.text() });
        const clase = r.estado === 'coincide' ? 'ok' : (r.estado === 'mismas_filas_otra_clasificacion' || r.estado === 'filas_recortadas') ? 'aviso' : 'alerta';
        out.innerHTML = `<span class="chip ${clase}">${esc(t('cert_estado_' + r.estado + '_chip'))}</span> ${esc(fx('cert_estado_' + r.estado, { version: r.programa_certificado, ahora: r.programa_ahora }))}`
          + `<br><span class="muted">${esc(t('cert_sello'))}: <code>${esc(r.sello)}</code></span>`
          + (r.cadena_ok ? '' : `<br><span class="chip alerta">${esc(t('cert_chip_cadena_mal'))}</span>`)
          + (r.medido_coincide || r.estado !== 'coincide' ? '' : `<br><span class="muted">${esc(t('cert_medido_distinto'))}</span>`);
      } catch (e) { out.textContent = String(e.message || e); }
    });

    // ----- your right, with one click -----------------------------------------------------
    let D = null;
    const jur = (c) => (D ? D.jurisdicciones.find((j) => j.codigo === c) : null);
    const porDefecto = () => {
      const guardado = leeLocal(RECUERDO_PAIS);
      if (guardado && jur(guardado)) return guardado;
      const partes = (navigator.language || '').split('-');
      const idioma = (partes[0] || '').toLowerCase();
      const region = (partes[1] || '').toUpperCase();
      if (region === 'CA' && idioma === 'fr') return 'ca_qc';
      const j = D.jurisdicciones.find((x) => x.regiones.includes(region));
      return j ? j.codigo : 'otro';
    };
    const pintaLey = () => {
      const j = jur($('d-pais').value);
      if (!j) return;
      $('d-ley').textContent = $('d-tipo').value === 'acceso' ? j.corta_acceso : j.corta;
      $('d-nota').textContent = j.nota;
    };
    const rellena = (c) => c.cuerpo.split('[[NOMBRE]]').join($('d-nombre').value.trim() || c.hueco_nombre).split('[[CORREO]]').join($('d-correo').value.trim() || c.hueco_correo);
    const copia = async (s) => { try { await navigator.clipboard.writeText(s); return true; } catch (_) { return false; } };
    const rangoTexto = (rs) => rs.map(([a, z]) => (a === z ? dia(a) : fx('der_rango', { desde: dia(a), hasta: dia(z) }))).join(', ');
    const chipExp = (x) => (x ? `<a class="chip ${x.vencido ? 'alerta' : x.aviso ? 'aviso' : 'info'}" href="/expedientes">${esc(x.estado_texto)}</a> <span class="muted">${esc(x.frase)}</span>` : '');

    function pintaDerechos() {
      $('d-periodo').textContent = fx('der_periodo', { desde: dia(D.desde_dia), hasta: dia(D.hasta_dia) })
        + (D.cobertura_completa ? '' : ' ' + t('der_cobertura_parcial'))
        + (D.comprobada ? '' : ' ' + t('cert_chip_cadena_mal'));
      $('d-vacio').classList.toggle('hidden', D.empresas.length > 0);
      $('d-cortadas').classList.toggle('hidden', !D.solo_cortadas.length);
      $('d-cortadas').textContent = D.solo_cortadas.length ? fx('der_solo_cortadas', { empresas: D.solo_cortadas.join(', ') }) : '';
      $('d-lista').innerHTML = D.empresas.map((e, i) => {
        const canal = e.url
          ? `<a href="${esc(e.url)}" target="_blank" rel="noopener noreferrer">${esc(t(e.curado ? 'der_canal_curado' : 'der_canal_web'))}</a>`
          : esc(t('der_canal_ninguno'));
        const fuente = e.curado && e.fuentes.length ? ` <span class="muted">${esc(fx('der_canal_fuente', { fecha: dia(e.fuentes[0][1]) }))}</span>` : '';
        return `<article class="card d-empresa" data-i="${i}">
          <div class="d-cabeza"><h3><a class="enlace-empresa" href="/empresa?e=${encodeURIComponent(e.empresa)}">${esc(e.empresa)}</a></h3>${e.pais_nombre ? `<span class="chip">${esc(e.pais_nombre)}</span>` : ''} ${chipExp(e.expediente)}</div>
          <p>${esc(fx('der_resumen_empresa', { veces: tn('der_carta_veces', e.contactos), dias: tn('der_carta_dias', e.dias) }))}</p>
          <p class="muted">${esc(rangoTexto(e.rangos.slice(0, 6)))}${e.rangos.length > 6 ? ' …' : ''}</p>
          <p class="muted">${esc(t('der_canal'))} ${canal}${fuente}</p>
          <p class="d-acciones"><button type="button" class="boton principal d-preparar" data-i="${i}">${esc(t('der_preparar'))}</button></p>
          <div class="d-carta hidden"></div>
          <p class="d-msg muted" role="status" aria-live="polite"></p>
        </article>`;
      }).join('');
    }

    async function cargaDerechos() {
      const p = periodo();
      D = await api('/api/derechos?' + qs(p));
      if (!$('d-pais').options.length) {
        $('d-pais').innerHTML = D.jurisdicciones.map((j) => `<option value="${esc(j.codigo)}">${esc(j.nombre)}</option>`).join('');
        $('d-pais').value = porDefecto();
        const j = jur($('d-pais').value);
        if (j) $('d-idioma').value = j.idioma;
      }
      pintaLey();
      pintaDerechos();
    }

    $('d-pais').addEventListener('change', () => {
      guardaLocal(RECUERDO_PAIS, $('d-pais').value);
      const j = jur($('d-pais').value);
      if (j) $('d-idioma').value = j.idioma;
      pintaLey();
      document.querySelectorAll('.d-carta').forEach((c) => { c.classList.add('hidden'); c.innerHTML = ''; });
    });
    $('d-tipo').addEventListener('change', () => {
      pintaLey();
      document.querySelectorAll('.d-carta').forEach((c) => { c.classList.add('hidden'); c.innerHTML = ''; });
    });

    const cartas = new Map();
    async function prepara(i, caja, msg) {
      const e = D.empresas[i];
      const p = periodo();
      const R = await api('/api/derechos/carta?' + qs({ empresa: e.empresa, jurisdiccion: $('d-pais').value, tipo: $('d-tipo').value, idioma: $('d-idioma').value, ...p }));
      cartas.set(i, R);
      const c = R.carta;
      caja.innerHTML = `<p><b>${esc(t('der_para'))}</b> ${esc(c.para || t('der_para_formulario'))}</p>
        <p><b>${esc(t('der_asunto'))}</b> ${esc(c.asunto)}</p>
        <pre class="d-texto">${esc(rellena(c))}</pre>
        <p class="d-acciones">
          <button type="button" class="boton principal d-correo">${esc(t('der_abrir_correo'))}</button>
          <button type="button" class="boton d-copiar">${esc(t('der_copiar'))}</button>
          ${c.url ? `<a class="boton" href="${esc(c.url)}" target="_blank" rel="noopener noreferrer">${esc(t('der_abrir_formulario'))}</a>` : ''}
        </p>
        <p class="muted">${esc(t('der_adjunta'))}</p>
        <p class="d-acciones">
          <button type="button" class="boton d-ex-html">${esc(t('der_extracto_html'))}</button>
          <button type="button" class="boton d-ex-json">${esc(t('der_extracto_json'))}</button>
          <button type="button" class="boton d-ex-sha">${esc(t('cert_guardar_sello'))}</button>
        </p>
        <p class="d-acciones">
          <button type="button" class="boton d-exp-preparada">${esc(t('der_expediente_abrir'))}</button>
          <button type="button" class="boton d-exp-enviada">${esc(t('der_expediente_enviada'))}</button>
        </p>`;
      caja.classList.remove('hidden');
      msg.textContent = '';
    }

    async function abreExpediente(i, estado, msg) {
      const e = D.empresas[i];
      const lista = await api('/api/expedientes', { method: 'POST', body: { empresa: e.empresa, jurisdiccion: $('d-pais').value, tipo: $('d-tipo').value, estado } });
      const x = lista.find((y) => y.empresa === e.empresa) || lista[0];
      e.expediente = x || null;
      msg.innerHTML = (x ? esc(x.frase) + ' ' : '') + `<a href="/expedientes">${esc(t('der_ver_expedientes'))}</a>`;
      const cab = msg.closest('.d-empresa').querySelector('.d-cabeza');
      const viejo = cab.querySelectorAll('a.chip, .d-cabeza > span.muted');
      viejo.forEach((n) => n.remove());
      cab.insertAdjacentHTML('beforeend', ' ' + chipExp(x));
    }

    $('d-lista').addEventListener('click', async (ev) => {
      const b = ev.target.closest('button');
      if (!b) return;
      const art = b.closest('.d-empresa');
      const i = Number(art.dataset.i);
      const caja = art.querySelector('.d-carta');
      const msg = art.querySelector('.d-msg');
      try {
        if (b.classList.contains('d-preparar')) { await prepara(i, caja, msg); return; }
        const R = cartas.get(i);
        if (!R) return;
        const c = R.carta, x = R.extracto;
        if (b.classList.contains('d-correo')) {
          const texto = rellena(c);
          const mailto = (a, cuerpo) => 'mailto:' + encodeURIComponent(c.para || '').replace(/%40/g, '@') + '?subject=' + encodeURIComponent(a) + '&body=' + encodeURIComponent(cuerpo);
          let url = mailto(c.asunto, texto);
          const pistas = [c.para ? 'borrado_pista_correo' : 'ficha_carta_pista_sin_para'];
          if (url.length > MAILTO_MAX) { await copia(c.asunto + '\n\n' + texto); url = mailto(c.asunto, t('borrado_mailto_pega')); pistas.splice(0, pistas.length, 'borrado_pista_pegar_correo'); }
          location.href = url;
          msg.textContent = pistas.map((k) => t(k)).join(' ') + ' ' + t('der_adjunta');
        } else if (b.classList.contains('d-copiar')) {
          msg.textContent = t((await copia(c.asunto + '\n\n' + rellena(c))) ? 'borrado_copiado' : 'ficha_no_copiado');
        } else if (b.classList.contains('d-ex-html')) {
          avisaGuardado(msg, await guarda(x.archivo_html, x.html, 'text/html;charset=utf-8'), x.archivo_html);
        } else if (b.classList.contains('d-ex-json')) {
          avisaGuardado(msg, await guarda(x.archivo_json, x.json, 'application/json'), x.archivo_json);
        } else if (b.classList.contains('d-ex-sha')) {
          avisaGuardado(msg, await guarda(x.archivo_sha256, x.linea_sha256, 'text/plain;charset=utf-8'), x.archivo_sha256);
        } else if (b.classList.contains('d-exp-preparada')) {
          await abreExpediente(i, 'preparada', msg);
        } else if (b.classList.contains('d-exp-enviada')) {
          await abreExpediente(i, 'enviada', msg);
        }
      } catch (e) { msg.textContent = String(e.message || e); }
    });
    // The name and the e-mail fill the letter as they are typed, in this page only.
    ['d-nombre', 'd-correo'].forEach((id) => $(id).addEventListener('input', () => {
      cartas.forEach((R, i) => {
        const art = document.querySelector(`.d-empresa[data-i="${i}"]`);
        const pre = art && art.querySelector('.d-texto');
        if (pre) pre.textContent = rellena(R.carta);
      });
    }));

    // ----- first load: the last whole month ----------------------------------------------
    try {
      C = await api('/api/certificado');
      $('c-mes').innerHTML = C.meses.map((m) => `<option value="${esc(m)}">${esc(mesNombre(m))}</option>`).join('')
        + `<option value="otro">${esc(t('cert_otro_periodo'))}</option>`;
      $('c-mes').value = C.mes_anterior;
      pintaCertificado(C);
      await cargaDerechos();
    } catch (e) { $('c-msg').textContent = t('cert_error') + ' ' + String(e.message || e); }
  }
})();
