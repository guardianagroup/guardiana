// «Sala de entrega» (18 Oct 2026, the committee and the owner): files of any kind and size, for
// someone outside the company, with a link that works once and a separate code. This page only
// hands the files to the program in pieces and draws what the program says
// (crates/zero/src/entrega.rs); it never sees a key, and the program never sees an address.
'use strict';
(() => {
  const { manda, en, t, esc, $ } = Z;
  const caja = $('sala-entrega');
  const abrir = $('ent-abrir');
  if (!caja || !abrir) return;
  const PIEZA = 64 * 1024, MIB = 1024 * 1024;
  let lista = { entregas: [] }, subiendo = null, idioma = 'es';
  en('textos', (m) => { idioma = m.idioma || 'es'; pinta(); });
  const tam = (b) => {
    const u = ['B', 'KB', 'MB', 'GB', 'TB']; let i = 0, v = Number(b || 0);
    while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
    return (i ? v.toFixed(v < 10 ? 1 : 0) : String(v)) + ' ' + u[i];
  };
  const hora = (s) => (s ? new Date(s * 1000).toLocaleString(idioma, { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' }) : '—');
  const pide = () => manda({ tipo: 'secure', accion: 'entrega_estado' });

  // Shown instead of the start screen, and back.
  const muestra = (si) => {
    caja.hidden = !si;
    // 1.0.46 (C2-a): a 400 px panel on the right, over what is open (sala.css), so a
    // conversation stays on screen behind it.
    document.body.classList.toggle('sala-con-entrega', si);
    if (si) { pide(); pinta(); }
  };
  abrir.addEventListener('click', () => muestra(true));

  const ESTADO = { preparando: '', esperando: 'info', abierta: 'info', entregada: 'ok', caducada: '', revocada: '', destruida: 'alerta' };
  // 1.0.46 (C2-a, «Sala de entrega» as a 400 px panel): the four steps as they really go, and
  // the bytes, the speed and the time left measured from the receiver's own verified pieces
  // between two updates — never estimated beforehand.
  const muestras = new Map();
  const duracion = (s) => { s = Math.max(0, Math.round(s)); return s < 60 ? s + ' s' : Math.floor(s / 60) + ' min' + (s % 60 ? ' ' + (s % 60) + ' s' : ''); };
  function cifras(e) {
    const p = e.progreso || {}, total = p.trozos_total || 0;
    if (e.estado !== 'abierta' || !total || !e.bytes) { muestras.delete(e.id); return ''; }
    const hecho = Math.min(e.bytes, Math.round(e.bytes * (p.verificados || 0) / total));
    const ahora = Date.now(), m = muestras.get(e.id);
    let v = m ? m.v : 0;
    if (!m || hecho !== m.hecho) {
      if (m && ahora > m.t) v = Math.max(0, (hecho - m.hecho) / ((ahora - m.t) / 1000));
      muestras.set(e.id, { t: ahora, hecho, v });
    }
    if (!v) return `<p class="ent-cifras">${esc(tam(hecho) + ' / ' + tam(e.bytes))}</p>`;
    return `<p class="ent-cifras">${esc(t('sala_ent_cifras', { hecho: tam(hecho), total: tam(e.bytes), v: tam(v), r: duracion((e.bytes - hecho) / v) }))}</p>`;
  }
  function pasos(e) {
    const orden = ['preparando', 'esperando', 'abierta', 'entregada'];
    const i = orden.indexOf(e.estado);
    if (i < 0) return '';
    const hecho = (k) => (k < i || (k === 3 && i === 3) ? ' hecho' : k === i ? ' ahora' : '');
    return `<ol class="ent-pasos">${['sala_ent_p1', 'sala_ent_p2', 'sala_ent_p3', 'sala_ent_p4'].map((c, k) => `<li class="${hecho(k)}">${esc(t(c))}</li>`).join('')}</ol>`;
  }
  function tarjeta(e) {
    const p = e.progreso || {};
    const total = p.trozos_total || 0;
    const viaje = e.estado === 'abierta' || e.estado === 'entregada'
      // A → B, with the real counters: pieces sent from here, pieces the receiver verified.
      ? `<div class="ent-viaje"><span>${esc(t('ent_tu_pc'))}</span><i></i><span>${esc(t('ent_rele'))}</span><i></i><span>${esc(t('ent_destino'))}</span></div>`
        + `<p class="ent-cifras">${esc(t('ent_cifras', { enviados: p.trozos_enviados || 0, total, ok: p.verificados || 0, mal: p.fallidos || 0 }))}</p>`
        + `<div class="ent-barra"><i style="width:${total ? Math.min(100, Math.round(100 * (p.verificados || 0) / total)) : 0}%"></i></div>`
      : '';
    const enlace = e.enlace
      ? `<div class="ent-enlace"><div class="qr">${e.qr || ''}</div><div><span class="rotulo">${esc(t('ent_enlace'))}</span>`
        + `<input class="campo" readonly value="${esc(e.enlace)}"><div class="c-fila"><button type="button" class="boton chico" data-copia="${esc(e.enlace)}">${esc(t('ent_copiar_enlace'))}</button></div>`
        + `<span class="rotulo">${esc(t('ent_codigo'))}</span><b class="ent-codigo">${esc(e.codigo || '')}</b>`
        + `<p class="t">${esc(t('ent_codigo_aparte'))}</p></div></div>`
      : '';
    const archivos = (e.archivos || []).map((a) => `<li><span>${esc(a.nombre)}</span><span>${esc(tam(a.bytes))}</span><span>${a.recibido ? '✓' : ''}</span></li>`).join('');
    const motivo = e.estado === 'destruida' ? `<p class="ent-motivo">${esc(t('ent_motivo_' + e.motivo))}</p>` : '';
    return `<article class="tarjeta ent-una" data-id="${esc(e.id)}"><div class="ent-cab"><b>${esc(e.titulo || t('ent_sin_titulo'))}</b>`
      + `<span class="chip ${ESTADO[e.estado] || ''}">${esc(t('ent_estado_' + e.estado))}</span></div>`
      + `<p class="t">${esc(t('ent_resumen', { n: (e.archivos || []).length, bytes: tam(e.bytes), vence: hora(e.vence), cad: t('ent_cad_' + e.caducidad) }))}</p>`
      + motivo + pasos(e) + enlace + viaje + cifras(e) + `<ul class="ent-archivos">${archivos}</ul>`
      + `<div class="c-fila">`
      + (e.recibo ? `<button type="button" class="boton chico principal" data-recibo="${esc(e.id)}">${esc(t('ent_recibo'))}</button>` : '')
      + (['esperando', 'abierta', 'preparando'].includes(e.estado) ? `<button type="button" class="boton chico corta" data-revoca="${esc(e.id)}">${esc(t('ent_revocar'))}</button>` : '')
      + (['entregada', 'caducada', 'revocada', 'destruida'].includes(e.estado) ? `<button type="button" class="boton chico" data-olvida="${esc(e.id)}">${esc(t('ent_quitar'))}</button>` : '')
      + `</div></article>`;
  }

  function pinta() {
    if (caja.hidden) return;
    const activo = document.activeElement;
    if (activo && caja.contains(activo) && (activo.tagName === 'SELECT' || activo.tagName === 'INPUT') && activo.type !== 'file' && !activo.readOnly) return;
    caja.innerHTML = `<div class="ent-arriba"><button type="button" class="boton chico secundario" id="ent-volver">${esc(t('sec_volver'))}</button><h2>${esc(t('ent_titulo'))}</h2></div>`
      + `<p class="t">${esc(t('ent_frase'))}</p>`
      + `<form class="ent-nueva" id="ent-form" autocomplete="off">`
      + `<label class="ent-soltar" id="ent-soltar"><input type="file" id="ent-archivos" multiple hidden><span>${esc(t('ent_elige'))}</span></label>`
      + `<div class="c-fila"><button type="button" class="enlace" id="ent-carpeta-b">${esc(t('ent_carpeta'))}</button><input type="file" id="ent-carpeta" webkitdirectory multiple hidden></div>`
      + `<ul class="ent-archivos" id="ent-elegidos"></ul>`
      + `<label class="c-nombre"><span>${esc(t('ent_titulo_campo'))}</span><input class="campo" id="ent-nombre" maxlength="80"></label>`
      + `<div class="c-fila"><label class="c-nombre"><span>${esc(t('ent_caducidad'))}</span><select class="campo" id="ent-cad"><option value="24h">${esc(t('ent_cad_1d'))}</option><option value="7d">${esc(t('ent_cad_7d'))}</option><option value="primera_descarga">${esc(t('ent_cad_primera_descarga'))}</option></select></label>`
      + `<label class="c-nombre"><span>${esc(t('ent_digitos'))}</span><select class="campo" id="ent-dig"><option value="4">4</option><option value="6" selected>6</option><option value="8">8</option></select></label></div>`
      + `<label class="ent-casilla"><input type="checkbox" id="ent-nombres" checked><span>${esc(t('ent_nombres'))}</span></label>`
      + `<div class="c-fila"><button type="submit" class="boton principal" id="ent-crear" disabled>${esc(t('ent_crear'))}</button></div>`
      + `<p class="t ent-subiendo" id="ent-subiendo" ${textoSubiendo ? '' : 'hidden'}>${esc(textoSubiendo)}</p>`
      + `</form>`
      + `<section class="ent-como"><span class="rotulo">${esc(t('ent_como_titulo'))}</span><p class="t">${esc(t('ent_como'))}</p><p class="t">${esc(t('ent_limite'))}</p>`
      + `<div class="c-fila"><button type="button" class="boton chico" id="ent-libro">${esc(t('ent_libro'))}</button></div>`
      + (lista.dpapi === false ? `<p class="t">${esc(t('ent_sin_dpapi'))}</p>` : '')
      + (lista.huella ? `<p class="t">${esc(t('ent_huella', { h: lista.huella }))}</p>` : '') + `</section>`
      + `<div class="ent-lista">${(lista.entregas || []).map(tarjeta).join('')}</div>`;
    engancha();
  }

  let elegidos = [];
  function engancha() {
    $('ent-volver').addEventListener('click', () => muestra(false));
    const alElegir = (files) => {
      elegidos = Array.from(files || []);
      $('ent-elegidos').innerHTML = elegidos.map((f) => `<li><span>${esc(f.webkitRelativePath || f.name)}</span><span>${esc(tam(f.size))}</span></li>`).join('');
      $('ent-crear').disabled = !elegidos.length || !!subiendo;
    };
    $('ent-archivos').addEventListener('change', (e) => alElegir(e.target.files));
    $('ent-carpeta-b').addEventListener('click', () => $('ent-carpeta').click());
    $('ent-carpeta').addEventListener('change', (e) => alElegir(e.target.files));
    const zona = $('ent-soltar');
    zona.addEventListener('dragover', (e) => { e.preventDefault(); zona.classList.add('encima'); });
    zona.addEventListener('dragleave', () => zona.classList.remove('encima'));
    zona.addEventListener('drop', (e) => { e.preventDefault(); zona.classList.remove('encima'); alElegir(e.dataTransfer.files); });
    alElegir(elegidos);
    $('ent-form').addEventListener('submit', (e) => { e.preventDefault(); sube(); });
    $('ent-libro').addEventListener('click', () => manda({ tipo: 'secure', accion: 'entrega_libro' }));
    caja.querySelectorAll('[data-copia]').forEach((b) => b.addEventListener('click', async () => { try { await navigator.clipboard.writeText(b.dataset.copia); b.textContent = t('ent_copiado'); } catch (_) {} }));
    caja.querySelectorAll('[data-revoca]').forEach((b) => b.addEventListener('click', () => manda({ tipo: 'secure', accion: 'entrega_revocar', id: b.dataset.revoca })));
    caja.querySelectorAll('[data-olvida]').forEach((b) => b.addEventListener('click', () => manda({ tipo: 'secure', accion: 'entrega_olvidar', id: b.dataset.olvida })));
    caja.querySelectorAll('[data-recibo]').forEach((b) => b.addEventListener('click', () => manda({ tipo: 'secure', accion: 'entrega_recibo', id: b.dataset.recibo })));
  }

  const b64De = (buf) => { let s = ''; const u = new Uint8Array(buf); for (let i = 0; i < u.length; i += 0x8000) s += String.fromCharCode.apply(null, u.subarray(i, i + 0x8000)); return btoa(s); };
  const espera = (ms) => new Promise((r) => setTimeout(r, ms));
  // Pieces handed over only as fast as the program seals them (8 MiB waiting at most).
  const sellados = () => { const e = (lista.entregas || []).find((x) => x.estado === 'preparando'); return e ? (e.sellados || 0) : 0; };
  let textoSubiendo = '';

  async function sube() {
    if (subiendo || !elegidos.length) return;
    const archivos = elegidos.slice();
    subiendo = { hecho: 0, total: archivos.reduce((a, f) => a + f.size, 0) };
    const aviso = (txt) => { textoSubiendo = txt; const p = $('ent-subiendo'); if (p) { p.hidden = !txt; p.textContent = txt; } };
    manda({ tipo: 'secure', accion: 'entrega_nueva', caducidad: $('ent-cad').value, digitos: Number($('ent-dig').value), nombres: $('ent-nombres').checked, titulo: $('ent-nombre').value });
    await espera(150);
    try {
      let antes = 0;
      for (const f of archivos) {
        manda({ tipo: 'secure', accion: 'entrega_archivo', nombre: f.webkitRelativePath || f.name, bytes: f.size });
        const piezas = Math.max(1, Math.ceil(f.size / PIEZA));
        for (let i = 0; i < piezas; i++) {
          const debe = antes + Math.floor((i * PIEZA) / MIB);
          while (debe - sellados() > 8) { pide(); await espera(120); }
          const trozo = await f.slice(i * PIEZA, (i + 1) * PIEZA).arrayBuffer();
          manda({ tipo: 'secure', accion: 'entrega_trozo', b64: b64De(trozo), ultimo: i + 1 === piezas });
          subiendo.hecho += trozo.byteLength;
          if (i % 32 === 0) { aviso(t('ent_sellando', { n: f.webkitRelativePath || f.name, hecho: tam(subiendo.hecho), total: tam(subiendo.total) })); pide(); }
          await espera(0);
        }
        antes += Math.max(1, Math.ceil(f.size / MIB));
      }
      manda({ tipo: 'secure', accion: 'entrega_lista', idioma });
      elegidos = [];
    } finally {
      subiendo = null;
      aviso('');
      pinta();
    }
  }

  const descarga = (nombre, texto) => {
    const a = document.createElement('a');
    a.href = URL.createObjectURL(new Blob([texto], { type: 'application/json' }));
    a.download = nombre; document.body.appendChild(a); a.click();
    setTimeout(() => { URL.revokeObjectURL(a.href); a.remove(); }, 1000);
  };
  en('entregas', (m) => { lista = m; pinta(); });
  en('entrega_recibo', (m) => descarga(`guardiana-recibo-entrega-${m.id}.json`, m.json));
  en('entrega_libro', (m) => descarga('guardiana-libro-entregas.json', m.json));
})();
