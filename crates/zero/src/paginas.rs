//! What GUARDIANA ZERO puts inside web pages, all of it here so it can be read in one place:
//!
//! - the page shown instead of an address that was cut (a mandate's limit, a site the person
//!   cut, the decoy leaving), written in the person's language with no outside resource;
//! - the script every page of a web tab gets before its own code runs: the Global Privacy
//!   Control signal (`navigator.globalPrivacyControl`, sent as `Sec-GPC: 1` by the shell), the
//!   form guard that asks before personal data leaves in a form, and the measurement of reads
//!   that serve to recognise the computer (decision 206), in the page and in its frames; with
//!   maximum protection, other companies' scripts get a slightly different canvas and audio and a
//!   generic graphics card, fixed for each site and run;
//! - in a mandate's tab, before that, WebRTC and WebTransport turned off: connections the
//!   request filter does not see, which a page could use to take data outside the task.
//!
//! The script never reads anything the person has not typed into a form being sent, and what it
//! reads goes only to the browser itself (never to a server): the browser decides, in the side
//! panel, out of the page's reach.

use crate::textos::Textos;

/// Escape for HTML text and attributes.
#[must_use]
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// The page shown in place of a cut address. `texto` is the reason, already in the person's
/// language; `url` the address that was not opened; `tema` the person's day or night (`"dia"`,
/// `"noche"`, or `""` to follow Windows), like the browser's own pages.
#[must_use]
pub fn pagina_cortada(t: &Textos, texto: &str, url: &str, tema: &str) -> String {
    const NOCHE: &str = "--hoja:#0F1528;--marco:#060914;--tinta:#E8ECF6;--gris:#98A1B8;--linea:#232C45;--azul:#7B98FF;--rojo:#FF7A6E;--rojo-suave:#3A1814;--sobre:#0B1020";
    let (esquema, noche) = match tema {
        "dia" => ("light", String::new()),
        "noche" => ("dark", format!(":root{{{NOCHE}}}")),
        _ => (
            "light dark",
            format!("@media (prefers-color-scheme:dark){{:root{{{NOCHE}}}}}"),
        ),
    };
    let titulo = t.t("pagina_cortada_titulo");
    let mut url_visible: String = url.chars().take(300).collect();
    if url.chars().count() > 300 {
        url_visible.push('…');
    }
    format!(
        r#"<!doctype html><html lang="{lang}"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width"><meta name="robots" content="noindex"><title>{titulo}</title><style>
:root{{color-scheme:{esquema};--hoja:#FFFFFF;--marco:#E8ECF4;--tinta:#0B1020;--gris:#5B6275;--linea:#D9DEE8;--azul:#1F4BFF;--rojo:#C0301A;--rojo-suave:#FBE7E3;--sobre:#FFFFFF}}
{noche}
html,body{{margin:0;min-height:100%;background:var(--hoja);color:var(--tinta);font:16px/1.55 "Segoe UI Variable Text","Segoe UI",system-ui,sans-serif}}
main{{max-width:600px;margin:0 auto;padding:16vh 28px 48px}}
.marca{{display:flex;align-items:center;gap:10px;color:var(--gris);font-size:12.5px;font-weight:600;letter-spacing:.03em}}
.marca i{{width:10px;height:10px;border-radius:50%;background:var(--rojo);box-shadow:0 0 0 4px var(--rojo-suave)}}
h1{{font-size:28px;line-height:1.2;margin:20px 0 12px;letter-spacing:-.01em;font-weight:600}}
p{{margin:0 0 12px;color:var(--gris)}}
code{{display:block;margin-top:18px;padding:10px 12px;border-radius:10px;background:var(--marco);font:13px/1.45 Consolas,ui-monospace,monospace;color:var(--tinta);word-break:break-all}}
a{{display:inline-block;margin-top:26px;padding:9px 18px;border-radius:19px;background:var(--azul);color:var(--sobre);text-decoration:none;font-weight:600;font-size:14px}}
a:focus-visible{{outline:3px solid var(--linea);outline-offset:2px}}
</style></head><body><main><div class="marca"><i></i>GUARDIANA ZERO</div><h1>{titulo}</h1><p>{texto}</p><code>{url}</code><a href="javascript:history.back()">{volver}</a></main></body></html>"#,
        lang = t.idioma().codigo(),
        titulo = esc(&titulo),
        texto = esc(texto),
        url = esc(&url_visible),
        volver = esc(&t.t("volver")),
    )
}

/// The page that answers an address with tracking tags: it reopens the clean address in its
/// place (the history keeps only the clean one). The tagged address never reaches the site.
#[must_use]
pub fn reabre(limpia: &str) -> String {
    let js = serde_json::to_string(limpia).unwrap_or_else(|_| "\"about:blank\"".into());
    // `</` cannot close the script from inside the string.
    let js = js.replace("</", "<\\/");
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><meta name="referrer" content="no-referrer"><meta http-equiv="refresh" content="0;url={u}"><script>location.replace({js});</script></head><body></body></html>"#,
        u = esc(limpia),
    )
}

/// The script every document of a web tab runs first: Global Privacy Control, the hidden
/// channel, the form guard and the measurement of reads for a fingerprint ([`crate::medidas`]),
/// with maximum protection's different values for other companies' scripts. In a frame only the
/// measurement and the protection run, on the frame's own channel. Nothing it defines can be
/// found by the page by name: no global name, the channel to the browser is kept in a closure
/// and the answers come back on it. The measured functions are wrapped (a page that compares
/// them across frames can tell; decision 206 says so).
#[must_use]
pub fn guion_paginas() -> String {
    GUION.to_string()
}

/// The script of a mandate's tab: WebRTC and WebTransport turned off, then the same as every
/// page. It runs in every frame it reaches; a frame made from the page's own code, which the
/// engine may not give this script first, is cleaned when the page reaches into it
/// (`contentWindow`, `contentDocument`). A page that still found a way would use a connection
/// GUARDIANA ZERO does not see, and the receipt says so (review of 10 Oct 2026, grave 4).
#[must_use]
pub fn guion_mandato() -> String {
    format!("{APAGA_CONEXIONES}\n{GUION}")
}

const APAGA_CONEXIONES: &str = r#"(() => {
  'use strict';
  const NOMBRES = ['RTCPeerConnection', 'webkitRTCPeerConnection', 'RTCDataChannel', 'RTCSessionDescription', 'RTCIceCandidate', 'RTCRtpSender', 'RTCRtpReceiver', 'RTCRtpTransceiver', 'WebTransport'];
  const apaga = (w) => {
    for (const k of NOMBRES) {
      try { delete w[k]; } catch (_) {}
      try { Object.defineProperty(w, k, { value: undefined, configurable: false, writable: false, enumerable: false }); } catch (_) {}
    }
  };
  apaga(window);
  for (const [C, k] of [[window.HTMLIFrameElement, 'contentWindow'], [window.HTMLIFrameElement, 'contentDocument'], [window.HTMLFrameElement, 'contentWindow'], [window.HTMLFrameElement, 'contentDocument'], [window.HTMLObjectElement, 'contentWindow'], [window.HTMLObjectElement, 'contentDocument']]) {
    if (!C) continue;
    const d = Object.getOwnPropertyDescriptor(C.prototype, k);
    if (!d || !d.get) continue;
    const leer = d.get;
    try {
      Object.defineProperty(C.prototype, k, {
        configurable: false, enumerable: d.enumerable,
        get: function () {
          const r = leer.call(this);
          try { const w = k === 'contentWindow' ? r : (r && r.defaultView); if (w) apaga(w); } catch (_) {}
          return r;
        },
      });
    } catch (_) {}
  }
})();"#;

const GUION: &str = r#"(() => {
  'use strict';
  // Global Privacy Control: the person does not want their data sold or shared.
  try {
    Object.defineProperty(Navigator.prototype, 'globalPrivacyControl', { get: () => true, configurable: true, enumerable: true });
  } catch (_) {}
  if (location.origin === 'https://zero.guardiana') return;
  // Frames get this script too (WebView2 runs it in «all future top level document and child
  // frame navigations»): there it only measures, and protects, on the frame's own channel.
  const marco = window.top !== window;
  const c = window.chrome;
  const host = c && c.webview;
  if (!host || typeof host.postMessage !== 'function') return;
  const manda = host.postMessage.bind(host);
  const escucha = host.addEventListener.bind(host);
  // Web pages do not need the channel to the browser: hidden for good, so a page can neither
  // talk to the browser nor use it to tell this browser apart.
  try { Object.defineProperty(c, 'webview', { value: undefined, configurable: false, writable: false }); } catch (_) {}
  const ficha = new Uint32Array(4);
  crypto.getRandomValues(ficha);
  const yo = Array.from(ficha, (x) => x.toString(16)).join('');
  // What maximum protection gives other companies' scripts on this page (decision 206): `null`
  // until the browser answers, `false` when it is off. Decided once per document, so a page
  // never sees a value change under it.
  let proteccion = null;
  const pon = (h) => {
    if (proteccion !== null) return;
    proteccion = h && typeof h === 'object' && typeof h.semilla === 'number' && typeof h.sitio === 'string' && Array.isArray(h.propios)
      ? { semilla: h.semilla >>> 0, sitio: h.sitio, propios: h.propios.filter((x) => typeof x === 'string').slice(0, 400) }
      : false;
  };
  // Measured (decision 206): reads that serve to recognise this computer without cookies (a
  // canvas with text read back, the graphics card's names, audio rendered off screen, many
  // fonts measured), and the first time the person types in a field. The browser gets the kind
  // of read and the web addresses on the stack of whoever asked, never what was read or typed.
  // Each original is called as it was and returns what it returned, keeping its name, length and
  // native text; only with maximum protection on, and only for a script of another company (by
  // the stack; never the site itself, its same owner, or an unknown caller), the canvas and the
  // audio come back with a tiny change fixed for this site and run, and the graphics card's
  // names come back generic.
  try {
    const R = Reflect.apply;
    const Px = Proxy;
    const Err = Error;
    const U = URL;
    const mul = Math.imul;
    const ahora = Date.now;
    const doc = window.document;
    const tarde = window.setTimeout.bind(window);
    const originales = new WeakMap();
    const aTexto = Function.prototype.toString;
    const finge = new Px(aTexto, { apply: (f, ese, a) => R(f, (typeof ese === 'function' && originales.get(ese)) || ese, a) });
    originales.set(finge, aTexto);
    Object.defineProperty(Function.prototype, 'toString', { value: finge });
    // `antes` may hand back how to make the call instead (only ever with maximum protection);
    // if that fails in any way, the original is called as it was.
    const envuelve = (o, k, antes) => {
      const d = o && Object.getOwnPropertyDescriptor(o, k);
      if (!d || typeof d.value !== 'function') return;
      const p = new Px(d.value, { apply: function trampa(f, ese, a) {
        let x;
        try { x = antes(ese, a, trampa); } catch (_) {}
        if (typeof x === 'function') { try { return x(f, ese, a); } catch (_) {} }
        return R(f, ese, a);
      } });
      originales.set(p, d.value);
      Object.defineProperty(o, k, { value: p });
    };
    const envuelveLector = (o, k, despues) => {
      const d = o && Object.getOwnPropertyDescriptor(o, k);
      if (!d || typeof d.get !== 'function') return;
      const g = new Px(d.get, { apply: (f, ese, a) => { const r = R(f, ese, a); try { despues(ese, r); } catch (_) {} return r; } });
      originales.set(g, d.get);
      Object.defineProperty(o, k, { get: g });
    };
    const lector = (o, k) => { const d = o && Object.getOwnPropertyDescriptor(o, k); return d && d.get; };
    const escritor = (o, k) => { const d = o && Object.getOwnPropertyDescriptor(o, k); return d && d.set; };
    // The web addresses on the stack, closest first, without the trap's own frames (V8's
    // captureStackTrace starts below `desde`), their line numbers or their queries.
    const pila = (desde) => {
      const out = [];
      try {
        const o = {};
        if (typeof Err.captureStackTrace === 'function') Err.captureStackTrace(o, desde); else o.stack = new Err().stack;
        const s = String(o.stack || '');
        const re = /https?:\/\/[^\s()'"<>]+/g;
        let m;
        while ((m = re.exec(s)) && out.length < 4) {
          const u = m[0].replace(/:\d+(?::\d+)?$/, '').split(/[?#]/)[0].slice(0, 500);
          if (!out.includes(u)) out.push(u);
        }
      } catch (_) {}
      return out;
    };
    // Whether the caller is another company's script: decided once per name and document, and
    // «no» while the browser has not answered yet (so it stays the same all along).
    const decididos = new Map();
    const ajeno = (u) => {
      if (proteccion === false || !u[0]) return false;
      let h = '';
      try { h = new U(u[0]).hostname.toLowerCase(); } catch (_) { return false; }
      if (decididos.has(h)) return decididos.get(h);
      const bajo = (s) => h === s || h.endsWith('.' + s);
      const si = !!proteccion && !bajo(proteccion.sitio) && !proteccion.propios.some(bajo);
      decididos.set(h, si);
      return si;
    };
    // Told in batches, each kind, script and outcome once per page, at most 100 per page; the
    // stack is asked for on the first ten reads of a kind and then at most once a second.
    const cola = [];
    const vistos = new Set();
    const ultimo = {};
    const veces = {};
    let dichos = 0;
    let enCamino = false;
    const apunta = (t, desde, r, dada) => {
      if (dichos >= 100) return;
      const n = (veces[t] = (veces[t] || 0) + 1);
      const ya = ahora();
      if (!r && n > 10 && ya - (ultimo[t] || 0) < 1000) return;
      ultimo[t] = ya;
      const u = dada || pila(desde);
      const k = t + ' ' + (u[0] || '') + (r ? ' r' : '');
      if (vistos.has(k)) return;
      vistos.add(k);
      dichos++;
      cola.push(r ? { t, u, r: true } : { t, u });
      if (enCamino) return;
      enCamino = true;
      tarde(() => { enCamino = false; const lista = cola.splice(0, cola.length); if (lista.length) manda({ tipo: 'zg_huella', zg: yo, lista }); }, 500);
    };
    // A number mixed with the site's seed: the same input, the same output, all along.
    const mezcla = (a, b) => {
      let x = mul((a ^ b) >>> 0, 0x9E3779B1);
      x ^= x >>> 15; x = mul(x, 0x85EBCA77); x ^= x >>> 13; x = mul(x, 0xC2B2AE35);
      return (x ^ (x >>> 16)) >>> 0;
    };
    const C2D = window.CanvasRenderingContext2D && CanvasRenderingContext2D.prototype;
    const HC = window.HTMLCanvasElement && HTMLCanvasElement.prototype;
    const ID = window.ImageData && ImageData.prototype;
    const lienzoDe = lector(C2D, 'canvas');
    const anchoDe = lector(HC, 'width');
    const altoDe = lector(HC, 'height');
    const anchoPon = escritor(HC, 'width');
    const altoPon = escritor(HC, 'height');
    const fuenteDe = lector(C2D, 'font');
    const datosDe = lector(ID, 'data');
    const anchoImg = lector(ID, 'width');
    const altoImg = lector(ID, 'height');
    const crea = window.Document && Document.prototype.createElement;
    const pideCtx = HC && HC.getContext;
    const dibuja = C2D && C2D.drawImage;
    const leeDatos = C2D && C2D.getImageData;
    const ponDatos = C2D && C2D.putImageData;
    // Canvas: only one with text of ten different characters or more drawn on it, read back
    // whole or in a piece of at least 16 × 16 (after Englehardt and Narayanan, «Online
    // tracking: a 1-million-site measurement», 2016). Charts, photos and games rarely qualify.
    const conTexto = new WeakMap();
    const escrito = (ctx, a) => {
      if (typeof a[0] !== 'string') return;
      const cv = R(lienzoDe, ctx, []);
      let e = conTexto.get(cv);
      if (e === true) return;
      if (!e) { e = new Set(); conTexto.set(cv, e); }
      for (const ch of a[0].slice(0, 64)) e.add(ch);
      if (e.size >= 10) conTexto.set(cv, true);
    };
    const cuenta = (cv, w, h) => conTexto.get(cv) === true && w >= 16 && h >= 16;
    const numero = (x) => (typeof x === 'number' ? Math.abs(x) : 0);
    // Like Brave's «farbling»: about one opaque pixel in 32 gets the lowest bit of one colour
    // flipped, chosen by the site's seed and the pixel's place in the canvas.
    const ensucia = (d, x0, y0, ancho, alto, w) => {
      let n = 0;
      const s = proteccion.semilla;
      for (let y = 0; y < alto; y++) {
        for (let x = 0; x < ancho; x++) {
          const k = (y * ancho + x) * 4;
          if (d[k + 3] !== 255) continue;
          const z = mezcla(s, (y0 + y) * w + (x0 + x));
          if ((z & 31) !== 0) continue;
          d[k + ((z >>> 5) % 3)] ^= 1;
          n++;
        }
      }
      return n;
    };
    // A copy of the canvas with the change, to read from instead (the page's canvas is never
    // touched); `null` when nothing would change or it is too big.
    const copia = (cv) => {
      const w = R(anchoDe, cv, []);
      const h = R(altoDe, cv, []);
      if (w * h > 4000000) return null;
      const t = R(crea, doc, ['canvas']);
      R(anchoPon, t, [w]);
      R(altoPon, t, [h]);
      const x = R(pideCtx, t, ['2d']);
      R(dibuja, x, [cv, 0, 0]);
      const img = R(leeDatos, x, [0, 0, w, h]);
      if (!ensucia(R(datosDe, img, []), 0, 0, w, h, w)) return null;
      R(ponDatos, x, [img, 0, 0]);
      return t;
    };
    if (C2D && lienzoDe) {
      envuelve(C2D, 'fillText', escrito);
      envuelve(C2D, 'strokeText', escrito);
      envuelve(C2D, 'getImageData', (ctx, a, desde) => {
        const cv = R(lienzoDe, ctx, []);
        if (!cuenta(cv, numero(a[2]), numero(a[3]))) return undefined;
        if (proteccion === false) { apunta('canvas', desde, false); return undefined; }
        const u = pila(desde);
        if (!ajeno(u) || !datosDe || !anchoImg || !altoImg) { apunta('canvas', desde, false, u); return undefined; }
        return (f, ese, args) => {
          const img = R(f, ese, args);
          let n = 0;
          try {
            const sx = typeof args[0] === 'number' ? args[0] : 0;
            const sy = typeof args[1] === 'number' ? args[1] : 0;
            const x0 = Math.floor(args[2] < 0 ? sx + args[2] : sx);
            const y0 = Math.floor(args[3] < 0 ? sy + args[3] : sy);
            n = ensucia(R(datosDe, img, []), x0, y0, R(anchoImg, img, []), R(altoImg, img, []), R(anchoDe, cv, []));
          } catch (_) {}
          apunta('canvas', desde, n > 0, u);
          return img;
        };
      });
    }
    if (HC && anchoDe && altoDe) {
      for (const k of ['toDataURL', 'toBlob']) {
        envuelve(HC, k, (cv, a, desde) => {
          if (!cuenta(cv, R(anchoDe, cv, []), R(altoDe, cv, []))) return undefined;
          if (proteccion === false) { apunta('canvas', desde, false); return undefined; }
          const u = pila(desde);
          let otra = null;
          if (ajeno(u) && crea && anchoPon && altoPon && pideCtx && dibuja && leeDatos && ponDatos && datosDe) {
            try { otra = copia(cv); } catch (_) { otra = null; }
          }
          apunta('canvas', desde, !!otra, u);
          return otra ? (f, ese, args) => R(f, otra, args) : undefined;
        });
      }
    }
    // WebGL: the graphics card's own names (UNMASKED_VENDOR_WEBGL, UNMASKED_RENDERER_WEBGL).
    for (const G of [window.WebGLRenderingContext, window.WebGL2RenderingContext]) {
      if (!G) continue;
      envuelve(G.prototype, 'getParameter', (ctx, a, desde) => {
        const p = a[0];
        if (p !== 0x9245 && p !== 0x9246) return undefined;
        if (proteccion === false) { apunta('webgl', desde, false); return undefined; }
        const u = pila(desde);
        if (!ajeno(u)) { apunta('webgl', desde, false, u); return undefined; }
        return (f, ese, args) => {
          const r = R(f, ese, args);
          const otro = typeof r === 'string';
          apunta('webgl', desde, otro, u);
          return otro ? (p === 0x9245 ? 'Google Inc.' : 'ANGLE (Generic GPU)') : r;
        };
      });
    }
    // Audio rendered off screen, never heard: with maximum protection, about one sample in 16
    // of another company's render moves by a hundred-thousandth, fixed for this site and run.
    const OAC = window.OfflineAudioContext;
    const AB = window.AudioBuffer && AudioBuffer.prototype;
    const canalDe = AB && AB.getChannelData;
    const canalesDe = lector(AB, 'numberOfChannels');
    const alLlegar = Promise.prototype.then;
    const ajenos = new WeakSet();
    const tocados = new WeakMap();
    const ensuciaAudio = (buf) => {
      if (tocados.has(buf)) return tocados.get(buf);
      let n = 0;
      const canales = R(canalesDe, buf, []);
      for (let ch = 0; ch < canales; ch++) {
        const d = R(canalDe, buf, [ch]);
        const s = proteccion.semilla ^ mul(ch + 1, 0x27D4EB2D);
        for (let i = 0; i < d.length; i++) {
          if (d[i] === 0) continue;
          const z = mezcla(s, i);
          if ((z & 15) !== 0) continue;
          d[i] *= (z & 16) ? 1.00001 : 0.99999;
          n++;
        }
      }
      tocados.set(buf, n);
      return n;
    };
    if (OAC) {
      envuelve(OAC.prototype, 'startRendering', (ctx, a, desde) => {
        if (proteccion === false) { apunta('audio', desde, false); return undefined; }
        const u = pila(desde);
        if (!ajeno(u) || !canalDe || !canalesDe) { apunta('audio', desde, false, u); return undefined; }
        ajenos.add(ctx);
        return (f, ese, args) => R(alLlegar, R(f, ese, args), [(buf) => {
          let n = 0;
          try { n = ensuciaAudio(buf); } catch (_) {}
          apunta('audio', desde, n > 0, u);
          return buf;
        }]);
      });
      // The same render handed over by its «complete» event.
      const destinoDe = lector(window.Event && Event.prototype, 'target');
      const OACE = window.OfflineAudioCompletionEvent && OfflineAudioCompletionEvent.prototype;
      if (destinoDe) envuelveLector(OACE, 'renderedBuffer', (ev, buf) => { if (proteccion && buf && ajenos.has(R(destinoDe, ev, []))) ensuciaAudio(buf); });
    }
    // Fonts: FUENTES = 20 different families measured on canvases of this page (measured only).
    const FUENTES = 20;
    const familias = new Set();
    let ultimaFuente = null;
    if (C2D && fuenteDe) {
      envuelve(C2D, 'measureText', (ctx, a, desde) => {
        if (familias.size >= FUENTES) return;
        const f = R(fuenteDe, ctx, []);
        if (f === ultimaFuente) return;
        ultimaFuente = f;
        familias.add(String(f).replace(/^.*?\d[\d.]*(?:px|pt|pc|em|rem|ex|ch|%|in|cm|mm|vw|vh)(?:\s*\/\s*\S+)?\s+/i, '').toLowerCase());
        if (familias.size >= FUENTES) apunta('fuentes', desde, false);
      });
    }
    // Typing: the first time the person types in a field of this page, and nothing of it.
    let escribio = false;
    window.addEventListener('input', (e) => {
      if (escribio || !e.isTrusted || typeof e.inputType !== 'string') return;
      escribio = true;
      manda({ tipo: 'zg_escribe', zg: yo });
    }, true);
  } catch (_) {}
  if (marco) {
    escucha('message', (e) => { const m = e && e.data; if (m && m.zg === yo && m.huella !== undefined) pon(m.huella); });
    manda({ tipo: 'zg_marco_pide', zg: yo });
    return;
  }
  // The form guard. Built-ins are taken now, before the page can replace them.
  const P = HTMLFormElement.prototype;
  const elementos = Object.getOwnPropertyDescriptor(P, 'elements').get;
  const accionDe = Object.getOwnPropertyDescriptor(P, 'action').get;
  const enviarNativo = P.submit;
  const pedirNativo = P.requestSubmit;
  const de = (o, k) => { const d = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(o), k); return d && d.get ? d.get.call(o) : o[k]; };
  const esperas = new Map();
  const pasan = new WeakSet();
  let n = 0;
  const valores = (form) => {
    const out = [];
    for (const el of Array.from(elementos.call(form) || [])) {
      const tipo = String(de(el, 'type') || '').toLowerCase();
      if (['password', 'file', 'submit', 'button', 'reset', 'image'].includes(tipo)) continue;
      if ((tipo === 'checkbox' || tipo === 'radio') && !de(el, 'checked')) continue;
      const v = de(el, 'value');
      const t = typeof v === 'string' ? v.trim() : '';
      if (t) out.push(t.slice(0, 300));
      if (out.length >= 60) break;
    }
    return out;
  };
  escucha('message', (e) => {
    const m = e && e.data;
    if (!m || m.zg !== yo) return;
    if (m.huella !== undefined) pon(m.huella);
    if (m.cookies !== undefined) { if (m.cookies === true) avisosDeCookies(); return; }
    const f = esperas.get(m.id);
    if (f) { esperas.delete(m.id); f(!!m.enviar); }
  });
  // Ask before a form carries personal data out; send it exactly as it was asked about.
  const guarda = (form, boton, nativo) => {
    const lista = valores(form);
    const accion = String(accionDe.call(form) || location.href);
    if (!lista.length) return false;
    const id = ++n;
    const antes = JSON.stringify([accion, lista]);
    esperas.set(id, (enviar) => {
      if (!enviar) return;
      if (JSON.stringify([String(accionDe.call(form) || location.href), valores(form)]) !== antes) {
        // The page changed the form while you were deciding: it is asked about again.
        guarda(form, boton, nativo);
        return;
      }
      pasan.add(form);
      try { nativo ? enviarNativo.call(form) : (boton ? pedirNativo.call(form, boton) : pedirNativo.call(form)); }
      catch (_) { enviarNativo.call(form); }
      pasan.delete(form);
    });
    manda({ tipo: 'zg_formulario', zg: yo, id, accion, valores: lista });
    return true;
  };
  // Cookie notices of the consent managers most sites use: «reject all» is pressed when the
  // notice has it, and the notice is hidden when it has not (then nothing was accepted either).
  // Only these known managers, by their own markup: a page's other content is never touched.
  // The browser says whether to (the setting, and only with the subscription on).
  const GESTORES = [
    ['OneTrust', '#onetrust-banner-sdk, #onetrust-pc-sdk, .onetrust-pc-dark-filter', '#onetrust-reject-all-handler, .ot-pc-refuse-all-handler'],
    ['Cookiebot', '#CybotCookiebotDialog', '#CybotCookiebotDialogBodyButtonDecline, #CybotCookiebotDialogBodyLevelButtonLevelOptinDeclineAll'],
    ['Didomi', '#didomi-host .didomi-popup-container, #didomi-host #didomi-notice', '#didomi-notice-disagree-button'],
    ['Usercentrics', '#usercentrics-root, #usercentrics-cmp-ui', (r) => (r.shadowRoot || r).querySelector('[data-testid="uc-deny-all-button"], #deny')],
    ['Quantcast', '.qc-cmp2-container', null],
    ['Sourcepoint', 'div[id^="sp_message_container"]', null],
    ['TrustArc', '#truste-consent-track, .truste_box_overlay, .truste_overlay', '#truste-consent-required'],
    ['CookieYes', '.cky-consent-container, .cky-overlay, .cky-modal', '.cky-btn-reject'],
    ['Osano', '.osano-cm-window', '.osano-cm-denyAll'],
    ['Complianz', '#cmplz-cookiebanner-container, .cmplz-cookiebanner', '.cmplz-btn.cmplz-deny'],
    ['iubenda', '#iubenda-cs-banner', '.iubenda-cs-reject-btn'],
    ['Klaro', '.klaro .cookie-notice, .klaro .cookie-modal', '.klaro .cm-btn-decline'],
    ['Axeptio', '#axeptio_overlay', '#axeptio_btn_dismiss'],
    ['Google', '.fc-consent-root', '.fc-cta-do-not-consent'],
    ['consentmanager', '#cmpbox, #cmpbox2', '.cmpboxbtnno'],
    ['Borlabs', '#BorlabsCookieBox', 'a[data-cookie-refuse]'],
    ['CookieFirst', '.cookiefirst-root', '[data-cookiefirst-action="reject"]'],
    ['CookieScript', '#cookiescript_injected', '#cookiescript_reject'],
    ['Cookie Notice', '#cookie-notice', '#cn-refuse-cookie'],
    ['Moove', '#moove_gdpr_cookie_info_bar', '.moove-gdpr-infobar-reject-btn'],
    ['tarteaucitron', '#tarteaucitronRoot', '#tarteaucitronAllDenied2'],
  ];
  const visible = (el) => { try { return !!(el && el.getClientRects().length && getComputedStyle(el).visibility !== 'hidden'); } catch (_) { return false; } };
  let cookiesEnMarcha = false;
  const avisosDeCookies = () => {
    if (cookiesEnMarcha) return;
    cookiesEnMarcha = true;
    const hechos = new Set();
    const escondidos = [];
    const estilo = document.createElement('style');
    const esconde = (sel) => {
      escondidos.push(sel);
      estilo.textContent = escondidos.join(',') + '{display:none!important}';
      if (!estilo.isConnected) (document.head || document.documentElement).appendChild(estilo);
      // A hidden notice must not leave the page frozen under it.
      for (const el of [document.documentElement, document.body]) {
        try { if (el && getComputedStyle(el).overflow === 'hidden') el.style.setProperty('overflow', 'auto', 'important'); } catch (_) {}
      }
      document.documentElement.classList.remove('sp-message-open');
    };
    const repasa = () => {
      for (const [gestor, caja, boton] of GESTORES) {
        if (hechos.has(gestor)) continue;
        let c = null;
        try { c = Array.from(document.querySelectorAll(caja)).find(visible) || null; } catch (_) {}
        if (!c) continue;
        let b = null;
        try { b = typeof boton === 'function' ? boton(c) : (boton ? document.querySelector(boton) : null); } catch (_) {}
        if (b && visible(b)) {
          hechos.add(gestor);
          try { b.click(); } catch (_) {}
          manda({ tipo: 'zg_cookies', zg: yo, gestor, accion: 'rechazado' });
        } else if (!boton || Date.now() - inicio > 1500) {
          // No «reject» button (or it never showed up): hidden, and nothing accepted.
          hechos.add(gestor);
          esconde(caja);
          manda({ tipo: 'zg_cookies', zg: yo, gestor, accion: 'escondido' });
        }
      }
    };
    const inicio = Date.now();
    let pendiente = false;
    const mira = new MutationObserver(() => {
      if (pendiente) return;
      pendiente = true;
      setTimeout(() => { pendiente = false; repasa(); }, 250);
    });
    const empieza = () => {
      repasa();
      try { mira.observe(document.documentElement, { childList: true, subtree: true }); } catch (_) {}
      setTimeout(repasa, 1600);
      // Notices show up in the first seconds of a page; after that nobody is watched.
      setTimeout(() => { mira.disconnect(); repasa(); }, 20000);
    };
    if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', empieza, { once: true });
    else empieza();
  };
  manda({ tipo: 'zg_cookies_pide', zg: yo });
  // On window and in the capture phase, registered before any of the page's own listeners.
  window.addEventListener('submit', (e) => {
    const form = e.target;
    if (!(form instanceof HTMLFormElement) || pasan.has(form)) return;
    const boton = e.submitter && de(e.submitter, 'form') === form ? e.submitter : null;
    if (guarda(form, boton, false)) { e.preventDefault(); e.stopImmediatePropagation(); }
  }, true);
  // `form.submit()` fires no event: it goes through the same question.
  try {
    Object.defineProperty(P, 'submit', {
      value: function submit() { if (pasan.has(this) || !guarda(this, null, true)) enviarNativo.call(this); },
      configurable: false, writable: false,
    });
  } catch (_) {}
})();"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::textos::Idioma;

    #[test]
    fn the_cut_page_escapes_what_it_shows() {
        let t = Textos::de(Idioma::Es);
        let p = pagina_cortada(
            &t,
            "Cortaste <b>x</b>.",
            "https://x.example/?a=<script>",
            "",
        );
        assert!(p.contains("Cortaste &lt;b&gt;x&lt;/b&gt;."));
        assert!(p.contains("?a=&lt;script&gt;"));
        assert!(p.contains("lang=\"es\""));
        assert!(!p.contains("<script>"));
    }

    #[test]
    fn the_cut_page_follows_day_or_night() {
        let t = Textos::de(Idioma::Es);
        let sistema = pagina_cortada(&t, "x", "https://x.example/", "");
        assert!(sistema.contains("prefers-color-scheme:dark"));
        let noche = pagina_cortada(&t, "x", "https://x.example/", "noche");
        assert!(noche.contains("color-scheme:dark") && !noche.contains("prefers-color-scheme"));
        let dia = pagina_cortada(&t, "x", "https://x.example/", "dia");
        assert!(dia.contains("color-scheme:light;") && !dia.contains("#0F1528"));
    }

    #[test]
    fn the_page_script_leaves_nothing_a_page_can_find() {
        let g = guion_paginas();
        assert!(g.contains("globalPrivacyControl"));
        assert!(!g.contains("window.__"));
        assert!(g.contains("getRandomValues"));
    }

    #[test]
    fn the_page_script_measures_reads_for_a_fingerprint_without_changing_them() {
        let g = guion_paginas();
        // Each kind the session knows, by the key it posts.
        for t in [
            crate::medidas::Tecnica::Canvas,
            crate::medidas::Tecnica::Webgl,
            crate::medidas::Tecnica::Audio,
            crate::medidas::Tecnica::Fuentes,
        ] {
            assert!(g.contains(&format!("'{}'", t.clave())), "{t:?}");
        }
        assert!(g.contains("tipo: 'zg_huella'") && g.contains("tipo: 'zg_escribe'"));
        // The originals are called with what they were given and their result is returned,
        // unless the browser said maximum protection is on (and then only for another company).
        assert!(g.contains("return R(f, ese, a);"));
        assert!(g.contains("if (proteccion === false || !u[0]) return false;"));
        // Frames measure too, and leave before the form guard and the cookie notices, which are
        // the page's alone; they ask on their own channel.
        let marcos = g.find("if (marco) {").unwrap_or(usize::MAX);
        assert!(g.find("zg_huella").unwrap_or(usize::MAX) < marcos);
        assert!(marcos < g.find("HTMLFormElement.prototype").unwrap_or(0));
        assert!(marcos < g.find("zg_cookies_pide").unwrap_or(0));
        assert!(g.contains("tipo: 'zg_marco_pide'"));
        // Navigator's properties (plugins, hardwareConcurrency, deviceMemory) are not touched.
        for no in ["hardwareConcurrency", "deviceMemory", "'plugins'"] {
            assert!(!g.contains(no), "{no}");
        }
    }

    #[test]
    fn a_mandate_s_tab_turns_webrtc_and_webtransport_off_first() {
        let g = guion_mandato();
        let apaga = g.find("RTCPeerConnection").unwrap_or(usize::MAX);
        assert!(g.contains("'WebTransport'"));
        assert!(apaga < g.find("globalPrivacyControl").unwrap_or(0));
        // Every other tab keeps them.
        assert!(!guion_paginas().contains("RTCPeerConnection"));
        assert!(!guion_paginas().contains("WebTransport"));
    }

    #[test]
    fn the_clean_reopening_page_cannot_be_broken_out_of() {
        let p = reabre("https://x.example/a?b=</script><script>alert(1)</script>");
        assert!(!p.contains("</script><script>alert"));
        assert!(p.contains("location.replace("));
    }
}
