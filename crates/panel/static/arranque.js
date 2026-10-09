// Runs in <head>, before the page is drawn: the night theme from the first frame (it used to
// switch from light to dark once app.js ran at the end of the page), and the page held back
// until its texts, its banner and its first figures are in place, then shown in one go. Opening
// the panel jumped: the wait page, then an empty page whose words, trial banner and rows
// arrived one after the other and pushed everything down (the responsible, 9 Oct 2026: «hace
// como un pequeño tirón»). If app.js never says it is ready, style.css shows the page anyway.
'use strict';
(function () {
  var html = document.documentElement;
  try { if (localStorage.getItem('guardiana_tema') === 'noche') html.dataset.tema = 'noche'; } catch (_) {}
  html.classList.add('cargando');
})();
