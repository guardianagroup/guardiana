// «Continuar en el móvil» without a gap (18 Oct 2026, the committee's spec: make before break).
// This computer's page keeps its own WebRTC leg to the other person for the whole call. When the
// person's phone takes the call, the phone opens a second WebRTC leg to this page (relay only,
// like every call here: neither end ever sees an address), and only once sound is arriving on it
// does this page swap the voice it sends to the other person for the phone's, close its own
// microphone and go quiet. The other person's connection is never renegotiated, so they hear no
// cut. If the phone's leg does not come up in 12 s, nothing here changed (the program then tells
// both screens «No se pudo pasar — vuelve a escanear»). «Volver al PC» is the same in reverse:
// the microphone opens again and replaces the phone's voice on the same connection.
// The decision lives in the program (crates/zero/src/relevo.rs); this file only carries sound.
'use strict';
(() => {
  const { manda, en } = Z;
  const PASO_MS = 12000;
  let E = null;
  // The bridge: its connection, the session id the phone chose, the phone's voice, whether it
  // carries sound already, and candidates that came before the offer was in.
  let p = null;
  const L = () => window.GuardianaLlamada;
  const dice = (d) => manda({ tipo: 'secure', accion: 'puente', d });
  const paso = (m) => ((m && m.movil) || {}).paso || {};
  const vozDelOtro = () => { const r = L() && L().remoto(); return (r && r.getAudioTracks()[0]) || null; };
  const transceptor = () => (p && p.pc.getTransceivers().find((x) => x.receiver && x.receiver.track && x.receiver.track.kind === 'audio')) || null;

  const cierra = (avisar) => {
    if (!p) return;
    const fue = p;
    p = null;
    clearTimeout(fue.reloj); clearInterval(fue.oido);
    try { fue.pc.close(); } catch (_) {}
    // The voice comes back here only if it had gone to the phone.
    if (fue.viva && L()) L().alPc();
    if (avisar) dice({ t: 'fin', s: fue.s });
  };
  const falla = () => { cierra(true); manda({ tipo: 'secure', accion: 'puente_fallo', sin_rtc: false }); };

  // Sound is arriving from the phone (its packets are counted, not assumed): only now does the
  // voice move.
  const vigilaSonido = () => {
    const mio = p;
    if (!mio || mio.oido) return;
    mio.oido = setInterval(async () => {
      if (p !== mio) { clearInterval(mio.oido); return; }
      let st; try { st = await mio.pc.getStats(); } catch (_) { return; }
      let paquetes = 0;
      st.forEach((x) => { if (x.type === 'inbound-rtp' && x.kind === 'audio') paquetes = x.packetsReceived || 0; });
      if (paquetes > 5 && mio.pista && mio.pc.connectionState === 'connected') {
        clearInterval(mio.oido);
        if (p !== mio || mio.viva) return;
        mio.viva = true; clearTimeout(mio.reloj);
        if (L()) L().alMovil(mio.pista);
        manda({ tipo: 'secure', accion: 'puente_listo' });
      }
    }, 200);
  };

  en('secure_estado', (m) => {
    E = m;
    const x = paso(m);
    // The program moved the voice back (asked, failed, the phone went away) or the call ended.
    if (p && (!m.llamada || x.relevo === 'pc' || !x.puente)) cierra(true);
  });

  en('secure_puente', async (m) => {
    const d = m.d || {};
    if (d.t === 'fin') { if (p && (!d.s || d.s === p.s)) cierra(false); return; }
    if (d.t === 'ice') {
      if (!p || d.s !== p.s || !d.c) return;
      if (p.pc.remoteDescription) p.pc.addIceCandidate(d.c).catch(() => {});
      else if (p.pendientes.length < 200) p.pendientes.push(d.c);
      return;
    }
    if (d.t !== 'offer' || typeof d.sdp !== 'string') return;
    const l = E && E.llamada;
    if (!l || !l.rtc || !l.rtc.ice || !window.RTCPeerConnection) { falla(); return; }
    cierra(false);
    const pc = new RTCPeerConnection({ iceServers: l.rtc.ice, iceTransportPolicy: 'relay', bundlePolicy: 'max-bundle' });
    p = { pc, s: String(d.s || ''), pista: null, viva: false, pendientes: [], oido: null, reloj: null };
    const mio = p;
    mio.reloj = setTimeout(() => { if (p === mio && !mio.viva) falla(); }, PASO_MS);
    pc.onicecandidate = (e) => { if (e.candidate && p === mio) dice({ t: 'ice', s: mio.s, c: e.candidate.toJSON() }); };
    pc.ontrack = (e) => { if (p === mio && e.track.kind === 'audio') { mio.pista = e.track; vigilaSonido(); } };
    pc.onconnectionstatechange = () => {
      if (p !== mio) return;
      if (pc.connectionState === 'connected') vigilaSonido();
      else if (pc.connectionState === 'failed' || pc.connectionState === 'closed') falla();
    };
    try {
      await pc.setRemoteDescription({ type: 'offer', sdp: d.sdp });
      const tr = transceptor();
      if (tr) { tr.direction = 'sendrecv'; const a = vozDelOtro(); if (a) await tr.sender.replaceTrack(a); }
      const a = await pc.createAnswer();
      if (window.GuardianaOpus) a.sdp = window.GuardianaOpus.sdp(a.sdp);
      await pc.setLocalDescription(a);
      if (p !== mio) return;
      dice({ t: 'answer', s: mio.s, sdp: pc.localDescription.sdp });
      mio.pendientes.splice(0).forEach((c) => pc.addIceCandidate(c).catch(() => {}));
    } catch (_) { if (p === mio) falla(); }
  });

  // The other person's voice reached this page (or a new connection to them came up): it is
  // what the phone hears.
  window.addEventListener('guardiana-remoto', () => { const tr = transceptor(), a = vozDelOtro(); if (tr && a) tr.sender.replaceTrack(a).catch(() => {}); });
  // How this computer's leg sounds (opus.js, every 2 s): the phone shows the worse of the two.
  window.addEventListener('guardiana-calidad', (e) => { if (E && E.llamada && e.detail && e.detail.nivel) manda({ tipo: 'secure', accion: 'calidad', nivel: e.detail.nivel }); });

  window.GuardianaRelevo = {
    // This page's own leg left WebRTC: the bridge cannot serve this call any more.
    // (secure.js already takes the voice back itself, so the bridge just closes.)
    rtcCayo: () => { const habia = !!p; if (p) p.viva = false; cierra(true); if (habia || ((paso(E).relevo || 'pc') !== 'pc')) manda({ tipo: 'secure', accion: 'puente_fallo', sin_rtc: true }); },
    // The call goes by the bridge (this page keeps its leg while the phone talks).
    modo: () => !!paso(E).puente,
    vivo: () => !!(p && p.viva),
  };
})();
