// The voice of a call at its best (18 Oct 2026, the committee — Claude, ChatGPT, Grok, Kimi —
// after the owner kept voice only: «llamada de voz de máxima calidad»). Everything here is the
// browser's own WebRTC, tuned; no library, no server.
// - Opus, mono, full band (48 kHz), in-band FEC on, DTX off (no gaps of comfort noise), VBR,
//   20 ms frames (40 at most), up to 64 kb/s on the wire and a target of 48 kb/s per direction.
// - RED (RFC 2198, «audio/red») first when both browsers offer it: each packet also carries the
//   previous frame, so a lost packet costs nothing; Opus' FEC stays on underneath.
// - The bitrate follows the network: down to 32 or 24 kb/s while loss or delay rise, back to 48
//   after ten quiet seconds. The jitter buffer is left adaptive.
// - The browser's echo canceller stays on (never two cancellers); noise suppression and gain too.
// - Every 2 s the call's own statistics give a simple estimate (ITU-T G.107 E-model, simplified)
//   of how it sounds: «excelente», «buena» or «débil», for the shield. It is an estimate and the
//   shield says so; the raw figures stay in the call panel.
'use strict';
(function (raiz) {
  const PARAMS = { stereo: '0', 'sprop-stereo': '0', maxplaybackrate: '48000', 'sprop-maxcapturerate': '48000', maxaveragebitrate: '64000', useinbandfec: '1', usedtx: '0', cbr: '0' };
  const OBJETIVO = 48000, MEDIO = 32000, BAJO = 24000;

  // The audio part of an SDP with Opus as we want it; anything we do not understand is left as it
  // came (a wrong edit would break the call, a missing one only costs quality).
  function sdp(texto) {
    try {
      const lineas = String(texto).split(/\r\n/);
      const m = /^a=rtpmap:(\d+) opus\/48000/i;
      const pt = (lineas.map((l) => m.exec(l)).find(Boolean) || [])[1];
      if (!pt) return texto;
      let fmtp = false;
      const fuera = lineas.map((l) => {
        if (l.startsWith('a=fmtp:' + pt + ' ')) {
          fmtp = true;
          const v = {};
          l.slice(('a=fmtp:' + pt + ' ').length).split(';').map((x) => x.trim()).filter(Boolean).forEach((x) => { const [a, b] = x.split('='); v[a] = b; });
          Object.assign(v, PARAMS);
          return 'a=fmtp:' + pt + ' ' + Object.entries(v).map(([a, b]) => a + '=' + b).join(';');
        }
        return l;
      });
      const i = fuera.findIndex((l) => m.test(l));
      if (!fmtp) fuera.splice(i + 1, 0, 'a=fmtp:' + pt + ' ' + Object.entries(PARAMS).map(([a, b]) => a + '=' + b).join(';'));
      // ptime and maxptime once, inside the audio section.
      const inicio = fuera.findIndex((l) => l.startsWith('m=audio'));
      if (inicio >= 0) {
        let fin = fuera.findIndex((l, k) => k > inicio && l.startsWith('m='));
        if (fin < 0) fin = fuera.length;
        const seccion = fuera.slice(inicio, fin);
        const extra = [];
        if (!seccion.some((l) => l.startsWith('a=ptime:'))) extra.push('a=ptime:20');
        if (!seccion.some((l) => l.startsWith('a=maxptime:'))) extra.push('a=maxptime:40');
        // Before the trailing empty line of the section, if any.
        let donde = fin;
        while (donde > inicio + 1 && fuera[donde - 1] === '') donde--;
        fuera.splice(donde, 0, ...extra);
      }
      return fuera.join('\r\n');
    } catch (_) { return texto; }
  }

  // The microphone as the call wants it.
  function restricciones() {
    return { echoCancellation: true, noiseSuppression: true, autoGainControl: true, channelCount: 1, sampleRate: 48000 };
  }

  // A simplified E-model (ITU-T G.107) for a wideband codec: delay one way (ms) and loss (%) to
  // a MOS between 1 and 4.5. Opus' impairment is taken as 0 and its robustness to loss as 20
  // (an approximation, said as such on the screen).
  function mos(unaViaMs, perdidaPct) {
    const d = Math.max(0, unaViaMs || 0), p = Math.max(0, perdidaPct || 0);
    const id = 0.024 * d + (d > 177.3 ? 0.11 * (d - 177.3) : 0);
    const ie = 95 * p / (p + 20);
    const r = Math.max(0, Math.min(100, 93.2 - id - ie));
    return 1 + 0.035 * r + 7e-6 * r * (r - 60) * (100 - r);
  }
  function nivel(m, ocultadoPct) {
    if ((ocultadoPct || 0) > 5 || m < 3.5) return 'debil';
    return m >= 4.0 ? 'excelente' : 'buena';
  }
  // The bitrate for what the network shows now.
  function caudal(actual, perdidaPct, rttMs, tranquilos) {
    if (perdidaPct > 10 || rttMs > 600) return BAJO;
    if (perdidaPct > 4 || rttMs > 350) return Math.min(actual, MEDIO);
    if (tranquilos >= 5) return OBJETIVO;
    return actual;
  }

  // One call: codec order (RED first), the sender's limits, the statistics every 2 s.
  function prepara(pc) {
    try {
      const caps = raiz.RTCRtpReceiver && RTCRtpReceiver.getCapabilities && RTCRtpReceiver.getCapabilities('audio');
      if (caps && pc.getTransceivers) {
        const red = caps.codecs.filter((c) => /audio\/red/i.test(c.mimeType));
        const opus = caps.codecs.filter((c) => /audio\/opus/i.test(c.mimeType));
        const resto = caps.codecs.filter((c) => !red.includes(c) && !opus.includes(c));
        pc.getTransceivers().forEach((tr) => {
          if (tr.sender && tr.sender.track && tr.sender.track.kind === 'audio' && tr.setCodecPreferences && opus.length) {
            try { tr.setCodecPreferences([...red, ...opus, ...resto]); } catch (_) {}
          }
        });
      }
    } catch (_) {}
    const emisor = () => pc.getSenders().find((s) => s.track && s.track.kind === 'audio');
    let bitrate = OBJETIVO;
    const aplica = (b) => {
      const s = emisor(); if (!s) return;
      try {
        const p = s.getParameters();
        p.encodings = p.encodings && p.encodings.length ? p.encodings : [{}];
        p.encodings[0].maxBitrate = b;
        p.encodings[0].priority = 'high';
        p.encodings[0].networkPriority = 'high';
        s.setParameters(p).catch(() => {});
      } catch (_) {}
    };
    aplica(bitrate);
    let antes = null, tranquilos = 0;
    const reloj = setInterval(async () => {
      if (pc.connectionState === 'closed') { clearInterval(reloj); return; }
      let st; try { st = await pc.getStats(); } catch (_) { return; }
      let ent = null, rtt = null, remoto = null;
      st.forEach((x) => {
        if (x.type === 'inbound-rtp' && x.kind === 'audio') ent = x;
        if (x.type === 'remote-inbound-rtp' && x.kind === 'audio') remoto = x;
        if (x.type === 'candidate-pair' && x.nominated && typeof x.currentRoundTripTime === 'number') rtt = x.currentRoundTripTime * 1000;
      });
      if (!ent) return;
      // Loss in these two seconds, not since the start.
      const ahora = { perdidos: ent.packetsLost || 0, recibidos: ent.packetsReceived || 0, ocultados: ent.concealedSamples || 0, muestras: ent.totalSamplesReceived || 0 };
      let perdida = 0, ocultado = 0;
      if (antes) {
        const p = ahora.perdidos - antes.perdidos, r = ahora.recibidos - antes.recibidos;
        perdida = p + r > 0 ? (100 * Math.max(0, p)) / (p + r) : 0;
        const o = ahora.ocultados - antes.ocultados, mu = ahora.muestras - antes.muestras;
        ocultado = mu > 0 ? (100 * Math.max(0, o)) / mu : 0;
      }
      antes = ahora;
      if (rtt === null && remoto && typeof remoto.roundTripTime === 'number') rtt = remoto.roundTripTime * 1000;
      const jitter = (ent.jitter || 0) * 1000;
      const unaVia = (rtt || 0) / 2 + jitter * 2 + 20;
      const m = mos(unaVia, perdida);
      // The other side's view of our stream says how our sending fares.
      const suPerdida = remoto && typeof remoto.fractionLost === 'number' ? remoto.fractionLost * 100 : perdida;
      tranquilos = suPerdida < 1 && (rtt || 0) < 250 ? tranquilos + 1 : 0;
      const nuevo = caudal(bitrate, suPerdida, rtt || 0, tranquilos);
      if (nuevo !== bitrate) { bitrate = nuevo; aplica(bitrate); }
      raiz.dispatchEvent(new CustomEvent('guardiana-calidad', { detail: { nivel: nivel(m, ocultado), mos: Math.round(m * 10) / 10, perdida: Math.round(perdida * 10) / 10, rtt_ms: rtt === null ? null : Math.round(rtt), kbps: bitrate / 1000, estimacion: true } }));
    }, 2000);
    return { para: () => clearInterval(reloj) };
  }

  // A change of network (Wi-Fi to 4G): the page restarts ICE on the same connection.
  function enCambioDeRed(f) {
    const c = raiz.navigator && navigator.connection;
    if (c && c.addEventListener) c.addEventListener('change', f);
    raiz.addEventListener && raiz.addEventListener('online', f);
  }

  const api = { sdp, restricciones, mos, nivel, caudal, prepara, enCambioDeRed, PARAMS };
  raiz.GuardianaOpus = api;
  if (typeof module !== 'undefined' && module.exports) module.exports = api;
})(typeof window !== 'undefined' ? window : globalThis);
