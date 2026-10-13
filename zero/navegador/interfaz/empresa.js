// A company's profile: what the lists say of it, what it tried with this person (from the list
// of cuts on this computer), and, when GUARDIANA has checked them, what it does, its money, its
// fines and its data breaches, each fact with its public source. Nothing here talks to the
// network: the sources open only when the person clicks them.
'use strict';
(() => {
  const { manda, en, t, tn, esc, $, n } = Z;
  const empresa = new URLSearchParams(location.search).get('e') || '';
  let datos = null;
  let pais = (c) => c || '';
  const lang = () => document.documentElement.lang || 'es';
  // A text in the person's language, or the first one there is.
  const L = (o) => (o && typeof o === 'object' ? o[lang()] || o.es || o.en || o.pt || '' : String(o || ''));
  const nombrePais = (c) => (!c ? '' : c[0] === '~' ? t('cortes_sin_identificar', { pais: pais(c.slice(1)) }) : pais(c));
  const web = (u) => typeof u === 'string' && /^https:\/\//.test(u);
  const enlace = (u, texto) => (web(u) ? `<a class="enlace-ext" href="${esc(u)}" target="_blank" rel="noopener">${esc(texto || u)}</a>` : '');

  // Money as people say it: «1.930 millones de USD», «40 millones de EUR».
  function dinero(valor, moneda) {
    const v = Number(valor);
    if (!Number.isFinite(v)) return '';
    const m = String(moneda || '').toUpperCase().slice(0, 3);
    if (Math.abs(v) >= 1e6) {
      const millones = (v / 1e6).toLocaleString(lang(), { maximumFractionDigits: Math.abs(v) >= 1e10 ? 0 : 1, useGrouping: 'always' });
      return t('ficha_millones', { n: millones, moneda: m });
    }
    try { return v.toLocaleString(lang(), { style: 'currency', currency: m, maximumFractionDigits: 0 }); } catch (_) { return `${n(v)} ${m}`; }
  }
  function fecha(f) {
    const s = String(f || '');
    if (/^\d{4}-\d{2}-\d{2}$/.test(s)) return new Date(s + 'T12:00:00').toLocaleDateString(lang(), { day: 'numeric', month: 'long', year: 'numeric' });
    if (/^\d{4}-\d{2}$/.test(s)) return new Date(s + '-15T12:00:00').toLocaleDateString(lang(), { month: 'long', year: 'numeric' });
    return s;
  }

  // Every source, numbered once, so the PDF carries them all.
  let fuentes = [];
  const cita = (u) => {
    if (!web(u)) return '';
    let i = fuentes.indexOf(u);
    if (i < 0) { fuentes.push(u); i = fuentes.length - 1; }
    return ` <span class="fuente">[<a href="${esc(u)}" target="_blank" rel="noopener">${i + 1}</a>]</span>`;
  };

  function pinta() {
    const d = datos;
    if (!d) return;
    fuentes = [];
    const f = d.ficha || null;
    document.title = `${d.empresa} — ${t('ficha_titulo')}`;
    $('nombre').textContent = d.empresa;
    const sede = [d.ciudad, nombrePais(d.pais)].filter(Boolean).join(', ');
    const partes = [];
    if (sede) partes.push(`${esc(t('ficha_sede'))}: <b>${esc(sede)}</b>`);
    if (d.grupo) partes.push(`${esc(t('ficha_grupo'))}: <button class="enlace" data-empresa="${esc(d.grupo)}">${esc(d.grupo)}</button>`);
    if (f && f.fundada) partes.push(`${esc(t('ficha_fundada'))}: <b>${esc(String(f.fundada))}</b>`);
    if (f && f.bolsa) partes.push(`${esc(t('ficha_bolsa'))}: <b>${esc(f.bolsa)}</b>`);
    if (!d.conocida) partes.push(esc(t('ficha_no_identificada')));
    $('sede').innerHTML = partes.join(' · ');
    const chips = [];
    if (d.corredor) chips.push(`<span class="chip rojo">${esc(t('corredor_etiqueta'))}</span>`);
    if (f && f.sanciones && f.sanciones.length) chips.push(`<span class="chip rojo">${esc(tn('ficha_n_sanciones', f.sanciones.length, { n: n(f.sanciones.length) }))}</span>`);
    if (f && f.filtraciones && f.filtraciones.length) chips.push(`<span class="chip ambar">${esc(tn('ficha_n_filtraciones', f.filtraciones.length, { n: n(f.filtraciones.length) }))}</span>`);
    $('chips').innerHTML = chips.join('');

    const c = d.contigo || {};
    $('c-cortes').textContent = n(c.cortes || 0);
    $('c-webs').textContent = n(c.webs || 0);
    $('contigo-webs').textContent = (c.ejemplos && c.ejemplos.length)
      ? t('ficha_contigo_ejemplos', { webs: c.ejemplos.join(', ') })
      : t('ficha_contigo_nada');

    const s = [];
    if (f) {
      if (f.que_hace) s.push(`<section><h2>${esc(t('ficha_que_hace'))}</h2><p>${esc(L(f.que_hace))}</p>${f.como_gana ? `<p>${esc(L(f.como_gana))}</p>` : ''}${f.fuente_que_hace ? `<p>${cita(f.fuente_que_hace)}</p>` : ''}</section>`);
      if (f.datos) s.push(`<section><h2>${esc(t('ficha_datos'))}</h2><p>${esc(L(f.datos))}${cita(f.fuente_datos)}</p></section>`);
      // How it turns your data into money, with an example; where it is incorporated; what it
      // does with taxes; what competition authorities found (the owner, 12 Oct 2026).
      for (const [k, titulo] of [['como_usa_datos', 'ficha_como_usa'], ['sociedad', 'ficha_sociedad'], ['fiscal', 'ficha_fiscal'], ['monopolio', 'ficha_monopolio']]) {
        if (f[k]) s.push(`<section><h2>${esc(t(titulo))}</h2><p>${esc(L(f[k]))}${cita(f['fuente_' + k])}</p></section>`);
      }
      const cifras = Array.isArray(f.cifras) ? f.cifras : [];
      if (cifras.length) {
        const cajas = cifras.map((x) => {
          const valor = x.moneda ? dinero(x.valor, x.moneda) : (Number(x.valor) || 0).toLocaleString(lang(), { useGrouping: 'always' });
          return `<div class="dinero"><b>${esc(valor)}</b><span>${esc(t('ficha_cifra_' + x.clave, { anio: String(x.anio || '') }))}${cita(x.fuente)}</span></div>`;
        });
        s.push(`<section><h2>${esc(t('ficha_dinero'))}</h2><div class="cifras-dinero">${cajas.join('')}</div></section>`);
      }
      const sanciones = Array.isArray(f.sanciones) ? f.sanciones : [];
      s.push(`<section><h2>${esc(t('ficha_sanciones'))}</h2>${sanciones.length ? totales(sanciones) + `<ul class="lista">${sanciones.map((x) => `<li><div class="linea1"><span>${esc(x.autoridad || '')}${x.pais ? ` · ${esc(pais(x.pais))}` : ''} · ${esc(fecha(x.fecha))}</span><span class="importe">${importe(x)}</span></div><div class="linea2">${esc(L(x.motivo))}${x.estado ? ` ${esc(L(x.estado))}` : ''}${cita(x.fuente)}</div></li>`).join('')}</ul>` : `<p class="vacia">${esc(t('ficha_sin_sanciones', { fecha: fecha(d.revisada) }))}</p>`}</section>`);
      const filtraciones = Array.isArray(f.filtraciones) ? f.filtraciones : [];
      s.push(`<section><h2>${esc(t('ficha_filtraciones'))}</h2>${filtraciones.length ? `<ul class="lista">${filtraciones.map((x) => `<li class="ambar"><div class="linea1"><span>${esc(fecha(x.fecha))}</span><span>${x.afectados ? esc(tn('ficha_afectados', x.afectados, { n: n(x.afectados) })) : ''}</span></div><div class="linea2">${esc(L(x.que))}${cita(x.fuente)}</div></li>`).join('')}</ul>` : `<p class="vacia">${esc(t('ficha_sin_filtraciones', { fecha: fecha(d.revisada) }))}</p>`}</section>`);
      if (f.web) s.push(`<section><h2>${esc(t('ficha_web'))}</h2><p>${enlace(f.web)}</p></section>`);
    } else {
      s.push(`<section><p class="vacia">${esc(t(d.conocida ? 'ficha_pendiente' : 'ficha_pendiente_desconocida', { empresa: d.empresa }))}</p></section>`);
    }
    // How to ask it to delete your data: its own page, or the one it gave the registry.
    const borrar = (f && f.borrar) || (d.corredor && d.corredor.url) || '';
    // The letter, ready: pick the law, download it as a PDF to attach, or open it in your mail.
    const leyes = ['ue', 'co', 'ca', 'br', 'otro'];
    s.push(`<section><h2>${esc(t('ficha_borrar'))}</h2>`
      + (web(borrar) ? `<p>${esc(t('ficha_borrar_explica'))}</p><p>${enlace(borrar, t('cortes_det_corredor_url'))}</p>` : '')
      + `<p>${esc(t('ficha_carta_explica'))}</p>`
      + `<div class="controles carta-controles"><select id="carta-ley">${leyes.map((l) => `<option value="${l}"${l === leyPorDefecto() ? ' selected' : ''}>${esc(t('carta_ley_' + l))}</option>`).join('')}</select>`
      + `<button class="boton principal" id="carta-pdf">${esc(t('ficha_carta_pdf'))}</button>`
      + `<button class="boton" id="carta-correo">${esc(t('ficha_carta_correo'))}</button></div>`
      + `<pre class="carta" id="carta"></pre></section>`);
    $('cuerpo').innerHTML = s.join('');

    const dom = d.dominios || [];
    $('s-dominios').classList.toggle('oculto', !dom.length);
    $('dominios').textContent = dom.join(' · ') + (d.n_dominios > dom.length ? ` · ${t('ficha_y_mas', { n: n(d.n_dominios - dom.length) })}` : '');
    $('s-fuentes').classList.toggle('oculto', !fuentes.length);
    $('fuentes').innerHTML = fuentes.map((u) => `<li><a href="${esc(u)}" target="_blank" rel="noopener">${esc(u)}</a></li>`).join('');
    $('nota').textContent = f && d.revisada ? t('ficha_nota', { fecha: fecha(d.revisada) }) : t('ficha_nota_sin');
  }

  // A fine as imposed; cut on appeal, the amount that stands beside it; annulled, said so.
  function importe(x) {
    if (!x.importe) return '';
    const puesta = esc(dinero(x.importe, x.moneda));
    if (x.vigente === false) return `<s>${puesta}</s> ${esc(t('ficha_anulada'))}`;
    if (x.importe_final) return `<s>${puesta}</s> ${esc(dinero(x.importe_final, x.moneda))}`;
    return puesta;
  }
  // The fines that stand, added up by currency (never converted: a rate would be a figure of
  // ours); an annulled one does not count, a reduced one counts as reduced.
  function totales(lista) {
    const suma = new Map();
    for (const x of lista) {
      if (x.vigente === false) continue;
      const v = Number(x.importe_final || x.importe);
      if (!Number.isFinite(v) || !v || !x.moneda) continue;
      suma.set(x.moneda, (suma.get(x.moneda) || 0) + v);
    }
    if (!suma.size) return '';
    const texto = Array.from(suma, ([m, v]) => dinero(v, m)).join(' + ');
    return `<p class="total">${esc(t('ficha_total_sanciones', { total: texto }))}</p>`;
  }

  // What goes out when the person shares: a few lines anyone can read, and the public page of
  // this company on GUARDIANA's site when there is one (otherwise the browser's own page).
  function textoParaCompartir() {
    const d = datos || {};
    const f = d.ficha || {};
    const lineas = [`${d.empresa}${d.ciudad || d.pais ? ` (${[d.ciudad, nombrePais(d.pais)].filter(Boolean).join(', ')})` : ''}`];
    if (f.que_hace) lineas.push(L(f.que_hace));
    const sanciones = Array.isArray(f.sanciones) ? f.sanciones : [];
    for (const x of sanciones.slice(0, 3)) lineas.push(`• ${[x.autoridad, fecha(x.fecha), x.importe ? dinero(x.importe, x.moneda) : ''].filter(Boolean).join(' · ')}: ${L(x.motivo)}`);
    const filtraciones = Array.isArray(f.filtraciones) ? f.filtraciones : [];
    for (const x of filtraciones.slice(0, 2)) lineas.push(`• ${fecha(x.fecha)}: ${L(x.que)}`);
    const c = d.contigo || {};
    if (c.cortes) lineas.push(t('ficha_compartir_contigo', { n: n(c.cortes), webs: n(c.webs || 0) }));
    return lineas.join('\n');
  }
  function enlacePublico() {
    const f = (datos && datos.ficha) || {};
    const slug = typeof f.slug === 'string' && /^[a-z0-9-]+$/.test(f.slug) ? f.slug : '';
    const pre = lang() === 'es' ? '' : `${lang()}/`;
    return slug ? `https://guardianagroup.com/${pre}empresas/${slug}.html` : `https://guardianagroup.com/${pre}zero.html`;
  }
  function aviso(texto) {
    const a = $('aviso');
    a.textContent = texto;
    a.classList.add('visto');
    clearTimeout(aviso.t);
    aviso.t = setTimeout(() => a.classList.remove('visto'), 4500);
  }
  $('compartir').addEventListener('click', async () => {
    if (!datos) return;
    const texto = textoParaCompartir();
    const url = enlacePublico();
    if (navigator.share) {
      try { await navigator.share({ title: datos.empresa, text: texto, url }); return; } catch (e) { if (e && e.name === 'AbortError') return; }
    }
    try {
      await navigator.clipboard.writeText(`${texto}\n${url}`);
      aviso(t('ficha_copiado'));
    } catch (_) { aviso(t('ficha_no_copiado')); }
  });
  $('pdf').addEventListener('click', () => manda({ tipo: 'ficha_pdf', empresa }));
  $('volver').addEventListener('click', () => manda({ tipo: 'volver' }));
  en('ficha', (m) => { if (m.empresa === empresa.trim()) { datos = m; pinta(); } });
  en('guardado', (m) => { document.body.classList.remove('solo-carta'); aviso(m.texto || ''); });
  // The law that most likely applies, from the browser's language (the person can change it).
  function leyPorDefecto() {
    const l = (navigator.language || '').toLowerCase();
    if (l.endsWith('-co')) return 'co';
    if (l.startsWith('pt')) return 'br';
    if (l === 'en-us') return 'ca';
    if (/^(es|fr|de|it|nl|pl|cs|pt-pt|sv|da|fi|el|ro|hu|sk|sl|hr|bg|lt|lv|et|ga|mt)/.test(l)) return 'ue';
    return lang() === 'es' ? 'ue' : 'otro';
  }
  let cartaLista = null;
  function pideCarta(despues) {
    cartaLista = despues;
    manda({ tipo: 'carta_ficha', empresa, ley: $('carta-ley').value });
  }
  en('carta_ficha', (m) => {
    if (m.empresa !== empresa.trim()) return;
    $('carta').textContent = `${m.asunto}\n\n${m.cuerpo}`;
    const f = cartaLista;
    cartaLista = null;
    if (f) f(m);
  });
  document.addEventListener('click', (e) => {
    const id = e.target && e.target.id;
    if (id === 'carta-pdf') {
      pideCarta(() => {
        document.body.classList.add('solo-carta');
        manda({ tipo: 'ficha_pdf', empresa, que: 'carta' });
      });
    } else if (id === 'carta-correo') {
      pideCarta((m) => manda({ tipo: 'abrir_correo', asunto: m.asunto, cuerpo: m.cuerpo }));
    }
  });
  en('textos', () => {
    try { const dn = new Intl.DisplayNames([lang()], { type: 'region' }); pais = (c) => { try { return dn.of(c); } catch (_) { return c; } }; } catch (_) {}
    if (datos) pinta(); else manda({ tipo: 'ficha', empresa });
  });
  manda({ tipo: 'listo', vista: 'empresa' });
})();
