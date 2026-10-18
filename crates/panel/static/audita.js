// «Audita a GUARDIANA» (1.0.46, Privacidad). One request (/api/audita) runs the whole audit on
// this computer: the installed program against the public record, every connection GUARDIANA
// made from its own records, and what it keeps on disk. The report is saved exactly as the
// program sealed it (the JSON bytes it hashed), its `.sha256` line and a page to print. It works
// at the free level too. app.js hands over the session, the texts and the language (`iniciar`).
(() => {
  'use strict';
  window.GuardianaAudita = { iniciar };

  async function iniciar(ctx) {
    const { api, t, esc, LOC } = ctx;
    const $ = (id) => document.getElementById(id);
    const num = (n) => Number(n || 0).toLocaleString(LOC);
    // «1 consulta» / «1,234 queries»: the singular when there is one, the locale's separators always.
    const tn = (k, n) => t(n === 1 && t(k + '_uno') !== k + '_uno' ? k + '_uno' : k).replace('{n}', num(n));
    const cuando = (ms) => new Date(ms).toLocaleString(LOC, { dateStyle: 'medium', timeStyle: 'short' });
    const hora = (ms) => new Date(ms).toLocaleTimeString(LOC, { hour: '2-digit', minute: '2-digit' });
    const tam = (b) => b >= 10485760 ? num(Math.ceil(b / 1048576)) + ' MB' : (b >= 1024 ? num(Math.ceil(b / 1024)) + ' KB' : num(b) + ' B');
    const existe = (k) => t(k) !== k;
    let V = null;

    const guarda = (texto, nombre, tipo) => {
      const a = document.createElement('a');
      a.href = URL.createObjectURL(new Blob([texto], { type: tipo }));
      a.download = nombre;
      document.body.appendChild(a);
      a.click();
      setTimeout(() => { URL.revokeObjectURL(a.href); a.remove(); }, 1000);
    };

    const estado = (clase, texto) => `<span class="qv-estado ${clase}">${esc(texto)}</span>`;
    function registro(m) {
      if (!m) return estado('', t('au_reg_sin_archivo'));
      if (m.found) return estado('ok', t('au_reg_coincide').replace('{v}', m.found.version).replace('{fecha}', String(m.found.date || '').slice(0, 10)));
      if (m === 'not_found') return estado('aviso', t('au_reg_no'));
      if (m.zip_only) return estado('', t('au_reg_zip'));
      return estado('', t('au_reg_sin_archivo'));
    }

    function pintaInstalado(i) {
      $('au-inst-estado').innerHTML = i.coincide ? estado('ok', t('au_inst_coincide_corto')) : estado('aviso', t('au_inst_revisar'));
      const fila = (nombre, sha, reg) => `<div class="qv-fila"><div class="qv-fila-cuerpo">
          <div class="qv-fila-tit"><b class="qv-mono">${esc(nombre)}</b><span class="qv-sp"></span>${registro(reg)}</div>
          <p class="qv-nota qv-mono">SHA-256 ${esc(sha || '—')}</p></div></div>`;
      $('au-instalado').innerHTML = fila(i.binario, i.sha256, i.registro)
        + i.otros.map((o) => fila(o.nombre, o.sha256, o.registro)).join('')
        + `<div class="qv-fila"><div class="qv-fila-cuerpo">
            <div class="qv-fila-tit"><span>${esc(t('au_firma'))}</span><span class="qv-sp"></span>${estado(i.firma === 'valid' ? 'ok' : (i.firma === 'invalid' ? 'alerta' : ''), t('au_firma_' + i.firma))}</div>
            ${i.rekor_url ? `<p class="qv-nota"><a href="${esc(i.rekor_url)}" target="_blank" rel="noopener">${esc(t('au_rekor'))}</a></p>` : ''}
            ${i.registro_zip ? `<p class="qv-nota">${esc(t('au_reg_zip_que'))}</p>` : ''}
            ${!i.registro_local ? `<p class="qv-nota">${esc(t('au_reg_sin_archivo_que'))}</p>` : ''}
          </div></div>`;
    }

    function pintaConexiones(c) {
      $('au-con-resumen').textContent = c.por_motivo.map(([m, n]) => t('proposito_' + m) + ' ' + num(n)).join(' · ');
      const filas = c.lista.map((x) => `<tr><td>${esc(cuando(x.ts))}</td><td>${esc(t('proposito_' + x.motivo))}</td><td class="qv-mono">${esc(x.host)}</td><td class="lente-num">${esc(num(x.bytes))}</td><td>${esc(t(x.pedida ? 'au_pedida' : 'au_programada'))}</td><td>${esc(t('au_origen_' + x.origen))}</td></tr>`).join('');
      $('au-conexiones').innerHTML = (c.lista.length
        ? `<div class="lente-tabla"><table><thead><tr><th>${esc(t('au_col_fecha'))}</th><th>${esc(t('au_col_motivo'))}</th><th>${esc(t('au_col_destino'))}</th><th class="lente-num">${esc(t('au_col_bytes'))}</th><th>${esc(t('au_col_quien'))}</th><th>${esc(t('au_col_origen'))}</th></tr></thead><tbody>${filas}</tbody></table></div>`
        : `<p class="qv-bien neutro">${esc(t('au_con_vacio'))}</p>`)
        + (c.total > c.lista.length ? `<p class="qv-nota qv-fila">${esc(tn('au_con_mas', c.total - c.lista.length))}</p>` : '')
        + `<p class="qv-nota qv-fila">${esc(t('au_con_nota'))}</p>`;
    }

    function pintaDisco(d) {
      $('au-disco-total').textContent = tam(d.bytes_carpeta);
      const ip = d.base.direcciones_publicas;
      const locales = d.base.direcciones_locales;
      const linea = ip === 0
        ? `<div class="qv-fila"><div class="qv-fila-cuerpo"><div class="qv-fila-tit">${estado('ok', t('au_ip_ninguna'))}</div>${locales ? `<p class="qv-nota">${esc(tn('au_ip_locales', locales))}</p>` : ''}<p class="qv-nota">${esc(t('au_contenido'))}</p></div></div>`
        : `<div class="qv-fila"><div class="qv-fila-cuerpo"><div class="qv-fila-tit">${estado('aviso', t('au_ip_hay').replace('{n}', num(ip)))}</div></div></div>`;
      const tablas = d.base.tablas.map((tb) => {
        const desc = existe('au_tabla_' + tb.nombre) ? t('au_tabla_' + tb.nombre) : '';
        const cols = tb.columnas.map((c) => `<li><span class="qv-mono">${esc(c.nombre)}</span><span class="qv-t3">${esc(c.tipo)}</span><span class="qv-sp"></span>${c.direcciones_publicas ? estado('aviso', t('au_col_ip_publica').replace('{n}', num(c.direcciones_publicas))) : (c.direcciones_locales ? `<span class="qv-t2">${esc(t('au_col_ip_local').replace('{n}', num(c.direcciones_locales)))}</span>` : '')}</li>`).join('');
        return `<tr><td><details><summary class="qv-mono">${esc(tb.nombre)}</summary><ul class="qv-sub">${cols}</ul></details>${desc ? `<div class="qv-nota">${esc(desc)}</div>` : ''}</td><td class="lente-num">${esc(num(tb.filas))}</td><td class="lente-num">${tb.bytes === null || tb.bytes === undefined ? '—' : esc(tam(tb.bytes))}</td></tr>`;
      }).join('');
      const archivos = d.archivos.map(([n, b]) => `<li><span class="qv-mono">${esc(n)}</span><span class="qv-sp"></span><span class="qv-t2">${esc(tam(b))}</span></li>`).join('');
      $('au-disco').innerHTML = linea
        + `<div class="lente-tabla"><table><thead><tr><th>${esc(t('au_col_tabla'))}</th><th class="lente-num">${esc(t('au_col_filas'))}</th><th class="lente-num">${esc(t('au_col_tamano'))}</th></tr></thead><tbody>${tablas}</tbody></table></div>`
        + `<details class="qv-mas"><summary>${esc(tn('au_archivos', d.archivos.length))} · <span class="qv-mono">${esc(d.carpeta)}</span></summary><ul class="qv-sub qv-fila">${archivos}</ul></details>`;
    }

    function pinta(v) {
      V = v;
      const i = v.informe;
      const ok = $('au-n-instalado');
      ok.textContent = t(i.instalado.coincide ? 'au_si' : 'au_revisar');
      ok.className = 'n chico ' + (i.instalado.coincide ? 'ok' : 'aviso');
      $('au-n-conexiones').textContent = num(i.conexiones.total);
      $('au-n-conexiones').className = 'n';
      $('au-n-tablas').textContent = num(i.disco.base.tablas.length);
      $('au-n-tablas').className = 'n';
      const ip = $('au-n-ip');
      ip.textContent = num(i.disco.base.direcciones_publicas);
      ip.className = 'n ' + (i.disco.base.direcciones_publicas ? 'aviso' : 'ok');
      pintaInstalado(i.instalado);
      pintaConexiones(i.conexiones);
      pintaDisco(i.disco);
      const bien = i.instalado.coincide && i.disco.base.direcciones_publicas === 0 && i.cadena.comprobada;
      const ver = $('au-veredicto');
      ver.className = 'lente-estado ' + (bien ? 'ok' : 'aviso');
      ver.textContent = t(bien ? 'au_veredicto_bien' : 'au_veredicto_revisar');
      $('au-sello').textContent = 'SHA-256 ' + v.sello;
      $('au-json').disabled = false;
      $('au-html').disabled = false;
      $('au-pie').textContent = t('au_ultima').replace('{h}', hora(i.generado));
    }

    async function auditar() {
      const b = $('au-auditar');
      b.disabled = true;
      $('au-veredicto').className = 'lente-estado';
      $('au-veredicto').textContent = t('au_trabajando_corto');
      try { pinta(await api('/api/audita')); } catch (e) {
        $('au-instalado').innerHTML = `<p class="qv-mirando">${esc(String(e.message || e))}</p>`;
      } finally { b.disabled = false; }
    }

    $('au-auditar').addEventListener('click', auditar);
    $('au-json').addEventListener('click', () => {
      if (!V) return;
      guarda(V.json, V.archivo_json, 'application/json');
      guarda(V.linea_sha256, V.archivo_sha256, 'text/plain');
    });
    $('au-html').addEventListener('click', () => { if (V) guarda(V.html, V.archivo_html, 'text/html'); });
    await auditar();
  }
})();
