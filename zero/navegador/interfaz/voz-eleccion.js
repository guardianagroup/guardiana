// «¿Cómo quieres hablar?» (18 Oct 2026, the owner): before a voice call starts or is answered,
// one calm choice — «Voz real» (the default, the best quality) or «Voz protegida» (the voice
// leaves pitch-shifted through voz.js; it puts a listener off, it is no guaranteed anonymity).
// The last choice is remembered per contact, only in this browser, and preselected. During the
// call the existing switch (#c-voz-prot in secure.js) shows the mode and changes it live; this
// file only sets it before the call, by pressing that same switch, so there is one source of truth.
'use strict';
(() => {
  const { en, t, esc, $ } = Z;
  const BOTONES = ['c-voz', 'c-devolver', 'c-contestar'];
  const CLAVE = 'secure_voz_por_contacto';
  let contacto = '';
  en('secure_estado', (m) => { contacto = m.contacto || ''; });
  const memoria = () => { try { return JSON.parse(localStorage.getItem(CLAVE) || '{}') || {}; } catch (_) { return {}; } };
  const recuerda = (modo) => { try { const m = memoria(); m[contacto || '·'] = modo; localStorage.setItem(CLAVE, JSON.stringify(m)); } catch (_) {} };
  const elegida = () => memoria()[contacto || '·'] || 'real';
  let pasando = false;

  function pregunta() {
    return new Promise((listo) => {
      const previa = elegida();
      const capa = document.createElement('div');
      capa.className = 've-capa';
      capa.setAttribute('role', 'dialog');
      capa.setAttribute('aria-modal', 'true');
      capa.innerHTML = `<section class="tarjeta ve-caja"><h2>${esc(t('ve_titulo'))}</h2>`
        + `<button type="button" class="ve-opcion${previa === 'real' ? ' elegida' : ''}" data-modo="real"><b>${esc(t('ve_real'))}</b><span>${esc(t('ve_real_texto'))}</span></button>`
        + `<button type="button" class="ve-opcion${previa === 'protegida' ? ' elegida' : ''}" data-modo="protegida"><b>${esc(t('ve_protegida'))}</b><span>${esc(t('ve_protegida_texto'))}</span></button>`
        + `<div class="c-fila"><button type="button" class="boton chico secundario" data-modo="">${esc(t('ve_cancelar'))}</button></div></section>`;
      const cierra = (modo) => { capa.remove(); document.removeEventListener('keydown', tecla); listo(modo); };
      const tecla = (e) => { if (e.key === 'Escape') cierra(''); if (e.key === 'Enter') cierra(previa); };
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

  // Put the call's switch where the person chose (it tells the other side by itself).
  function aplica(modo) {
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
