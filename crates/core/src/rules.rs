//! Rule evaluation (brief §6). Pure: given the active rules and one query,
//! which rule, if any, cuts it. The inviolable rules live here:
//!
//! - nothing WIDE is cut for a device observed for less than 24 hours: a whole category or a
//!   rule for the whole home waits the day. A name the person picked for one device (an exact
//!   name or a suffix) is cut from the first minute, as the panel promises when it creates it
//!   (decision of 20 Sep 2026). Until 1.0.1 the engine still made those wait too: the button
//!   turned into «desbloquear» and the name kept resolving for the rest of the first day;
//! - `esperado` names (system updates, resolvers, time, messaging, calls)
//!   are never cut by a category rule, and only by an explicit domain or
//!   suffix rule the user confirmed;
//! - the most specific rule wins (exact name > suffix > category); at equal
//!   specificity `permitir` beats `cortar`, and a device rule beats a home
//!   rule.

use crate::model::{Action, Category, MatchKind, Rule, Scope};
use crate::time::HOUR_MS;

/// Hours a device must be observed before a wide cut (category, whole home) applies (brief §6).
pub const OBSERVATION_HOURS: i64 = 24;

/// One query to evaluate.
#[derive(Debug, Clone, Copy)]
pub struct RuleInput<'a> {
    /// Device the query came from.
    pub device_id: &'a str,
    /// Queried name, lowercase, no trailing dot.
    pub name: &'a str,
    /// Category the lists gave it.
    pub category: Category,
    /// Milliseconds the device has been observed (`now - first_seen`).
    pub observed_ms: i64,
}

/// Whether the device has passed the observation gate.
#[must_use]
pub fn observation_complete(observed_ms: i64) -> bool {
    observed_ms >= OBSERVATION_HOURS * HOUR_MS
}

/// Hours observed so far, capped at the gate, for "X horas de 24".
#[must_use]
pub fn observed_hours(observed_ms: i64) -> i64 {
    (observed_ms / HOUR_MS).clamp(0, OBSERVATION_HOURS)
}

/// A name as people write it, turned into the name a DNS query carries: `https://www.tiktok.com/
/// @x` is `www.tiktok.com`, `*.tiktok.com` is `tiktok.com` with everything below it (the `true`).
/// `None` when what is left is not a host name a query could ask for (no dot, spaces, letters
/// outside ASCII, a label longer than 63).
///
/// Until 1.0.2 a rule kept what was typed, and `https://…` or `*.tiktok.com` were rules that the
/// panel listed as cutting and that never matched a single query; the declared scope of Guard
/// mode dropped any line with a `/` without a word, so `https://github.com` there meant github
/// cut (review of 28 Sep 2026, G4).
#[must_use]
pub fn normalizar_nombre(raw: &str) -> Option<(String, bool)> {
    let mut s = raw.trim().to_ascii_lowercase();
    if let Some((_, resto)) = s.split_once("://") {
        s = resto.to_owned();
    }
    if let Some(fin) = s.find(['/', '?', '#']) {
        s.truncate(fin);
    }
    if let Some((_, host)) = s.rsplit_once('@') {
        s = host.to_owned();
    }
    if let Some((host, puerto)) = s.rsplit_once(':') {
        if !puerto.is_empty() && puerto.bytes().all(|b| b.is_ascii_digit()) {
            s = host.to_owned();
        }
    }
    let mut comodin = false;
    let mut s = s.as_str();
    while let Some(r) = s.strip_prefix("*.").or_else(|| s.strip_prefix('.')) {
        comodin = true;
        s = r;
    }
    let s = s.trim_end_matches('.');
    let valido = s.contains('.')
        && s.len() <= 253
        && s.split('.').all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && l.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        });
    valido.then(|| (s.to_owned(), comodin))
}

fn name_matches(kind: MatchKind, pattern: &str, name: &str, category: Category) -> bool {
    let pattern = pattern.trim().trim_end_matches('.').to_ascii_lowercase();
    match kind {
        MatchKind::Domain => name == pattern,
        MatchKind::Suffix => {
            name == pattern
                || name
                    .strip_suffix(pattern.as_str())
                    .is_some_and(|rest| rest.ends_with('.'))
        }
        MatchKind::Category => pattern.parse::<Category>().is_ok_and(|c| c == category),
    }
}

/// A rule that reaches further than one name the person pointed at: a whole category, or any
/// rule for the whole home. Those wait for 24 hours of observation.
#[must_use]
pub fn is_wide(rule: &Rule) -> bool {
    rule.scope == Scope::Home || rule.match_kind == MatchKind::Category
}

fn specificity(kind: MatchKind) -> u8 {
    match kind {
        MatchKind::Domain => 3,
        MatchKind::Suffix => 2,
        MatchKind::Category => 1,
    }
}

/// The rule that decides this query, if any rule matches. `Some(rule)` with
/// `rule.action == Cortar` means block; `Permitir` means an explicit allow.
#[must_use]
pub fn decide<'r>(rules: &'r [Rule], input: RuleInput<'_>, now: i64) -> Option<&'r Rule> {
    let observed = observation_complete(input.observed_ms);
    let protected = input.category == Category::Esperado;
    let mut best: Option<(&Rule, (u8, u8, u8))> = None;
    for rule in rules.iter().filter(|r| r.is_active(now)) {
        // The same line the panel draws when it creates the rule (`create_rule`, «ancho»): what
        // is wide waits for the day of observation, a name picked for this device does not.
        if !observed && is_wide(rule) {
            continue;
        }
        let in_scope = match rule.scope {
            Scope::Home => true,
            Scope::Device => rule.device_id.as_deref() == Some(input.device_id),
        };
        if !in_scope || !name_matches(rule.match_kind, &rule.pattern, input.name, input.category) {
            continue;
        }
        if protected && rule.action == Action::Cortar {
            // Inviolable: category rules never cut expected traffic; explicit
            // rules only when the user confirmed the warning.
            if rule.match_kind == MatchKind::Category || !rule.confirmed {
                continue;
            }
        }
        let rank = (
            specificity(rule.match_kind),
            u8::from(rule.scope == Scope::Device),
            u8::from(rule.action == Action::Permitir),
        );
        if best.is_none_or(|(_, b)| rank > b) {
            best = Some((rule, rank));
        }
    }
    best.map(|(r, _)| r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_what_a_query_carries_whatever_was_typed() {
        let n = |s: &str| normalizar_nombre(s);
        assert_eq!(n("tiktok.com"), Some(("tiktok.com".into(), false)));
        assert_eq!(n("  TikTok.COM. "), Some(("tiktok.com".into(), false)));
        assert_eq!(
            n("https://www.tiktok.com/@x?y=1#z"),
            Some(("www.tiktok.com".into(), false))
        );
        assert_eq!(
            n("http://user@api.example.com:8443/v1"),
            Some(("api.example.com".into(), false))
        );
        assert_eq!(n("*.tiktok.com"), Some(("tiktok.com".into(), true)));
        assert_eq!(n(".tiktok.com"), Some(("tiktok.com".into(), true)));
        assert_eq!(
            n("_dmarc.example.com"),
            Some(("_dmarc.example.com".into(), false))
        );
        assert_eq!(n("localhost"), None);
        assert_eq!(n("tik tok.com"), None);
        assert_eq!(n("españa.es"), None);
        assert_eq!(n("https://"), None);
        assert_eq!(n("a..b"), None);
        assert_eq!(n(&format!("{}.com", "a".repeat(64))), None);
    }

    fn rule(
        id: i64,
        scope: Scope,
        device: Option<&str>,
        kind: MatchKind,
        pattern: &str,
        action: Action,
        confirmed: bool,
    ) -> Rule {
        Rule {
            id,
            scope,
            device_id: device.map(String::from),
            match_kind: kind,
            pattern: pattern.into(),
            action,
            created_at: 0,
            created_by: "usuario".into(),
            expires_at: None,
            undone_at: None,
            confirmed,
        }
    }

    const DAY: i64 = 24 * HOUR_MS;

    fn input<'a>(device: &'a str, name: &'a str, category: Category) -> RuleInput<'a> {
        RuleInput {
            device_id: device,
            name,
            category,
            observed_ms: 2 * DAY,
        }
    }

    #[test]
    fn nothing_before_24_hours() {
        let rules = vec![rule(
            1,
            Scope::Home,
            None,
            MatchKind::Category,
            "rastreador",
            Action::Cortar,
            false,
        )];
        let mut i = input("phone", "t.example", Category::Rastreador);
        i.observed_ms = 23 * HOUR_MS;
        assert!(decide(&rules, i, 10).is_none());
        i.observed_ms = 24 * HOUR_MS;
        assert_eq!(decide(&rules, i, 10).map(|r| r.id), Some(1));
        // A whole-home rule for one name is wide too: it waits.
        let casa = vec![rule(
            2,
            Scope::Home,
            None,
            MatchKind::Domain,
            "t.example",
            Action::Cortar,
            true,
        )];
        i.observed_ms = HOUR_MS;
        assert!(decide(&casa, i, 10).is_none());
        assert!(!observation_complete(HOUR_MS));
        assert_eq!(observed_hours(HOUR_MS * 5), 5);
        assert_eq!(observed_hours(HOUR_MS * 50), 24);
    }

    #[test]
    fn a_name_picked_for_this_device_is_cut_from_the_first_minute() {
        // What the panel promises when it creates the rule: one name, for one device, cut now.
        // Before 1.0.1 the button said «desbloquear» and the name kept resolving for a day.
        let rules = vec![
            rule(
                1,
                Scope::Device,
                Some("self"),
                MatchKind::Domain,
                "doubleclick.net",
                Action::Cortar,
                true,
            ),
            rule(
                2,
                Scope::Device,
                Some("self"),
                MatchKind::Suffix,
                "tiktok.com",
                Action::Cortar,
                true,
            ),
            rule(
                3,
                Scope::Device,
                Some("self"),
                MatchKind::Category,
                "publicidad",
                Action::Cortar,
                true,
            ),
        ];
        let mut i = input("self", "doubleclick.net", Category::Publicidad);
        i.observed_ms = 60_000;
        assert_eq!(decide(&rules, i, 10).map(|r| r.id), Some(1));
        i.name = "ads.tiktok.com";
        assert_eq!(decide(&rules, i, 10).map(|r| r.id), Some(2));
        // The category rule is wide: it waits for the day.
        i.name = "otro-anuncio.example";
        assert!(decide(&rules, i, 10).is_none());
        i.observed_ms = 25 * HOUR_MS;
        assert_eq!(decide(&rules, i, 10).map(|r| r.id), Some(3));
        // Another device's rule never applies here, early or late.
        i.device_id = "phone";
        i.name = "doubleclick.net";
        assert!(decide(&rules, i, 10).is_none());
    }

    #[test]
    fn specificity_and_allow_win() {
        let rules = vec![
            rule(
                1,
                Scope::Home,
                None,
                MatchKind::Category,
                "publicidad",
                Action::Cortar,
                false,
            ),
            rule(
                2,
                Scope::Home,
                None,
                MatchKind::Suffix,
                "ads.example",
                Action::Permitir,
                false,
            ),
            rule(
                3,
                Scope::Device,
                Some("phone"),
                MatchKind::Domain,
                "x.ads.example",
                Action::Cortar,
                false,
            ),
        ];
        // Category cut, but the suffix allow is more specific.
        assert_eq!(
            decide(
                &rules,
                input("tv", "y.ads.example", Category::Publicidad),
                10
            )
            .map(|r| r.id),
            Some(2)
        );
        // Exact device rule beats the suffix allow.
        assert_eq!(
            decide(
                &rules,
                input("phone", "x.ads.example", Category::Publicidad),
                10
            )
            .map(|r| r.id),
            Some(3)
        );
        // Other device: only the suffix allow applies.
        assert_eq!(
            decide(
                &rules,
                input("tv", "x.ads.example", Category::Publicidad),
                10
            )
            .map(|r| r.id),
            Some(2)
        );
        // Unrelated name with the category: cut by rule 1.
        assert_eq!(
            decide(&rules, input("tv", "z.example", Category::Publicidad), 10).map(|r| r.id),
            Some(1)
        );
        // At equal specificity, permitir wins.
        let tie = vec![
            rule(
                4,
                Scope::Home,
                None,
                MatchKind::Domain,
                "a.example",
                Action::Cortar,
                false,
            ),
            rule(
                5,
                Scope::Home,
                None,
                MatchKind::Domain,
                "a.example",
                Action::Permitir,
                false,
            ),
        ];
        assert_eq!(
            decide(&tie, input("tv", "a.example", Category::Desconocido), 10).map(|r| r.id),
            Some(5)
        );
    }

    #[test]
    fn expected_traffic_is_inviolable_unless_confirmed() {
        let rules = vec![
            rule(
                1,
                Scope::Home,
                None,
                MatchKind::Category,
                "esperado",
                Action::Cortar,
                true,
            ),
            rule(
                2,
                Scope::Home,
                None,
                MatchKind::Suffix,
                "windowsupdate.com",
                Action::Cortar,
                false,
            ),
            rule(
                3,
                Scope::Home,
                None,
                MatchKind::Domain,
                "time.windows.com",
                Action::Cortar,
                true,
            ),
        ];
        assert!(decide(
            &rules,
            input("pc", "dl.windowsupdate.com", Category::Esperado),
            10
        )
        .is_none());
        assert_eq!(
            decide(
                &rules,
                input("pc", "time.windows.com", Category::Esperado),
                10
            )
            .map(|r| r.id),
            Some(3)
        );
    }

    #[test]
    fn undone_and_expired_rules_do_not_apply() {
        let mut r = rule(
            1,
            Scope::Home,
            None,
            MatchKind::Domain,
            "a.example",
            Action::Cortar,
            false,
        );
        r.undone_at = Some(5);
        assert!(decide(&[r], input("tv", "a.example", Category::Desconocido), 10).is_none());
        let mut r = rule(
            2,
            Scope::Home,
            None,
            MatchKind::Domain,
            "a.example",
            Action::Cortar,
            false,
        );
        r.expires_at = Some(5);
        assert!(decide(&[r], input("tv", "a.example", Category::Desconocido), 10).is_none());
    }
}
