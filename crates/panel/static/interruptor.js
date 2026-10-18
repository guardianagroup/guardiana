// GUARDIANA — the cut switch, one component for the panel, ZERO and SECURE (18 Oct 2026).
// The owner: «un interruptor profesional que pase de verde a rojo». The common standard: on
// (aria-checked="true", green) is «Permitido»; off (aria-checked="false", red) is «Cortado». The
// state is always said in words beside it, so colour is never the only sign. It is a
// real switch (role="switch", aria-checked, Space and Enter), with a hit area of 44 px, a short
// «Cortando…» while the service confirms, and it goes back, saying why, if the service refuses.
// It also carries GuardianaRespuesta, the few rules that make every click answer (see below).
// This file is the same, byte for byte, in crates/panel/static/interruptor.js and
// zero/navegador/interfaz/interruptor.js; a test of the panel checks it. Its look is in diseno.css.
//
//   GuardianaInterruptor.html({ cortado, ocupado, texto, etiqueta, datos, chico })  -> markup
//     cortado: true when cut (red, knob to the right); ocupado: 'cortar' | 'permitir' while working;
//     texto: { permitido, cortado, cortando, permitiendo } (the words, already translated; cortado
//     may carry the hour: «Cortado 14:32»); etiqueta: what it cuts, for screen readers;
//     datos: data-* attributes for whoever handles the change.
//   GuardianaInterruptor.engancha(raiz, alCambiar, texto)
//     One listener for every switch under raiz. alCambiar(boton, quiereCortar) does the work and
//     resolves when done (the caller then redraws), or rejects with the reason, and then the
//     switch goes back to where it was and says the reason beside it.
'use strict';
(function () {
  var esc = function (s) { return String(s == null ? '' : s).replace(/[&<>"']/g, function (c) { return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]; }); };
  function html(o) {
    o = o || {};
    var t = o.texto || {};
    var ocupado = o.ocupado === 'cortar' || o.ocupado === 'permitir';
    // While it works the knob is already where it is going, and the words say it is not done.
    var rojo = ocupado ? o.ocupado === 'cortar' : !!o.cortado;
    // On means allowed (the common standard): a cut switch is off.
    var permitido = !rojo;
    var palabra = ocupado ? (o.ocupado === 'cortar' ? t.cortando : t.permitiendo) : (o.cortado ? t.cortado : t.permitido);
    var datos = '';
    var d = o.datos || {};
    Object.keys(d).forEach(function (k) { if (d[k] !== undefined && d[k] !== null && d[k] !== '') datos += ' data-' + k + '="' + esc(d[k]) + '"'; });
    return '<span class="interruptor-caja' + (o.chico ? ' chico' : '') + (rojo ? ' cortado' : '') + (ocupado ? ' ocupado' : '') + '">'
      + '<button type="button" role="switch" class="interruptor" aria-checked="' + (permitido ? 'true' : 'false') + '"'
      + (o.etiqueta ? ' aria-label="' + esc(o.etiqueta) + '" title="' + esc(o.etiqueta) + '"' : '')
      + (ocupado ? ' aria-busy="true" disabled' : '') + datos + '><span class="interruptor-bola" aria-hidden="true"></span></button>'
      + '<span class="interruptor-estado" aria-live="polite">' + esc(palabra || '') + '</span></span>';
  }
  function engancha(raiz, alCambiar, texto) {
    if (!raiz || raiz.dataset.interruptor) return;
    raiz.dataset.interruptor = '1';
    var t = texto || {};
    raiz.addEventListener('click', function (e) {
      var b = e.target.closest('button.interruptor[role="switch"]');
      if (!b || !raiz.contains(b) || b.disabled) return;
      e.preventDefault();
      var caja = b.closest('.interruptor-caja');
      var estado = caja && caja.querySelector('.interruptor-estado');
      // On is «Permitido»: pressing an allowed switch asks to cut, and the other way round.
      var antes = b.getAttribute('aria-checked') !== 'true';
      var antesTexto = estado ? estado.textContent : '';
      var quiere = !antes;
      b.setAttribute('aria-checked', String(!quiere));
      b.setAttribute('aria-busy', 'true');
      b.disabled = true;
      if (caja) { caja.classList.add('ocupado'); caja.classList.toggle('cortado', quiere); caja.classList.remove('fallo'); }
      if (estado) estado.textContent = (quiere ? t.cortando : t.permitiendo) || '';
      var vuelve = function (motivo) {
        if (!b.isConnected) return;
        b.setAttribute('aria-checked', String(!antes));
        b.removeAttribute('aria-busy');
        b.disabled = false;
        if (caja) { caja.classList.remove('ocupado'); caja.classList.toggle('cortado', antes); caja.classList.add('fallo'); }
        if (estado) { estado.textContent = antesTexto; if (motivo) estado.title = String(motivo); }
        if (motivo && caja) {
          var aviso = document.createElement('span');
          aviso.className = 'interruptor-motivo';
          aviso.setAttribute('role', 'alert');
          aviso.textContent = String(motivo);
          caja.appendChild(aviso);
          setTimeout(function () { if (aviso.isConnected) aviso.remove(); if (caja) caja.classList.remove('fallo'); }, 7000);
        }
      };
      var r;
      try { r = alCambiar(b, quiere); } catch (err) { vuelve(err && err.message || err); return; }
      Promise.resolve(r).then(function (hecho) {
        // false: nothing was done (a question cancelled): back where it was, without a reason.
        if (hecho === false) vuelve('');
        else if (b.isConnected) { b.removeAttribute('aria-busy'); b.disabled = false; if (caja) caja.classList.remove('ocupado'); }
      }, function (err) { vuelve(err && err.message || err); });
    });
    // Space and Enter are a button's own keys; nothing else is needed.
  }
  window.GuardianaInterruptor = { html: html, engancha: engancha };

  // ---------- Every click answers (18 Oct 2026, the owner's «revisión extrema») ----------
  // No control may leave the person unsure whether the click registered:
  //  · a control at work says so within a frame (aria-busy: a small spinner, no second click);
  //  · when it ends it says how: a short «hecho» mark, or back as it was with the reason;
  //  · a link to another page shows a thin bar at the top at once, until the page arrives;
  //  · a second click on something still working is ignored.
  // GuardianaRespuesta.trabaja(control, promesa) wraps any work started from a control;
  // GuardianaRespuesta.breve(texto) is a two-second notice («Guardado»).
  function trabaja(c, promesa) {
    if (!c || c.getAttribute('aria-busy') === 'true') return Promise.resolve(promesa);
    var estabaDesactivado = c.disabled;
    c.setAttribute('aria-busy', 'true');
    c.classList.add('trabajando');
    if ('disabled' in c) c.disabled = true;
    var fin = function (ok, motivo) {
      c.removeAttribute('aria-busy');
      c.classList.remove('trabajando');
      if ('disabled' in c && !estabaDesactivado) c.disabled = false;
      c.classList.add(ok ? 'respondio' : 'fallo');
      if (!ok && motivo) c.title = String(motivo);
      setTimeout(function () { c.classList.remove('respondio', 'fallo'); }, ok ? 900 : 2400);
    };
    return Promise.resolve(promesa).then(function (v) { fin(true); return v; }, function (e) { fin(false, e && e.message || e); throw e; });
  }
  var avisoBreve = null;
  function breve(texto) {
    if (!avisoBreve) {
      avisoBreve = document.createElement('div');
      avisoBreve.className = 'aviso-breve';
      avisoBreve.setAttribute('role', 'status');
      document.body.appendChild(avisoBreve);
    }
    avisoBreve.textContent = texto;
    avisoBreve.classList.add('visto');
    clearTimeout(breve.t);
    breve.t = setTimeout(function () { avisoBreve.classList.remove('visto'); }, 2000);
  }
  function barra() {
    var b = document.getElementById('gd-progreso');
    if (!b) { b = document.createElement('div'); b.id = 'gd-progreso'; b.className = 'gd-progreso'; b.setAttribute('aria-hidden', 'true'); document.body.appendChild(b); }
    b.classList.remove('va'); void b.offsetWidth; b.classList.add('va');
  }
  // A second click on a control still at work does nothing.
  document.addEventListener('click', function (e) {
    var c = e.target.closest && e.target.closest('[aria-busy="true"]');
    if (c) { e.preventDefault(); e.stopImmediatePropagation(); return; }
    var a = e.target.closest && e.target.closest('a[href]');
    if (!a || e.defaultPrevented || e.button !== 0 || e.ctrlKey || e.metaKey || e.shiftKey || e.altKey) return;
    if (a.target && a.target !== '_self') return;
    var h = a.getAttribute('href') || '';
    if (!h || h.charAt(0) === '#' || /^(mailto|tel|javascript):/i.test(h) || a.hasAttribute('download')) return;
    try { if (new URL(a.href, location.href).origin !== location.origin) return; } catch (_) { return; }
    barra();
  }, true);
  window.addEventListener('pageshow', function () { var b = document.getElementById('gd-progreso'); if (b) b.classList.remove('va'); });
  window.GuardianaRespuesta = { trabaja: trabaja, breve: breve, barra: barra };
})();
