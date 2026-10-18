// «¿Cómo prefieres que te escuchen?» (1.0.46, C2-b, the design the owner chose on 18 Oct 2026):
// before a voice call starts or is answered, one calm choice of what really exists here —
// «Voz natural» (the microphone as it is, the echo canceller on) or «Voz con reducción de
// ruido» (the browser's own noise suppression on top; the default, as every call had it until
// now). Only offered when this browser can turn that suppression on and off. The pitch-shifted
// «Voz protegida» of 18 Oct (voz.js) stays as a third, quieter option: it puts a listener off,
// it is no guaranteed anonymity. The last choice is remembered per contact, only in this
// browser, and preselected. The microphone reads the choice from `window.GuardianaVoz`
// (opus.js `restricciones`); the protected voice goes through the call's own switch
// (#c-voz-prot in secure.js), so there is one source of truth for it.
'use strict';
(() => {
  const { en, t, esc, $ } = Z;
  const BOTONES = ['c-voz', 'c-devolver', 'c-contestar'];
  const CLAVE = 'secure_voz_por_contacto';
  let contacto = '';
  en('secure_estado', (m) => { contacto = m.contacto || ''; });
  const memoria = () => { try { return JSON.parse(localStorage.getItem(CLAVE) || '{}') || {}; } catch (_) { return {}; } };
  const recuerda = (modo) => { try { const m = memoria(); m[contacto || '·'] = modo; localStorage.setItem(CLAVE, JSON.stringify(m)); } catch (_) {} };
  // Whether the browser lets a page switch its noise suppression (WebView2 and Chrome do).
  const hayFiltro = () => { try { return !!(navigator.mediaDevices && navigator.mediaDevices.getSupportedConstraints().noiseSuppression); } catch (_) { return false; } };
  // The remembered choice; «real» (1.0.45) was the voice with the browser's suppression on.
  const elegida = () => { const v = memoria()[contacto || '·']; return v === 'real' || !v ? 'ruido' : v; };
  window.GuardianaVoz = window.GuardianaVoz || { modo: 'ruido', ruido: true };
  let pasando = false;

  function pregunta() {
    return new Promise((listo) => {
      const filtro = hayFiltro();
      let previa = elegida();
      if (!filtro && previa !== 'protegida') previa = 'natural';
      const opcion = (modo, titulo, texto, extra) => `<button type="button" class="ve-opcion${extra || ''}${previa === modo ? ' elegida' : ''}" data-modo="${modo}" aria-pressed="${previa === modo}"><b>${esc(t(titulo))}</b><span>${esc(t(texto))}</span></button>`;
      const capa = document.createElement('div');
      capa.className = 've-capa';
      capa.setAttribute('role', 'dialog');
      capa.setAttribute('aria-modal', 'true');
      capa.setAttribute('aria-labelledby', 've-titulo');
      capa.innerHTML = `<section class="tarjeta ve-caja"><h2 id="ve-titulo">${esc(t('sala_ve_titulo'))}</h2>`
        + opcion('natural', 'sala_ve_natural', 'sala_ve_natural_t')
        + (filtro ? opcion('ruido', 'sala_ve_ruido', 'sala_ve_ruido_t') : '')
        + opcion('protegida', 'sala_ve_protegida', 've_protegida_texto', ' ve-tercera')
        + `<div class="c-fila"><button type="button" class="boton chico secundario" data-modo="">${esc(t('ve_cancelar'))}</button></div></section>`;
      const cierra = (modo) => { capa.remove(); document.removeEventListener('keydown', tecla); listo(modo); };
      const tecla = (e) => { if (e.key === 'Escape') cierra(''); if (e.key === 'Enter' && !e.target.closest('[data-modo]')) cierra(previa); };
      capa.addEventListener('click', (e) => {
        const b = e.target.closest('[data-modo]');
        if (b) cierra(b.dataset.modo);
        else if (e.target === capa) cierra('');
      });
      document.addEventListener('keydown', tecla);
      document.body.appendChild(capa);
      const foco = capa.querySelector(`[data-modo="${previa}"]`);
      if (foco) foco.focus();
    });
  }

  // The microphone as chosen, and the call's switch for the protected voice where it belongs.
  function aplica(modo) {
    window.GuardianaVoz = { modo, ruido: modo !== 'natural' };
    window.dispatchEvent(new CustomEvent('guardiana-voz', { detail: window.GuardianaVoz }));
    const s = $('c-voz-prot');
    if (!s) return;
    const activa = s.getAttribute('aria-pressed') === 'true';
    if ((modo === 'protegida') !== activa) s.click();
  }

  BOTONES.forEach((id) => {
    const b = $(id);
    if (!b) return;
    // In the capture phase, before secure.js starts the call.
    b.addEventListener('click', async (e) => {
      if (pasando || b.disabled) return;
      e.preventDefault();
      e.stopImmediatePropagation();
      const modo = await pregunta();
      if (!modo) return;
      recuerda(modo);
      aplica(modo);
      pasando = true;
      try { b.click(); } finally { pasando = false; }
    }, true);
  });
})();
