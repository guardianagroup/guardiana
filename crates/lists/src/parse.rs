//! Parsers for the three text formats (docs/LISTS.md). Each returns the bare
//! domains; anything a DNS resolver cannot apply is skipped.

use serde::Serialize;

/// Text format of a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    /// Adblock Plus syntax; only `||domain^` rules without options are used.
    Abp,
    /// hosts file: `127.0.0.1 domain` or `0.0.0.0 domain`.
    Hosts,
    /// Guardiana's own: one domain per line, `#` comments, `@section` headers.
    Guardiana,
}

fn is_domain_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '.' || c == '-'
}

fn looks_like_domain(s: &str) -> bool {
    !s.is_empty()
        && s.contains('.')
        && !s.starts_with('.')
        && !s.ends_with('.')
        && !s.starts_with('-')
        && s.chars().all(is_domain_char)
}

/// Domains from `||domain^` rules. Rules with options, wildcards, paths or
/// exceptions are ignored because a resolver cannot honour them.
pub fn parse_abp_domains(text: &str) -> impl Iterator<Item = &str> {
    text.lines().filter_map(|line| {
        let line = line.trim();
        let rest = line.strip_prefix("||")?;
        let domain = rest.strip_suffix('^')?;
        looks_like_domain(domain).then_some(domain)
    })
}

/// Domains from hosts-format lines pointing at `127.0.0.1` or `0.0.0.0`.
pub fn parse_hosts(text: &str) -> impl Iterator<Item = &str> {
    text.lines().filter_map(|line| {
        let line = line.split('#').next().unwrap_or("").trim();
        let mut parts = line.split_whitespace();
        let ip = parts.next()?;
        if ip != "127.0.0.1" && ip != "0.0.0.0" {
            return None;
        }
        let domain = parts.next()?;
        (looks_like_domain(domain) && domain != "localhost.localdomain").then_some(domain)
    })
}

/// `(section, domain)` pairs from Guardiana's own format. `section` is the
/// last `@name` header seen, without the `@`.
pub fn parse_guardiana(text: &str) -> impl Iterator<Item = (Option<&str>, &str)> {
    let mut section: Option<&str> = None;
    text.lines().filter_map(move |line| {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            return None;
        }
        if let Some(name) = line.strip_prefix('@') {
            section = Some(name.trim());
            return None;
        }
        looks_like_domain(line).then_some((section, line))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abp_keeps_only_plain_domain_rules() {
        let text = "! comment\n||a.example^\n||b.example^$third-party\n||c.example/path\n@@||d.example^\n||*.e.example^\n||f.example^\n";
        let v: Vec<&str> = parse_abp_domains(text).collect();
        assert_eq!(v, vec!["a.example", "f.example"]);
    }

    #[test]
    fn hosts_reads_both_sinkhole_addresses() {
        let text = "# c\n127.0.0.1 a.example # trailing\n0.0.0.0 b.example\n127.0.0.1 localhost\n::1 c.example\n";
        let v: Vec<&str> = parse_hosts(text).collect();
        assert_eq!(v, vec!["a.example", "b.example"]);
    }

    #[test]
    fn guardiana_tracks_sections() {
        let text = "# c\n@hora\npool.ntp.org\n\n@mensajeria\nwhatsapp.net # x\nbad_name\n";
        let v: Vec<(Option<&str>, &str)> = parse_guardiana(text).collect();
        assert_eq!(
            v,
            vec![
                (Some("hora"), "pool.ntp.org"),
                (Some("mensajeria"), "whatsapp.net")
            ]
        );
    }
}
