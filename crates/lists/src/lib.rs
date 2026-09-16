//! Open block lists (brief §5): the bundled lists, their licences and
//! attribution, the three lists Guardiana maintains, and a catalog that
//! answers "which category is this name?".
//!
//! Nothing here touches the network. Downloading fresh lists is a user
//! action handled elsewhere and recorded in `outbound`; this crate only
//! parses text it is given and the copies embedded at build time.
//!
//! Every entry matches the domain itself and all of its subdomains.
//! Precedence when several lists match (docs/LISTS.md):
//! `esperado` > `telemetria` > `rastreador` > `publicidad`.

mod parse;

use std::collections::HashMap;

use guardiana_core::Category;
use serde::{Deserialize, Serialize};

pub use parse::{parse_abp_domains, parse_guardiana, parse_hosts, Format};

/// Bundled copy of EasyPrivacy, trimmed to domain rules by `build/fetch-lists.sh`.
pub const EASYPRIVACY: &str = include_str!("../data/easyprivacy.txt");
/// Bundled copy of Peter Lowe's list, hosts format.
pub const PETERLOWE: &str = include_str!("../data/peterlowe.txt");
/// Guardiana's own "esperado" list.
pub const ESPERADO: &str = include_str!("../data/esperado.txt");
/// Guardiana's own telemetry list.
pub const TELEMETRIA: &str = include_str!("../data/telemetria.txt");
/// Known encrypted-DNS resolver names, for the `evasion_dns` signal.
pub const EVASION_DNS: &str = include_str!("../data/evasion_dns.txt");
/// Manifest written by `build/fetch-lists.sh`: url, date, sha256 and count per list.
pub const MANIFEST_JSON: &str = include_str!("../data/MANIFEST.json");
/// Guardiana's own "empresas" list: who owns each name (decision 61). A label, never a verdict.
pub const EMPRESAS: &str = include_str!("../data/empresas.txt");

/// Guardiana's own "IA" list: which names belong to an artificial-intelligence service (decision 62).
pub const IA: &str = include_str!("../data/ia.txt");

static COMPANIES: std::sync::OnceLock<HashMap<&'static str, &'static str>> =
    std::sync::OnceLock::new();
static AI_SERVICES: std::sync::OnceLock<HashMap<&'static str, &'static str>> =
    std::sync::OnceLock::new();

/// Longest matching domain of a name map: `graph.facebook.com` matches an entry `facebook.com`.
fn lookup(map: &HashMap<&'static str, &'static str>, qname: &str) -> Option<&'static str> {
    let name = qname.trim_end_matches('.').to_ascii_lowercase();
    let mut rest = name.as_str();
    loop {
        if let Some(v) = map.get(rest) {
            return Some(v);
        }
        let i = rest.find('.')?;
        rest = &rest[i + 1..];
    }
}

/// The artificial-intelligence service a queried name belongs to (`api.anthropic.com` →
/// `Anthropic`), or `None`. A label about who owns the destination, never a verdict about what
/// it does: Guardiana sees names, not what an agent reads.
#[must_use]
pub fn ai_service_of(qname: &str) -> Option<&'static str> {
    let map = AI_SERVICES.get_or_init(|| {
        parse_guardiana(IA)
            .filter_map(|(section, domain)| section.map(|c| (domain, c)))
            .collect()
    });
    lookup(map, qname)
}

/// The company behind a queried name, by longest matching domain of the "empresas" list
/// (`graph.facebook.com` → `Meta`). `None` when no list entry owns it.
#[must_use]
pub fn company_of(qname: &str) -> Option<&'static str> {
    let map = COMPANIES.get_or_init(|| {
        parse_guardiana(EMPRESAS)
            .filter_map(|(section, domain)| section.map(|c| (domain, c)))
            .collect()
    });
    lookup(map, qname)
}

/// A third-party list: where it comes from and under what terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SourceInfo {
    /// Short identifier, also the `list_source` stored in the ledger.
    pub id: &'static str,
    /// Human name.
    pub name: &'static str,
    /// Where it is downloaded from.
    pub url: &'static str,
    /// Licence, as text.
    pub license: &'static str,
    /// Who to credit.
    pub attribution: &'static str,
    /// Category every entry of this list receives.
    pub category: Category,
    /// Text format.
    pub format: Format,
}

/// The third-party lists shipped in 1.0. Disconnect and DuckDuckGo Tracker
/// Radar are not here: their CC BY-NC-SA licence forbids commercial use
/// (docs/LISTS.md).
pub const SOURCES: &[SourceInfo] = &[
    SourceInfo {
        id: "easyprivacy",
        name: "EasyPrivacy",
        url: "https://easylist.to/easylist/easyprivacy.txt",
        license: "GPL-3.0-or-later or CC BY-SA 3.0",
        attribution: "The EasyList authors, https://easylist.to",
        category: Category::Rastreador,
        format: Format::Abp,
    },
    SourceInfo {
        id: "peterlowe",
        name: "Peter Lowe's Ad and tracking server list",
        url: "https://pgl.yoyo.org/adservers/serverlist.php?hostformat=hosts&showintro=0&mimetype=plaintext",
        license: "Redistribution permitted by the author (no formal licence)",
        attribution: "Peter Lowe, https://pgl.yoyo.org/adservers/",
        category: Category::Publicidad,
        format: Format::Hosts,
    },
];

/// Identifier of Guardiana's own lists in `list_source`.
pub const OWN_SOURCE_ESPERADO: &str = "guardiana-esperado";
/// Identifier of Guardiana's telemetry list in `list_source`.
pub const OWN_SOURCE_TELEMETRIA: &str = "guardiana-telemetria";

/// Kind of expected traffic, from the `@` sections of `esperado.txt`.
/// Decides the confirmation sentence of brief §6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedKind {
    /// System and browser updates.
    Actualizaciones,
    /// Time synchronisation.
    Hora,
    /// Messaging apps.
    Mensajeria,
    /// Video calls.
    Videollamada,
    /// The system resolvers.
    Resolutores,
    /// Guardiana's own domain.
    Guardiana,
}

impl ExpectedKind {
    fn from_section(name: &str) -> Option<Self> {
        Some(match name {
            "actualizaciones" => Self::Actualizaciones,
            "hora" => Self::Hora,
            "mensajeria" => Self::Mensajeria,
            "videollamada" => Self::Videollamada,
            "resolutores" => Self::Resolutores,
            "guardiana" => Self::Guardiana,
            _ => return None,
        })
    }
}

/// Result of looking a name up in the catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    /// Category to store.
    pub category: Category,
    /// Which list decided it (`list_source` in the ledger).
    pub source: &'static str,
    /// The list entry that matched: the name itself or one of its parents.
    pub matched: String,
    /// For `esperado`, which kind.
    pub expected: Option<ExpectedKind>,
}

#[derive(Debug, Clone)]
struct Entry {
    category: Category,
    source: &'static str,
    expected: Option<ExpectedKind>,
}

/// Bundled-list metadata as written by `build/fetch-lists.sh`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// When the lists were fetched, RFC 3339 UTC.
    pub fetched: String,
    /// One entry per third-party list.
    pub lists: Vec<ManifestEntry>,
}

/// One bundled list in the manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEntry {
    /// Matches [`SourceInfo::id`].
    pub id: String,
    /// File name under `crates/lists/data/`.
    pub file: String,
    /// Download URL.
    pub url: String,
    /// SHA-256 of the bundled file.
    pub sha256: String,
    /// Number of domain entries.
    pub entries: u64,
}

/// Parse the bundled manifest.
pub fn manifest() -> Result<Manifest, serde_json::Error> {
    serde_json::from_str(MANIFEST_JSON)
}

/// Every known domain → category, with precedence resolved at insert time.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    entries: HashMap<String, Entry>,
    evasion: HashMap<String, ()>,
}

fn rank(category: Category) -> u8 {
    match category {
        Category::Esperado => 4,
        Category::Telemetria => 3,
        Category::Rastreador => 2,
        Category::Publicidad => 1,
        Category::Desconocido => 0,
    }
}

/// Lowercase, trailing dot removed.
fn normalize(name: &str) -> String {
    name.trim().trim_end_matches('.').to_ascii_lowercase()
}

/// The name and each parent with at least two labels: `a.b.c.d` → `a.b.c.d`, `b.c.d`, `c.d`.
fn suffixes(name: &str) -> impl Iterator<Item = &str> {
    let mut rest = Some(name);
    std::iter::from_fn(move || {
        let current = rest?;
        rest = current.find('.').map(|i| &current[i + 1..]);
        Some(current)
    })
    .filter(|s| s.contains('.'))
}

impl Catalog {
    /// An empty catalog.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The catalog built from everything embedded in the binary.
    #[must_use]
    pub fn bundled() -> Self {
        let mut c = Self::new();
        c.add_guardiana(ESPERADO, Category::Esperado, OWN_SOURCE_ESPERADO);
        c.add_guardiana(TELEMETRIA, Category::Telemetria, OWN_SOURCE_TELEMETRIA);
        c.add_source(&SOURCES[0], EASYPRIVACY);
        c.add_source(&SOURCES[1], PETERLOWE);
        c.add_evasion(EVASION_DNS);
        c
    }

    /// Add a third-party list's text under its source metadata.
    pub fn add_source(&mut self, source: &SourceInfo, text: &str) {
        let domains: Vec<String> = match source.format {
            Format::Abp => parse_abp_domains(text).map(String::from).collect(),
            Format::Hosts => parse_hosts(text).map(String::from).collect(),
            Format::Guardiana => parse_guardiana(text)
                .map(|(_, d)| String::from(d))
                .collect(),
        };
        for d in domains {
            self.insert(&d, source.category, source.id, None);
        }
    }

    /// Add one of Guardiana's own lists (`@` sections become [`ExpectedKind`]).
    pub fn add_guardiana(&mut self, text: &str, category: Category, source: &'static str) {
        for (section, domain) in parse_guardiana(text) {
            let expected = if category == Category::Esperado {
                section.and_then(ExpectedKind::from_section)
            } else {
                None
            };
            self.insert(domain, category, source, expected);
        }
    }

    /// Add encrypted-resolver names for the `evasion_dns` signal.
    pub fn add_evasion(&mut self, text: &str) {
        for (_, domain) in parse_guardiana(text) {
            self.evasion.insert(normalize(domain), ());
        }
    }

    fn insert(
        &mut self,
        domain: &str,
        category: Category,
        source: &'static str,
        expected: Option<ExpectedKind>,
    ) {
        let key = normalize(domain);
        if key.is_empty() || !key.contains('.') {
            return;
        }
        let replace = self
            .entries
            .get(&key)
            .is_none_or(|e| rank(category) > rank(e.category));
        if replace {
            self.entries.insert(
                key,
                Entry {
                    category,
                    source,
                    expected,
                },
            );
        }
    }

    /// Category of `name`, checking the name and then each parent domain.
    /// The most specific matching entry wins.
    #[must_use]
    pub fn lookup(&self, name: &str) -> Option<Match> {
        let n = normalize(name);
        for candidate in suffixes(&n) {
            if let Some(e) = self.entries.get(candidate) {
                return Some(Match {
                    category: e.category,
                    source: e.source,
                    matched: candidate.to_owned(),
                    expected: e.expected,
                });
            }
        }
        None
    }

    /// Category only, `Desconocido` when no list knows the name.
    #[must_use]
    pub fn category(&self, name: &str) -> Category {
        self.lookup(name)
            .map_or(Category::Desconocido, |m| m.category)
    }

    /// True when `name` is (or is under) a known encrypted resolver.
    #[must_use]
    pub fn is_evasion_resolver(&self, name: &str) -> bool {
        let n = normalize(name);
        let hit = suffixes(&n).any(|s| self.evasion.contains_key(s));
        hit
    }

    /// Number of distinct domains known.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when nothing was loaded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn bundled_catalog_loads_and_matches_known_names() {
        let c = Catalog::bundled();
        assert!(c.len() > 40_000, "got {}", c.len());
        let m = c.lookup("Sub.Doubleclick.Net.").unwrap();
        assert_eq!(m.category, Category::Publicidad);
        assert_eq!(m.source, "peterlowe");
        assert_eq!(m.matched, "doubleclick.net");
        let m = c.lookup("dl.delivery.mp.microsoft.com").unwrap();
        assert_eq!(m.category, Category::Esperado);
        assert_eq!(m.expected, Some(ExpectedKind::Actualizaciones));
        assert_eq!(c.category("example.invalid"), Category::Desconocido);
        assert!(c.is_evasion_resolver("mozilla.cloudflare-dns.com"));
        assert!(!c.is_evasion_resolver("cloudflare.com"));
    }

    #[test]
    fn expected_beats_other_lists_and_more_specific_entry_wins() {
        let mut c = Catalog::new();
        c.add_source(&SOURCES[0], "||microsoft.com^\n");
        c.add_guardiana("@hora\ntime.windows.com\n", Category::Esperado, "own");
        c.add_guardiana(
            "@actualizaciones\nmicrosoft.com\n",
            Category::Esperado,
            "own",
        );
        c.add_source(&SOURCES[1], "0.0.0.0 microsoft.com\n");
        let m = c.lookup("windowsupdate.microsoft.com").unwrap();
        assert_eq!(m.category, Category::Esperado);
        assert_eq!(m.expected, Some(ExpectedKind::Actualizaciones));
        let m = c.lookup("time.windows.com").unwrap();
        assert_eq!(m.expected, Some(ExpectedKind::Hora));
    }

    #[test]
    fn manifest_matches_sources() {
        let m = manifest().unwrap();
        assert_eq!(m.lists.len(), SOURCES.len());
        for (entry, source) in m.lists.iter().zip(SOURCES) {
            assert_eq!(entry.id, source.id);
            assert_eq!(entry.url, source.url);
            assert!(entry.entries > 1000);
        }
    }

    #[test]
    fn suffix_walk_stops_at_two_labels() {
        let v: Vec<&str> = suffixes("a.b.c.d").collect();
        assert_eq!(v, vec!["a.b.c.d", "b.c.d", "c.d"]);
        assert!(suffixes("localhost").next().is_none());
    }
}

#[cfg(test)]
mod company_tests {
    use super::company_of;

    #[test]
    fn longest_suffix_wins_and_unknown_is_none() {
        assert_eq!(company_of("graph.facebook.com"), Some("Meta"));
        assert_eq!(company_of("mcs-sg.tiktokv.com."), Some("ByteDance"));
        assert_eq!(company_of("WWW.MERCADOLIBRE.COM.CO"), Some("Mercado Libre"));
        assert_eq!(company_of("gateway.fe2.apple-dns.net"), Some("Apple"));
        assert_eq!(company_of("e673.dsce9.akamaiedge.net"), Some("Akamai"));
        assert_eq!(company_of("ejemplo.example"), None);
    }

    #[test]
    fn ai_services_are_labelled_by_owner() {
        use super::ai_service_of;
        assert_eq!(ai_service_of("api.anthropic.com"), Some("Anthropic"));
        assert_eq!(ai_service_of("chatgpt.com."), Some("OpenAI"));
        assert_eq!(
            ai_service_of("api.individual.githubcopilot.com"),
            Some("Microsoft Copilot")
        );
        assert_eq!(ai_service_of("gemini.google.com"), Some("Google Gemini"));
        // google.com as a whole is not an AI service: only the names of the list are.
        assert_eq!(ai_service_of("www.google.com"), None);
        assert_eq!(ai_service_of("eltiempo.com"), None);
    }
}
