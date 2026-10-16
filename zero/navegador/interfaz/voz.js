// «Voz protegida» (16 Oct 2026, the owner: «que nadie pueda clonar nuestra voz»): the voice
// leaves transformed, before it is encrypted, so this call is no clean sample of the real voice.
// Pitch and timbre change with nothing but standard Web Audio nodes (two modulated delay lines
// cross-faded, the classic granular shifter, plus a tilt in the spectrum): no library, no
// server, the same code on the computer and in the phone's page. It does not make a voice
// impossible to clone from other recordings; it makes this one useless for it.
(function () {
  'use strict';
  const PERIODO = 0.1, FUNDIDO = 0.04, BASE = 0.12;
  // One looping buffer: a ramp 0→1 (the delay sweep) or a window (the cross-fade).
  const fuente = (ctx, f, fase) => {
    const n = Math.round(PERIODO * ctx.sampleRate);
    const b = ctx.createBuffer(1, n, ctx.sampleRate), d = b.getChannelData(0);
    for (let i = 0; i < n; i++) d[i] = f(((i / n) + fase) % 1);
    const s = ctx.createBufferSource(); s.buffer = b; s.loop = true; s.start();
    return s;
  };
  const rampa = (x) => x;
  const ventana = (x) => { const f = FUNDIDO / PERIODO; return x < f ? x / f : x > 1 - f ? (1 - x) / f : 1; };
  // The chain for one source: `semitonos` below zero lowers the voice; a tilt moves the timbre.
  function desplazador(ctx, semitonos) {
    const r = Math.pow(2, semitonos / 12);
    const A = (1 - r) * PERIODO;
    const entrada = ctx.createGain(), salida = ctx.createGain();
    for (const fase of [0, 0.5]) {
      const d = ctx.createDelay(1); d.delayTime.value = BASE;
      const g = ctx.createGain(); g.gain.value = A;
      fuente(ctx, rampa, fase).connect(g).connect(d.delayTime);
      const v = ctx.createGain(); v.gain.value = 0;
      fuente(ctx, ventana, fase).connect(v.gain);
      entrada.connect(d).connect(v).connect(salida);
    }
    // Timbre: less chest, more presence, so the voice reads as another person's.
    const grave = ctx.createBiquadFilter(); grave.type = 'lowshelf'; grave.frequency.value = 300; grave.gain.value = -6;
    const medio = ctx.createBiquadFilter(); medio.type = 'peaking'; medio.frequency.value = 1700; medio.Q.value = 0.9; medio.gain.value = 5;
    salida.connect(grave).connect(medio);
    return { entrada, salida: medio };
  }
  // A new stream: the audio transformed, the video tracks as they are. `parar()` releases it.
  function vozProtegida(stream, semitonos) {
    const ctx = new (window.AudioContext || window.webkitAudioContext)({ sampleRate: 48000 });
    const origen = ctx.createMediaStreamSource(new MediaStream(stream.getAudioTracks()));
    const destino = ctx.createMediaStreamDestination();
    const d = desplazador(ctx, semitonos);
    origen.connect(d.entrada); d.salida.connect(destino);
    const s = new MediaStream([...destino.stream.getAudioTracks(), ...stream.getVideoTracks()]);
    return { stream: s, parar: () => { try { ctx.close(); } catch (_) {} } };
  }
  window.vozProtegida = vozProtegida;
})();
