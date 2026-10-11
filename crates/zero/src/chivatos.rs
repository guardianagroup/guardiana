//! «Los chivatos»: what a page's tracking pixels tried to tell an advertising network about the
//! person, read from the request itself: which network, which event (a purchase, a sign-up, a
//! page seen), the amount and currency when the request carries them, and whether it carries an
//! email or a phone in the hashed form those networks use, and if that hash is the person's own
//! (one of the forms of a value they marked, see `tinta`).
//!
//! Only what the request says, never a guess (review of 10 Oct 2026, improvement A):
//! - The event is the network's own word, normalised to a few kinds, and its original name is
//!   kept beside it. An event this module does not know is «another event», with its name.
//! - «Your email» only when the hash matches a value the person marked; any other hash is «an
//!   email», whoever's it is. Nothing here ever says that an email was *not* sent.
//! - An amount is shown as it travels, with the currency it names.
//! - A request to the page's own server in a network's format (a server-side tag) is said to be
//!   that: the web sending it to itself, which may forward it.
//!
//! The parameters follow each network's public documentation and the requests their own
//! scripts make; they change without notice, and what this does not recognise is left unsaid.

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::dominio::{normaliza_host, sitio};
use crate::tinta::Forma;

/// Bodies are read up to this size: a pixel's data is small.
const MAX_CUERPO: usize = 256 * 1024;
/// Text kept from what a request names (an event, an amount, an id), at most.
const MAX_TEXTO: usize = 60;

/// The advertising and analytics networks whose pixels are read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Red {
    /// Meta (Facebook, Instagram): `facebook.com/tr`.
    Meta,
    /// Google Ads: conversions and remarketing lists.
    GoogleAds,
    /// Google Analytics 4: `…/g/collect`.
    GoogleAnalytics,
    /// TikTok: `analytics.tiktok.com/api/v2/pixel`.
    Tiktok,
    /// Pinterest: `ct.pinterest.com/v3/`.
    Pinterest,
    /// Snap: `tr.snapchat.com`.
    Snap,
    /// LinkedIn: `px.ads.linkedin.com`.
    Linkedin,
    /// X (Twitter): `…/i/adsct`.
    X,
    /// Criteo: `…criteo.com/event`.
    Criteo,
    /// Microsoft Advertising (UET): `bat.bing.com/action`.
    Microsoft,
}

impl Red {
    /// The key of its name in the texts (`chivato_red_<clave>`).
    #[must_use]
    pub const fn clave(self) -> &'static str {
        match self {
            Self::Meta => "meta",
            Self::GoogleAds => "google_ads",
            Self::GoogleAnalytics => "google_analytics",
            Self::Tiktok => "tiktok",
            Self::Pinterest => "pinterest",
            Self::Snap => "snap",
            Self::Linkedin => "linkedin",
            Self::X => "x",
            Self::Criteo => "criteo",
            Self::Microsoft => "microsoft",
        }
    }

    /// The company behind it: Google Ads and Google Analytics are one company.
    #[must_use]
    pub const fn empresa(self) -> &'static str {
        match self {
            Self::Meta => "Meta",
            Self::GoogleAds | Self::GoogleAnalytics => "Google",
            Self::Tiktok => "TikTok",
            Self::Pinterest => "Pinterest",
            Self::Snap => "Snap",
            // LinkedIn and Microsoft Advertising are one company, as the lists of owners say.
            Self::Linkedin | Self::Microsoft => "Microsoft",
            Self::X => "X",
            Self::Criteo => "Criteo",
        }
    }
}

/// What the page said happened, in a few kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Evento {
    /// A purchase.
    Compra,
    /// Paying started (checkout, payment details).
    PagoIniciado,
    /// Something added to the cart.
    Carrito,
    /// A sign-up.
    Registro,
    /// Contact details left (a «lead»).
    ClientePotencial,
    /// A search.
    Busqueda,
    /// A product or a piece of content looked at.
    VerProducto,
    /// A page seen.
    VerPagina,
    /// A conversion: whatever that web counts as one (the request does not say what).
    Conversion,
    /// Another event, by its own name.
    Otro,
}

impl Evento {
    /// The key in the texts (`chivato_ev_<clave>`).
    #[must_use]
    pub const fn clave(self) -> &'static str {
        match self {
            Self::Compra => "compra",
            Self::PagoIniciado => "pago_iniciado",
            Self::Carrito => "carrito",
            Self::Registro => "registro",
            Self::ClientePotencial => "cliente_potencial",
            Self::Busqueda => "busqueda",
            Self::VerProducto => "ver_producto",
            Self::VerPagina => "ver_pagina",
            Self::Conversion => "conversion",
            Self::Otro => "otro",
        }
    }

    /// How much it says about the person: a purchase says more than a page seen.
    #[must_use]
    pub const fn peso(self) -> u8 {
        match self {
            Self::Compra => 9,
            Self::PagoIniciado => 8,
            Self::Registro => 7,
            Self::ClientePotencial => 6,
            Self::Carrito => 5,
            Self::Conversion => 4,
            Self::Busqueda => 3,
            Self::VerProducto => 2,
            Self::Otro => 1,
            Self::VerPagina => 0,
        }
    }
}

/// An email or a phone a pixel carried.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dato {
    /// It is one of the values the person marked (as written or in one of its hashed forms).
    pub tuyo: bool,
    /// It travelled hashed (SHA-256, MD5…), not as written.
    pub cifrado: bool,
}

/// One pixel request, read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chivato {
    /// The network.
    pub red: Red,
    /// The event, normalised.
    pub evento: Evento,
    /// The event's name as the request carried it (`Purchase`, `begin_checkout`).
    pub nombre: String,
    /// The amount, as it travelled (`89900`, `89.90`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub importe: Option<String>,
    /// Its currency (three letters).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moneda: Option<String>,
    /// An email it carried.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correo: Option<Dato>,
    /// A phone it carried.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub telefono: Option<Dato>,
    /// The event's own id (or the order's), to tell one event sent twice from two events. Kept
    /// in memory only, never shown.
    #[serde(skip)]
    pub id: Option<String>,
    /// The site it went to when that is not the network's own (a server-side tag on the web's
    /// own name): that server may forward it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub servidor: Option<String>,
    /// The site of the page it says it was sent from (`dl`, Meta and Google Analytics): a
    /// beacon of the page before carries the page before. In memory only.
    #[serde(skip)]
    pub pagina: Option<String>,
}

/// Whether the address is one of the pixel endpoints this module reads (so the shell reads the
/// body of a request to it, where most of them put their data).
#[must_use]
pub fn interesa(url: &str) -> bool {
    url::Url::parse(url).ok().is_some_and(|u| {
        u.host_str()
            .is_some_and(|h| fuente(&normaliza_host(h), u.path()).is_some())
    })
}

/// What a request to a known pixel tried to tell, from its address and its body. `formas` are
/// the person's marked values in the forms they travel in (`tinta::formas`): a hash that matches
/// one is «yours». `None` when it is no known pixel or says no event.
#[must_use]
pub fn lee(url: &str, metodo: &str, cuerpo: &[u8], formas: &[Forma]) -> Option<Chivato> {
    let u = url::Url::parse(url).ok()?;
    let host = normaliza_host(u.host_str()?);
    lee_en(&u, &host, metodo, cuerpo, formas)
}

/// [`lee`] for an address the caller already read, with its host normalised: the browser reads
/// each request's address once. Most requests are no pixel and leave at the first check, without
/// taking any memory.
#[must_use]
pub fn lee_en(
    u: &url::Url,
    host: &str,
    metodo: &str,
    cuerpo: &[u8],
    formas: &[Forma],
) -> Option<Chivato> {
    let red = fuente(host, u.path())?;
    let consulta: Vec<(String, String)> = u
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    let cuerpo = if metodo.eq_ignore_ascii_case("GET") || metodo.eq_ignore_ascii_case("HEAD") {
        &[][..]
    } else {
        &cuerpo[..cuerpo.len().min(MAX_CUERPO)]
    };
    let l = Lectura {
        host,
        ruta: u.path(),
        consulta,
        cuerpo,
        formas,
    };
    let mut c = match red {
        Red::Meta => l.meta(),
        Red::GoogleAnalytics => l.ga4(),
        Red::GoogleAds => l.google_ads(),
        Red::Tiktok => l.tiktok(),
        Red::Pinterest => l.pinterest(),
        Red::Snap => l.snap(),
        Red::Linkedin => Some(l.linkedin()),
        Red::X => l.x(),
        Red::Criteo => l.criteo(),
        Red::Microsoft => l.uet(),
    }?;
    c.nombre = corto(&c.nombre);
    c.id = c.id.as_deref().map(corto).filter(|s| !s.is_empty());
    Some(c)
}

/// Google's own country names its ad tags also use (`www.google.com.co/pagead/…`).
const GOOGLE_PAISES: &[&str] = &[
    "com", "com.co", "com.mx", "es", "com.br", "com.ar", "cl", "com.pe", "co.uk", "de", "fr", "it",
];

/// The segments of a path, without the empty ones.
fn tramos(ruta: &str) -> impl DoubleEndedIterator<Item = &str> {
    ruta.split('/').filter(|t| !t.is_empty())
}

/// `host` is `sufijo` or a name under it.
fn bajo(host: &str, sufijo: &str) -> bool {
    host.strip_suffix(sufijo)
        .is_some_and(|r| r.is_empty() || r.ends_with('.'))
}

/// The path starts with these segments.
fn empieza(ruta: &str, p: &[&str]) -> bool {
    let mut t = tramos(ruta);
    p.iter().all(|x| t.next() == Some(*x))
}

/// The segment after the first `a` of the path.
fn tras<'a>(ruta: &'a str, a: &str) -> Option<&'a str> {
    let mut t = tramos(ruta);
    while let Some(x) = t.next() {
        if x == a {
            return t.next();
        }
    }
    None
}

/// `www.google.com`, `google.com.co`…: Google's own names in the countries listed.
fn es_google(host: &str) -> bool {
    let h = host.strip_prefix("www.").unwrap_or(host);
    h.strip_prefix("google.")
        .is_some_and(|pais| GOOGLE_PAISES.contains(&pais))
}

/// The network whose endpoint `host` and `ruta` are. It runs for every request: no memory is
/// taken here.
fn fuente(host: &str, ruta: &str) -> Option<Red> {
    if bajo(host, "facebook.com")
        && (empieza(ruta, &["tr"])
            || empieza(ruta, &["privacy_sandbox", "pixel", "register", "trigger"]))
    {
        return Some(Red::Meta);
    }
    let mut fin = tramos(ruta).rev();
    if fin.next() == Some("collect") && fin.next() == Some("g") {
        return Some(Red::GoogleAnalytics);
    }
    let google =
        es_google(host) || bajo(host, "googleadservices.com") || bajo(host, "doubleclick.net");
    if google
        && tras(ruta, "pagead").is_some_and(|t| {
            matches!(
                t,
                "conversion"
                    | "1p-conversion"
                    | "viewthroughconversion"
                    | "1p-user-list"
                    | "form-data"
            )
        })
    {
        return Some(Red::GoogleAds);
    }
    if bajo(host, "tiktok.com")
        && host.starts_with("analytics")
        && empieza(ruta, &["api", "v2", "pixel"])
    {
        return Some(Red::Tiktok);
    }
    if host == "ct.pinterest.com" && empieza(ruta, &["v3"]) {
        return Some(Red::Pinterest);
    }
    if matches!(host, "tr.snapchat.com" | "tr-shadow.snapchat.com") {
        return Some(Red::Snap);
    }
    if host == "px.ads.linkedin.com" {
        return Some(Red::Linkedin);
    }
    if matches!(host, "analytics.twitter.com" | "t.co") && tras(ruta, "i") == Some("adsct") {
        return Some(Red::X);
    }
    if (bajo(host, "criteo.com") || bajo(host, "criteo.net"))
        && tramos(ruta).next_back() == Some("event")
    {
        return Some(Red::Criteo);
    }
    if host == "bat.bing.com" && empieza(ruta, &["action"]) {
        return Some(Red::Microsoft);
    }
    None
}

/// The site of the page a pixel says it was sent from (`dl`).
fn pagina_de(dl: Option<String>) -> Option<String> {
    let u = url::Url::parse(&dl?).ok()?;
    Some(sitio(&normaliza_host(u.host_str()?)))
}

/// The event kind for a network's own name, by its letters only (`AddToCart`, `add_to_cart`
/// and `ADD_CART` are one).
fn evento_de(nombre: &str) -> Evento {
    let n: String = nombre
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase();
    match n.as_str() {
        "purchase" | "completepayment" | "placeanorder" | "trans" | "tracktransaction" => {
            Evento::Compra
        }
        // «checkout» is a purchase only for Pinterest (see `pinterest`); elsewhere, starting to
        // pay.
        "checkout" | "initiatecheckout" | "begincheckout" | "startcheckout"
        | "checkoutinitiated" | "addpaymentinfo" | "addbilling" => Evento::PagoIniciado,
        "addtocart" | "addcart" | "addedtocart" => Evento::Carrito,
        "completeregistration" | "signup" => Evento::Registro,
        "lead" | "generatelead" | "submitform" => Evento::ClientePotencial,
        "search" | "viewsearchresults" => Evento::Busqueda,
        "viewcontent" | "viewitem" | "contentview" => Evento::VerProducto,
        "pageview" | "pagevisit" | "pageload" | "page" | "viewhome" => Evento::VerPagina,
        "conversion" => Evento::Conversion,
        _ => Evento::Otro,
    }
}

/// Text from a request, short and printable.
fn corto(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .take(MAX_TEXTO)
        .collect::<String>()
        .trim()
        .to_string()
}

/// An amount as it travelled: digits with their separators, nothing else.
fn importe(v: Option<String>) -> Option<String> {
    let v = v?.trim().to_string();
    let ok = !v.is_empty()
        && v.len() <= 24
        && v.chars().any(|c| c.is_ascii_digit())
        && v.chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-'));
    ok.then_some(v)
}

/// A currency: three letters.
fn moneda(v: Option<String>) -> Option<String> {
    let v = v?.trim().to_ascii_uppercase();
    (v.len() == 3 && v.chars().all(|c| c.is_ascii_uppercase())).then_some(v)
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// The bytes of a hash written in base64 or base64url, with or without padding.
fn de_base64(v: &str) -> Option<Vec<u8>> {
    use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
    [STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD]
        .iter()
        .find_map(|e| e.decode(v).ok())
        .filter(|b| matches!(b.len(), 16 | 20 | 32))
}

/// Whether a value is a hash: SHA-256 in hex or in base64/base64url (what these networks ask
/// for), or the hex of MD5 or SHA-1 (Criteo's email is an MD5).
fn es_hash(v: &str) -> bool {
    (matches!(v.len(), 32 | 40 | 64) && v.chars().all(|c| c.is_ascii_hexdigit()))
        || (matches!(v.len(), 43 | 44) && de_base64(v).is_some_and(|b| b.len() == 32))
}

/// Which slot of a pixel a value came in.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Hueco {
    Correo,
    Telefono,
}

/// An email or phone a pixel carried: whose (one the person marked or not) and how. Only a real
/// hash, an email written out (with its «@») or a phone with seven digits or more: an empty
/// slot, `undefined` or anything else carried nothing.
fn dato(v: Option<String>, formas: &[Forma], hueco: Hueco) -> Option<Dato> {
    let v = v?.trim().to_string();
    if v.is_empty() || v.len() > 300 {
        return None;
    }
    let cifrado = es_hash(&v);
    let claro = match hueco {
        Hueco::Correo => v
            .split_once('@')
            .is_some_and(|(a, b)| !a.is_empty() && b.contains('.') && !v.contains(' ')),
        Hueco::Telefono => {
            v.chars().filter(char::is_ascii_digit).count() >= 7
                && v.chars()
                    .all(|c| c.is_ascii_digit() || matches!(c, '+' | ' ' | '-' | '(' | ')' | '.'))
        }
    };
    if !cifrado && !claro {
        return None;
    }
    let minus = v.to_ascii_lowercase();
    let mut candidatos = vec![minus.clone()];
    if hueco == Hueco::Telefono && claro {
        // A phone written out is compared by its digits, as the marked one is kept.
        candidatos.push(v.chars().filter(char::is_ascii_digit).collect());
    }
    if let Some(b) = de_base64(&v) {
        candidatos.push(hex(&b));
    }
    let mut tuyo = formas.iter().any(|f| candidatos.contains(&f.aguja));
    // Google and TikTok hash a phone with its «+» (E.164); the marked forms keep its digits.
    if !tuyo && cifrado {
        tuyo = formas
            .iter()
            .filter(|f| f.como == "tal_cual" && f.aguja.len() >= 7)
            .filter(|f| f.aguja.chars().all(|c| c.is_ascii_digit()))
            .any(|f| {
                let h = hex(&Sha256::digest(format!("+{}", f.aguja).as_bytes()));
                candidatos.contains(&h)
            });
    }
    Some(Dato { tuyo, cifrado })
}

fn correo(v: Option<String>, formas: &[Forma]) -> Option<Dato> {
    dato(v, formas, Hueco::Correo)
}

fn telefono(v: Option<String>, formas: &[Forma]) -> Option<Dato> {
    dato(v, formas, Hueco::Telefono)
}

/// The pairs of a body sent as a form (`a=1&b=2`) or as `multipart/form-data`.
fn formulario(cuerpo: &[u8]) -> Vec<(String, String)> {
    let Ok(texto) = std::str::from_utf8(cuerpo) else {
        return Vec::new();
    };
    let t = texto.trim_start();
    if t.starts_with('{') || t.starts_with('[') {
        return Vec::new();
    }
    if texto.contains("Content-Disposition: form-data") {
        let mut out = Vec::new();
        for parte in texto
            .split("Content-Disposition: form-data; name=\"")
            .skip(1)
        {
            let Some((nombre, resto)) = parte.split_once('"') else {
                continue;
            };
            let Some((_, valor)) = resto.split_once("\r\n\r\n") else {
                continue;
            };
            let valor = valor.split("\r\n--").next().unwrap_or("");
            out.push((nombre.to_string(), valor.to_string()));
        }
        return out;
    }
    url::form_urlencoded::parse(texto.as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

fn json(cuerpo: &[u8]) -> Option<Value> {
    let v: Value = serde_json::from_slice(cuerpo).ok()?;
    (v.is_object() || v.is_array()).then_some(v)
}

/// A JSON value as text: strings as they are, numbers written out.
fn texto_json(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Array(a) => texto_json(a.first()),
        _ => None,
    }
}

struct Lectura<'a> {
    host: &'a str,
    ruta: &'a str,
    consulta: Vec<(String, String)>,
    cuerpo: &'a [u8],
    formas: &'a [Forma],
}

/// The first value of `k` in `pares`.
fn de(pares: &[(String, String)], k: &str) -> Option<String> {
    pares
        .iter()
        .find(|(a, _)| a == k)
        .map(|(_, v)| v.clone())
        .filter(|v| !v.is_empty())
}

/// The email and phone of Google's «user-provided data» (`tv.1~em.<hash>~pn.<hash>`).
fn de_google(v: Option<String>) -> (Option<String>, Option<String>) {
    let Some(v) = v else {
        return (None, None);
    };
    let mut em = None;
    let mut pn = None;
    for trozo in v.split('~') {
        if let Some(x) = trozo.strip_prefix("em.") {
            em = Some(x.to_string());
        } else if let Some(x) = trozo.strip_prefix("pn.") {
            pn = Some(x.to_string());
        }
    }
    (em, pn)
}

/// The one to keep of several events in a request: the one that says most.
fn el_mayor(v: Vec<Chivato>) -> Option<Chivato> {
    v.into_iter()
        .enumerate()
        .max_by_key(|(i, c)| (c.evento.peso(), std::cmp::Reverse(*i)))
        .map(|(_, c)| c)
}

impl Lectura<'_> {
    fn nuevo(&self, red: Red, nombre: &str) -> Chivato {
        Chivato {
            red,
            evento: evento_de(nombre),
            nombre: nombre.to_string(),
            importe: None,
            moneda: None,
            correo: None,
            telefono: None,
            id: None,
            servidor: None,
            pagina: None,
        }
    }

    /// The address's pairs, then the body's (a form).
    fn pares(&self) -> Vec<(String, String)> {
        let mut p = self.consulta.clone();
        p.extend(formulario(self.cuerpo));
        p
    }

    /// Meta: `ev`, `cd[value]`, `cd[currency]`, `ud[em]`, `ud[ph]`, `eid`; GET or a POSTed form.
    fn meta(&self) -> Option<Chivato> {
        let p = self.pares();
        let ev = de(&p, "ev")?;
        let mut c = self.nuevo(Red::Meta, &ev);
        c.importe = importe(de(&p, "cd[value]"));
        c.moneda = moneda(de(&p, "cd[currency]"));
        c.correo = correo(de(&p, "ud[em]").or_else(|| de(&p, "udff[em]")), self.formas);
        c.telefono = telefono(de(&p, "ud[ph]").or_else(|| de(&p, "udff[ph]")), self.formas);
        c.id = de(&p, "eid");
        c.pagina = pagina_de(de(&p, "dl"));
        Some(c)
    }

    /// Google Analytics 4: `en` (the event), `epn.value`, `cu`, `em=tv.1~em.…`; several events
    /// in one request travel one per line of the body, each line adding to the address's pairs.
    fn ga4(&self) -> Option<Chivato> {
        let base = self.consulta.clone();
        let ok = de(&base, "v").as_deref() == Some("2")
            || de(&base, "tid").is_some_and(|t| t.starts_with("G-"));
        if !ok {
            return None;
        }
        // Google's own names (`google-analytics.com`, `stats.g.doubleclick.net`…), by the list of
        // owners; any other is a server of the web's that may pass it on.
        let google = guardiana_lists::company_of(self.host) == Some("Google");
        let servidor = (!google).then(|| sitio(self.host));
        let pagina = pagina_de(de(&base, "dl"));
        let lineas: Vec<Vec<(String, String)>> = std::str::from_utf8(self.cuerpo)
            .unwrap_or("")
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                url::form_urlencoded::parse(l.trim().as_bytes())
                    .map(|(k, v)| (k.into_owned(), v.into_owned()))
                    .collect()
            })
            .collect();
        let eventos: Vec<Vec<(String, String)>> = if lineas.is_empty() {
            vec![base.clone()]
        } else {
            lineas
                .into_iter()
                .map(|mut l| {
                    l.extend(base.iter().cloned());
                    l
                })
                .collect()
        };
        let todos = eventos
            .iter()
            .filter_map(|p| {
                let en = de(p, "en")?;
                let mut c = self.nuevo(Red::GoogleAnalytics, &en);
                c.importe = importe(de(p, "epn.value").or_else(|| de(p, "ep.value")));
                c.moneda = moneda(de(p, "cu").or_else(|| de(p, "ep.currency")));
                let (em, pn) = de_google(de(p, "em"));
                c.correo = correo(em, self.formas);
                c.telefono = telefono(pn, self.formas);
                c.id = de(p, "ep.transaction_id").or_else(|| de(p, "ep.event_id"));
                c.servidor.clone_from(&servidor);
                c.pagina = pagina_de(de(p, "dl")).or_else(|| pagina.clone());
                Some(c)
            })
            .collect();
        el_mayor(todos)
    }

    /// Google Ads: a conversion (`value`, `currency_code`, `oid`, `em`), or a remarketing visit.
    fn google_ads(&self) -> Option<Chivato> {
        let p = self.pares();
        let tramo = tras(self.ruta, "pagead").unwrap_or("conversion");
        let (nombre, evento) = match de(&p, "en") {
            Some(en) => {
                let e = evento_de(&en);
                (en, e)
            }
            // `form-data` carries what was typed in a form for a conversion («enhanced
            // conversions»).
            None if matches!(tramo, "conversion" | "1p-conversion" | "form-data") => {
                (tramo.to_string(), Evento::Conversion)
            }
            None => (tramo.to_string(), Evento::VerPagina),
        };
        let mut c = self.nuevo(Red::GoogleAds, &nombre);
        c.evento = evento;
        c.importe = importe(de(&p, "value"));
        c.moneda = moneda(de(&p, "currency_code"));
        let (em, pn) = de_google(de(&p, "em"));
        c.correo = correo(em, self.formas);
        c.telefono = telefono(pn, self.formas);
        c.id = de(&p, "oid");
        Some(c)
    }

    /// TikTok: a JSON body with `event`, `properties.value`, `properties.currency`,
    /// `context.user.email` and `.phone_number` (hashed), `event_id`; or a `batch` of them.
    fn tiktok(&self) -> Option<Chivato> {
        let v = json(self.cuerpo)?;
        let lista: Vec<&Value> = match v.get("batch").and_then(Value::as_array) {
            Some(b) => b.iter().collect(),
            None => vec![&v],
        };
        let todos = lista
            .into_iter()
            .filter_map(|e| {
                let ev = texto_json(e.get("event"))?;
                let mut c = self.nuevo(Red::Tiktok, &ev);
                let pr = e.get("properties");
                c.importe = importe(texto_json(pr.and_then(|p| p.get("value"))));
                c.moneda = moneda(texto_json(pr.and_then(|p| p.get("currency"))));
                let u = e.get("context").and_then(|x| x.get("user"));
                let campo = |a: &str, b: &str| {
                    texto_json(u.and_then(|u| u.get(a)))
                        .or_else(|| texto_json(u.and_then(|u| u.get(b))))
                };
                c.correo = correo(campo("email", "sha256_email"), self.formas);
                c.telefono = telefono(campo("phone_number", "sha256_phone_number"), self.formas);
                c.id = texto_json(e.get("event_id"));
                Some(c)
            })
            .collect();
        el_mayor(todos)
    }

    /// Pinterest: `event`, `ed` (JSON: `value`, `currency`, `order_id`), `pd` (JSON: `em`).
    fn pinterest(&self) -> Option<Chivato> {
        let p = self.pares();
        let ev = de(&p, "event")?;
        let mut c = self.nuevo(Red::Pinterest, &ev);
        // Pinterest's «checkout» is the purchase done («people who complete transactions»).
        if ev.eq_ignore_ascii_case("checkout") {
            c.evento = Evento::Compra;
        }
        let ed = de(&p, "ed").and_then(|s| json(s.as_bytes()));
        let pd = de(&p, "pd").and_then(|s| json(s.as_bytes()));
        let en = |v: &Option<Value>, k: &str| texto_json(v.as_ref().and_then(|v| v.get(k)));
        c.importe = importe(en(&ed, "value").or_else(|| en(&ed, "order_value")));
        c.moneda = moneda(en(&ed, "currency"));
        c.correo = correo(en(&pd, "em").or_else(|| de(&p, "pd[em]")), self.formas);
        c.telefono = telefono(en(&pd, "ph").or_else(|| de(&p, "pd[ph]")), self.formas);
        c.id = en(&ed, "event_id")
            .or_else(|| de(&p, "event_id"))
            .or_else(|| en(&ed, "order_id"));
        Some(c)
    }

    /// Snap: `e` / `ev` / `event_type`, `price`, `currency`, `u_hem`, `u_hpn`; as a form or
    /// as JSON.
    fn snap(&self) -> Option<Chivato> {
        let mut p = self.pares();
        if let Some(Value::Object(o)) = json(self.cuerpo) {
            p.extend(
                o.iter()
                    .filter_map(|(k, v)| Some((k.clone(), texto_json(Some(v))?))),
            );
        }
        let ev = de(&p, "e")
            .or_else(|| de(&p, "ev"))
            .or_else(|| de(&p, "event_type"))?;
        let mut c = self.nuevo(Red::Snap, &ev);
        c.importe = importe(de(&p, "price"));
        c.moneda = moneda(de(&p, "currency"));
        c.correo = correo(de(&p, "u_hem"), self.formas);
        c.telefono = telefono(de(&p, "u_hpn"), self.formas);
        c.id = de(&p, "client_dedup_id").or_else(|| de(&p, "event_id"));
        Some(c)
    }

    /// LinkedIn: a conversion (`conversionId`) or the visit itself (`pid`).
    fn linkedin(&self) -> Chivato {
        let p = self.pares();
        match de(&p, "conversionId").or_else(|| de(&p, "conversion_id")) {
            Some(id) => {
                let mut c = self.nuevo(Red::Linkedin, "conversion");
                c.id = de(&p, "eventId").or(Some(id));
                c
            }
            None => {
                let mut c = self.nuevo(Red::Linkedin, "pageview");
                c.evento = Evento::VerPagina;
                c
            }
        }
    }

    /// X: `events` (`[["pageview",{…}]]`) or `event`, `tw_sale_amount`, `txn_id`, `event_id`.
    fn x(&self) -> Option<Chivato> {
        let p = self.pares();
        let lista = de(&p, "events").and_then(|s| json(s.as_bytes()));
        let primero = lista.as_ref().and_then(|l| l.get(0));
        let nombre = texto_json(primero.and_then(|e| e.get(0)))
            .or_else(|| de(&p, "event").filter(|e| !e.starts_with('{')))
            .unwrap_or_else(|| "adsct".to_string());
        let datos = primero
            .and_then(|e| e.get(1))
            .cloned()
            .or_else(|| de(&p, "event").and_then(|s| json(s.as_bytes())));
        let mut c = self.nuevo(Red::X, &nombre);
        if c.evento == Evento::Otro && (nombre == "adsct" || nombre.starts_with("tw-")) {
            // A conversion of that web's own, by its id: the request does not say what.
            c.evento = Evento::Conversion;
        }
        let en = |k: &str| texto_json(datos.as_ref().and_then(|d| d.get(k)));
        c.importe = importe(de(&p, "tw_sale_amount").or_else(|| en("value")));
        c.moneda = moneda(en("currency").or_else(|| de(&p, "tw_currency")));
        c.correo = correo(en("email_address"), self.formas);
        c.telefono = telefono(en("phone_number"), self.formas);
        c.id = de(&p, "event_id")
            .or_else(|| en("conversion_id"))
            .or_else(|| de(&p, "tw_order_id"));
        Some(c)
    }

    /// Criteo: each `pN` is one event (`e=trans&id=…`, `e=vp`, `e=ce&m=<hash>`…).
    fn criteo(&self) -> Option<Chivato> {
        let p = self.pares();
        let eventos: Vec<Vec<(String, String)>> = p
            .iter()
            .filter(|(k, _)| {
                k.len() > 1 && k.starts_with('p') && k[1..].chars().all(|c| c.is_ascii_digit())
            })
            .map(|(_, v)| {
                url::form_urlencoded::parse(v.as_bytes())
                    .map(|(a, b)| (a.into_owned(), b.into_owned()))
                    .collect()
            })
            .collect();
        let m = eventos
            .iter()
            .find_map(|e| de(e, "m").or_else(|| de(e, "sha256_hashed_email")));
        let todos = eventos
            .iter()
            .filter_map(|e| {
                let codigo = de(e, "e")?;
                let (nombre, evento) = match codigo.as_str() {
                    "trans" => ("trackTransaction", Evento::Compra),
                    "vp" => ("viewItem", Evento::VerProducto),
                    "vh" => ("viewHome", Evento::VerPagina),
                    "vl" => ("viewList", Evento::VerPagina),
                    "vb" => ("viewBasket", Evento::Otro),
                    "vs" => ("viewSearch", Evento::Busqueda),
                    _ => return None,
                };
                let mut c = self.nuevo(Red::Criteo, nombre);
                c.evento = evento;
                c.moneda = moneda(de(e, "currency"));
                c.id = de(e, "id").filter(|_| codigo == "trans");
                Some(c)
            })
            .collect();
        let mut c = el_mayor(todos)?;
        c.correo = correo(m, self.formas);
        Some(c)
    }

    /// Microsoft Advertising (UET): `evt` (`pageLoad`, `custom`), `ea` (the action), `gv`
    /// (value), `gc` (currency).
    fn uet(&self) -> Option<Chivato> {
        let p = self.pares();
        let evt = de(&p, "evt")?;
        let nombre = if evt.eq_ignore_ascii_case("pageLoad") {
            evt
        } else {
            de(&p, "ea").unwrap_or(evt)
        };
        let mut c = self.nuevo(Red::Microsoft, &nombre);
        c.importe = importe(de(&p, "gv"));
        c.moneda = moneda(de(&p, "gc"));
        c.correo = correo(de(&p, "em"), self.formas);
        Some(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tinta::{formas, Marcado, Tipo};

    fn sha(s: &str) -> String {
        hex(&Sha256::digest(s.as_bytes()))
    }

    fn mias() -> Vec<Forma> {
        formas(&[
            Marcado {
                tipo: Tipo::Correo,
                valor: "ana@correo.co".into(),
            },
            Marcado {
                tipo: Tipo::Telefono,
                valor: "+57 300 123 4567".into(),
            },
        ])
    }

    fn tuyo() -> Option<Dato> {
        Some(Dato {
            tuyo: true,
            cifrado: true,
        })
    }

    #[test]
    fn meta_s_pixel_says_the_purchase_the_amount_and_whose_email() {
        let f = mias();
        let url = format!(
            "https://www.facebook.com/tr/?id=1234&ev=Purchase&dl=https%3A%2F%2Ftienda.co%2Fgracias&cd[value]=89900&cd[currency]=COP&cd[content_ids]=%5B%22SKU-1%22%5D&ud[em]={}&eid=ord-77",
            sha("ana@correo.co")
        );
        let c = lee(&url, "GET", b"", &f).unwrap_or_else(|| unreachable!());
        assert_eq!((c.red, c.evento), (Red::Meta, Evento::Compra));
        assert_eq!(c.nombre, "Purchase");
        assert_eq!(c.importe.as_deref(), Some("89900"));
        assert_eq!(c.moneda.as_deref(), Some("COP"));
        assert_eq!(c.correo, tuyo());
        assert_eq!(c.id.as_deref(), Some("ord-77"));
        // Someone else's hash is «an email», never «yours».
        let otro = url.replace(&sha("ana@correo.co"), &sha("otra@correo.co"));
        let c = lee(&otro, "GET", b"", &f).unwrap_or_else(|| unreachable!());
        assert_eq!(
            c.correo,
            Some(Dato {
                tuyo: false,
                cifrado: true
            })
        );
        // As a POSTed form (what `fbevents.js` does with long data), and multipart too.
        let cuerpo = format!(
            "id=1&ev=AddToCart&cd%5Bvalue%5D=12.5&cd%5Bcurrency%5D=usd&ud%5Bph%5D={}",
            sha("573001234567")
        );
        let c = lee(
            "https://www.facebook.com/tr/",
            "POST",
            cuerpo.as_bytes(),
            &f,
        )
        .unwrap_or_else(|| unreachable!());
        assert_eq!(c.evento, Evento::Carrito);
        assert_eq!(
            (c.importe.as_deref(), c.moneda.as_deref()),
            (Some("12.5"), Some("USD"))
        );
        assert_eq!(c.telefono, tuyo());
        let multi = b"--b\r\nContent-Disposition: form-data; name=\"id\"\r\n\r\n1\r\n--b\r\nContent-Disposition: form-data; name=\"ev\"\r\n\r\nLead\r\n--b--\r\n";
        let c = lee("https://www.facebook.com/tr/", "POST", multi, &f)
            .unwrap_or_else(|| unreachable!());
        assert_eq!(c.evento, Evento::ClientePotencial);
        // Not the pixel: nothing to say.
        assert!(lee(
            "https://www.facebook.com/plugins/like.php?ev=Purchase",
            "GET",
            b"",
            &f
        )
        .is_none());
        assert!(lee("https://www.facebook.com/tr?id=1", "GET", b"", &f).is_none());
    }

    #[test]
    fn google_analytics_4_reads_each_event_of_a_batch() {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let f = mias();
        let em = URL_SAFE_NO_PAD.encode(Sha256::digest(b"ana@correo.co"));
        let url = format!(
            "https://region1.google-analytics.com/g/collect?v=2&tid=G-ABC123&cid=1.2&dl=https%3A%2F%2Ftienda.co%2F&em=tv.1~em.{em}"
        );
        let cuerpo = b"en=page_view&_et=1\nen=purchase&ep.transaction_id=T-9&epn.value=89900&cu=COP\nen=scroll";
        let c = lee(&url, "POST", cuerpo, &f).unwrap_or_else(|| unreachable!());
        assert_eq!((c.red, c.evento), (Red::GoogleAnalytics, Evento::Compra));
        assert_eq!(c.nombre, "purchase");
        assert_eq!(c.importe.as_deref(), Some("89900"));
        assert_eq!(c.moneda.as_deref(), Some("COP"));
        assert_eq!(c.correo, tuyo());
        assert_eq!(c.id.as_deref(), Some("T-9"));
        assert_eq!(c.servidor, None);
        // One event in the address alone.
        let c = lee(
            "https://www.google-analytics.com/g/collect?v=2&tid=G-1&en=add_to_cart&epn.value=10&cu=EUR",
            "POST",
            b"",
            &f,
        )
        .unwrap_or_else(|| unreachable!());
        assert_eq!(
            (c.evento, c.moneda.as_deref()),
            (Evento::Carrito, Some("EUR"))
        );
        // A server-side tag on the web's own name: the web sends it to itself.
        let c = lee(
            "https://sgtm.tienda.co/g/collect?v=2&tid=G-1&en=begin_checkout",
            "GET",
            b"",
            &f,
        )
        .unwrap_or_else(|| unreachable!());
        assert_eq!(c.evento, Evento::PagoIniciado);
        assert_eq!(c.servidor.as_deref(), Some("tienda.co"));
        // Any other `/g/collect` is not GA4.
        assert!(lee("https://otra.co/g/collect?x=1", "GET", b"", &f).is_none());
    }

    #[test]
    fn google_ads_says_a_conversion_not_what_it_was() {
        let f = mias();
        let c = lee(
            "https://www.googleadservices.com/pagead/conversion/123456/?label=AbC&value=89900&currency_code=COP&oid=ord-77",
            "GET",
            b"",
            &f,
        )
        .unwrap_or_else(|| unreachable!());
        assert_eq!((c.red, c.evento), (Red::GoogleAds, Evento::Conversion));
        assert_eq!(c.importe.as_deref(), Some("89900"));
        assert_eq!(c.id.as_deref(), Some("ord-77"));
        let c = lee(
            &format!(
                "https://www.google.com/pagead/1p-conversion/123/?en=purchase&em=tv.1~em.{}",
                sha("ana@correo.co")
            ),
            "GET",
            b"",
            &f,
        )
        .unwrap_or_else(|| unreachable!());
        assert_eq!(c.evento, Evento::Compra);
        assert_eq!(c.correo, tuyo());
        let c = lee(
            "https://googleads.g.doubleclick.net/pagead/viewthroughconversion/123/?random=1",
            "GET",
            b"",
            &f,
        )
        .unwrap_or_else(|| unreachable!());
        assert_eq!(c.evento, Evento::VerPagina);
    }

    #[test]
    fn tiktok_reads_its_json_and_hashed_phone_with_the_plus() {
        let f = mias();
        let cuerpo = format!(
            r#"{{"event":"CompletePayment","event_id":"e-1","message_id":"m-1","properties":{{"value":89900,"currency":"COP","contents":[{{"content_id":"SKU-1"}}]}},"context":{{"user":{{"email":"{}","phone_number":"{}"}}}}}}"#,
            sha("ana@correo.co"),
            sha("+573001234567")
        );
        let c = lee(
            "https://analytics.tiktok.com/api/v2/pixel",
            "POST",
            cuerpo.as_bytes(),
            &f,
        )
        .unwrap_or_else(|| unreachable!());
        assert_eq!((c.red, c.evento), (Red::Tiktok, Evento::Compra));
        assert_eq!(c.importe.as_deref(), Some("89900"));
        assert_eq!(c.correo, tuyo());
        assert_eq!(c.telefono, tuyo());
        assert_eq!(c.id.as_deref(), Some("e-1"));
        let lote =
            br#"{"batch":[{"event":"Pageview"},{"event":"AddToCart","properties":{"value":"5"}}]}"#;
        let c = lee(
            "https://analytics.tiktok.com/api/v2/pixel/batch",
            "POST",
            lote,
            &f,
        )
        .unwrap_or_else(|| unreachable!());
        assert_eq!(c.evento, Evento::Carrito);
        assert!(interesa("https://analytics.tiktok.com/api/v2/pixel"));
        assert!(!interesa("https://www.tiktok.com/@alguien"));
    }

    #[test]
    fn pinterest_snap_linkedin_x_criteo_and_microsoft() {
        let f = mias();
        let e = sha("ana@correo.co");
        let pin = format!(
            "https://ct.pinterest.com/v3/?event=checkout&ed=%7B%22value%22%3A%2289900%22%2C%22currency%22%3A%22COP%22%2C%22order_id%22%3A%227%22%7D&pd=%7B%22em%22%3A%22{e}%22%7D&tid=26"
        );
        let c = lee(&pin, "GET", b"", &f).unwrap_or_else(|| unreachable!());
        assert_eq!((c.red, c.evento), (Red::Pinterest, Evento::Compra));
        assert_eq!(
            (c.importe.as_deref(), c.moneda.as_deref()),
            (Some("89900"), Some("COP"))
        );
        assert_eq!(c.correo, tuyo());

        let snap =
            format!("pid=1&ev=PURCHASE&price=89900&currency=COP&u_hem={e}&client_dedup_id=d7");
        let c = lee("https://tr.snapchat.com/p", "POST", snap.as_bytes(), &f)
            .unwrap_or_else(|| unreachable!());
        assert_eq!((c.red, c.evento), (Red::Snap, Evento::Compra));
        assert_eq!(c.correo, tuyo());
        assert_eq!(c.id.as_deref(), Some("d7"));

        let c = lee(
            "https://px.ads.linkedin.com/collect/?pid=123&conversionId=456&fmt=gif",
            "GET",
            b"",
            &f,
        )
        .unwrap_or_else(|| unreachable!());
        assert_eq!((c.red, c.evento), (Red::Linkedin, Evento::Conversion));
        let c = lee(
            "https://px.ads.linkedin.com/collect/?pid=123&fmt=gif",
            "GET",
            b"",
            &f,
        )
        .unwrap_or_else(|| unreachable!());
        assert_eq!(c.evento, Evento::VerPagina);

        let c = lee(
            "https://analytics.twitter.com/i/adsct?txn_id=o1abc&p_id=Twitter&tw_sale_amount=89900&tw_order_quantity=1",
            "GET",
            b"",
            &f,
        )
        .unwrap_or_else(|| unreachable!());
        assert_eq!((c.red, c.evento), (Red::X, Evento::Conversion));
        assert_eq!(c.importe.as_deref(), Some("89900"));
        let c = lee(
            "https://t.co/i/adsct?txn_id=o1abc&events=%5B%5B%22pageview%22%2C%7B%7D%5D%5D",
            "GET",
            b"",
            &f,
        )
        .unwrap_or_else(|| unreachable!());
        assert_eq!(c.evento, Evento::VerPagina);

        let crit = format!(
            "https://sslwidget.criteo.com/event?a=1&v=5.23&p0=e%3Dexd%26site_type%3Dd&p1=e%3Dce%26m%3D{}&p2=e%3Dtrans%26id%3D77%26item%3DSKU-1~89900~1",
            crate::md5::digest(b"ana@correo.co").iter().map(|b| format!("{b:02x}")).collect::<String>()
        );
        let c = lee(&crit, "GET", b"", &f).unwrap_or_else(|| unreachable!());
        assert_eq!((c.red, c.evento), (Red::Criteo, Evento::Compra));
        assert_eq!(c.importe, None);
        assert_eq!(c.correo, tuyo());

        let c = lee(
            "https://bat.bing.com/action/0?ti=1&evt=custom&ea=purchase&gv=89900&gc=COP",
            "GET",
            b"",
            &f,
        )
        .unwrap_or_else(|| unreachable!());
        assert_eq!((c.red, c.evento), (Red::Microsoft, Evento::Compra));
        assert_eq!(
            (c.importe.as_deref(), c.moneda.as_deref()),
            (Some("89900"), Some("COP"))
        );
        let c = lee(
            "https://bat.bing.com/action/0?ti=1&evt=pageLoad",
            "GET",
            b"",
            &f,
        )
        .unwrap_or_else(|| unreachable!());
        assert_eq!(c.evento, Evento::VerPagina);
    }

    #[test]
    fn an_empty_or_odd_slot_carried_nothing() {
        let f = mias();
        for v in ["undefined", "", "null", "12", "x@", "nombre%20apellido"] {
            let c = lee(
                &format!("https://www.facebook.com/tr?ev=Lead&ud[em]={v}&ud[ph]={v}"),
                "GET",
                b"",
                &f,
            )
            .unwrap_or_else(|| unreachable!());
            assert_eq!((c.correo, c.telefono), (None, None), "{v}");
        }
        // A phone written out is a phone, and the person's when its digits are.
        let c = lee(
            "https://www.facebook.com/tr?ev=Lead&ud[ph]=%2B57%20300%20123%204567",
            "GET",
            b"",
            &f,
        )
        .unwrap_or_else(|| unreachable!());
        assert_eq!(
            c.telefono,
            Some(Dato {
                tuyo: true,
                cifrado: false
            })
        );
    }

    #[test]
    fn checkout_is_a_purchase_only_for_pinterest() {
        let f = mias();
        let c = lee("https://www.facebook.com/tr?ev=Checkout", "GET", b"", &f)
            .unwrap_or_else(|| unreachable!());
        assert_eq!(c.evento, Evento::PagoIniciado);
        let c = lee(
            "https://ct.pinterest.com/v3/?event=checkout&tid=1",
            "GET",
            b"",
            &f,
        )
        .unwrap_or_else(|| unreachable!());
        assert_eq!(c.evento, Evento::Compra);
    }

    #[test]
    fn google_s_country_names_and_other_endpoints_are_read_too() {
        let f = mias();
        let c = lee(
            "https://www.google.com.co/pagead/1p-user-list/123/?random=1",
            "GET",
            b"",
            &f,
        )
        .unwrap_or_else(|| unreachable!());
        assert_eq!((c.red, c.evento), (Red::GoogleAds, Evento::VerPagina));
        // The data of an «enhanced» conversion, as multipart form data.
        let cuerpo = format!(
            "--b\r\nContent-Disposition: form-data; name=\"em\"\r\n\r\ntv.1~em.{}\r\n--b--\r\n",
            sha("ana@correo.co")
        );
        let c = lee(
            "https://www.google.com.mx/pagead/form-data/123?gtm=1",
            "POST",
            cuerpo.as_bytes(),
            &f,
        )
        .unwrap_or_else(|| unreachable!());
        assert_eq!((c.red, c.evento), (Red::GoogleAds, Evento::Conversion));
        assert_eq!(c.correo, tuyo());
        assert!(lee(
            "https://www.google.com.co/recaptcha/api2/anchor",
            "GET",
            b"",
            &f
        )
        .is_none());
        assert!(lee("https://www.google.co/pagead/conversion/1/", "GET", b"", &f).is_none());
        // Google Analytics through DoubleClick is Google's own, not a server of the web's.
        let c = lee(
            "https://stats.g.doubleclick.net/g/collect?v=2&tid=G-1&en=purchase&dl=https%3A%2F%2Fwww.tienda.co%2Fgracias",
            "POST",
            b"",
            &f,
        )
        .unwrap_or_else(|| unreachable!());
        assert_eq!((c.red, c.servidor.as_deref()), (Red::GoogleAnalytics, None));
        assert_eq!(c.pagina.as_deref(), Some("tienda.co"));
        // Meta's Privacy Sandbox trigger.
        let c = lee(
            "https://www.facebook.com/privacy_sandbox/pixel/register/trigger/?id=1&ev=Purchase&dl=https%3A%2F%2Fotra.co%2F",
            "GET",
            b"",
            &f,
        )
        .unwrap_or_else(|| unreachable!());
        assert_eq!((c.red, c.evento), (Red::Meta, Evento::Compra));
        assert_eq!(c.pagina.as_deref(), Some("otra.co"));
    }

    #[test]
    fn nothing_odd_is_taken_as_an_amount_or_a_name() {
        let f = mias();
        let c = lee(
            "https://www.facebook.com/tr?ev=Purchase&cd[value]=%3Cb%3E1%3C%2Fb%3E&cd[currency]=pesos",
            "GET",
            b"",
            &f,
        )
        .unwrap_or_else(|| unreachable!());
        assert_eq!((c.importe, c.moneda), (None, None));
        let largo = "x".repeat(500);
        let c = lee(
            &format!("https://www.facebook.com/tr?ev={largo}"),
            "GET",
            b"",
            &f,
        )
        .unwrap_or_else(|| unreachable!());
        assert_eq!(c.evento, Evento::Otro);
        assert_eq!(c.nombre.chars().count(), MAX_TEXTO);
        // A plain email in the email slot is «an email» (not hashed), and if it is the
        // person's, «yours».
        let c = lee(
            "https://www.facebook.com/tr?ev=Lead&ud[em]=ana%40correo.co",
            "GET",
            b"",
            &f,
        )
        .unwrap_or_else(|| unreachable!());
        assert_eq!(
            c.correo,
            Some(Dato {
                tuyo: true,
                cifrado: false
            })
        );
    }
}
