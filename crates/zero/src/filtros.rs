//! The network rules of the lists every serious blocker uses (EasyList, EasyPrivacy, uBlock
//! Origin's filters, badware and the URLhaus malware list), on top of ZERO's own lists by
//! company (the owner, 13 Oct 2026: «que se bloquee toda la basura al nivel de Brave o más»).
//!
//! Brave and uBlock Origin match addresses against these rules; ZERO does the same with its own
//! small matcher, written here so no new crate comes in: the rule syntax of Adblock Plus with
//! uBlock's extensions that matter for network requests (`||`, `|`, `^`, `*`, `@@`, and the
//! options `third-party`, `domain=`, `to=`, the request types, `important`, `badfilter`,
//! `popup`). A rule with an option this matcher does not understand is left out rather than
//! guessed: a missed advert is better than a broken page. Element hiding and scriptlets are not
//! network rules and are not here.
//!
//! The lists are copies taken on the day of each release, written to `data/filtros/` without
//! comments or cosmetic rules; their source, licence and date are at the top of each file.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::mandato::Recurso;

/// Where a rule comes from, which says why it cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Lista {
    /// Advertising (EasyList, uBlock filters).
    Anuncios,
    /// Tracking (EasyPrivacy, uBlock privacy).
    Rastreo,
    /// Malware, scams and phishing (uBlock badware, URLhaus).
    Peligro,
}

const LISTAS: &[(&str, Lista)] = &[
    (
        include_str!("../data/filtros/easylist.txt"),
        Lista::Anuncios,
    ),
    (
        include_str!("../data/filtros/easyprivacy.txt"),
        Lista::Rastreo,
    ),
    (include_str!("../data/filtros/ubo.txt"), Lista::Anuncios),
    (
        include_str!("../data/filtros/ubo-privacy.txt"),
        Lista::Rastreo,
    ),
    (
        include_str!("../data/filtros/ubo-badware.txt"),
        Lista::Peligro,
    ),
    (
        include_str!("../data/filtros/ubo-unbreak.txt"),
        Lista::Anuncios,
    ),
    (include_str!("../data/filtros/urlhaus.txt"), Lista::Peligro),
];

// Request types, as bits.
const T_SCRIPT: u16 = 1;
const T_IMAGE: u16 = 1 << 1;
const T_STYLE: u16 = 1 << 2;
const T_XHR: u16 = 1 << 3;
const T_FRAME: u16 = 1 << 4;
const T_MEDIA: u16 = 1 << 5;
const T_FONT: u16 = 1 << 6;
const T_PING: u16 = 1 << 7;
const T_SOCKET: u16 = 1 << 8;
const T_OTHER: u16 = 1 << 9;
const T_OBJECT: u16 = 1 << 10;
const T_POPUP: u16 = 1 << 11;
const T_DOCUMENT: u16 = 1 << 12;
/// What a rule without type options applies to: everything but a page and a popup.
const T_DEFECTO: u16 = T_SCRIPT
    | T_IMAGE
    | T_STYLE
    | T_XHR
    | T_FRAME
    | T_MEDIA
    | T_FONT
    | T_PING
    | T_SOCKET
    | T_OTHER
    | T_OBJECT;

const fn tipo_de(r: Recurso) -> u16 {
    match r {
        Recurso::Documento => T_DOCUMENT,
        Recurso::Ventana => T_POPUP,
        Recurso::Marco => T_FRAME,
        Recurso::Script => T_SCRIPT,
        Recurso::Imagen => T_IMAGE,
        Recurso::Estilo => T_STYLE,
        Recurso::Fuente => T_FONT,
        Recurso::Media => T_MEDIA,
        Recurso::Datos => T_XHR,
        Recurso::Aviso => T_PING,
        Recurso::Socket => T_SOCKET,
        Recurso::Otro => T_OTHER,
    }
}

/// One network rule, compiled.
#[derive(Debug)]
struct Regla {
    /// The pattern split at `*`, lower case; `^` kept as the separator placeholder.
    partes: Vec<Box<str>>,
    /// `||`: starts at the beginning of the host name or of one of its labels.
    ancla_dominio: bool,
    /// `|` at the start: at the beginning of the address.
    ancla_inicio: bool,
    /// `|` at the end: at the end of the address.
    ancla_fin: bool,
    tipos: u16,
    /// `Some(true)`: only third-party requests; `Some(false)`: only first-party.
    tercero: Option<bool>,
    /// `domain=`: (site, negated). Empty: any page.
    paginas: Box<[(Box<str>, bool)]>,
    /// `to=`: (site, negated) of the destination. Empty: any.
    destinos: Box<[(Box<str>, bool)]>,
    importante: bool,
    lista: Lista,
}

/// The compiled lists.
#[derive(Debug, Default)]
pub struct Motor {
    bloqueo: Indice,
    excepcion: Indice,
    /// How many rules were read.
    pub leidas: usize,
    /// How many were left out (unknown options, regular expressions, switched off).
    pub omitidas: usize,
}

#[derive(Debug, Default)]
struct Indice {
    reglas: Vec<Regla>,
    por_token: HashMap<u64, Vec<u32>>,
    sin_token: Vec<u32>,
}

impl Indice {
    fn mete(&mut self, r: Regla) {
        let i = u32::try_from(self.reglas.len()).unwrap_or(u32::MAX);
        match mejor_token(&r) {
            Some(t) => self.por_token.entry(t).or_default().push(i),
            None => self.sin_token.push(i),
        }
        self.reglas.push(r);
    }

    fn busca<'s>(&'s self, p: &Pedido<'_>, tokens: &[u64]) -> Option<&'s Regla> {
        let mut mejor: Option<&'s Regla> = None;
        let candidatos = tokens
            .iter()
            .filter_map(|t| self.por_token.get(t))
            .flatten()
            .chain(self.sin_token.iter());
        for i in candidatos {
            let Some(r) = self.reglas.get(*i as usize) else {
                continue;
            };
            if (mejor.is_none() || r.importante) && r.aplica(p) {
                mejor = Some(r);
                if r.importante {
                    break;
                }
            }
        }
        mejor
    }
}

/// One request, as the rules see it.
struct Pedido<'a> {
    /// The address, lower case.
    url: &'a str,
    /// Where the host name starts and ends in `url`.
    host: (usize, usize),
    tipo: u16,
    tercero: bool,
    sitio_pagina: &'a str,
    host_destino: &'a str,
}

impl Regla {
    fn aplica(&self, p: &Pedido<'_>) -> bool {
        if self.tipos & p.tipo == 0 {
            return false;
        }
        if self.tercero.is_some_and(|t| t != p.tercero) {
            return false;
        }
        if !self.paginas.is_empty() && !casa_dominios(&self.paginas, p.sitio_pagina) {
            return false;
        }
        if !self.destinos.is_empty() && !casa_dominios(&self.destinos, p.host_destino) {
            return false;
        }
        self.casa_url(p)
    }

    fn casa_url(&self, p: &Pedido<'_>) -> bool {
        let u = p.url.as_bytes();
        let Some((primera, resto)) = self.partes.split_first() else {
            return true;
        };
        let intenta = |inicio: usize| -> bool {
            let Some(fin) = literal_en(u, inicio, primera.as_bytes()) else {
                return false;
            };
            sigue(u, fin, resto, self.ancla_fin)
        };
        if self.ancla_dominio {
            let (h0, h1) = p.host;
            if intenta(h0) {
                return true;
            }
            (h0..h1)
                .filter(|&i| u.get(i) == Some(&b'.'))
                .any(|i| intenta(i + 1))
        } else if self.ancla_inicio {
            intenta(0)
        } else if primera.is_empty() {
            sigue(u, 0, resto, self.ancla_fin)
        } else {
            let pb = primera.as_bytes();
            let mut desde = 0;
            while desde <= u.len() {
                // Jump to the next place the first literal piece can start.
                let Some(i) = siguiente(u, desde, pb) else {
                    return false;
                };
                if intenta(i) {
                    return true;
                }
                desde = i + 1;
            }
            false
        }
    }
}

/// The rest of the pieces, each anywhere after the previous one; the last one at the end of
/// the address when the rule is anchored there.
fn sigue(u: &[u8], desde: usize, resto: &[Box<str>], ancla_fin: bool) -> bool {
    let Some((pieza, mas)) = resto.split_first() else {
        return !ancla_fin || desde == u.len();
    };
    let pb = pieza.as_bytes();
    let mut i = desde;
    while i <= u.len() {
        let Some(j) = siguiente(u, i, pb) else {
            return false;
        };
        if let Some(fin) = literal_en(u, j, pb) {
            if sigue(u, fin, mas, ancla_fin) {
                return true;
            }
            // Without an end anchor the earliest match of each piece is enough.
            if !ancla_fin {
                return false;
            }
        }
        i = j + 1;
    }
    false
}

/// Where a piece could next match from `desde`: its first plain character found, or `desde`
/// when it starts with a separator or is empty.
fn siguiente(u: &[u8], desde: usize, pieza: &[u8]) -> Option<usize> {
    match pieza.first() {
        None | Some(b'^') => (desde <= u.len()).then_some(desde),
        Some(c) => u
            .get(desde..)?
            .iter()
            .position(|x| x == c)
            .map(|p| desde + p),
    }
}

/// Whether `pieza` matches at `i`, and where it ends. `^` is any separator or the end of the
/// address.
fn literal_en(u: &[u8], i: usize, pieza: &[u8]) -> Option<usize> {
    let mut k = i;
    for (n, c) in pieza.iter().enumerate() {
        if *c == b'^' {
            match u.get(k) {
                Some(x) if es_separador(*x) => k += 1,
                Some(_) => return None,
                // The end of the address counts as a separator, only for trailing ones.
                None => {
                    if pieza[n..].iter().all(|x| *x == b'^') {
                        return Some(k);
                    }
                    return None;
                }
            }
        } else {
            if u.get(k) != Some(c) {
                return None;
            }
            k += 1;
        }
    }
    Some(k)
}

const fn es_separador(c: u8) -> bool {
    !(c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.' | b'%'))
}

fn casa_dominios(lista: &[(Box<str>, bool)], sitio: &str) -> bool {
    if sitio.is_empty() {
        return lista.iter().all(|(_, neg)| *neg);
    }
    // The page is known by its registrable site only, so a rule for one of its subdomains
    // counts for it when it switches the rule on, never when it switches it off.
    let casa = |d: &str, neg: bool| {
        if let Some(raiz) = d.strip_suffix(".*") {
            // uBlock's «entity»: google.* is any google.<tld>.
            sitio.starts_with(&format!("{raiz}.")) || sitio.contains(&format!(".{raiz}."))
        } else {
            sitio == d
                || sitio.ends_with(&format!(".{d}"))
                || (!neg && d.ends_with(&format!(".{sitio}")))
        }
    };
    let mut hay_positivo = false;
    let mut si = false;
    for (d, neg) in lista {
        if *neg {
            if casa(d, true) {
                return false;
            }
        } else {
            hay_positivo = true;
            if casa(d, false) {
                si = true;
            }
        }
    }
    !hay_positivo || si
}

fn hash(s: &[u8]) -> u64 {
    // FNV-1a.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

const COMUNES: &[&str] = &["http", "https", "www", "com", "net", "org", "html", "js"];

/// The token a rule is filed under: the longest run of letters and digits of its pattern that
/// is whole in any address it matches (bounded on both sides), so looking up the address's own
/// tokens finds it. `None` files it with the rules checked for every request.
fn mejor_token(r: &Regla) -> Option<u64> {
    let mut mejor: Option<&[u8]> = None;
    let n = r.partes.len();
    for (ip, parte) in r.partes.iter().enumerate() {
        let b = parte.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if !b[i].is_ascii_alphanumeric() {
                i += 1;
                continue;
            }
            let ini = i;
            while i < b.len() && b[i].is_ascii_alphanumeric() {
                i += 1;
            }
            let izq = if ini > 0 {
                true
            } else {
                // At the start of a piece: bounded only at the start of the pattern with an
                // anchor (after `*` it could be the middle of a word).
                ip == 0 && (r.ancla_dominio || r.ancla_inicio)
            };
            let der = if i < b.len() {
                true
            } else {
                ip + 1 == n && r.ancla_fin
            };
            let t = &b[ini..i];
            if izq
                && der
                && t.len() >= 2
                && !COMUNES.iter().any(|c| c.as_bytes() == t)
                && mejor.is_none_or(|m| t.len() > m.len())
            {
                mejor = Some(t);
            }
        }
    }
    mejor.map(hash)
}

/// The tokens of an address: its runs of letters and digits.
fn tokens(url: &str) -> Vec<u64> {
    let b = url.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if !b[i].is_ascii_alphanumeric() {
            i += 1;
            continue;
        }
        let ini = i;
        while i < b.len() && b[i].is_ascii_alphanumeric() {
            i += 1;
        }
        out.push(hash(&b[ini..i]));
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Read one line of a list. `None` for comments, cosmetic rules and anything left out.
fn compila(linea: &str, lista: Lista) -> Option<(Regla, bool, Option<String>)> {
    let l = linea.trim();
    if l.is_empty()
        || l.starts_with('!')
        || l.starts_with('[')
        || l.contains("##")
        || l.contains("#@#")
        || l.contains("#?#")
        || l.contains("#$#")
        || l.contains("#%#")
    {
        return None;
    }
    let (excepcion, l) = l.strip_prefix("@@").map_or((false, l), |r| (true, r));
    // Options after the last `$` that is followed by an option name.
    let (patron, opciones) = match l.rfind('$') {
        Some(i)
            if l[i + 1..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '~' || c == '_') =>
        {
            (&l[..i], Some(&l[i + 1..]))
        }
        _ => (l, None),
    };
    // A regular expression: no regex engine here, left out.
    if patron.len() > 2 && patron.starts_with('/') && patron.ends_with('/') {
        return None;
    }
    let mut tipos_si: u16 = 0;
    let mut tipos_no: u16 = 0;
    let mut tercero = None;
    let mut paginas: Vec<(Box<str>, bool)> = Vec::new();
    let mut destinos: Vec<(Box<str>, bool)> = Vec::new();
    let mut importante = false;
    let mut badfilter = false;
    if let Some(o) = opciones {
        for op in o.split(',') {
            let op = op.trim();
            let (neg, nombre) = op.strip_prefix('~').map_or((false, op), |r| (true, r));
            let (nombre, valor) = nombre.split_once('=').map_or((nombre, ""), |(a, b)| (a, b));
            let tipo = match nombre {
                "script" => Some(T_SCRIPT),
                "image" | "img" => Some(T_IMAGE),
                "stylesheet" | "css" => Some(T_STYLE),
                "xmlhttprequest" | "xhr" => Some(T_XHR),
                "subdocument" | "frame" => Some(T_FRAME),
                "media" => Some(T_MEDIA),
                "font" => Some(T_FONT),
                "ping" | "beacon" => Some(T_PING),
                "websocket" => Some(T_SOCKET),
                "other" => Some(T_OTHER),
                "object" | "object-subrequest" => Some(T_OBJECT),
                "popup" => Some(T_POPUP),
                "document" | "doc" => Some(T_DOCUMENT),
                "all" => Some(T_DEFECTO | T_POPUP | T_DOCUMENT),
                _ => None,
            };
            if let Some(t) = tipo {
                if neg {
                    tipos_no |= t;
                } else {
                    tipos_si |= t;
                }
                continue;
            }
            match nombre {
                "third-party" | "3p" => tercero = Some(!neg),
                "first-party" | "1p" => tercero = Some(neg),
                "strict3p" => tercero = Some(true),
                "strict1p" => tercero = Some(false),
                "domain" | "from" => {
                    for d in valor.split('|').filter(|d| !d.is_empty()) {
                        let (n, d) = d.strip_prefix('~').map_or((false, d), |r| (true, r));
                        paginas.push((d.to_ascii_lowercase().into(), n));
                    }
                }
                "to" => {
                    for d in valor.split('|').filter(|d| !d.is_empty()) {
                        let (n, d) = d.strip_prefix('~').map_or((false, d), |r| (true, r));
                        destinos.push((d.to_ascii_lowercase().into(), n));
                    }
                }
                "important" => importante = true,
                "badfilter" => badfilter = true,
                // uBlock blocks these and serves a harmless stand-in: blocking is the safe part.
                "redirect" | "empty" | "mp4" => {}
                // Hints that change nothing for a network block.
                "match-case" | "_" | "__" | "reason" => {}
                _ => return None,
            }
        }
    }
    let tipos = if tipos_si == 0 {
        T_DEFECTO & !tipos_no
    } else {
        tipos_si & !tipos_no
    };
    if tipos == 0 {
        return None;
    }
    let mut p = patron.to_ascii_lowercase();
    let ancla_dominio = p.starts_with("||");
    if ancla_dominio {
        p.drain(..2);
    }
    let ancla_inicio = !ancla_dominio && p.starts_with('|');
    if ancla_inicio {
        p.drain(..1);
    }
    let ancla_fin = p.ends_with('|');
    if ancla_fin {
        p.pop();
    }
    // A host list line («0.0.0.0 host» or a bare host) is a host rule.
    if let Some(h) = p
        .strip_prefix("0.0.0.0 ")
        .or_else(|| p.strip_prefix("127.0.0.1 "))
    {
        p = format!("{h}^");
    }
    // A rule that would match everything is not a rule.
    if p.trim_matches(|c| c == '*' || c == '^').is_empty() && paginas.is_empty() {
        return None;
    }
    let partes: Vec<Box<str>> = p.split('*').map(Into::into).collect::<Vec<Box<str>>>();
    // Leading and trailing `*` mean nothing more than no anchor.
    let partes: Vec<Box<str>> = {
        let mut v = partes;
        while v.len() > 1 && v.last().is_some_and(|x| x.is_empty()) {
            v.pop();
        }
        v
    };
    let r = Regla {
        partes,
        ancla_dominio,
        ancla_inicio,
        ancla_fin,
        tipos,
        tercero,
        paginas: paginas.into_boxed_slice(),
        destinos: destinos.into_boxed_slice(),
        importante,
        lista,
    };
    let quitar = badfilter.then(|| {
        // The same rule without `badfilter`: the one this line switches off.
        let sin: Vec<&str> = opciones
            .unwrap_or("")
            .split(',')
            .filter(|o| o.trim() != "badfilter")
            .collect();
        let base = if excepcion {
            format!("@@{patron}")
        } else {
            patron.to_string()
        };
        if sin.is_empty() {
            base
        } else {
            format!("{base}${}", sin.join(","))
        }
    });
    Some((r, excepcion, quitar))
}

impl Motor {
    /// Compile lists given as text (tests use small ones).
    #[must_use]
    pub fn de(listas: &[(&str, Lista)]) -> Self {
        let mut apagadas = std::collections::HashSet::new();
        for (texto, _) in listas {
            for l in texto.lines() {
                if l.contains("badfilter") {
                    if let Some((_, _, Some(q))) = compila(l, Lista::Anuncios) {
                        apagadas.insert(q);
                    }
                }
            }
        }
        let mut m = Self::default();
        for (texto, lista) in listas {
            for l in texto.lines() {
                let l = l.trim();
                if l.is_empty() || l.starts_with('!') {
                    continue;
                }
                if apagadas.contains(l) {
                    m.omitidas += 1;
                    continue;
                }
                match compila(l, *lista) {
                    Some((_, _, Some(_))) => {}
                    Some((r, true, None)) => {
                        m.leidas += 1;
                        m.excepcion.mete(r);
                    }
                    Some((r, false, None)) => {
                        m.leidas += 1;
                        m.bloqueo.mete(r);
                    }
                    None => m.omitidas += 1,
                }
            }
        }
        m
    }

    /// Whether the lists block this request, and which kind of list. `sitio_pagina` is the
    /// registrable site of the page; `tercero`, whether the destination belongs to another
    /// site. A page the person opens is never blocked here.
    #[must_use]
    pub fn bloquea(
        &self,
        url: &str,
        recurso: Recurso,
        tercero: bool,
        sitio_pagina: &str,
    ) -> Option<Lista> {
        if recurso == Recurso::Documento {
            return None;
        }
        let bajo = url.to_ascii_lowercase();
        let host = rango_host(&bajo);
        let host_destino = &bajo[host.0..host.1];
        let p = Pedido {
            url: &bajo,
            host,
            tipo: tipo_de(recurso),
            tercero,
            sitio_pagina,
            host_destino,
        };
        let t = tokens(&bajo);
        let r = self.bloqueo.busca(&p, &t)?;
        if !r.importante && self.excepcion.busca(&p, &t).is_some() {
            return None;
        }
        Some(r.lista)
    }
}

fn rango_host(url: &str) -> (usize, usize) {
    let ini = url.find("://").map_or(0, |i| i + 3);
    let resto = &url[ini..];
    let fin = resto
        .find(['/', '?', '#', ':'])
        .map_or(url.len(), |i| ini + i);
    // Without the user part of `user@host`.
    let ini = url[ini..fin].rfind('@').map_or(ini, |a| ini + a + 1);
    (ini, fin)
}

/// The lists of this release, compiled once, the first time a request asks.
#[must_use]
pub fn motor() -> &'static Motor {
    static M: OnceLock<Motor> = OnceLock::new();
    M.get_or_init(|| Motor::de(LISTAS))
}

// ----- hiding what the lists know as adverts on the page (cosmetic rules) -----------------

const COSMETICOS: &str = include_str!("../data/filtros/cosmeticos.txt");

/// The hiding rules of EasyList and uBlock filters: CSS selectors of advert boxes, for every
/// site (generic) or for some sites, with their exceptions. Like Brave, ZERO hides them so the
/// page shows no empty boxes where the cut adverts were.
#[derive(Debug, Default)]
pub struct Cosmetica {
    genericas: Vec<Box<str>>,
    /// Generic selectors switched off on some sites («~site##sel», «site#@#sel»).
    excepto: HashMap<Box<str>, Vec<Box<str>>>,
    /// Selectors for some sites only.
    por_sitio: HashMap<Box<str>, Vec<Box<str>>>,
}

impl Cosmetica {
    /// Compile cosmetic rules given as text.
    #[must_use]
    pub fn de(texto: &str) -> Self {
        let mut c = Self::default();
        for l in texto.lines() {
            let l = l.trim();
            if l.is_empty() || l.starts_with('!') {
                continue;
            }
            let (dominios, excepcion, sel) = if let Some((d, s)) = l.split_once("#@#") {
                (d, true, s)
            } else if let Some((d, s)) = l.split_once("##") {
                (d, false, s)
            } else {
                continue;
            };
            let sel: Box<str> = sel.trim().into();
            if sel.is_empty() {
                continue;
            }
            let ds: Vec<(bool, String)> = dominios
                .split(',')
                .map(str::trim)
                .filter(|d| !d.is_empty())
                .map(|d| {
                    d.strip_prefix('~')
                        .map_or((false, d.to_ascii_lowercase()), |r| {
                            (true, r.to_ascii_lowercase())
                        })
                })
                .collect();
            let positivos: Vec<&String> = ds.iter().filter(|(n, _)| !n).map(|(_, d)| d).collect();
            if excepcion {
                // An exception switches a selector off on its sites (a generic one, or one for
                // those sites).
                for d in &positivos {
                    c.excepto
                        .entry((*d).as_str().into())
                        .or_default()
                        .push(sel.clone());
                }
                continue;
            }
            if positivos.is_empty() {
                c.genericas.push(sel.clone());
                for (_, d) in ds.iter().filter(|(n, _)| *n) {
                    c.excepto
                        .entry(d.as_str().into())
                        .or_default()
                        .push(sel.clone());
                }
            } else {
                for d in positivos {
                    c.por_sitio
                        .entry(d.as_str().into())
                        .or_default()
                        .push(sel.clone());
                }
            }
        }
        // Each generic selector once, in the lists' order.
        let mut vistos = std::collections::HashSet::new();
        c.genericas.retain(|s| vistos.insert(s.clone()));
        c
    }

    /// The selectors to hide on a page of `host`: the generic ones and that site's own, without
    /// the ones switched off there.
    #[must_use]
    pub fn para(&self, host: &str) -> Vec<&str> {
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        let mut claves: Vec<String> = Vec::new();
        let mut h = host.as_str();
        loop {
            claves.push(h.to_string());
            if let Some((ini, _)) = h.rsplit_once('.') {
                claves.push(format!("{ini}.*"));
            }
            match h.split_once('.') {
                Some((_, resto)) if resto.contains('.') => h = resto,
                _ => break,
            }
        }
        let mut fuera: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for k in &claves {
            if let Some(v) = self.excepto.get(k.as_str()) {
                fuera.extend(v.iter().map(|s| &**s));
            }
        }
        let mut out: Vec<&str> = self
            .genericas
            .iter()
            .map(|s| &**s)
            .filter(|s| !fuera.contains(s))
            .collect();
        for k in &claves {
            if let Some(v) = self.por_sitio.get(k.as_str()) {
                out.extend(v.iter().map(|s| &**s).filter(|s| !fuera.contains(s)));
            }
        }
        out
    }
}

/// The hiding rules of this release, compiled once.
#[must_use]
pub fn cosmetica() -> &'static Cosmetica {
    static C: OnceLock<Cosmetica> = OnceLock::new();
    C.get_or_init(|| Cosmetica::de(COSMETICOS))
}

/// The script that hides a page's advert boxes: it keeps the selectors this engine accepts
/// (`CSS.supports`), so one the engine does not know cannot switch off the rest, and adds them
/// as one style sheet.
#[must_use]
pub fn guion_ocultar(host: &str) -> Option<String> {
    let s = cosmetica().para(host);
    if s.is_empty() {
        return None;
    }
    let lista = serde_json::to_string(&s).ok()?;
    Some(format!(
        "(()=>{{try{{if(document.querySelector('style[data-guardiana-zero]'))return;const s={lista};const ok=s.filter(x=>{{try{{return CSS.supports('selector('+x+')')}}catch(e){{return false}}}});if(!ok.length)return;const t=[];for(let i=0;i<ok.length;i+=200)t.push(ok.slice(i,i+200).join(',\\n')+'{{display:none!important}}');const e=document.createElement('style');e.setAttribute('data-guardiana-zero','');e.textContent=t.join('\\n');(document.head||document.documentElement).appendChild(e)}}catch(e){{}}}})()"
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn m(reglas: &str) -> Motor {
        Motor::de(&[(reglas, Lista::Anuncios)])
    }

    #[test]
    fn the_rule_syntax_matches_like_ublock() {
        let e = m("||ads.example.com^\n/banner/*/img^\n||tracker.io/pixel.gif|\n@@||ads.example.com/ok^\n|https://bad.test/\n||x.com^$third-party\n||y.com^$domain=a.com|~b.a.com\n||z.com^$script\n||w.com^$important\n@@||w.com^\n||p.com^$popup");
        let b = |u: &str, r: Recurso, t: bool, s: &str| e.bloquea(u, r, t, s).is_some();
        assert!(b(
            "https://ads.example.com/x.js",
            Recurso::Script,
            true,
            "news.co"
        ));
        assert!(b(
            "https://sub.ads.example.com/",
            Recurso::Imagen,
            true,
            "news.co"
        ));
        assert!(!b(
            "https://badads.example.com/",
            Recurso::Imagen,
            true,
            "news.co"
        ));
        assert!(!b(
            "https://ads.example.com/ok/1",
            Recurso::Imagen,
            true,
            "news.co"
        ));
        assert!(b(
            "https://cdn.news.co/banner/300/img?x=1",
            Recurso::Imagen,
            false,
            "news.co"
        ));
        assert!(b(
            "https://tracker.io/pixel.gif",
            Recurso::Imagen,
            true,
            "news.co"
        ));
        assert!(!b(
            "https://tracker.io/pixel.gif?x=1",
            Recurso::Imagen,
            true,
            "news.co"
        ));
        assert!(b("https://bad.test/a", Recurso::Script, true, "news.co"));
        assert!(!b("https://x.com/a", Recurso::Script, false, "x.com"));
        assert!(b("https://x.com/a", Recurso::Script, true, "news.co"));
        assert!(b("https://y.com/a", Recurso::Imagen, true, "a.com"));
        assert!(!b("https://y.com/a", Recurso::Imagen, true, "b.a.com"));
        assert!(!b("https://y.com/a", Recurso::Imagen, true, "c.com"));
        assert!(b("https://z.com/a.js", Recurso::Script, true, "c.com"));
        assert!(!b("https://z.com/a.png", Recurso::Imagen, true, "c.com"));
        assert!(b("https://w.com/a", Recurso::Imagen, true, "c.com"));
        // A page the person opens is never blocked; a popup only by a popup rule.
        assert!(!b("https://ads.example.com/", Recurso::Documento, true, ""));
        assert!(b("https://p.com/", Recurso::Ventana, true, "c.com"));
        assert!(!b(
            "https://ads.example.com/",
            Recurso::Ventana,
            true,
            "c.com"
        ));
    }

    #[test]
    fn hiding_rules_are_generic_or_per_site_with_their_exceptions() {
        let c = Cosmetica::de("##.ad-box\n###banner\nnews.co##.patrocinado\n~shop.co##.ad-box\nblog.io#@##banner\ngoogle.*##.g-ad");
        let en = |h: &str| {
            c.para(h)
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
        };
        assert_eq!(en("www.otra.org"), vec![".ad-box", "#banner"]);
        assert!(en("www.news.co").contains(&".patrocinado".to_string()));
        assert!(!en("shop.co").contains(&".ad-box".to_string()));
        assert!(!en("blog.io").contains(&"#banner".to_string()));
        assert!(en("www.google.com").contains(&".g-ad".to_string()));
        let g = guion_ocultar("www.example.org").unwrap();
        assert!(g.contains("CSS.supports") && g.contains("display:none!important"));
        assert!(cosmetica().para("example.org").len() > 5_000);
    }

    #[test]
    fn badfilter_and_unknown_options_take_rules_out() {
        let e = m("||a.com^\n||a.com^$badfilter\n||b.com^$csp=script-src 'none'\n/^https?:\\/\\/re\\.gex/\n||c.com^$redirect=noop.js,script");
        assert!(e
            .bloquea("https://a.com/", Recurso::Script, true, "x.co")
            .is_none());
        assert!(e
            .bloquea("https://b.com/", Recurso::Script, true, "x.co")
            .is_none());
        assert!(e
            .bloquea("https://c.com/x.js", Recurso::Script, true, "x.co")
            .is_some());
    }

    #[test]
    fn the_real_lists_block_what_brave_blocks() {
        let e = motor();
        assert!(e.leidas > 100_000, "{}", e.leidas);
        let b = |u: &str, r: Recurso, t: bool, s: &str| e.bloquea(u, r, t, s);
        assert!(b(
            "https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js",
            Recurso::Script,
            true,
            "eltiempo.com"
        )
        .is_some());
        assert!(b(
            "https://www.google-analytics.com/analytics.js",
            Recurso::Script,
            true,
            "eltiempo.com"
        )
        .is_some());
        assert!(b(
            "https://connect.facebook.net/en_US/fbevents.js",
            Recurso::Script,
            true,
            "eltiempo.com"
        )
        .is_some());
        assert!(b(
            "https://static.hotjar.com/c/hotjar-1.js",
            Recurso::Script,
            true,
            "eltiempo.com"
        )
        .is_some());
        // An ad path on the page's own site is blocked too, as Brave does.
        assert!(b(
            "https://www.eltiempo.com/ads/banners/728.gif",
            Recurso::Imagen,
            false,
            "eltiempo.com"
        )
        .is_some());
        assert!(b(
            "https://www.eltiempo.com/ads/custom_ads.js",
            Recurso::Script,
            false,
            "eltiempo.com"
        )
        .is_some());
        // Ordinary things pass.
        assert!(b(
            "https://www.eltiempo.com/static/app.js",
            Recurso::Script,
            false,
            "eltiempo.com"
        )
        .is_none());
        assert!(b(
            "https://fonts.gstatic.com/s/roboto/v30/a.woff2",
            Recurso::Fuente,
            true,
            "eltiempo.com"
        )
        .is_none());
        assert!(b(
            "https://upload.wikimedia.org/wikipedia/commons/a/a9/x.jpg",
            Recurso::Imagen,
            true,
            "wikipedia.org"
        )
        .is_none());
    }
}

#[cfg(test)]
mod medida {
    #[test]
    #[ignore = "a measure, not a check"]
    fn cuanto_tarda() {
        let t0 = std::time::Instant::now();
        let e = super::motor();
        let comp = t0.elapsed();
        let urls = [
            "https://www.eltiempo.com/static/app.js",
            "https://securepubads.g.doubleclick.net/tag/js/gpt.js",
            "https://cdn.eltiempo.com/img/2026/10/13/foto-principal-1200x800.jpg?w=800&q=80",
            "https://www.google-analytics.com/g/collect?v=2&tid=G-1&cid=1.2&dl=https%3A%2F%2Feltiempo.com",
            "https://fonts.gstatic.com/s/roboto/v30/a.woff2",
        ];
        let t1 = std::time::Instant::now();
        let mut n = 0;
        for _ in 0..2000 {
            for u in urls {
                if e.bloquea(u, crate::mandato::Recurso::Script, true, "eltiempo.com")
                    .is_some()
                {
                    n += 1;
                }
            }
        }
        println!(
            "compilar {:?}; {} reglas, {} fuera, {} sin token; 10000 consultas {:?} ({n} bloqueos)",
            comp,
            e.leidas,
            e.omitidas,
            e.bloqueo.sin_token.len() + e.excepcion.sin_token.len(),
            t1.elapsed()
        );
    }
}
