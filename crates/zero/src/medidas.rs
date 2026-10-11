//! «Lo que ZERO midió en esta página» (decision 206): four things a page does that the person
//! cannot see, measured from what the browser itself saw. Nothing here cuts anything or changes
//! what is cut: it counts, and says it in one sentence each.
//!
//! - **La huella.** Reads that serve to recognise a computer without cookies (a canvas with text
//!   on it read back, the graphics card's names through WebGL, audio rendered off screen, many
//!   fonts measured), reported by the page script ([`crate::paginas`]) with the addresses on the
//!   call stack: the first one says whose script asked. Always «intentó»: a read is not a
//!   recognition, and some webs read for other reasons (drawing, games).
//! - **Grabación de sesiones.** A request to one of the services of `data/grabacion.txt`, and
//!   whether one went after the person first typed in a field of the page.
//! - **Datos cifrados en la dirección.** A request to another company whose address carries, in
//!   a parameter made for that (`ud[em]`, `pd[em]`, `em=tv.1~em.…`), a value shaped like a hash.
//!   Only the fact and the company are kept, never the value.
//! - **Envíos con datos.** Requests with a body to other companies, and their size when the
//!   browser read the body; without it they count without a size, never a guessed one.
//!
//! Everything lives in memory (a tab's shield, and the session's total): nothing is written to
//! disk.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use serde::Serialize;

use crate::chivatos::es_hash;
use crate::destino::es_tercero;
use crate::dominio::{bajo_sufijo_privado, normaliza_host, sitio};
use crate::textos::{Idioma, Textos};

const GRABACION: &str = include_str!("../data/grabacion.txt");

/// The longest body the shell reads (`zero/navegador`, `lee_cuerpo`): one this long may have
/// been longer, and its size is said as «al menos».
pub const MAX_CUERPO: usize = 2 * 1024 * 1024;

/// Bounds against a page that calls endless names: authors and scripts per page, companies per
/// page, webs and companies in the session.
const MAX_AUTORES: usize = 50;
const MAX_SCRIPTS: usize = 20;
const MAX_EMPRESAS: usize = 200;
const MAX_WEBS: usize = 5_000;
/// Companies named in the session's sentence about hashed data.
const EMPRESAS_EN_FRASE: usize = 5;

/// A kind of read that serves to recognise the computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tecnica {
    /// A canvas with text drawn on it, read back (`toDataURL`, `toBlob`, `getImageData`).
    Canvas,
    /// The graphics card's own names through WebGL (`UNMASKED_VENDOR_WEBGL`,
    /// `UNMASKED_RENDERER_WEBGL`).
    Webgl,
    /// Audio rendered off screen (`OfflineAudioContext.startRendering`).
    Audio,
    /// Many font families measured on a canvas (`measureText`).
    Fuentes,
}

impl Tecnica {
    /// Its key, in the page script's messages and in the texts (`medida_tecnica_<clave>`).
    #[must_use]
    pub const fn clave(self) -> &'static str {
        match self {
            Self::Canvas => "canvas",
            Self::Webgl => "webgl",
            Self::Audio => "audio",
            Self::Fuentes => "fuentes",
        }
    }

    /// From its key.
    #[must_use]
    pub fn de_clave(s: &str) -> Option<Self> {
        Some(match s {
            "canvas" => Self::Canvas,
            "webgl" => Self::Webgl,
            "audio" => Self::Audio,
            "fuentes" => Self::Fuentes,
            _ => return None,
        })
    }
}

/// Whose script asked for a read.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Autor {
    /// A script of another company, by its registrable site.
    Fuera(String),
    /// The page's own web (an inline script, one of its files, or a name of the same owner).
    Propia,
    /// The stack said nothing usable (the page can shorten or rewrite it).
    Desconocido,
}

/// Whose script asked, from the addresses the page script found on the call stack, closest
/// first: the first web address is the script that called. Also returns that script's address
/// without its query (kept in memory to count scripts, never shown).
#[must_use]
pub fn autor(direcciones: &[&str], sitio_pagina: &str) -> (Autor, Option<String>) {
    for d in direcciones.iter().take(8) {
        let Ok(u) = url::Url::parse(d) else {
            continue;
        };
        if !matches!(u.scheme(), "http" | "https") {
            continue;
        }
        let Some(h) = u.host_str() else {
            continue;
        };
        let host = normaliza_host(h);
        if host.is_empty() {
            continue;
        }
        let script: String = format!("{}://{}{}", u.scheme(), host, u.path())
            .chars()
            .take(300)
            .collect();
        let a = if !sitio_pagina.is_empty() && !es_tercero(&host, sitio_pagina) {
            Autor::Propia
        } else {
            Autor::Fuera(sitio(&host))
        };
        return (a, Some(script));
    }
    (Autor::Desconocido, None)
}

static GRABADORES: OnceLock<Vec<(String, &'static str)>> = OnceLock::new();

fn grabadores() -> &'static [(String, &'static str)] {
    GRABADORES.get_or_init(|| {
        GRABACION
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .filter_map(|l| {
                let (nombre, servicio) = l.split_once('|')?;
                let nombre = nombre.trim().to_ascii_lowercase();
                let servicio = servicio.trim();
                (!nombre.is_empty() && !servicio.is_empty()).then_some((nombre, servicio))
            })
            .collect()
    })
}

/// The session recording service a request to `host` goes to, if it is one of
/// `data/grabacion.txt`: that name or any name under it.
#[must_use]
pub fn grabador(host: &str) -> Option<&'static str> {
    let host = host.trim_end_matches('.');
    grabadores()
        .iter()
        .find(|(n, _)| {
            host.strip_suffix(n.as_str())
                .is_some_and(|r| r.is_empty() || r.ends_with('.'))
        })
        .map(|(_, s)| *s)
}

/// What travels hashed in an address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Cifrado {
    /// An email.
    Correo,
    /// A phone.
    Telefono,
    /// A first or last name.
    Nombre,
}

impl Cifrado {
    /// Its key in the texts (`medida_dato_<clave>`).
    #[must_use]
    pub const fn clave(self) -> &'static str {
        match self {
            Self::Correo => "correo",
            Self::Telefono => "telefono",
            Self::Nombre => "nombre",
        }
    }
}

/// The parameters made to carry a hashed email, phone or name, and what each carries.
/// Sources (11 Oct 2026): Meta's «Advanced Matching» (`ud[em]`, `ud[fn]`, `ud[ln]`, `ud[ph]`;
/// `udff` is the copy its own script sends), Pinterest's «Enhanced match» (`pd[em]`), Google's
/// enhanced conversions (`em=tv.1~em.…`, read apart), the names of Google's and Snap's tags for
/// the same (`sha256_email_address`, `user_hashed_email`…), Snap's pixel requests (`u_hem`,
/// `u_hpn`, as `chivatos` reads them), and the names that say it themselves (`hashed_email`,
/// `email_sha256`, `hem`…). Only with a value shaped like a hash: the name alone says nothing.
fn parametro(k: &str) -> Option<Cifrado> {
    Some(match k {
        "ud[em]"
        | "udff[em]"
        | "pd[em]"
        | "u_hem"
        | "sha256_email_address"
        | "user_hashed_email"
        | "hashed_email"
        | "email_sha256"
        | "sha256_email"
        | "email_hash"
        | "md5_email"
        | "hem" => Cifrado::Correo,
        "ud[ph]"
        | "udff[ph]"
        | "u_hpn"
        | "sha256_phone_number"
        | "user_hashed_phone_number"
        | "hashed_phone"
        | "phone_sha256"
        | "sha256_phone" => Cifrado::Telefono,
        "ud[fn]" | "ud[ln]" | "udff[fn]" | "udff[ln]" => Cifrado::Nombre,
        _ => return None,
    })
}

/// What an address's query carries hashed, by the parameter's name and the value's shape
/// (SHA-256, SHA-1 or MD5 in hex, or SHA-256 in base64). The values are looked at here and
/// dropped: nothing of them is kept.
#[must_use]
pub fn cifrados(consulta: &str) -> BTreeSet<Cifrado> {
    let mut out = BTreeSet::new();
    for (k, v) in url::form_urlencoded::parse(consulta.as_bytes()).take(300) {
        let v = v.trim();
        if k == "em" {
            // Google's enhanced conversions: `tv.1~em.<hash>~pn.<hash>`.
            if let Some(resto) = v.strip_prefix("tv.1~") {
                for trozo in resto.split('~') {
                    if let Some(h) = trozo.strip_prefix("em.") {
                        if es_hash(h) {
                            out.insert(Cifrado::Correo);
                        }
                    } else if let Some(h) = trozo.strip_prefix("pn.") {
                        if es_hash(h) {
                            out.insert(Cifrado::Telefono);
                        }
                    }
                }
            }
            continue;
        }
        if let Some(c) = parametro(&k) {
            if es_hash(v) {
                out.insert(c);
            }
        }
    }
    out
}

/// One request to another company, as the browser decided it.
#[derive(Debug, Clone, Copy)]
pub struct Vista<'a> {
    /// Its host, normalised.
    pub host: &'a str,
    /// Its query, without the `?`.
    pub consulta: Option<&'a str>,
    /// The company (or the site) behind it, as the shield names it.
    pub quien: &'a str,
    /// It carries a body (POST, PUT, PATCH).
    pub con_cuerpo: bool,
    /// The body's size, when the browser read it. `None`: not known (never guessed).
    pub bytes: Option<u64>,
    /// It was cut: it never left.
    pub cortada: bool,
    /// It was cut by maximum protection for being a session recording service.
    pub por_grabacion: bool,
}

/// What one request was, of the things measured here.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hallado {
    /// The session recording service it goes to.
    pub grabador: Option<&'static str>,
    /// What its address carries hashed.
    pub cifrados: BTreeSet<Cifrado>,
}

/// Look at one request to another company.
#[must_use]
pub fn mira(v: &Vista<'_>) -> Hallado {
    Hallado {
        grabador: grabador(v.host),
        cifrados: v
            .consulta
            .filter(|q| !q.is_empty())
            .map(cifrados)
            .unwrap_or_default(),
    }
}

/// Requests with a body, on one side (they left, or they were cut).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Envios {
    /// How many.
    pub n: u32,
    /// Of them, how many with a known size.
    pub medidos: u32,
    /// The bytes of those.
    pub bytes: u64,
    /// One was as long as the browser reads: the total is a floor.
    pub al_menos: bool,
}

impl Envios {
    fn anota(&mut self, bytes: Option<u64>) {
        self.n = self.n.saturating_add(1);
        if let Some(b) = bytes {
            self.medidos = self.medidos.saturating_add(1);
            self.bytes = self.bytes.saturating_add(b);
            if b >= MAX_CUERPO as u64 {
                self.al_menos = true;
            }
        }
    }
}

/// Requests with a body to other companies.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Subida {
    /// Those that left.
    pub salieron: Envios,
    /// Those that were cut before leaving.
    pub cortados: Envios,
}

impl Subida {
    fn anota(&mut self, v: &Vista<'_>) {
        if !v.con_cuerpo {
            return;
        }
        if v.cortada {
            self.cortados.anota(v.bytes);
        } else {
            self.salieron.anota(v.bytes);
        }
    }
}

/// The reads of one author on a page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Huella {
    tecnicas: BTreeSet<Tecnica>,
    /// The kinds for which the script got a different value (maximum protection).
    ruido: BTreeSet<Tecnica>,
    scripts: BTreeSet<String>,
}

/// One session recording service on a page.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Grabacion {
    peticiones: u32,
    cortadas: u32,
    /// Cut by maximum protection for being a recording service.
    por_maxima: u32,
    tras_escribir: u32,
    tras_escribir_cortadas: u32,
}

/// What one company got (or would have got) hashed in addresses on a page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Cifrados {
    que: BTreeSet<Cifrado>,
    pasaron: u32,
    cortadas: u32,
}

/// One line of what was measured, for the shield.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Linea {
    /// `huella`, `grabacion`, `cifrado` or `subida`.
    pub clase: &'static str,
    /// The sentence, in the person's language.
    pub texto: String,
    /// `cortado` (every attempt of it was cut), `paso` (something of it got through) or `""`
    /// (measured only: the reads happen inside the page, there is nothing to cut).
    pub estado: &'static str,
    /// The sentence already says its fate («cortado: …»): the panel does not repeat it.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub dicho: bool,
}

/// What was measured on one page (a tab's shield; a new page starts it again).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pagina {
    huellas: BTreeMap<Autor, Huella>,
    grabadores: BTreeMap<&'static str, Grabacion>,
    escribio: bool,
    cifrados: BTreeMap<String, Cifrados>,
    subida: Subida,
}

impl Pagina {
    /// The person typed in a field of this page. Returns whether it is the first time.
    pub fn escribe(&mut self) -> bool {
        !std::mem::replace(&mut self.escribio, true)
    }

    /// A read of kind `t` by `autor` (and its script, when known); `ruido`: the script got a
    /// different value than the real one. Returns whether it says something new (a new kind,
    /// a new script, or a kind newly given a different value).
    pub fn huella(
        &mut self,
        t: Tecnica,
        autor: Autor,
        script: Option<String>,
        ruido: bool,
    ) -> bool {
        if self.huellas.len() >= MAX_AUTORES && !self.huellas.contains_key(&autor) {
            return false;
        }
        let h = self.huellas.entry(autor).or_default();
        let mut nuevo = h.tecnicas.insert(t);
        if ruido {
            nuevo |= h.ruido.insert(t);
        }
        if let Some(s) = script {
            if h.scripts.len() < MAX_SCRIPTS && h.scripts.insert(s) {
                nuevo = true;
            }
        }
        nuevo
    }

    /// One request to another company, with what [`mira`] found in it.
    pub fn anota(&mut self, v: &Vista<'_>, h: &Hallado) {
        if let Some(g) = h.grabador {
            let e = self.grabadores.entry(g).or_default();
            e.peticiones = e.peticiones.saturating_add(1);
            if v.cortada {
                e.cortadas = e.cortadas.saturating_add(1);
            }
            if v.por_grabacion {
                e.por_maxima = e.por_maxima.saturating_add(1);
            }
            if self.escribio {
                e.tras_escribir = e.tras_escribir.saturating_add(1);
                if v.cortada {
                    e.tras_escribir_cortadas = e.tras_escribir_cortadas.saturating_add(1);
                }
            }
        }
        if !h.cifrados.is_empty()
            && (self.cifrados.len() < MAX_EMPRESAS || self.cifrados.contains_key(v.quien))
        {
            let e = self.cifrados.entry(v.quien.to_string()).or_default();
            e.que.extend(h.cifrados.iter().copied());
            if v.cortada {
                e.cortadas = e.cortadas.saturating_add(1);
            } else {
                e.pasaron = e.pasaron.saturating_add(1);
            }
        }
        self.subida.anota(v);
    }

    /// Nothing was measured.
    #[must_use]
    pub fn vacia(&self) -> bool {
        self.lineas(&Textos::de(Idioma::Es)).is_empty()
    }

    /// What was measured, one sentence per thing.
    #[must_use]
    pub fn lineas(&self, t: &Textos) -> Vec<Linea> {
        let mut out = Vec::new();
        if !self.huellas.is_empty() {
            let scripts: u64 = self
                .huellas
                .values()
                .map(|h| u64::try_from(h.scripts.len().max(1)).unwrap_or(u64::MAX))
                .sum();
            // «Le dimos una distinta» only where it really happened: for the whole sentence when
            // every read of every script got a different value, else per script.
            let todos = self.huellas.values().all(|h| h.ruido == h.tecnicas);
            let lista = self
                .huellas
                .iter()
                .map(|(a, h)| {
                    let ruido = if todos || h.ruido.is_empty() {
                        String::new()
                    } else if h.ruido == h.tecnicas {
                        t.t("medida_huella_ruido_autor")
                    } else {
                        t.tf(
                            "medida_huella_ruido_parte",
                            &[("tecnicas", &tecnicas(&h.ruido, t))],
                        )
                    };
                    format!(
                        "{} ({}{ruido})",
                        nombre_autor(a, t),
                        tecnicas(&h.tecnicas, t)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            out.push(Linea {
                clase: "huella",
                texto: t.tn(
                    if todos {
                        "medida_huella_ruido"
                    } else {
                        "medida_huella"
                    },
                    scripts,
                    &[("n", &t.numero(scripts)), ("lista", &lista)],
                ),
                estado: "",
                dicho: false,
            });
        }
        for (g, e) in &self.grabadores {
            // If something of it got through once, it got through.
            let paso = e.cortadas < e.peticiones;
            if !paso && e.por_maxima > 0 {
                // Maximum protection cut it for what it is (decision 206).
                let mut texto = t.tf("medida_grabacion_maxima", &[("servicio", g)]);
                if e.tras_escribir > 0 {
                    texto.push_str(&t.t("medida_grabacion_maxima_tras_escribir"));
                }
                out.push(Linea {
                    clase: "grabacion",
                    texto,
                    estado: "cortado",
                    dicho: true,
                });
                continue;
            }
            let mut texto = t.tf(
                if paso {
                    "medida_grabacion"
                } else {
                    "medida_grabacion_cortada"
                },
                &[("servicio", g)],
            );
            if e.tras_escribir > e.tras_escribir_cortadas {
                texto.push_str(&t.t("medida_grabacion_tras_escribir"));
            } else if e.tras_escribir > 0 {
                texto.push_str(&t.t("medida_grabacion_tras_escribir_cortada"));
            }
            out.push(Linea {
                clase: "grabacion",
                texto,
                estado: if paso { "paso" } else { "cortado" },
                dicho: false,
            });
        }
        for (q, e) in &self.cifrados {
            let paso = e.pasaron > 0;
            let que = lista_y(
                e.que
                    .iter()
                    .map(|c| t.t(&format!("medida_dato_{}", c.clave())))
                    .collect(),
                t,
            );
            out.push(Linea {
                clase: "cifrado",
                texto: t.tf(
                    if paso {
                        "medida_cifrado"
                    } else {
                        "medida_cifrado_cortado"
                    },
                    &[("empresa", q), ("que", &que)],
                ),
                estado: if paso { "paso" } else { "cortado" },
                dicho: false,
            });
        }
        for (e, clave, estado) in [
            (&self.subida.salieron, "medida_subida", "paso"),
            (&self.subida.cortados, "medida_subida_cortada", "cortado"),
        ] {
            if e.n > 0 {
                let n = u64::from(e.n);
                out.push(Linea {
                    clase: "subida",
                    texto: t.tn(clave, n, &[("n", &t.numero(n)), ("tam", &tamano_de(e, t))]),
                    estado,
                    dicho: false,
                });
            }
        }
        out
    }

    /// What each kind of line means and what it cannot see, once per kind shown.
    #[must_use]
    pub fn notas(&self, t: &Textos) -> Vec<String> {
        let mut out = Vec::new();
        if !self.huellas.is_empty() {
            out.push(t.t("medida_nota_huella"));
        }
        if self.huellas.values().any(|h| !h.ruido.is_empty()) {
            out.push(t.t("medida_nota_ruido"));
        }
        if !self.grabadores.is_empty() {
            out.push(t.t("medida_nota_grabacion"));
        }
        if !self.cifrados.is_empty() {
            out.push(t.t("medida_nota_cifrado"));
        }
        if self.subida.salieron.n > 0 || self.subida.cortados.n > 0 {
            out.push(t.t("medida_nota_subida"));
        }
        out
    }
}

/// What was measured since the browser opened, across the person's own webs (never an
/// isolated tab nor a mandate's): in memory, gone when the browser closes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Total {
    huella_webs: BTreeSet<String>,
    huella_ruido: BTreeSet<String>,
    huella_tecnicas: BTreeSet<Tecnica>,
    grabacion: BTreeMap<&'static str, BTreeSet<String>>,
    cifrados: BTreeMap<String, BTreeSet<String>>,
    subida: Subida,
}

impl Total {
    /// A read of kind `t` on a page of `web`; `ruido`: the script got a different value.
    pub fn huella(&mut self, web: &str, t: Tecnica, ruido: bool) {
        self.huella_tecnicas.insert(t);
        if !web.is_empty() && self.huella_webs.len() < MAX_WEBS {
            self.huella_webs.insert(web.to_string());
            if ruido {
                self.huella_ruido.insert(web.to_string());
            }
        }
    }

    /// One request to another company from a page of `web`.
    pub fn anota(&mut self, web: &str, v: &Vista<'_>, h: &Hallado) {
        if let Some(g) = h.grabador {
            let w = self.grabacion.entry(g).or_default();
            if !web.is_empty() && w.len() < MAX_WEBS {
                w.insert(web.to_string());
            }
        }
        if !h.cifrados.is_empty()
            && (self.cifrados.len() < MAX_EMPRESAS || self.cifrados.contains_key(v.quien))
        {
            let w = self.cifrados.entry(v.quien.to_string()).or_default();
            if !web.is_empty() && w.len() < MAX_WEBS {
                w.insert(web.to_string());
            }
        }
        self.subida.anota(v);
    }

    /// The session in a few sentences.
    #[must_use]
    pub fn lineas(&self, t: &Textos) -> Vec<Linea> {
        let mut out = Vec::new();
        if !self.huella_tecnicas.is_empty() {
            let n = u64::try_from(self.huella_webs.len()).unwrap_or(u64::MAX);
            let mut texto = t.tn(
                "medida_sesion_huella",
                n,
                &[
                    ("n", &t.numero(n)),
                    ("tecnicas", &tecnicas(&self.huella_tecnicas, t)),
                ],
            );
            if !self.huella_ruido.is_empty() {
                let m = u64::try_from(self.huella_ruido.len()).unwrap_or(u64::MAX);
                texto.push_str(&t.tf("medida_sesion_huella_ruido", &[("m", &t.numero(m))]));
            }
            out.push(Linea {
                clase: "huella",
                texto,
                estado: "",
                dicho: false,
            });
        }
        if !self.grabacion.is_empty() {
            let webs: BTreeSet<&String> = self.grabacion.values().flatten().collect();
            let n = u64::try_from(webs.len()).unwrap_or(u64::MAX);
            let servicios = lista_y(self.grabacion.keys().map(|s| (*s).to_string()).collect(), t);
            out.push(Linea {
                clase: "grabacion",
                texto: t.tn(
                    "medida_sesion_grabacion",
                    n,
                    &[("n", &t.numero(n)), ("servicios", &servicios)],
                ),
                estado: "",
                dicho: false,
            });
        }
        if !self.cifrados.is_empty() {
            let n = u64::try_from(self.cifrados.len()).unwrap_or(u64::MAX);
            // The companies seen on most webs first.
            let mut empresas: Vec<(&String, usize)> =
                self.cifrados.iter().map(|(q, w)| (q, w.len())).collect();
            empresas.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
            let mut nombres: Vec<String> = empresas
                .iter()
                .take(EMPRESAS_EN_FRASE)
                .map(|(q, _)| (*q).clone())
                .collect();
            if empresas.len() > EMPRESAS_EN_FRASE {
                nombres.push(t.t("medida_y_otras"));
            }
            out.push(Linea {
                clase: "cifrado",
                texto: t.tn(
                    "medida_sesion_cifrado",
                    n,
                    &[("n", &t.numero(n)), ("empresas", &lista_y(nombres, t))],
                ),
                estado: "",
                dicho: false,
            });
        }
        let s = &self.subida;
        if s.salieron.n > 0 || s.cortados.n > 0 {
            out.push(Linea {
                clase: "subida",
                texto: t.tf(
                    "medida_sesion_subida",
                    &[
                        ("salieron", &t.numero(u64::from(s.salieron.n))),
                        ("tam", &tamano_de(&s.salieron, t)),
                        ("cortados", &t.numero(u64::from(s.cortados.n))),
                    ],
                ),
                estado: "",
                dicho: false,
            });
        }
        out
    }
}

/// The seed of the different values a page of `ambito` gets (maximum protection): the same
/// for every read of that site while the browser stays open, another for another site, another
/// after a restart. An HMAC of the site under a secret of this run, which never leaves memory.
#[must_use]
pub fn semilla(secreto: &[u8], ambito: &str) -> u32 {
    let k = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, secreto);
    let t = ring::hmac::sign(&k, ambito.as_bytes());
    let b = t.as_ref();
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

static DUENOS: OnceLock<BTreeMap<&'static str, Vec<&'static str>>> = OnceLock::new();

/// The names the page's own site counts as its own (its owner's other names, by the lists of
/// owners), so the page script never gives them a different value: the page's site first, at
/// most 400. The same rule as [`es_tercero`], which the shield uses.
#[must_use]
pub fn propios(sitio_pagina: &str) -> Vec<String> {
    if sitio_pagina.is_empty() {
        return Vec::new();
    }
    let mut out = vec![sitio_pagina.to_string()];
    if bajo_sufijo_privado(sitio_pagina) {
        return out;
    }
    let Some(dueno) = guardiana_lists::company_of(sitio_pagina) else {
        return out;
    };
    let duenos = DUENOS.get_or_init(|| {
        let mut m: BTreeMap<&'static str, Vec<&'static str>> = BTreeMap::new();
        for (seccion, dominio) in guardiana_lists::parse_guardiana(guardiana_lists::EMPRESAS) {
            if let Some(s) = seccion {
                m.entry(s.split('|').next().unwrap_or(s))
                    .or_default()
                    .push(dominio);
            }
        }
        m
    });
    for d in duenos.get(dueno).map(Vec::as_slice).unwrap_or_default() {
        if out.len() >= 400 {
            break;
        }
        if *d != sitio_pagina && !es_tercero(d, sitio_pagina) {
            out.push((*d).to_string());
        }
    }
    out
}

/// How an author is named.
fn nombre_autor(a: &Autor, t: &Textos) -> String {
    match a {
        Autor::Fuera(s) => s.clone(),
        Autor::Propia => t.t("medida_autor_propia"),
        Autor::Desconocido => t.t("medida_autor_desconocido"),
    }
}

/// The kinds of read, in words, in a fixed order.
fn tecnicas(ts: &BTreeSet<Tecnica>, t: &Textos) -> String {
    ts.iter()
        .map(|x| t.t(&format!("medida_tecnica_{}", x.clave())))
        .collect::<Vec<_>>()
        .join(", ")
}

/// `a`, `a y b`, `a, b y c` in the person's language.
fn lista_y(mut v: Vec<String>, t: &Textos) -> String {
    let Some(ultimo) = v.pop() else {
        return String::new();
    };
    if v.is_empty() {
        return ultimo;
    }
    format!("{}{}{ultimo}", v.join(", "), t.t("medida_y"))
}

/// The size of some requests with a body, as far as it is known.
fn tamano_de(e: &Envios, t: &Textos) -> String {
    if e.medidos == 0 {
        return t.t("medida_subida_sin_tam");
    }
    let b = tamano(e.bytes, t.idioma());
    let b = if e.al_menos {
        t.tf("medida_al_menos", &[("tam", &b)])
    } else {
        b
    };
    if e.medidos >= e.n {
        b
    } else {
        t.tf(
            "medida_subida_parte",
            &[("tam", &b), ("m", &t.numero(u64::from(e.medidos)))],
        )
    }
}

/// A size in bytes, written the person's way: `812 B`, `23,4 KB`, `1,2 MB` (binary units, one
/// decimal, a comma in Spanish and Portuguese).
#[must_use]
pub fn tamano(bytes: u64, idioma: Idioma) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    let s = if bytes < KB {
        return format!("{bytes} B");
    } else if bytes < MB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    };
    match idioma {
        Idioma::En => s,
        Idioma::Es | Idioma::Pt => s.replacen('.', ",", 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn sha(s: &str) -> String {
        Sha256::digest(s.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    fn vista<'a>(host: &'a str, consulta: Option<&'a str>, cortada: bool) -> Vista<'a> {
        Vista {
            host,
            consulta,
            quien: host,
            con_cuerpo: false,
            bytes: None,
            cortada,
            por_grabacion: false,
        }
    }

    #[test]
    fn the_author_is_the_first_web_address_on_the_stack() {
        let (a, s) = autor(
            &[
                "https://cdn.tracker.example/fp/v3.js",
                "https://www.tienda.co/app.js",
            ],
            "tienda.co",
        );
        assert_eq!(a, Autor::Fuera("tracker.example".into()));
        assert_eq!(s.as_deref(), Some("https://cdn.tracker.example/fp/v3.js"));
        // An inline script is the page's own address: the web itself.
        let (a, _) = autor(&["https://www.tienda.co/carrito?x=1"], "tienda.co");
        assert_eq!(a, Autor::Propia);
        // The query is not kept with the script.
        let (_, s) = autor(&["https://otra.example/a.js?id=SECRETO"], "tienda.co");
        assert_eq!(s.as_deref(), Some("https://otra.example/a.js"));
        // Nothing usable: unknown, never a guess.
        for pila in [&[][..], &["<anonymous>", "blob:https://x.example/1"][..]] {
            assert_eq!(autor(pila, "tienda.co"), (Autor::Desconocido, None));
        }
        // A name of the same owner is not another company.
        let (a, _) = autor(&["https://www.gstatic.com/x.js"], "google.com");
        assert_eq!(a, Autor::Propia);
    }

    #[test]
    fn the_page_line_names_each_script_and_what_it_read() {
        let t = Textos::de(Idioma::Es);
        let mut p = Pagina::default();
        assert!(p.vacia());
        let (a, s) = autor(&["https://cdn.tracker.example/fp.js"], "tienda.co");
        assert!(p.huella(Tecnica::Canvas, a.clone(), s.clone(), false));
        assert!(p.huella(Tecnica::Webgl, a.clone(), s.clone(), false));
        // The same again says nothing new.
        assert!(!p.huella(Tecnica::Webgl, a, s, false));
        assert!(p.huella(Tecnica::Audio, Autor::Desconocido, None, false));
        let l = p.lineas(&t);
        assert_eq!(l.len(), 1);
        assert_eq!(
            l[0].texto,
            "2 scripts intentaron leer la huella de tu equipo: tracker.example (canvas, WebGL), origen desconocido (audio)"
        );
        assert_eq!(l[0].estado, "");
        let mut q = Pagina::default();
        assert!(q.huella(Tecnica::Fuentes, Autor::Propia, None, false));
        assert_eq!(
            q.lineas(&t)[0].texto,
            "1 script intentó leer la huella de tu equipo: esta misma web (fuentes)"
        );
        // Never «te identificó» nor «malicioso», in any language.
        for idioma in [Idioma::Es, Idioma::En, Idioma::Pt] {
            let t = Textos::de(idioma);
            for x in p
                .lineas(&t)
                .iter()
                .map(|l| l.texto.to_lowercase())
                .chain(p.notas(&t))
            {
                for no in ["identific", "malicio", "consigui", "succeed", "conseguiu"] {
                    assert!(!x.to_lowercase().contains(no), "{x}");
                }
            }
        }
    }

    #[test]
    fn session_recording_is_known_by_its_names_and_typing_is_noticed() {
        assert_eq!(grabador("www.clarity.ms"), Some("Microsoft Clarity"));
        assert_eq!(grabador("k.clarity.ms"), Some("Microsoft Clarity"));
        assert_eq!(grabador("static.hotjar.com"), Some("Hotjar"));
        assert_eq!(grabador("content.hotjar.io"), Some("Hotjar"));
        assert_eq!(grabador("edge.fullstory.com"), Some("FullStory"));
        assert_eq!(grabador("r.lr-ingest.io"), Some("LogRocket"));
        assert_eq!(
            grabador("mc.webvisor.org"),
            Some("Yandex Metrica (Webvisor)")
        );
        // Shared names and look-alikes are not a recording service.
        for h in [
            "c.bing.com",
            "mc.yandex.ru",
            "www.fullstory.com",
            "notclarity.ms",
            "hotjar.com.evil.example",
        ] {
            assert_eq!(grabador(h), None, "{h}");
        }
        let t = Textos::de(Idioma::Es);
        let mut p = Pagina::default();
        let h = mira(&vista("www.clarity.ms", None, false));
        p.anota(&vista("www.clarity.ms", None, false), &h);
        assert_eq!(
            p.lineas(&t)[0].texto,
            "Esta página carga un servicio de grabación de sesiones (Microsoft Clarity)"
        );
        assert_eq!(p.lineas(&t)[0].estado, "paso");
        assert!(p.escribe());
        assert!(!p.escribe());
        p.anota(&vista("k.clarity.ms", None, false), &h);
        assert_eq!(
            p.lineas(&t)[0].texto,
            "Esta página carga un servicio de grabación de sesiones (Microsoft Clarity), y le envió datos después de que escribieras en un campo"
        );
        // All of it cut: «intentó».
        let mut q = Pagina::default();
        let h = mira(&vista("static.hotjar.com", None, true));
        q.anota(&vista("static.hotjar.com", None, true), &h);
        let _ = q.escribe();
        q.anota(&vista("in.hotjar.com", None, true), &h);
        let l = q.lineas(&t);
        assert_eq!(
            l[0].texto,
            "Esta página intentó cargar un servicio de grabación de sesiones (Hotjar), e intentó enviarle datos después de que escribieras en un campo"
        );
        assert_eq!(l[0].estado, "cortado");
    }

    #[test]
    fn hashed_data_in_an_address_is_known_by_its_parameter_and_its_shape() {
        let h = sha("ana@correo.co");
        let md5 = "0123456789abcdef0123456789abcdef";
        assert_eq!(
            cifrados(&format!(
                "id=1&ev=Purchase&ud[em]={h}&ud[ph]={h}&ud[fn]={h}"
            )),
            [Cifrado::Correo, Cifrado::Telefono, Cifrado::Nombre].into()
        );
        assert_eq!(
            cifrados(&format!("tid=1&pd%5Bem%5D={md5}")),
            [Cifrado::Correo].into()
        );
        assert_eq!(
            cifrados(&format!("en=purchase&em=tv.1~em.{h}~pn.{h}")),
            [Cifrado::Correo, Cifrado::Telefono].into()
        );
        // The name alone, or a value that is no hash, says nothing.
        for q in [
            "ud[em]=ana%40correo.co",
            "ud[em]=",
            "em=12345",
            &format!("em={h}"),
            &format!("email={h}"),
            &format!("session={h}"),
            "u_hem=undefined",
        ] {
            assert!(cifrados(q).is_empty(), "{q}");
        }
        let t = Textos::de(Idioma::Es);
        let mut p = Pagina::default();
        let q = format!("ev=Lead&ud[em]={h}&ud[ph]={h}");
        let mut v = vista("www.facebook.com", Some(&q), false);
        v.quien = "Meta";
        p.anota(&v, &mira(&v));
        let mut v2 = vista(
            "ct.pinterest.com",
            Some("pd[em]=0123456789abcdef0123456789abcdef"),
            true,
        );
        v2.quien = "Pinterest";
        p.anota(&v2, &mira(&v2));
        let l = p.lineas(&t);
        assert_eq!(
            l[0].texto,
            "En la dirección de una petición a Meta iba un correo y un teléfono en forma cifrada"
        );
        assert_eq!(l[0].estado, "paso");
        assert_eq!(
            l[1].texto,
            "Una petición a Pinterest intentó llevar en su dirección un correo en forma cifrada"
        );
        assert_eq!(l[1].estado, "cortado");
        // Nothing of the value is in what is shown.
        assert!(l.iter().all(|x| !x.texto.contains(&h[..12])));
    }

    #[test]
    fn bodies_count_with_their_size_only_when_it_was_read() {
        let t = Textos::de(Idioma::Es);
        let mut p = Pagina::default();
        let cuerpo = |bytes: Option<u64>, cortada: bool| Vista {
            host: "collect.otra.example",
            consulta: None,
            quien: "otra.example",
            con_cuerpo: true,
            bytes,
            cortada,
            por_grabacion: false,
        };
        for v in [
            cuerpo(Some(2048), false),
            cuerpo(Some(1024), false),
            cuerpo(None, false),
            cuerpo(Some(500), true),
        ] {
            p.anota(&v, &mira(&v));
        }
        // A GET carries no body: not counted here.
        let g = vista("collect.otra.example", Some("a=1"), false);
        p.anota(&g, &mira(&g));
        let l = p.lineas(&t);
        assert_eq!(
            l[0].texto,
            "Salieron 3 envíos con datos hacia otras empresas: 3,0 KB medidos en 2 de ellos"
        );
        assert_eq!(l[0].estado, "paso");
        assert_eq!(
            l[1].texto,
            "Se cortó 1 envío con datos antes de salir: 500 B"
        );
        assert_eq!(l[1].estado, "cortado");
        // None measured: no size at all, never a guess; and a body as long as the browser reads
        // is a floor.
        let mut q = Pagina::default();
        let v = cuerpo(None, false);
        q.anota(&v, &mira(&v));
        assert_eq!(
            q.lineas(&t)[0].texto,
            "Salió 1 envío con datos hacia otras empresas: sin tamaño conocido"
        );
        let mut r = Pagina::default();
        let v = cuerpo(Some(MAX_CUERPO as u64), false);
        r.anota(&v, &mira(&v));
        assert_eq!(
            r.lineas(&t)[0].texto,
            "Salió 1 envío con datos hacia otras empresas: al menos 2,0 MB"
        );
        assert_eq!(tamano(1_234_567, Idioma::En), "1.2 MB");
        assert_eq!(tamano(1023, Idioma::Pt), "1023 B");
    }

    #[test]
    fn the_session_adds_up_the_webs_without_keeping_anything_on_disk() {
        let t = Textos::de(Idioma::Es);
        let mut s = Total::default();
        assert!(s.lineas(&t).is_empty());
        s.huella("tienda.co", Tecnica::Canvas, false);
        s.huella("eltiempo.com", Tecnica::Webgl, false);
        s.huella("tienda.co", Tecnica::Canvas, false);
        let v = vista("www.clarity.ms", None, false);
        s.anota("tienda.co", &v, &mira(&v));
        let h = sha("x");
        let q = format!("ud[em]={h}");
        let mut m = vista("www.facebook.com", Some(&q), true);
        m.quien = "Meta";
        s.anota("tienda.co", &m, &mira(&m));
        s.anota("eltiempo.com", &m, &mira(&m));
        let mut b = vista("collect.otra.example", None, false);
        b.con_cuerpo = true;
        b.bytes = Some(100);
        s.anota("tienda.co", &b, &mira(&b));
        let textos: Vec<String> = s.lineas(&t).into_iter().map(|l| l.texto).collect();
        assert_eq!(
            textos,
            [
                "En 2 webs, algún script intentó leer la huella de tu equipo (canvas, WebGL)",
                "En 1 web había un servicio de grabación de sesiones: Microsoft Clarity",
                "1 empresa recibió o intentó recibir un correo, un teléfono o un nombre cifrado en la dirección: Meta",
                "Envíos con datos a otras empresas: salieron 1 (100 B) y se cortaron 0",
            ]
        );
    }

    #[test]
    fn every_measurement_has_its_words_in_the_three_languages() {
        for idioma in [Idioma::Es, Idioma::En, Idioma::Pt] {
            let t = Textos::de(idioma);
            let mut claves = vec![
                "medida_titulo",
                "motivo_grabacion",
                "medida_grabacion_maxima",
                "medida_grabacion_maxima_tras_escribir",
                "medida_huella_ruido",
                "medida_huella_ruido_uno",
                "medida_huella_ruido_autor",
                "medida_huella_ruido_parte",
                "medida_sesion_huella_ruido",
                "medida_nota_ruido",
                "medida_sesion_titulo",
                "medida_huella",
                "medida_huella_uno",
                "medida_autor_propia",
                "medida_autor_desconocido",
                "medida_grabacion",
                "medida_grabacion_cortada",
                "medida_grabacion_tras_escribir",
                "medida_grabacion_tras_escribir_cortada",
                "medida_cifrado",
                "medida_cifrado_cortado",
                "medida_subida",
                "medida_subida_uno",
                "medida_subida_cortada",
                "medida_subida_cortada_uno",
                "medida_subida_sin_tam",
                "medida_subida_parte",
                "medida_al_menos",
                "medida_y",
                "medida_y_otras",
                "medida_sesion_huella",
                "medida_sesion_huella_uno",
                "medida_sesion_grabacion",
                "medida_sesion_grabacion_uno",
                "medida_sesion_cifrado",
                "medida_sesion_cifrado_uno",
                "medida_sesion_subida",
                "medida_nota_huella",
                "medida_nota_grabacion",
                "medida_nota_cifrado",
                "medida_nota_subida",
            ];
            let tecnicas: Vec<String> = [
                Tecnica::Canvas,
                Tecnica::Webgl,
                Tecnica::Audio,
                Tecnica::Fuentes,
            ]
            .iter()
            .map(|x| format!("medida_tecnica_{}", x.clave()))
            .chain(
                [Cifrado::Correo, Cifrado::Telefono, Cifrado::Nombre]
                    .iter()
                    .map(|c| format!("medida_dato_{}", c.clave())),
            )
            .collect();
            claves.extend(tecnicas.iter().map(String::as_str));
            for k in claves {
                assert_ne!(t.t(k), k, "{k} en {idioma:?}");
            }
            // Every technique round-trips by its key.
            for x in [
                Tecnica::Canvas,
                Tecnica::Webgl,
                Tecnica::Audio,
                Tecnica::Fuentes,
            ] {
                assert_eq!(Tecnica::de_clave(x.clave()), Some(x));
            }
            assert_eq!(Tecnica::de_clave("plugins"), None);
        }
    }

    #[test]
    fn a_different_value_is_said_only_where_it_was_given() {
        let t = Textos::de(Idioma::Es);
        let fuera = Autor::Fuera("tracker.example".into());
        let mut p = Pagina::default();
        assert!(p.huella(Tecnica::Canvas, fuera.clone(), None, true));
        assert!(p.huella(Tecnica::Webgl, fuera.clone(), None, true));
        assert_eq!(
            p.lineas(&t)[0].texto,
            "1 script intentó leer la huella de tu equipo; le dimos una distinta: tracker.example (canvas, WebGL)"
        );
        assert!(p.notas(&t).iter().any(|n| n.contains("huella distinta")));
        // The same read without a different value, later, does not take back what was given,
        // but another script that got the real one is said apart.
        assert!(p.huella(Tecnica::Audio, Autor::Propia, None, false));
        assert_eq!(
            p.lineas(&t)[0].texto,
            "2 scripts intentaron leer la huella de tu equipo: tracker.example (canvas, WebGL; le dimos una distinta), esta misma web (audio)"
        );
        let mut q = Pagina::default();
        assert!(q.huella(Tecnica::Canvas, fuera.clone(), None, true));
        assert!(q.huella(Tecnica::Fuentes, fuera, None, false));
        assert_eq!(
            q.lineas(&t)[0].texto,
            "1 script intentó leer la huella de tu equipo: tracker.example (canvas, fuentes; en canvas le dimos una distinta)"
        );
        // Without a different value nothing says so.
        let mut r = Pagina::default();
        assert!(r.huella(Tecnica::Canvas, Autor::Desconocido, None, false));
        assert!(!r.lineas(&t)[0].texto.contains("distinta"));
        assert!(!r.notas(&t).iter().any(|n| n.contains("huella distinta")));
        let mut s = Total::default();
        s.huella("tienda.co", Tecnica::Canvas, true);
        s.huella("eltiempo.com", Tecnica::Canvas, false);
        assert_eq!(
            s.lineas(&t)[0].texto,
            "En 2 webs, algún script intentó leer la huella de tu equipo (canvas); en 1 le dimos una distinta"
        );
    }

    #[test]
    fn maximum_protection_cutting_a_recording_service_says_it_plainly() {
        let t = Textos::de(Idioma::Es);
        let mut p = Pagina::default();
        let mut v = vista("www.clarity.ms", None, true);
        v.por_grabacion = true;
        p.anota(&v, &mira(&v));
        let l = &p.lineas(&t)[0];
        assert_eq!(
            l.texto,
            "cortado: grabación de sesiones (Microsoft Clarity)"
        );
        assert_eq!((l.estado, l.dicho), ("cortado", true));
        let _ = p.escribe();
        p.anota(&v, &mira(&v));
        assert_eq!(
            p.lineas(&t)[0].texto,
            "cortado: grabación de sesiones (Microsoft Clarity) · intentó enviarle datos después de que escribieras en un campo"
        );
    }

    #[test]
    fn the_seed_is_one_per_site_and_run_and_the_owner_s_names_are_its_own() {
        let a = semilla(b"secreto-de-esta-vez", "general|tienda.co");
        assert_eq!(a, semilla(b"secreto-de-esta-vez", "general|tienda.co"));
        assert_ne!(a, semilla(b"secreto-de-esta-vez", "general|otra.co"));
        assert_ne!(a, semilla(b"otro-arranque", "general|tienda.co"));
        let g = propios("google.com");
        assert_eq!(g.first().map(String::as_str), Some("google.com"));
        assert!(g.iter().any(|d| d == "gstatic.com"), "{g:?}");
        assert!(!g.iter().any(|d| d == "facebook.com"));
        assert!(g.len() <= 400);
        assert_eq!(
            propios("tienda-sin-dueno.example"),
            ["tienda-sin-dueno.example"]
        );
        assert!(propios("").is_empty());
        // A rented name is nobody else's.
        assert_eq!(propios("ana.github.io"), ["ana.github.io"]);
    }

    #[test]
    fn the_list_of_services_is_well_formed() {
        let g = grabadores();
        assert!(g.len() >= 30);
        for (n, s) in g {
            assert!(
                n.contains('.') && !n.starts_with('.') && !n.starts_with('*'),
                "{n}"
            );
            assert!(!s.is_empty());
        }
        let servicios: BTreeSet<&str> = g.iter().map(|(_, s)| *s).collect();
        assert_eq!(servicios.len(), 11, "{servicios:?}");
    }
}
