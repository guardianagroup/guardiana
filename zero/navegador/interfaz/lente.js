// GUARDIANA ZERO · la Lente de actividad de la portada (30 Oct 2026). Sustituye al escudo de puntos (el dueño: «ese escudo…
// sustitúyelo por algo más profesional aún y vistoso»; agencia, ronda 78: Astra propuso la Lente y Claude coincidió en el
// NÚCLEO dentro y el anillo alrededor).
//
// Un disco de cristal oscuro con la cifra de hoy en el centro y, alrededor, 60 barras: una por minuto de la última hora.
//   · el largo de cada barra, los intentos cortados en ese minuto (escala logarítmica, para que una ráfaga no borre lo demás);
//   · rojo los minutos pasados, ámbar el minuto en curso (aún sin cerrar);
//   · una marca corta de acero: minuto visto, sin cortes; un punto tenue: minuto sin registro (ninguna pestaña lo vio).
// Dentro, la trama del NÚCLEO de la web, quieta y tenue: es la firma de la marca, no representa empresas ni conexiones.
// Nunca una cifra inventada; sin bucles de animación (se dibuja cuando llegan datos).
'use strict';
const ZeroLente = (() => {
  // La trama del NÚCLEO: puntos repartidos en una esfera (espiral de Fibonacci), girada un poco y vista de frente, y cada
  // punto unido a sus tres vecinos más cercanos. Siempre la misma.
  const TRAMA = (() => {
    const n = 34, p = [];
    const oro = Math.PI * (3 - Math.sqrt(5));
    for (let i = 0; i < n; i++) {
      const y = 1 - (2 * (i + 0.5)) / n, r = Math.sqrt(1 - y * y), a = i * oro;
      let x = Math.cos(a) * r, z = Math.sin(a) * r, yy = y;
      // giro fijo: 0,5 rad sobre X y 0,35 sobre Y
      const c1 = Math.cos(0.5), s1 = Math.sin(0.5), c2 = Math.cos(0.35), s2 = Math.sin(0.35);
      [yy, z] = [yy * c1 - z * s1, yy * s1 + z * c1];
      [x, z] = [x * c2 + z * s2, -x * s2 + z * c2];
      p.push([x, yy, z]);
    }
    const aristas = new Set();
    p.forEach((a, i) => {
      const d = p.map((b, j) => [j, (a[0] - b[0]) ** 2 + (a[1] - b[1]) ** 2 + (a[2] - b[2]) ** 2]).filter(([j]) => j !== i).sort((u, v) => u[1] - v[1]);
      for (const [j] of d.slice(0, 3)) aristas.add(i < j ? i + '-' + j : j + '-' + i);
    });
    return { p, aristas: [...aristas].map((k) => k.split('-').map(Number)) };
  })();

  // Dibuja la lente en `lienzo` (canvas). o = { numero: '342', rotulo: 'CORTADOS HOY', serie: [60 × (n | null)] }
  function dibuja(lienzo, o) {
    const dpr = window.devicePixelRatio || 1;
    const caja = lienzo.getBoundingClientRect();
    const W = caja.width || 340, H = caja.height || 340;
    lienzo.width = Math.round(W * dpr); lienzo.height = Math.round(H * dpr);
    const x = lienzo.getContext('2d');
    x.setTransform(dpr, 0, 0, dpr, 0, 0);
    x.clearRect(0, 0, W, H);
    const cx = W / 2, cy = H / 2, R = Math.min(W, H) * 0.46;
    const ACERO = (a) => `rgba(156,195,228,${a})`;

    // el halo, muy suave, y el disco de cristal
    const halo = x.createRadialGradient(cx, cy, R * 0.6, cx, cy, R * 1.12);
    halo.addColorStop(0, ACERO(0.07)); halo.addColorStop(1, ACERO(0));
    x.fillStyle = halo; x.beginPath(); x.arc(cx, cy, R * 1.12, 0, Math.PI * 2); x.fill();
    const disco = x.createRadialGradient(cx, cy - R * 0.45, R * 0.05, cx, cy, R);
    disco.addColorStop(0, '#131E2B'); disco.addColorStop(0.55, '#0D141D'); disco.addColorStop(1, '#0A1018');
    x.fillStyle = disco; x.beginPath(); x.arc(cx, cy, R - 1.5, 0, Math.PI * 2); x.fill();
    // el reflejo de arriba
    x.save(); x.beginPath(); x.arc(cx, cy, R - 1.5, 0, Math.PI * 2); x.clip();
    const refl = x.createLinearGradient(0, cy - R, 0, cy - R * 0.25);
    refl.addColorStop(0, 'rgba(244,246,248,.10)'); refl.addColorStop(1, 'rgba(244,246,248,0)');
    x.fillStyle = refl; x.beginPath(); x.ellipse(cx, cy - R * 0.62, R * 0.86, R * 0.42, 0, 0, Math.PI * 2); x.fill();
    x.restore();
    // los dos contornos
    x.lineWidth = 1; x.strokeStyle = ACERO(0.32); x.beginPath(); x.arc(cx, cy, R, 0, Math.PI * 2); x.stroke();
    x.lineWidth = 0.75; x.strokeStyle = ACERO(0.11); x.beginPath(); x.arc(cx, cy, R * 0.70, 0, Math.PI * 2); x.stroke();

    // la trama del NÚCLEO
    const rn = R * 0.5;
    const pr = TRAMA.p.map(([a, b, c]) => [cx + a * rn, cy + b * rn, c]);
    x.lineWidth = 0.65;
    for (const [i, j] of TRAMA.aristas) {
      const zf = (pr[i][2] + pr[j][2]) / 2;
      x.strokeStyle = ACERO(0.07 + 0.08 * (zf + 1) / 2);
      x.beginPath(); x.moveTo(pr[i][0], pr[i][1]); x.lineTo(pr[j][0], pr[j][1]); x.stroke();
    }
    for (const [a, b, c] of pr) { x.fillStyle = ACERO(0.14 + 0.16 * (c + 1) / 2); x.beginPath(); x.arc(a, b, 1.2, 0, Math.PI * 2); x.fill(); }

    // las 60 barras, desde arriba y en el sentido de las agujas del reloj: la más antigua primero, el minuto en curso al final
    const serie = o.serie || [];
    const r0 = R * 0.745, L = R * 0.2;
    const max = Math.max(0, ...serie.filter((n) => n != null));
    x.lineCap = 'round';
    for (let i = 0; i < 60; i++) {
      const n = serie[i];
      const ang = -Math.PI / 2 + (i + 0.5) * (Math.PI * 2 / 60);
      const ux = Math.cos(ang), uy = Math.sin(ang);
      if (n == null) {
        x.fillStyle = ACERO(0.18); x.beginPath(); x.arc(cx + ux * (r0 + 1.5), cy + uy * (r0 + 1.5), 0.9, 0, Math.PI * 2); x.fill();
        continue;
      }
      const largo = n > 0 && max > 0 ? Math.max(4, L * Math.log1p(n) / Math.log1p(max)) : 2.5;
      x.strokeStyle = n === 0 ? ACERO(0.45) : i === 59 ? '#E0B66A' : '#E07A6C';
      x.lineWidth = n === 0 ? 1.5 : 3;
      x.beginPath(); x.moveTo(cx + ux * r0, cy + uy * r0); x.lineTo(cx + ux * (r0 + largo), cy + uy * (r0 + largo)); x.stroke();
    }

    // en el centro, la cifra de hoy y su rótulo
    const numero = String(o.numero == null ? '0' : o.numero);
    let fs = R * 0.36;
    x.font = `300 ${fs}px "IBM Plex Sans", "Segoe UI", system-ui, sans-serif`;
    while (fs > R * 0.22 && x.measureText(numero).width > R * 1.05) { fs -= 1; x.font = `300 ${fs}px "IBM Plex Sans", "Segoe UI", system-ui, sans-serif`; }
    x.textAlign = 'center'; x.textBaseline = 'alphabetic'; x.fillStyle = '#F4F6F8';
    x.fillText(numero, cx, cy + fs * 0.28);
    if (o.rotulo) {
      const fr = Math.max(9, R * 0.068);
      x.font = `500 ${fr}px "IBM Plex Mono", Consolas, monospace`;
      try { x.letterSpacing = (fr * 0.22).toFixed(1) + 'px'; } catch (_) {}
      x.fillStyle = '#8fa9bd';
      x.fillText(o.rotulo, cx, cy + fs * 0.28 + fr * 2.1);
      try { x.letterSpacing = '0px'; } catch (_) {}
    }
  }
  return { dibuja };
})();
