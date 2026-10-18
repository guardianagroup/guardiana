// «Tu visita, a subasta», «No me subastes», «Tus datos, sin valor» and «El rastro de tus datos»
// (1.0.46). The side summary of ZERO (panel.html, agent 3's frame) leaves one place for it,
// <section id="zero-subasta" hidden>, and calls window.ZeroSubasta.montar(el, pagina) on every
// update of the page. What the auction looked like comes with the shield's own message
// («escudo», field `subasta`, worked out by crates/zero/src/subasta.rs): this file only words
// and draws it. Every text comes from the program (sub_* keys); nothing here talks to a site.
//
// Rules kept here: a bid is a bid per thousand ads (CPM), never «the price of your data»; what
// cannot be seen from this computer is said every time; the names of the fields that went, never
// their values; the person sends the letters, ZERO never does.
'use strict';
(() => {
  if (typeof Z === 'undefined') return;
  const { manda, en, t, tn, esc } = Z;
  let caja = null;          // the section given by montar
  let datos = null;         // the last `subasta` of the shield's message
  let pagina = null;        // what the frame said of the page (sitio, protege…)
  let carta = null;         // the last letter written, while it is for this site
  let recibo = '';          // the receipt's answer, while it is for this site
  let abierto = false;      // «El rastro de tus datos» unfolded
  let sitioVisto = '';

  const lang = () => document.documentElement.lang || 'en';
  const n = (x) => Number(x || 0).toLocaleString(lang());
  const segundos = (ms) => (Number(ms) / 1000).toLocaleString(lang(), { minimumFractionDigits: 1, maximumFractionDigits: 1 });
  const cpm = (v, moneda) => {
    const m = String(moneda || '').toUpperCase();
    try {
      if (/^[A-Z]{3}$/.test(m)) return Number(v).toLocaleString(lang(), { style: 'currency', currency: m, minimumFractionDigits: 2, maximumFractionDigits: 3 });
    } catch (_) { /* an unknown currency: the number alone */ }
    return Number(v).toLocaleString(lang(), { minimumFractionDigits: 2, maximumFractionDigits: 3 }) + (m ? ' ' + m : '');
  };
  const pais = (c) => {
    if (!c) return '';
    try { return new Intl.DisplayNames([lang()], { type: 'region' }).of(c) || c; } catch (_) { return c; }
  };
  const texto = (k) => ({ permitido: t('interruptor_permitido'), cortado: t('interruptor_cortado'), cortando: t('interruptor_cortando'), permitiendo: t('interruptor_permitiendo') });

  // The headline: «This site offered your visit to 6 companies in 1.3 s. None could bid:
  // GUARDIANA blocked them.» Without the page's auction program read, the companies seen in its
  // requests; with only Google Ad Manager, that its auction cannot be seen from here.
  function frase(d) {
    const ofrecida = d.ofrecida || [];
    if (ofrecida.length) {
      const k = d.ms != null ? 'sub_frase_oferta' : 'sub_frase_oferta_sin_tiempo';
      const una = tn(k, ofrecida.length, { n: n(ofrecida.length), s: d.ms != null ? segundos(d.ms) : '' });
      let dos;
      if (d.cortadas && d.cortadas >= ofrecida.length && !d.pujaron) dos = tn('sub_frase_cortadas_todas', ofrecida.length);
      else if (d.cortadas) dos = t('sub_frase_cortadas_parte', { c: n(d.cortadas) }) + ' ' + tn('sub_frase_pujaron', d.pujaron, { p: n(d.pujaron) });
      else dos = tn('sub_frase_pujaron', d.pujaron, { p: n(d.pujaron) });
      return una + ' ' + dos;
    }
    const e = d.empresas || [];
    if (e.length) {
      const una = tn('sub_frase_peticiones', e.length, { n: n(e.length) });
      const c = Number(d.empresas_cortadas || 0);
      if (c && c >= e.length) return una + ' ' + tn('sub_frase_peticiones_todas', e.length);
      if (c) return una + ' ' + t('sub_frase_cortadas_parte', { c: n(c) });
      return una;
    }
    if (d.programas && d.programas.gpt) return t('sub_frase_servidor');
    return '';
  }

  function hayAlgo(d) {
    return !!d && ((d.ofrecida || []).length || (d.empresas || []).length || (d.cadenas || []).length
      || (d.programas && (d.programas.prebid || d.programas.apstag || d.programas.gpt)));
  }

  const ESTADO = { cortada: ['sub_estado_cortada', 'alerta'], salio: ['sub_estado_salio', ''], no_vista: ['sub_estado_no_vista', ''] };
  function fila(nombre, sub, estado, quien) {
    const [k, clase] = ESTADO[estado] || ESTADO.no_vista;
    return '<li class="sub-fila"><span class="sub-quien"><b>' + esc(nombre) + '</b><small>' + sub + '</small></span>'
      + '<span class="sub-estado ' + clase + '">' + esc(t(k)) + '</span>'
      + (quien ? '<button type="button" class="enlace sub-ficha" data-quien="' + esc(quien) + '">' + esc(t('ficha_ver')) + '</button>' : '') + '</li>';
  }

  function empresas(d) {
    const ofrecida = d.ofrecida || [];
    if (ofrecida.length) {
      const filas = ofrecida.map((e) => {
        const sub = [e.pais ? esc(pais(e.pais)) : '<i>' + esc(t('sub_pais_sin')) + '</i>', esc(tn('sub_peticiones_fila', e.peticiones, { n: n(e.peticiones) }))];
        return fila(e.quien, sub.join(' · '), e.estado, e.quien !== e.codigo ? e.quien : '');
      });
      return '<h3 class="sub-cab"><span>' + esc(t('sub_empresas_cab')) + '</span></h3><ul class="sub-lista">' + filas.join('') + '</ul>';
    }
    const e = d.empresas || [];
    if (!e.length) return '';
    const filas = e.map((x) => {
      const papeles = (x.papeles || []).map((p) => t('sub_papel_' + p)).join(', ');
      const sub = [x.pais ? esc(pais(x.pais)) : '<i>' + esc(t('sub_pais_sin')) + '</i>', esc(papeles)];
      const estado = x.peticiones && x.cortadas >= x.peticiones ? 'cortada' : 'salio';
      return fila(x.quien, sub.join(' · '), estado, x.quien !== x.sitio ? x.quien : '');
    });
    return '<h3 class="sub-cab"><span>' + esc(t('sub_empresas_vistas_cab')) + '</span></h3><ul class="sub-lista">' + filas.join('') + '</ul>';
  }

  function pujas(d) {
    if (!(d.ofrecida || []).length) return '';
    const p = d.pujas || [];
    let h = '<h3 class="sub-cab"><span>' + esc(t('sub_pujas_cab')) + '</span></h3>';
    if (!p.length) h += '<p class="sub-nota">' + esc(t('sub_pujas_ninguna')) + '</p>';
    else h += '<ul class="sub-pujas">' + p.map((x) => '<li><span>' + esc(x.quien) + '</span><b>' + esc(cpm(x.cpm, x.moneda)) + ' <small>CPM</small></b></li>').join('') + '</ul>';
    if (d.ganador && d.ganador.puja) h += '<p class="sub-ganador">' + esc(t('sub_ganador', { quien: d.ganador.quien, cpm: cpm(d.ganador.puja.cpm, d.ganador.puja.moneda) })) + '</p>';
    return h + '<p class="sub-nota">' + esc(t('sub_pujas_nota')) + '</p>';
  }

  function enviado(d) {
    const datos2 = d.datos || [];
    const ids = d.ids || [];
    const c = d.consentimiento || {};
    if (!datos2.length && !ids.length && !(d.ofrecida || []).length) return '';
    let h = '<h3 class="sub-cab"><span>' + esc(t('sub_datos_cab')) + '</span></h3>';
    if (datos2.length) h += '<div class="sub-chips">' + datos2.map((k) => '<span class="chip">' + esc(t('sub_dato_' + k)) + '</span>').join('') + '</div>';
    if (ids.length) h += '<p class="sub-linea">' + esc(t('sub_ids_sistemas', { ids: ids.join(', ') })) + '</p>';
    if ((d.ofrecida || []).length) h += '<p class="sub-linea">' + esc(t(c.tcf || c.gpp || c.usp ? 'sub_tcf_si' : 'sub_tcf_no')) + '</p>';
    return h + '<p class="sub-nota">' + esc(t('sub_datos_nota')) + '</p>';
  }

  // Identifier syncing, with the opt-out page of each identifier system when it is published.
  function cruces(d) {
    const c = d.cadenas || [];
    const bajas = (d.empresas || []).filter((e) => e.sistema && e.baja);
    if (!c.length && !bajas.length) return '';
    let h = '<h3 class="sub-cab"><span>' + esc(t('sub_cruces_cab')) + '</span></h3>';
    if (c.length) {
      h += '<p class="sub-linea">' + esc(tn('sub_cruces_n', c.length, { n: n(c.length) })) + '</p><ul class="sub-cadenas">';
      h += c.slice(0, 6).map((x) => '<li><code>' + x.saltos.map(esc).join(' <span aria-hidden="true">→</span> ') + '</code>'
        + (x.cortada ? '<span class="sub-estado alerta">' + esc(t('sub_estado_cortada')) + '</span>' : '') + '</li>').join('');
      h += '</ul>';
    }
    for (const e of bajas) h += '<p class="sub-linea"><button type="button" class="enlace sub-abrir" data-url="' + esc(e.baja) + '">' + esc(t('sub_baja', { sistema: e.sistema })) + '</button></p>';
    return h;
  }

  // «El rastro de tus datos»: the tree with its four marks.
  function rastro(d) {
    const r = d.rastro || {};
    const ramas = r.ramas || [];
    const marca = (m) => '<span class="sub-marca ' + m + '">' + esc(t('sub_marca_' + m)) + '</span>';
    // A company announced inside a crossing shows only its mark: what was cut is the request
    // that announced it, said on its parent.
    const nodo = (x, hijo) => '<li><span class="sub-nodo"><code>' + esc(x.nombre) + '</code>' + marca(x.marca)
      + (x.cortada && !hijo ? '<span class="sub-estado alerta">' + esc(t('sub_estado_cortada')) + '</span>' : '') + '</span>'
      + '<small>' + esc([x.quien !== x.nombre ? x.quien : '', x.pais ? pais(x.pais) : '', (x.papeles || []).map((p) => t('sub_papel_' + p)).join(', ')].filter(Boolean).join(' · ')) + '</small>'
      + ((x.hijos || []).length ? '<ul>' + x.hijos.map((h) => nodo(h, true)).join('') + '</ul>' : '') + '</li>';
    let h = '<details class="sub-rastro"' + (abierto ? ' open' : '') + '><summary>' + esc(t('sub_rastro_cab')) + '</summary>';
    h += '<dl class="sub-leyenda">' + ['visto', 'declarado', 'probable', 'invisible'].map((m) => '<div><dt>' + marca(m) + '</dt><dd>' + esc(t('sub_marca_' + m + '_explica')) + '</dd></div>').join('') + '</dl>';
    h += '<ul class="sub-arbol"><li><span class="sub-nodo"><code>' + esc(r.raiz || d.sitio || '') + '</code></span><ul>' + ramas.map((x) => nodo(x, false)).join('');
    const a = d.adstxt;
    if (a && a.hay) {
      h += '<li><span class="sub-nodo"><code>ads.txt</code>' + marca('declarado') + '</span><small>' + esc(t('sub_adstxt_resumen', { n: n(a.lineas), d: n(a.directos), r: n(a.revendedores) })) + '</small><ul>'
        + (r.declarado || []).slice(0, 20).map((x) => '<li><span class="sub-nodo"><code>' + esc(x.nombre) + '</code>' + marca('declarado') + '</span><small>'
          + esc([x.quien !== x.nombre ? x.quien : '', x.pais ? pais(x.pais) : ''].filter(Boolean).join(' · ')) + ' <button type="button" class="enlace sub-abrir" data-url="' + esc(x.sellers) + '">sellers.json</button></small></li>').join('')
        + '</ul></li>';
    }
    h += '<li><span class="sub-nodo">' + marca('invisible') + '</span><ul>' + (r.invisible || []).map((k) => '<li><small>' + esc(t('sub_invisible_' + k)) + '</small></li>').join('') + '</ul></li>';
    h += '</ul></li></ul>';
    if (a && !a.hay) h += '<p class="sub-nota">' + esc(t('sub_adstxt_no')) + '</p>';
    if (!a) h += '<button type="button" class="boton chico sub-adstxt">' + esc(t('sub_adstxt_leer')) + '</button>';
    return h + '</details>';
  }

  function interruptor(d) {
    if (!d.protege) return '<p class="sub-nota">' + esc(t('sub_plus')) + '</p>';
    if (!d.activo) return '<p class="sub-nota">' + esc(t('sub_apagado')) + '</p>';
    const G = window.GuardianaInterruptor;
    if (!G) return '';
    return '<div class="sub-permitir"><label>' + esc(t('sub_interruptor')) + '</label><span class="sub-sw">'
      + G.html({ cortado: !d.permitida, texto: texto(), etiqueta: t('sub_interruptor'), chico: true }) + '</span></div>';
  }

  function cartas(d) {
    const quienes = [];
    for (const e of d.ofrecida || []) if (e.sitio && !quienes.some((q) => q[0] === e.sitio)) quienes.push([e.sitio, e.quien]);
    for (const e of d.empresas || []) if (!quienes.some((q) => q[0] === e.sitio)) quienes.push([e.sitio, e.quien]);
    if (!quienes.length) return '';
    let h = '<h3 class="sub-cab"><span>' + esc(t('sub_carta_cab')) + '</span></h3><div class="sub-carta-form">'
      + '<label><span>' + esc(t('sub_carta_empresa')) + '</span><select class="campo" id="sub-carta-empresa">' + quienes.map((q) => '<option value="' + esc(q[0]) + '">' + esc(q[1]) + '</option>').join('') + '</select></label>'
      + '<label><span>' + esc(t('sub_carta_tipo')) + '</span><select class="campo" id="sub-carta-tipo">' + ['acceso', 'oposicion', 'habeas'].map((k) => '<option value="' + k + '">' + esc(t('sub_carta_' + k)) + '</option>').join('') + '</select></label>'
      + '<button type="button" class="boton chico" id="sub-carta-preparar">' + esc(t('sub_carta_preparar')) + '</button></div>';
    if (carta) {
      h += '<div class="sub-carta"><b>' + esc(carta.asunto) + '</b><textarea class="campo" readonly rows="9">' + esc(carta.cuerpo) + '</textarea>'
        + (carta.carta === 'habeas' ? '<p class="sub-nota">' + esc(t('sub_carta_habeas_nota')) + '</p>' : '')
        + '<div class="sub-acciones"><button type="button" class="boton chico" id="sub-carta-copiar">' + esc(t('carta_copiar')) + '</button><button type="button" class="boton chico" id="sub-carta-correo">' + esc(t('carta_correo')) + '</button></div>'
        + '<p class="sub-nota">' + esc(t('carta_nota')) + '</p></div>';
    }
    return h;
  }

  function dibuja() {
    if (!caja) return;
    const d = datos;
    if (!hayAlgo(d)) { caja.hidden = true; caja.innerHTML = ''; return; }
    if (d.sitio !== sitioVisto) { sitioVisto = d.sitio; carta = null; recibo = ''; abierto = false; }
    const p = d.programas || {};
    const otros = [p.apstag ? 'Amazon (apstag)' : '', p.gpt ? 'Google Ad Manager' : ''].filter(Boolean);
    const ofrecida = (d.ofrecida || []).length;
    let h = '<h2 class="sub-titulo"><span>' + esc(t('sub_titulo')) + '</span>' + (p.prebid ? '<small>Prebid.js ' + esc(p.prebid) + '</small>' : '') + '</h2>';
    h += '<p class="sub-frase">' + esc(frase(d)) + '</p>';
    if (ofrecida) {
      h += '<div class="sub-cifras">'
        + '<div><b>' + n(ofrecida) + '</b><span>' + esc(tn('sub_n_empresas', ofrecida)) + '</span></div>'
        + '<div><b>' + n(d.peticiones) + '</b><span>' + esc(tn('sub_n_peticiones', d.peticiones)) + '</span></div>'
        + '<div><b>' + n(d.espacios) + '</b><span>' + esc(tn('sub_n_espacios', d.espacios)) + '</span></div></div>';
    }
    const meta = [d.limite ? t('sub_limite', { ms: n(d.limite) }) : '', otros.join(', ')].filter(Boolean);
    if (meta.length) h += '<p class="sub-meta">' + esc(meta.join(' · ')) + '</p>';
    h += interruptor(d);
    h += empresas(d) + pujas(d) + enviado(d) + cruces(d) + rastro(d);
    const hoy = d.hoy || {}; const semana = d.semana || {};
    h += '<div class="sub-dia"><p>' + esc(tn('sub_hoy', hoy.subastas, { s: n(hoy.subastas), e: n(hoy.empresas) })) + '</p><p>' + esc(tn('sub_semana', semana.subastas, { s: n(semana.subastas), e: n(semana.empresas) })) + '</p></div>';
    if (d.aislada) h += '<p class="sub-nota">' + esc(t('sub_aislada')) + '</p>';
    h += '<p class="sub-nota">' + esc(t(d.gpc ? 'sub_gpc_si' : 'sub_gpc_no')) + '</p>';
    h += '<p class="sub-limites">' + esc(t('sub_limites')) + '</p>';
    h += '<div class="sub-acciones"><button type="button" class="boton chico" id="sub-recibo"' + ((d.ofrecida || []).length || (d.empresas || []).length ? '' : ' disabled') + '>' + esc(t('sub_recibo')) + '</button></div>';
    if (recibo) h += '<p class="sub-nota sub-recibo-txt" role="status">' + esc(recibo) + '</p>';
    h += cartas(d);
    // Keep the person's place: the focused control and the chosen letter survive a redraw.
    const elegida = caja.querySelector('#sub-carta-empresa');
    const tipo = caja.querySelector('#sub-carta-tipo');
    const antes = { e: elegida && elegida.value, t: tipo && tipo.value };
    caja.innerHTML = h;
    caja.hidden = false;
    if (antes.e && caja.querySelector('#sub-carta-empresa')) caja.querySelector('#sub-carta-empresa').value = antes.e;
    if (antes.t && caja.querySelector('#sub-carta-tipo')) caja.querySelector('#sub-carta-tipo').value = antes.t;
    const G = window.GuardianaInterruptor;
    const sw = caja.querySelector('.sub-sw');
    // On is «Permitido»: the auctions of this site may run. Off is «Cortado».
    if (G && sw) G.engancha(sw, (b, quiereCortar) => { manda({ tipo: 'subasta_permitir', sitio: d.sitio, permitir: !quiereCortar }); return Promise.resolve(); }, texto());
  }

  function engancha(el) {
    if (el.dataset.subEnganchada) return;
    el.dataset.subEnganchada = '1';
    el.addEventListener('click', (e) => {
      const b = e.target.closest('button');
      if (!b || !el.contains(b)) return;
      if (b.classList.contains('sub-ficha')) manda({ tipo: 'abrir_ficha', empresa: b.dataset.quien });
      else if (b.classList.contains('sub-abrir')) manda({ tipo: 'subasta_abrir', url: b.dataset.url });
      else if (b.classList.contains('sub-adstxt')) { b.disabled = true; abierto = true; manda({ tipo: 'subasta_adstxt' }); }
      else if (b.id === 'sub-recibo') { b.setAttribute('aria-busy', 'true'); manda({ tipo: 'subasta_recibo' }); }
      else if (b.id === 'sub-carta-preparar') {
        manda({ tipo: 'subasta_carta', sitio: el.querySelector('#sub-carta-empresa').value, carta: el.querySelector('#sub-carta-tipo').value });
      } else if (b.id === 'sub-carta-copiar' && carta) {
        const txt = carta.asunto + '\n\n' + carta.cuerpo;
        (navigator.clipboard ? navigator.clipboard.writeText(txt) : Promise.reject()).then(() => { b.textContent = t('copiado'); }, () => {
          const a = el.querySelector('.sub-carta textarea'); if (a) { a.select(); document.execCommand('copy'); }
        });
      } else if (b.id === 'sub-carta-correo' && carta) manda({ tipo: 'abrir_correo', asunto: carta.asunto, cuerpo: carta.cuerpo });
    });
    el.addEventListener('toggle', (e) => { if (e.target.classList && e.target.classList.contains('sub-rastro')) abierto = e.target.open; }, true);
  }

  en('escudo', (m) => { datos = m.subasta || null; dibuja(); });
  en('sub_recibo', (m) => { recibo = m.texto || ''; dibuja(); });
  en('sub_carta', (m) => { carta = m; dibuja(); const c = caja && caja.querySelector('.sub-carta'); if (c) c.scrollIntoView({ block: 'nearest', behavior: 'smooth' }); });

  // --- settings (panel.html, v-ajustes, block «1.0.46 subastas») -----------------------------------
  const ajustes = [['sub-a-subastas', 'subastas'], ['sub-a-gpc', 'gpc'], ['sub-a-ruido', 'huella_ruido'], ['sub-a-uniforme', 'huella_uniforme']];
  for (const [id, clave] of ajustes) {
    const x = document.getElementById(id);
    if (x) x.addEventListener('change', (e) => manda({ tipo: 'ajuste', clave, valor: e.target.checked }));
  }
  en('ajustes', (m) => {
    for (const [id, clave] of ajustes) {
      const x = document.getElementById(id);
      if (!x) continue;
      x.checked = m[clave] === true;
      // GPC is a request any plan can send; the rest are protections of Plus.
      x.disabled = clave !== 'gpc' && m.sub_protege === false;
    }
    const plus = document.getElementById('sub-a-plus');
    if (plus) plus.classList.toggle('oculto', m.sub_protege !== false);
  });

  window.ZeroSubasta = {
    // `el`: the section to fill. `pagina`: what the frame says of the page; when it carries the
    // shield's `subasta` itself, that is used, otherwise the last one this file received.
    montar(el, pag) {
      if (!el) return;
      if (caja !== el) { caja = el; engancha(el); }
      pagina = pag || null;
      if (pagina && pagina.subasta) datos = pagina.subasta;
      dibuja();
    },
    // For tests and screenshots: the drawing of a given auction view.
    dibuja(el, d) { caja = el; engancha(el); datos = d; dibuja(); },
  };
})();
