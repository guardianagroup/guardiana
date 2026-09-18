//! Tiny argument parser: `command --flag value --flag=value --switch`.
//! Hand-written to keep the dependency list short (brief §2).

use std::collections::HashMap;

/// Flags that never take a value, so `--canary extra` leaves `extra` positional.
const SWITCHES: &[&str] = &[
    "canary",
    "check",
    "wipe",
    "self",
    "json",
    "csv",
    "yes",
    "status",
    "apply",
    "restore",
    "open",
    "no-panel",
    "self-test",
    "no-open",
];

/// Parsed options after the command word.
#[derive(Debug, Default)]
pub struct Opts {
    values: HashMap<String, Vec<String>>,
    positional: Vec<String>,
}

impl Opts {
    /// Last value given for `--key`, if any.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values
            .get(key)
            .and_then(|v| v.last())
            .map(String::as_str)
    }

    /// Every value given for `--key`.
    pub fn all(&self, key: &str) -> Vec<&str> {
        self.values
            .get(key)
            .map(|v| v.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    /// Whether `--key` appeared, with or without a value.
    pub fn has(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    /// Set a flag programmatically: `Some(value)` for `--key value`, `None` for a switch.
    pub fn set(&mut self, key: &str, value: Option<&str>) {
        let entry = self.values.entry(key.to_owned()).or_default();
        if let Some(v) = value {
            entry.push(v.to_owned());
        }
    }

    /// Words that were not flags.
    /// Append a positional word programmatically.
    pub fn positional_push(&mut self, word: &str) {
        self.positional.push(word.to_owned());
    }

    pub fn positional(&self) -> &[String] {
        &self.positional
    }
}

/// Split `argv` into the command word and its options.
pub fn parse(argv: &[String]) -> (Option<String>, Opts) {
    let mut opts = Opts::default();
    let mut command = None;
    let mut i = 0;
    // `--version`, `-V`, `--help` and `-h` are what anyone types first, and they look like flags, so
    // without this they were swallowed as options and the program opened the test menu instead
    // (seen on the first real Linux install, 15 Sep 2026). Treat them as the command they are.
    if let Some(first) = argv.first() {
        match first.as_str() {
            "--version" | "-V" => return (Some("version".to_owned()), opts),
            "--help" | "-h" => return (Some("help".to_owned()), opts),
            _ => {}
        }
    }
    while i < argv.len() {
        let a = &argv[i];
        if let Some(flag) = a.strip_prefix("--") {
            if let Some((k, v)) = flag.split_once('=') {
                opts.values
                    .entry(k.to_owned())
                    .or_default()
                    .push(v.to_owned());
            } else {
                let next = argv.get(i + 1);
                let takes_value =
                    !SWITCHES.contains(&flag) && next.is_some_and(|n| !n.starts_with("--"));
                let entry = opts.values.entry(flag.to_owned()).or_default();
                if takes_value {
                    if let Some(n) = next {
                        entry.push(n.clone());
                    }
                    i += 1;
                }
            }
        } else if command.is_none() {
            command = Some(a.clone());
        } else {
            opts.positional.push(a.clone());
        }
        i += 1;
    }
    (command, opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn parses_command_flags_and_switches() {
        let (c, o) = parse(&argv(
            "observe --listen 127.0.0.1:5353 --upstream 1.1.1.1 --upstream=9.9.9.9 --canary extra",
        ));
        assert_eq!(c.as_deref(), Some("observe"));
        assert_eq!(o.get("listen"), Some("127.0.0.1:5353"));
        assert_eq!(o.all("upstream"), vec!["1.1.1.1", "9.9.9.9"]);
        assert!(o.has("canary"));
        assert_eq!(o.get("canary"), None);
        assert_eq!(o.positional(), ["extra"]);
        assert!(!o.has("db"));
    }

    #[test]
    fn version_and_help_flags_are_commands_not_options() {
        // They start with "--", so the parser used to file them under options and leave the command
        // empty, which opened the test menu instead of printing the version.
        for (arg, want) in [
            ("--version", "version"),
            ("-V", "version"),
            ("--help", "help"),
            ("-h", "help"),
        ] {
            let (cmd, _) = parse(&[arg.to_owned()]);
            assert_eq!(cmd.as_deref(), Some(want), "{arg}");
        }
    }
}
