// GUARDIANA ZERO 1.0.46 (Z1, the owner's choice of 18 Oct 2026): the shield of dots. One dot per
// attempt cut, red; the ones cut in the last minute amber («cortándose ahora»); the rest of the
// shield, the day's free room, a faint steel. Never a made-up dot: when there are more attempts
// than room, each dot is the same whole number of attempts and the page says how many
// (`porPunto`). Used by the new tab (Z1-a) and the side summary (Z1-b).
'use strict';
const ZeroEscudo = (() => {
  // The shield's outline: straight top with rounded shoulders, a point below.
  function silueta(W, H) {
    const p = new Path2D();
    const m = 2, w = W - 2 * m, h = H - 2 * m;
    p.moveTo(m + w * 0.5, m);
    p.bezierCurveTo(m + w * 0.68, m + h * 0.07, m + w * 0.86, m + h * 0.09, m + w, m + h * 0.1);
    p.bezierCurveTo(m + w, m + h * 0.48, m + w * 0.92, m + h * 0.78, m + w * 0.5, m + h);
    p.bezierCurveTo(m + w * 0.08, m + h * 0.78, m, m + h * 0.48, m, m + h * 0.1);
    p.bezierCurveTo(m + w * 0.14, m + h * 0.09, m + w * 0.32, m + h * 0.07, m + w * 0.5, m);
    p.closePath();
    return p;
  }
  // The places for dots inside the outline, a hexagonal grid of step `sp`, bottom rows first.
  function rejilla(ctx, p, W, H, sp) {
    const pts = [];
    for (let y = 2 + sp * 0.8; y < H; y += sp * 0.866) {
      const fila = Math.round(y / (sp * 0.866));
      for (let x = 2 + (fila % 2 ? sp / 2 : 0) + sp * 0.5; x < W; x += sp) {
        if (ctx.isPointInPath(p, x, y)) pts.push([x, y]);
      }
    }
    pts.sort((a, b) => b[1] - a[1] || a[0] - b[0]);
    return pts;
  }
  // A fixed shuffle (the same picture for the same figures), so the edge of the filled part is
  // irregular like the mockup's but never moves on its own between two redraws.
  function azar(semilla) {
    let s = semilla >>> 0 || 1;
    return () => { s = (Math.imul(s ^ (s >>> 15), 2246822507) + 0x9e3779b9) >>> 0; return (s >>> 8) / 16777216; };
  }
  // How many places the shield has for `n` attempts: room for the day to grow, never fewer than
  // `minimo`, never more than `maximo` (beyond it, dots grow too small to see).
  function capacidad(n, minimo, maximo) {
    const c = Math.max(minimo, Math.ceil((n * 1.08) / 10) * 10);
    return Math.min(maximo, c);
  }
  // Draw on `lienzo` (a <canvas> already sized by CSS). o: { n, ambar, minimo, maximo }.
  // Returns { porPunto, puntos, rojos, ambar } for the legend.
  function dibuja(lienzo, o) {
    const n = Math.max(0, Math.floor(Number(o.n) || 0));
    const dpr = window.devicePixelRatio || 1;
    const W = lienzo.clientWidth, H = lienzo.clientHeight;
    if (!W || !H) return { porPunto: 1, puntos: 0, rojos: 0, ambar: 0 };
    lienzo.width = Math.round(W * dpr); lienzo.height = Math.round(H * dpr);
    const ctx = lienzo.getContext('2d');
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, W, H);
    const p = silueta(W, H);
    const maximo = o.maximo || 1400;
    const objetivo = capacidad(n, o.minimo || 120, maximo);
    // The step whose grid holds about `objetivo` places.
    let lo = 2, hi = 40;
    for (let i = 0; i < 26; i++) { const mid = (lo + hi) / 2; if (rejilla(ctx, p, W, H, mid).length > objetivo) lo = mid; else hi = mid; }
    const sp = lo;
    const pts = rejilla(ctx, p, W, H, sp);
    const porPunto = Math.max(1, Math.ceil(n / Math.max(1, pts.length)));
    const rojos = Math.min(pts.length, Math.ceil(n / porPunto));
    const ambar = Math.min(rojos, Math.ceil(Math.max(0, Math.floor(Number(o.ambar) || 0)) / porPunto));
    const r = azar(o.semilla || 7);
    // Fill from the bottom with an irregular edge; the newest (amber) are the last filled.
    const orden = pts.map((_, i) => i).sort((a, b) => (a + (r() - 0.5) * 40) - (b + (r() - 0.5) * 40));
    const estado = new Uint8Array(pts.length);
    for (let i = 0; i < rojos; i++) estado[orden[i]] = i >= rojos - ambar ? 2 : 1;
    const radio = sp * 0.36;
    const r2 = azar((o.semilla || 7) + 1);
    for (let i = 0; i < pts.length; i++) {
      ctx.fillStyle = estado[i] === 2 ? 'rgba(224,182,106,1)' : estado[i] === 1 ? `rgba(224,122,108,${0.55 + r2() * 0.45})` : 'rgba(156,195,228,0.13)';
      ctx.beginPath(); ctx.arc(pts[i][0], pts[i][1], radio, 0, Math.PI * 2); ctx.fill();
    }
    ctx.strokeStyle = 'rgba(156,195,228,0.35)'; ctx.lineWidth = 1; ctx.stroke(p);
    return { porPunto, puntos: pts.length, rojos, ambar };
  }
  return { dibuja };
})();
// Also as a property of the window: the side summary asks `window.ZeroEscudo` (a top-level const
// is not one).
window.ZeroEscudo = ZeroEscudo;
