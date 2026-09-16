//! Rule evaluation (brief §6). Pure: given the active rules and one query,
//! which rule, if any, cuts it. The inviolable rules live here:
//!
//! - nothing is cut for a device observed for less than 24 hours;
//! - `esperado` names (system updates, resolvers, time, messaging, calls)
//!   are never cut by a category rule, and only by an explicit domain or
//!   suffix rule the user confirmed;
//! - the most specific rule wins (exact name > suffix > category); at equal
//!   specificity `permitir` beats `cortar`, and a device rule beats a home
//!   rule.

use crate::model::{Action, Category, MatchKind, Rule, Scope};
use crate::time::HOUR_MS;

/// Hours a device must be observed before any cut applies (brief §6).
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
    if !observation_complete(input.observed_ms) {
        return None;
    }
    let protected = input.category == Category::Esperado;
    let mut best: Option<(&Rule, (u8, u8, u8))> = None;
    for rule in rules.iter().filter(|r| r.is_active(now)) {
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
        assert!(!observation_complete(HOUR_MS));
        assert_eq!(observed_hours(HOUR_MS * 5), 5);
        assert_eq!(observed_hours(HOUR_MS * 50), 24);
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
