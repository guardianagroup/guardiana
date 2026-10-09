// The new tab: what today's pages tried and what was stopped, then the search.
'use strict';
(() => {
  const { manda, en, t, tn, $, n } = Z;
  // «Your month in data», in the count's own form for each number.
  const fraseMes = (mes, nombreMes) => `${tn('mes_frase_empresas', mes.empresas, { mes: nombreMes, empresas: n(mes.empresas) })} ${tn('mes_frase_cortadas', mes.empresas_cortadas, { cortadas: n(mes.empresas_cortadas) })}`;
  let mes = null;
  let primeraVez = true;

  $('buscar').addEventListener('submit', (e) => {
    e.preventDefault();
    const texto = $('q').value.trim();
    if (texto) manda({ tipo: 'navegar', texto });
  });
  document.querySelectorAll('.acciones button').forEach((b) => b.addEventListener('click', () => manda({ tipo: 'panel', vista: b.dataset.vista })));
  $('encender').addEventListener('click', () => manda({ tipo: 'ajuste', clave: 'cortar_seguimiento', valor: true }));
  $('motor-cambiar').addEventListener('click', () => manda({ tipo: 'panel', vista: 'ajustes' }));

  // The numbers count up once, the first time the page opens: the one moment of motion here.
  function cifra(el, valor) {
    const fin = Number(valor || 0);
    if (!primeraVez || fin === 0) { el.textContent = n(fin); return; }
    const t0 = performance.now();
    const paso = (ahora) => {
      const k = Math.min(1, (ahora - t0) / 700);
      el.textContent = n(Math.round(fin * (1 - Math.pow(1 - k, 3))));
      if (k < 1) requestAnimationFrame(paso);
    };
    requestAnimationFrame(paso);
  }

  en('hoy', (m) => {
    const h = m.hoy || {};
    const frase = $('frase');
    if (h.empresas) {
      frase.classList.remove('cero');
      frase.innerHTML = `<span>${Z.esc(tn('inicio_hoy_1', h.empresas, { empresas: n(h.empresas) }))}</span> <span class="azul">${Z.esc(tn('inicio_hoy_2', h.empresas_cortadas, { cortadas: n(h.empresas_cortadas) }))}</span>`;
    } else {
      frase.classList.add('cero');
      frase.textContent = t('inicio_hoy_cero');
    }
    cifra($('h-cortadas'), h.cortadas);
    cifra($('h-datos'), h.datos_salvados);
    cifra($('h-parametros'), h.parametros_quitados);
    cifra($('h-corredores'), h.corredores);
    primeraVez = false;
    $('apagado').classList.toggle('oculto', !!m.cortando);
    mes = m.mes || {};
    const nombreMes = new Date().toLocaleDateString(document.documentElement.lang, { month: 'long' });
    $('mes-frase').textContent = fraseMes(mes, nombreMes);
    // Where searches go, said plainly, with the way to change it.
    $('motor-nota').textContent = m.motor ? t(m.motor_privado ? 'buscar_nota_privado' : 'buscar_nota_perfil', { motor: m.motor }) : '';
  });

  // «Your month in data» as an image: only the month's figures, never a website or the history.
  $('mes-guardar').addEventListener('click', async () => {
    const c = $('lienzo');
    const x = c.getContext('2d');
    await document.fonts.ready;
    const css = (v) => getComputedStyle(document.documentElement).getPropertyValue(v).trim();
    x.fillStyle = '#F4F6FA'; x.fillRect(0, 0, 1200, 630);
    x.strokeStyle = 'rgba(11,16,32,.05)';
    for (let i = 0; i < 1200; i += 44) { x.beginPath(); x.moveTo(i, 0); x.lineTo(i, 630); x.stroke(); }
    for (let j = 0; j < 630; j += 44) { x.beginPath(); x.moveTo(0, j); x.lineTo(1200, j); x.stroke(); }
    x.fillStyle = '#1F4BFF'; x.beginPath(); x.arc(84, 84, 9, 0, Math.PI * 2); x.fill();
    x.fillStyle = '#0B1020'; x.font = "600 24px Unbounded"; x.fillText('GUARDIANA ZERO', 108, 93);
    const nombreMes = new Date().toLocaleDateString(document.documentElement.lang, { month: 'long', year: 'numeric' });
    x.fillStyle = '#5B6275'; x.font = '500 22px Plex'; x.fillText(t('mes_titulo') + ' · ' + nombreMes, 72, 170);
    x.fillStyle = '#0B1020'; x.font = "600 46px Unbounded";
    const frase = fraseMes(mes, new Date().toLocaleDateString(document.documentElement.lang, { month: 'long' }));
    const palabras = frase.split(' '); let linea = ''; let y = 250;
    for (const p of palabras) {
      const prueba = linea ? linea + ' ' + p : p;
      if (x.measureText(prueba).width > 1056) { x.fillText(linea, 72, y); y += 60; linea = p; } else linea = prueba;
    }
    x.fillText(linea, 72, y);
    x.fillStyle = '#1F4BFF'; x.font = '500 20px PlexMono'; x.fillText('guardianagroup.com', 72, 570);
    x.fillStyle = css('--gris') || '#5B6275'; x.font = '400 18px Plex'; x.fillText(t('lema'), 72, 540);
    manda({ tipo: 'guardar_imagen', nombre: 'guardiana-zero-mes.png', datos: c.toDataURL('image/png') });
  });

  en('guardado', (m) => { const g = $('guardada'); g.textContent = m.texto || ''; g.classList.remove('oculto'); });

  manda({ tipo: 'listo', vista: 'inicio' });
})();
