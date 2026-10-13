//! «GUARDIANA vigila a tu antivirus» (the owner, 13 Oct 2026: «alguien de código abierto debe
//! vigilar al vigilante»).
//!
//! An antivirus is the most trusted program on a computer: it reads every file and, in most
//! products, every page. Guardiana does what it does with any other program and nothing more:
//! it sees which names the antivirus's own processes ask for, and puts next to each one what the
//! maker itself publishes about that name (updates, cloud scanning, samples, licence…). A name
//! the maker does not document is said as such, never as an accusation. Next to each product go
//! the official decisions of authorities about it (fines, bans, warnings), each with its source,
//! and only those: no lawsuits settled privately, no press reports.
//!
//! The list is `data/antivirus.txt`, open and kept by us. Its format is in its own header.

use std::collections::HashMap;
use std::sync::OnceLock;

/// Guardiana's own antivirus list (`antivirus.txt`).
pub const ANTIVIRUS: &str = include_str!("../data/antivirus.txt");

/// What a maker says one of its names is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DominioAv {
    /// The name; matches itself and its subdomains.
    pub dominio: &'static str,
    /// What it is for, one of [`USOS`].
    pub uso: &'static str,
    /// The maker's page that says so.
    pub fuente: &'static str,
}

/// An official decision of an authority about a product or its maker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sancion {
    /// Day of the decision, `YYYY-MM-DD`.
    pub fecha: &'static str,
    /// Who decided it, by its own name (`FTC`).
    pub autoridad: &'static str,
    /// Two-letter country of that authority.
    pub pais: &'static str,
    /// The amount and its currency (`16500000 USD`), or empty when there is none.
    pub importe: &'static str,
    /// One of [`TIPOS`]: a fine, an order with a payment, a court ruling, a ban, an official
    /// warning, or commitments accepted by an authority (no fine, no admission).
    pub tipo: &'static str,
    /// Key of the plain-words text that says what it was about (`av_sancion_<clave>`).
    pub clave: &'static str,
    /// The authority's own page.
    pub fuente: &'static str,
}

/// One antivirus product as the list knows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Antivirus {
    /// The product's name as people know it (`Avast`).
    pub nombre: &'static str,
    /// The company behind it today.
    pub empresa: &'static str,
    /// Two-letter country of that company's seat.
    pub pais: &'static str,
    /// File names of its Windows processes, as the system reports them.
    pub programas: Vec<&'static str>,
    /// The names its maker documents, with what each is for.
    pub dominios: Vec<DominioAv>,
    /// Official decisions about it.
    pub sanciones: Vec<Sancion>,
}

/// What a documented name can be for. The order is the order the page shows them in.
pub const USOS: &[&str] = &[
    "actualizaciones",
    "nube",
    "muestras",
    "licencia",
    "telemetria",
    "mensajes",
    "publicidad",
    "otros",
];

/// What a decision can be.
pub const TIPOS: &[&str] = &[
    "multa",
    "orden",
    "sentencia",
    "prohibicion",
    "advertencia",
    "compromiso",
];

fn parse(texto: &'static str) -> Result<Vec<Antivirus>, String> {
    let mut out: Vec<Antivirus> = Vec::new();
    for (n, linea) in texto.lines().enumerate() {
        let linea = linea.trim();
        if linea.is_empty() || linea.starts_with('#') {
            continue;
        }
        let err = |m: &str| format!("antivirus.txt:{}: {m}", n + 1);
        if let Some(nombre) = linea.strip_prefix('@') {
            out.push(Antivirus {
                nombre: nombre.trim(),
                empresa: "",
                pais: "",
                programas: Vec::new(),
                dominios: Vec::new(),
                sanciones: Vec::new(),
            });
            continue;
        }
        let Some(av) = out.last_mut() else {
            return Err(err("line before any @product"));
        };
        let Some((clave, valor)) = linea.split_once(':') else {
            return Err(err("expected `field: value`"));
        };
        let valor = valor.trim();
        let partes: Vec<&'static str> = valor.split(" | ").map(str::trim).collect();
        match clave.trim() {
            "empresa" => av.empresa = valor,
            "pais" => av.pais = valor,
            "programa" => av.programas.push(valor),
            "dominio" => {
                let [dominio, uso, fuente] = partes[..] else {
                    return Err(err("dominio: name | use | source"));
                };
                if !USOS.contains(&uso) {
                    return Err(err("unknown use"));
                }
                if !fuente.starts_with("https://") {
                    return Err(err("source must be an https link"));
                }
                av.dominios.push(DominioAv {
                    dominio,
                    uso,
                    fuente,
                });
            }
            "sancion" => {
                let [fecha, autoridad, pais, importe, tipo, clave, fuente] = partes[..] else {
                    return Err(err(
                        "sancion: date | authority | country | amount | kind | key | source",
                    ));
                };
                if pais.len() != 2 {
                    return Err(err("country must be two letters"));
                }
                if importe != "-"
                    && !importe
                        .split_once(' ')
                        .is_some_and(|(n, m)| n.bytes().all(|b| b.is_ascii_digit()) && m.len() == 3)
                {
                    return Err(err("amount must be `digits CUR` or -"));
                }
                if !TIPOS.contains(&tipo) {
                    return Err(err("unknown kind"));
                }
                if !fuente.starts_with("https://") {
                    return Err(err("source must be an https link"));
                }
                av.sanciones.push(Sancion {
                    fecha,
                    autoridad,
                    pais,
                    importe: if importe == "-" { "" } else { importe },
                    tipo,
                    clave,
                    fuente,
                });
            }
            _ => return Err(err("unknown field")),
        }
    }
    Ok(out)
}

/// Every antivirus the list knows, in the list's order.
///
/// # Panics
/// Never in a build that passed its tests: the list is checked by them.
#[must_use]
#[allow(clippy::expect_used)]
pub fn antivirus() -> &'static [Antivirus] {
    static L: OnceLock<Vec<Antivirus>> = OnceLock::new();
    L.get_or_init(|| parse(ANTIVIRUS).expect("antivirus.txt is checked by its tests"))
}

/// The antivirus a Windows process belongs to, by its file name (`AvastSvc.exe`), any case.
#[must_use]
pub fn antivirus_de_programa(programa: &str) -> Option<&'static Antivirus> {
    static M: OnceLock<HashMap<String, usize>> = OnceLock::new();
    let m = M.get_or_init(|| {
        antivirus()
            .iter()
            .enumerate()
            .flat_map(|(i, a)| a.programas.iter().map(move |p| (p.to_ascii_lowercase(), i)))
            .collect()
    });
    // The system may give the full path; only the file name counts.
    let nombre = programa
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(programa)
        .to_ascii_lowercase();
    m.get(&nombre).map(|i| &antivirus()[*i])
}

/// What the maker says `qname` is for, when it documents it (longest documented name wins).
#[must_use]
pub fn uso_documentado(av: &Antivirus, qname: &str) -> Option<DominioAv> {
    let n = qname.trim_end_matches('.').to_ascii_lowercase();
    av.dominios
        .iter()
        .filter(|d| n == d.dominio || n.ends_with(&format!(".{}", d.dominio)))
        .max_by_key(|d| d.dominio.len())
        .copied()
}

/// What each antivirus's own documents say (privacy policy, product notice, licence), in plain
/// words and checked sentence by sentence against the maker's page on the day of `revisado`
/// (`antivirus-terminos.json`, 13 Oct 2026). A summary with its sources, not legal advice.
pub const TERMINOS: &str = include_str!("../data/antivirus-terminos.json");

/// A text in the three languages of the panel.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Texto3 {
    /// Spanish, the master text.
    pub es: String,
    /// English.
    pub en: String,
    /// Portuguese.
    pub pt: String,
}

/// One official document that was read.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct DocumentoAv {
    /// Its title, as the maker gives it.
    pub titulo: String,
    /// Where it is.
    pub url: String,
    /// Its own «last updated», as written (`2026-03-06`, `2026-09`), or empty when it has none.
    pub actualizado: String,
}

/// One point of the summary, with the page that says it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct PuntoAv {
    /// What it is about, one of [`TEMAS`].
    pub tema: String,
    /// What it says, in plain words.
    #[serde(flatten)]
    pub texto: Texto3,
    /// The maker's page where it is written.
    pub fuente: String,
}

/// The documented way to limit what is collected or shared.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ComoLimitar {
    /// What to do, in plain words.
    #[serde(flatten)]
    pub texto: Texto3,
    /// The maker's page that says so.
    pub fuente: String,
}

/// What one antivirus's documents say.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct TerminosAv {
    /// The product, as `antivirus.txt` names it.
    pub producto: String,
    /// The day it was checked, `YYYY-MM-DD`.
    pub revisado: String,
    /// The documents read.
    pub documentos: Vec<DocumentoAv>,
    /// The points, most important first.
    pub puntos: Vec<PuntoAv>,
    /// How to limit it, when the maker documents a way.
    #[serde(default)]
    pub como_limitar: Option<ComoLimitar>,
}

#[derive(serde::Deserialize)]
struct ArchivoTerminos {
    antivirus: Vec<TerminosAv>,
}

/// What a point can be about. The order is the order the page shows them in.
pub const TEMAS: &[&str] = &[
    "datos",
    "venta",
    "compartir",
    "marketing",
    "conservacion",
    "transferencias",
    "cambios",
    "renovacion",
    "responsabilidad",
    "arbitraje",
];

/// What every antivirus's documents say, in the list's order.
///
/// # Panics
/// Never in a build that passed its tests: the file is checked by them.
#[must_use]
#[allow(clippy::expect_used)]
pub fn terminos() -> &'static [TerminosAv] {
    static L: OnceLock<Vec<TerminosAv>> = OnceLock::new();
    L.get_or_init(|| {
        serde_json::from_str::<ArchivoTerminos>(TERMINOS)
            .expect("antivirus-terminos.json is checked by its tests")
            .antivirus
    })
}

/// What one product's documents say, by its name in the list.
#[must_use]
pub fn terminos_de(producto: &str) -> Option<&'static TerminosAv> {
    terminos().iter().find(|t| t.producto == producto)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn every_summary_of_terms_is_complete_and_has_its_sources() {
        let nombres: Vec<&str> = antivirus().iter().map(|a| a.nombre).collect();
        let t = terminos();
        assert!(t.len() >= 10, "{}", t.len());
        let fuente = |u: &str| u.starts_with("https://") && !u.contains(' ');
        for a in t {
            assert!(
                nombres.contains(&a.producto.as_str()),
                "{} is not in antivirus.txt",
                a.producto
            );
            assert_eq!(a.revisado.len(), 10, "{}", a.producto);
            assert!(!a.documentos.is_empty(), "{} read no document", a.producto);
            for d in &a.documentos {
                assert!(
                    fuente(&d.url) && !d.titulo.is_empty(),
                    "{}: {}",
                    a.producto,
                    d.url
                );
            }
            assert!(
                !a.puntos.is_empty() && a.puntos.len() <= 6,
                "{}",
                a.producto
            );
            for p in &a.puntos {
                assert!(
                    TEMAS.contains(&p.tema.as_str()),
                    "{}: {}",
                    a.producto,
                    p.tema
                );
                assert!(fuente(&p.fuente), "{}: {}", a.producto, p.fuente);
                for x in [&p.texto.es, &p.texto.en, &p.texto.pt] {
                    assert!(!x.trim().is_empty(), "{}: empty text", a.producto);
                }
                // Plain words, short: a sentence a person reads at once.
                assert!(
                    p.texto.es.split_whitespace().count() <= 40,
                    "{}: {}",
                    a.producto,
                    p.texto.es
                );
            }
            if let Some(c) = &a.como_limitar {
                assert!(
                    fuente(&c.fuente) && !c.texto.es.is_empty(),
                    "{}",
                    a.producto
                );
            }
        }
        assert!(terminos_de("Avast").is_some());
    }

    #[test]
    fn the_list_reads_and_every_entry_has_its_source() {
        let l = parse(ANTIVIRUS).unwrap();
        assert!(l.len() >= 10, "{}", l.len());
        let mut vistos = std::collections::HashSet::new();
        for a in &l {
            assert!(!a.empresa.is_empty(), "{} has no company", a.nombre);
            assert_eq!(a.pais.len(), 2, "{} country", a.nombre);
            assert!(!a.programas.is_empty(), "{} has no process", a.nombre);
            for p in &a.programas {
                assert!(
                    p.to_ascii_lowercase().ends_with(".exe"),
                    "{}: {p}",
                    a.nombre
                );
                // One process, one product: otherwise the page would say the wrong maker.
                assert!(
                    vistos.insert(p.to_ascii_lowercase()),
                    "{p} is in two products"
                );
            }
            for s in &a.sanciones {
                assert_eq!(s.fecha.len(), 10, "{}", s.fecha);
                assert!(!s.clave.is_empty());
            }
        }
    }

    #[test]
    fn a_process_finds_its_product_and_a_name_its_use() {
        let d = antivirus_de_programa("MsMpEng.exe").unwrap();
        assert_eq!(d.nombre, "Microsoft Defender");
        assert!(antivirus_de_programa(
            r"C:\ProgramData\Microsoft\Windows Defender\Platform\4.18\MSMPENG.EXE"
        )
        .is_some());
        assert!(antivirus_de_programa("chrome.exe").is_none());
        let avast = antivirus().iter().find(|a| a.nombre == "Avast").unwrap();
        assert!(avast.sanciones.iter().any(|s| s.autoridad.contains("FTC")));
        assert!(uso_documentado(d, "nadie.example").is_none());
    }

    #[test]
    fn a_broken_line_is_said_not_skipped() {
        assert!(parse("programa: x.exe").is_err());
        assert!(parse("@X\ndominio: a.com | inventado | https://a").is_err());
        assert!(parse("@X\ndominio: a.com | nube | http://a").is_err());
        assert!(parse("@X\nsancion: 2024-01-01 | A | 1 | multa | k").is_err());
        assert!(
            parse("@X\nsancion: 2024-01-01 | A | US | 16,5 M | multa | k | https://a").is_err()
        );
        assert!(
            parse("@X\nsancion: 2024-01-01 | A | US | 16500000 USD | multa | k | https://a")
                .is_ok()
        );
    }
}
