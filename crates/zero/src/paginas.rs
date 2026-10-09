//! What GUARDIANA ZERO puts inside web pages, all of it here so it can be read in one place:
//!
//! - the page shown instead of an address that was cut (a mandate's limit, a site the person
//!   cut, the decoy leaving), written in the person's language with no outside resource;
//! - the script every page of a web tab gets before its own code runs: the Global Privacy
//!   Control signal (`navigator.globalPrivacyControl`, sent as `Sec-GPC: 1` by the shell), and
//!   the form guard that asks before personal data leaves in a form.
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
/// channel and the form guard. Nothing it defines can be found by the page: no global name, the
/// channel to the browser is kept in a closure and the answers come back on it.
#[must_use]
pub fn guion_paginas() -> String {
    GUION.to_string()
}

const GUION: &str = r"(() => {
  'use strict';
  // Global Privacy Control: the person does not want their data sold or shared.
  try {
    Object.defineProperty(Navigator.prototype, 'globalPrivacyControl', { get: () => true, configurable: true, enumerable: true });
  } catch (_) {}
  if (window.top !== window || location.origin === 'https://zero.guardiana') return;
  const c = window.chrome;
  const host = c && c.webview;
  if (!host || typeof host.postMessage !== 'function') return;
  const manda = host.postMessage.bind(host);
  const escucha = host.addEventListener.bind(host);
  // Web pages do not need the channel to the browser: hidden for good, so a page can neither
  // talk to the browser nor use it to tell this browser apart.
  try { Object.defineProperty(c, 'webview', { value: undefined, configurable: false, writable: false }); } catch (_) {}
  // The form guard. Built-ins are taken now, before the page can replace them.
  const P = HTMLFormElement.prototype;
  const elementos = Object.getOwnPropertyDescriptor(P, 'elements').get;
  const accionDe = Object.getOwnPropertyDescriptor(P, 'action').get;
  const enviarNativo = P.submit;
  const pedirNativo = P.requestSubmit;
  const de = (o, k) => { const d = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(o), k); return d && d.get ? d.get.call(o) : o[k]; };
  const ficha = new Uint32Array(4);
  crypto.getRandomValues(ficha);
  const yo = Array.from(ficha, (x) => x.toString(16)).join('');
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
})();";

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
    fn the_clean_reopening_page_cannot_be_broken_out_of() {
        let p = reabre("https://x.example/a?b=</script><script>alert(1)</script>");
        assert!(!p.contains("</script><script>alert"));
        assert!(p.contains("location.replace("));
    }
}
