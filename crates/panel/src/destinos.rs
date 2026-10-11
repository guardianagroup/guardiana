//! «A dónde viajan tus datos» (decision 203): the server that answered each query and the
//! week's summary by country and by business group, worked out here, in Rust, from the columns
//! of the extract. The extract keeps only what the bundled base said about the server when it
//! answered (kind, country, network, which base); never the address. Nothing is estimated and
//! nothing is asked of anybody outside the house.
//!
//! The words matter as much as the figures. It is the country of the **server that answered**,
//! according to the base («el servidor que respondió está en Irlanda, según DB-IP Lite de
//! octubre de 2026»), never «where your data is»: a front server forwards what it receives, and
//! an anycast address answers from wherever is nearest. Percentages are of queries, never of
//! bytes, which GUARDIANA does not see.

use std::collections::HashMap;

use guardiana_core::i18n::Texts;
use guardiana_core::{DestinationCount, ServerFilter, ServerInfo, ServerKind};
use guardiana_lists::FuenteRed;
use serde::Serialize;

/// The server of one row, as the pages show it.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub(crate) struct ServidorView {
    /// Two-letter country the base gave, `None` when it gave none or the server is anycast or
    /// local.
    pub(crate) pais_codigo: Option<String>,
    /// That country by name, in the panel's language.
    pub(crate) pais: Option<String>,
    /// The network's autonomous system number.
    pub(crate) asn: Option<u32>,
    /// Who the network belongs to: the company of `empresas.txt` when its `AS` line says so,
    /// otherwise the organisation name the bundled base gives.
    pub(crate) red: Option<String>,
    /// Anycast by its operator's own word: no country is given.
    pub(crate) anycast: bool,
    /// A private, local or reserved address.
    pub(crate) local: bool,
    /// Which base said it, as stored (`dbip-2026-10`; `muestra-…` for the test sample).
    pub(crate) base: String,
    /// The one line the row shows: «servido desde la red de Meta (AS32934) · el servidor que
    /// respondió está en Irlanda».
    pub(crate) frase: String,
}

/// The network's name as the panel says it.
pub(crate) fn nombre_red(asn: u32) -> Option<String> {
    if asn == 0 {
        return None;
    }
    guardiana_lists::company_of_asn(asn)
        .or_else(|| guardiana_lists::nombre_de_red(asn))
        .map(|n| n.trim().to_owned())
}

fn pais_nombre(t: &Texts, codigo: &str) -> String {
    let n = t.pais(codigo);
    if n.is_empty() {
        codigo.to_owned()
    } else {
        n.to_owned()
    }
}

/// The server of a row, as the pages show it, or `None` when the row has none.
pub(crate) fn servidor_view(t: &Texts, s: Option<&ServerInfo>) -> Option<ServidorView> {
    let s = s?;
    let local = s.kind == ServerKind::Local;
    let anycast = s.kind == ServerKind::Anycast;
    let codigo =
        (s.kind == ServerKind::Publico && !s.country.is_empty()).then(|| s.country.clone());
    let pais = codigo.as_deref().map(|c| pais_nombre(t, c));
    let red = nombre_red(s.asn);
    let tramo_red = (s.asn != 0).then(|| {
        t.panel("servidor_red")
            .replace("{red}", red.as_deref().unwrap_or("?"))
            .replace("{asn}", &s.asn.to_string())
    });
    let tramo_pais = if local {
        Some(t.panel("servidor_local").to_owned())
    } else if anycast {
        Some(t.panel("servidor_anycast").to_owned())
    } else {
        pais.as_ref()
            .map(|p| t.panel("servidor_en").replace("{pais}", p))
    };
    let frase = match (tramo_red, tramo_pais) {
        (Some(r), Some(p)) => format!("{r} · {p}"),
        (Some(r), None) => format!("{r} · {}", t.panel("servidor_sin_pais")),
        (None, Some(p)) => p,
        (None, None) => t.panel("servidor_sin_dato").to_owned(),
    };
    Some(ServidorView {
        pais_codigo: codigo,
        pais,
        asn: (s.asn != 0).then_some(s.asn),
        red,
        anycast,
        local,
        base: s.base.clone(),
        frase,
    })
}

/// The group a name's queries add up to, for the summary: the parent group when written, else
/// the owning company. A name served through a delivery network or rented on a cloud adds up to
/// nobody, as on the devices page: the road is not the destination (review of 8 Oct 2026).
pub(crate) fn grupo_de(qname: &str) -> Option<&'static str> {
    if guardiana_lists::delivery_of(qname).is_some() || guardiana_lists::hosting_of(qname).is_some()
    {
        return None;
    }
    guardiana_lists::group_or_company_of(qname)
}

/// The bucket a server falls in for the country summary and for the extract's filter: a
/// two-letter country, or one of the words below.
pub(crate) fn clave_de(kind: ServerKind, country: &str) -> String {
    match kind {
        ServerKind::Local => CLAVE_LOCAL.to_owned(),
        ServerKind::Anycast => CLAVE_ANYCAST.to_owned(),
        ServerKind::Publico if country.is_empty() => CLAVE_SIN_DATO.to_owned(),
        ServerKind::Publico => country.to_owned(),
    }
}

/// The extract's filter for a bucket key, `None` when the key is neither two capital letters nor
/// one of the words.
pub(crate) fn filtro_de(clave: &str) -> Option<ServerFilter> {
    Some(match clave {
        CLAVE_ANYCAST => ServerFilter::Anycast,
        CLAVE_LOCAL => ServerFilter::Local,
        CLAVE_SIN_DATO => ServerFilter::NoCountry,
        c if c.len() == 2 && c.chars().all(|x| x.is_ascii_uppercase()) => {
            ServerFilter::Country(c.to_owned())
        }
        _ => return None,
    })
}

/// Filter word for anycast addresses (no country).
pub(crate) const CLAVE_ANYCAST: &str = "anycast";
/// Filter word for private, local and reserved addresses.
pub(crate) const CLAVE_LOCAL: &str = "local";
/// Filter word for public addresses the base has no country for.
pub(crate) const CLAVE_SIN_DATO: &str = "sin-dato";

/// One country of the summary.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub(crate) struct PaisCuenta {
    /// The filter key: two letters, or `anycast`, `local`, `sin-dato`.
    pub(crate) clave: String,
    /// What the page says.
    pub(crate) nombre: String,
    /// Queries whose answer pointed there.
    pub(crate) consultas: i64,
    /// Whole percent of the queries that got an address.
    pub(crate) pct: i64,
}

/// One business group of the summary.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub(crate) struct GrupoCuenta {
    /// The group (`Alphabet`), or the company standing for itself (`Meta`).
    pub(crate) nombre: &'static str,
    /// The companies of the list seen under it this week, when the group is not just itself
    /// (`["Google"]` under Alphabet).
    pub(crate) empresas: Vec<&'static str>,
    /// Queries to names of the group.
    pub(crate) consultas: i64,
    /// Whole percent of the queries that left the house.
    pub(crate) pct: i64,
}

/// Where the countries come from, for the line under every place that shows one.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct FuenteView {
    pub(crate) fuente: String,
    /// `AAAA-MM`.
    pub(crate) fecha: String,
    pub(crate) licencia: String,
    pub(crate) licencia_url: String,
    pub(crate) atribucion: String,
    pub(crate) web: String,
    /// The bundled tables are the hand-made test sample, not DB-IP's data.
    pub(crate) muestra: bool,
}

pub(crate) fn fuente_view() -> FuenteView {
    let f: &FuenteRed = guardiana_lists::fuente_red();
    FuenteView {
        fuente: f.fuente.clone(),
        fecha: f.fecha.clone(),
        licencia: f.licencia.clone(),
        licencia_url: f.licencia_url.clone(),
        atribucion: f.atribucion.clone(),
        web: f.web.clone(),
        muestra: f.muestra,
    }
}

/// The week's «where did it go», by country of the server and by business group.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub(crate) struct ResumenDestinos {
    /// Forwarded queries of the week to names that are not the home network talking to itself:
    /// what left the house. The groups' percentages are of this.
    pub(crate) consultas: i64,
    /// Of those, the ones whose answer carried an address (rows written before decision 203, a
    /// name that does not exist, a record with no address, do not). The countries' percentages
    /// are of this.
    pub(crate) con_direccion: i64,
    /// Distinct countries among them, anycast, local and unknown not counted.
    pub(crate) n_paises: usize,
    /// Countries first, most queried first, then anycast, local and no data, each when present.
    pub(crate) paises: Vec<PaisCuenta>,
    /// Every group with queries, most first.
    pub(crate) grupos: Vec<GrupoCuenta>,
    /// How many groups the top line names (at most three).
    pub(crate) top_n: usize,
    /// Their queries together, and that as a whole percent of `consultas`.
    pub(crate) top_consultas: i64,
    pub(crate) top_pct: i64,
    pub(crate) fuente: FuenteView,
}

/// Whole percent, rounded half up; 0 of 0 is 0.
fn pct(parte: i64, todo: i64) -> i64 {
    if todo <= 0 {
        0
    } else {
        (parte * 200 + todo) / (2 * todo)
    }
}

/// The summary of the week's destinations (`Ledger::week_destinations`).
pub(crate) fn resumen(t: &Texts, filas: &[DestinationCount]) -> ResumenDestinos {
    let mut consultas = 0;
    let mut con_direccion = 0;
    let mut por_clave: HashMap<String, i64> = HashMap::new();
    let mut por_grupo: HashMap<&'static str, (i64, Vec<&'static str>)> = HashMap::new();
    for f in filas {
        if guardiana_lists::is_local_name(&f.qname) {
            continue;
        }
        consultas += f.queries;
        if let Some(kind) = f.kind {
            con_direccion += f.queries;
            *por_clave.entry(clave_de(kind, &f.country)).or_insert(0) += f.queries;
        }
        if let Some(g) = grupo_de(&f.qname) {
            let e = por_grupo.entry(g).or_insert((0, Vec::new()));
            e.0 += f.queries;
            if let Some(c) = guardiana_lists::company_of(&f.qname) {
                if !e.1.contains(&c) {
                    e.1.push(c);
                }
            }
        }
    }
    let especiales = [CLAVE_ANYCAST, CLAVE_LOCAL, CLAVE_SIN_DATO];
    let mut paises: Vec<PaisCuenta> = por_clave
        .iter()
        .filter(|(k, _)| !especiales.contains(&k.as_str()))
        .map(|(k, n)| PaisCuenta {
            clave: k.clone(),
            nombre: pais_nombre(t, k),
            consultas: *n,
            pct: pct(*n, con_direccion),
        })
        .collect();
    paises.sort_by(|a, b| b.consultas.cmp(&a.consultas).then(a.nombre.cmp(&b.nombre)));
    let n_paises = paises.len();
    for (clave, texto) in [
        (CLAVE_ANYCAST, "destinos_anycast"),
        (CLAVE_LOCAL, "destinos_local"),
        (CLAVE_SIN_DATO, "destinos_sin_dato"),
    ] {
        if let Some(n) = por_clave.get(clave) {
            paises.push(PaisCuenta {
                clave: clave.to_owned(),
                nombre: t.panel(texto).to_owned(),
                consultas: *n,
                pct: pct(*n, con_direccion),
            });
        }
    }
    let mut grupos: Vec<GrupoCuenta> = por_grupo
        .into_iter()
        .map(|(g, (n, mut empresas))| {
            empresas.sort_unstable();
            if empresas == [g] {
                empresas.clear();
            }
            GrupoCuenta {
                nombre: g,
                empresas,
                consultas: n,
                pct: pct(n, consultas),
            }
        })
        .collect();
    grupos.sort_by(|a, b| b.consultas.cmp(&a.consultas).then(a.nombre.cmp(b.nombre)));
    let top_n = grupos.len().min(3);
    let top_consultas = grupos.iter().take(top_n).map(|g| g.consultas).sum();
    ResumenDestinos {
        consultas,
        con_direccion,
        n_paises,
        paises,
        grupos,
        top_n,
        top_consultas,
        top_pct: pct(top_consultas, consultas),
        fuente: fuente_view(),
    }
}

/// A group as the sentences name it: «Alphabet (Google)» when the companies seen under it are
/// not just itself.
pub(crate) fn grupo_dicho(g: &GrupoCuenta) -> String {
    if g.empresas.is_empty() {
        g.nombre.to_owned()
    } else {
        format!("{} ({})", g.nombre, g.empresas.join(", "))
    }
}

/// The two sentences of the summary, for the report's text to share: the countries (only with
/// the real base: a sample is not something to send anybody) and the groups. Empty when there is
/// nothing to say.
pub(crate) fn frases_para_compartir(t: &Texts, r: &ResumenDestinos) -> Vec<String> {
    let mut out = Vec::new();
    if !r.fuente.muestra && r.n_paises > 0 {
        out.push(
            t.panel_n(
                "compartir_paises",
                i64::try_from(r.n_paises).unwrap_or(i64::MAX),
            )
            .replace("{fuente}", &r.fuente.fuente),
        );
    }
    if r.top_n > 0 {
        let nombres: Vec<String> = r.grupos.iter().take(r.top_n).map(grupo_dicho).collect();
        out.push(
            t.panel_n(
                "compartir_grupos",
                i64::try_from(r.top_n).unwrap_or(i64::MAX),
            )
            .replace("{pct}", &r.top_pct.to_string())
            .replace("{grupos}", &nombres.join(", ")),
        );
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use guardiana_core::i18n;
    use guardiana_core::{Hash, Ledger, NewEvent, Verdict, SELF_DEVICE_ID};

    #[test]
    fn percentages_round_half_up_and_never_divide_by_zero() {
        assert_eq!(pct(0, 0), 0);
        assert_eq!(pct(1, 3), 33);
        assert_eq!(pct(2, 3), 67);
        assert_eq!(pct(1, 8), 13);
        assert_eq!(pct(5, 5), 100);
    }

    #[test]
    fn the_week_adds_up_by_country_and_by_group_from_a_test_extract() {
        // A real ledger, written the way the resolver writes it, read back the way the report
        // reads it.
        let mut l = Ledger::open_in_memory(Hash::of(b"destinos")).unwrap();
        let ahora = 1_800_000_000_000i64;
        let mut ts = ahora - 3_600_000;
        let mut pon = |q: &str, ip: &str, veces: usize, v: Verdict| {
            for _ in 0..veces {
                ts += 1;
                let mut e = NewEvent::observed(ts, SELF_DEVICE_ID, "127.0.0.1", q, "A");
                // What the engine does at the moment of the answer: look the address up, keep
                // what the base said, drop the address.
                e.server =
                    (!ip.is_empty()).then(|| guardiana_lists::server_info_de(ip.parse().unwrap()));
                e.verdict = v;
                l.append(e).unwrap();
            }
        };
        pon("graph.facebook.com", "157.240.1.35", 6, Verdict::Observado);
        pon(
            "www.instagram.com",
            "2a03:2880:f100::1",
            2,
            Verdict::Observado,
        );
        pon("www.youtube.com", "142.250.1.1", 4, Verdict::Observado);
        pon("app.slack.com", "13.107.1.1", 1, Verdict::Observado);
        pon("dns.google", "8.8.8.8", 3, Verdict::Observado);
        pon("router.lan", "192.168.1.1", 1, Verdict::Observado);
        pon("nadie.example", "", 2, Verdict::Observado);
        pon("4.1.168.192.in-addr.arpa", "", 5, Verdict::Observado);
        pon("ads.example", "", 9, Verdict::Cortado);
        pon(
            "e1.dsce9.akamaiedge.net",
            "184.24.1.1",
            1,
            Verdict::Observado,
        );
        let filas = l.week_destinations(ahora).unwrap();
        let r = resumen(i18n::es(), &filas);
        // 6+2+4+1+3+1+2+1 left the house; the reverse lookups did not, the cut ones neither.
        assert_eq!(r.consultas, 20);
        // All but the two of nadie.example came with an address.
        assert_eq!(r.con_direccion, 18);
        // Groups: Meta 8, Alphabet 4 (YouTube is Google's), Salesforce 1; dns.google is in no
        // section of empresas.txt, so it adds up to no group.
        let g: Vec<(&str, i64)> = r.grupos.iter().map(|g| (g.nombre, g.consultas)).collect();
        assert_eq!(g[0], ("Meta", 8));
        assert!(g.contains(&("Salesforce", 1)), "{g:?}");
        assert_eq!(r.top_n, 3);
        assert_eq!(r.top_pct, pct(r.top_consultas, 20));
        let meta = &r.grupos[0];
        assert!(meta.empresas.is_empty(), "Meta is its own group");
        assert_eq!(meta.pct, 40);
        // The delivery network's name adds up to no group.
        assert!(!g.iter().any(|(n, _)| *n == "Akamai"), "{g:?}");
        // Anycast and local in their own buckets, never in a country.
        let clave = |k: &str| r.paises.iter().find(|p| p.clave == k).map(|p| p.consultas);
        assert_eq!(clave(CLAVE_ANYCAST), Some(3));
        assert_eq!(clave(CLAVE_LOCAL), Some(1));
        let suma: i64 = r.paises.iter().map(|p| p.consultas).sum();
        assert_eq!(suma, r.con_direccion, "every address falls in one bucket");
        if guardiana_lists::fuente_red().muestra {
            // With the sample: Ireland 6, Sweden 2, United States 4 + 1, Colombia 1.
            assert_eq!(clave("IE"), Some(6));
            assert_eq!(clave("SE"), Some(2));
            assert_eq!(clave("US"), Some(5));
            assert_eq!(r.n_paises, 4);
            assert_eq!(r.paises[0].nombre, "Irlanda");
            assert_eq!(r.paises[0].pct, 33);
        }
    }

    #[test]
    fn a_row_says_whose_network_and_which_country_or_why_not() {
        let t = i18n::es();
        let de = |ip: &str| {
            servidor_view(
                t,
                Some(&guardiana_lists::server_info_de(ip.parse().unwrap())),
            )
        };
        let v = de("8.8.8.8").unwrap();
        assert!(v.anycast && v.pais.is_none());
        assert_eq!(v.red.as_deref(), Some("Google"));
        assert!(v.frase.contains(t.panel("servidor_anycast")), "{}", v.frase);
        assert!(v.frase.contains("AS15169"), "{}", v.frase);
        let local = de("192.168.1.1").unwrap();
        assert!(local.local && local.pais.is_none() && local.red.is_none());
        assert_eq!(local.frase, t.panel("servidor_local"));
        assert!(servidor_view(t, None).is_none());
        if guardiana_lists::fuente_red().muestra {
            let meta = de("157.240.1.35").unwrap();
            assert_eq!(
                meta.frase,
                "servido desde la red de Meta (AS32934) · el servidor que respondió está en Irlanda"
            );
            let sin = de("203.0.114.9").unwrap();
            assert_eq!(sin.frase, t.panel("servidor_sin_dato"));
        }
    }
}
