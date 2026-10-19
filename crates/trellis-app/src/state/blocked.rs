//! Why an action is disabled, in two lengths: the whole sentence for hover and a short form for
//! a menu item's weak suffix (`ui::widgets::reason`). A site can give both; otherwise the short
//! form is derived from the sentence: its first clause, at most `SHORT_MAX` characters.

use std::borrow::Cow;

/// The longest derived short form, in characters, before its "…".
pub const SHORT_MAX: usize = 40;

/// Why an action can't start while another one runs.
pub const BUSY: &str = "Another operation is still running.";

/// Where a sentence's first clause ends.
const CLAUSE_ENDS: [&str; 5] = [". ", ": ", "; ", " (", ", so "];

/// The short form of `full`: cut at the first clause end, a trailing "." dropped (a trailing
/// "…" kept), and longer than `SHORT_MAX` characters cut at the last space within them with
/// "…" added. Casing is left as written.
pub fn short_form(full: &str) -> Cow<'_, str> {
    let full = full.trim();
    let end = CLAUSE_ENDS.iter().filter_map(|e| full.find(e)).min().unwrap_or(full.len());
    let mut clause = full[..end].trim_end();
    if !clause.ends_with("...")
        && let Some(s) = clause.strip_suffix('.')
    {
        clause = s;
    }
    if clause.chars().count() <= SHORT_MAX {
        return Cow::Borrowed(clause);
    }
    // The byte index just past the SHORT_MAX-th character.
    let limit = clause.char_indices().nth(SHORT_MAX).map_or(clause.len(), |(i, _)| i);
    let head = &clause[..limit];
    let cut = match head.rfind(' ') {
        Some(i) if i > 0 => &head[..i],
        _ => head,
    };
    Cow::Owned(format!("{}…", cut.trim_end_matches([' ', ',', ';', ':'])))
}

/// A disabled action's reason: a short form for the menu suffix and the whole sentence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Blocked<'a> {
    pub short: Cow<'a, str>,
    pub full: Cow<'a, str>,
}

impl<'a> Blocked<'a> {
    /// Both forms given by the site.
    pub fn new(short: impl Into<Cow<'a, str>>, full: impl Into<Cow<'a, str>>) -> Self {
        Self { short: short.into(), full: full.into() }
    }

    /// The short form derived from `full`.
    pub fn derived(full: &'a str) -> Self {
        Self { short: short_form(full), full: Cow::Borrowed(full) }
    }
}

impl<'a> From<&'a str> for Blocked<'a> {
    fn from(full: &'a str) -> Self {
        Self::derived(full)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_forms_take_the_first_clause() {
        assert_eq!(short_form("It is locked. Unlock it first."), "It is locked");
        assert_eq!(
            short_form("Pull isn't available on a detached HEAD: check out a branch first."),
            "Pull isn't available on a detached HEAD"
        );
        assert_eq!(short_form("Already at the largest zoom (200%)"), "Already at the largest zoom");
        assert_eq!(short_form("Pick one; then go"), "Pick one");
        assert_eq!(short_form("It changed, so reload it"), "It changed");
        assert!(matches!(short_form("Nothing to undo"), Cow::Borrowed("Nothing to undo")));
        assert_eq!(short_form("Waiting for the service…"), "Waiting for the service…");
        assert_eq!(short_form("Wait until core.sshCommand has been read."), "Wait until core.sshCommand has been read");
        assert_eq!(short_form(BUSY), "Another operation is still running");

        let long = "Choose the branch you want to compare against before starting";
        assert_eq!(long.chars().count(), 61);
        let s = short_form(long);
        assert_eq!(s, "Choose the branch you want to compare…");
        assert!(s.chars().count() <= SHORT_MAX + 1);

        let word = "x".repeat(50);
        assert_eq!(short_form(&word), format!("{}…", "x".repeat(40)));

        // Multibyte text cuts on character boundaries.
        assert_eq!(short_form("Zuerst das Kästchen oben ankreuzen"), "Zuerst das Kästchen oben ankreuzen");
        let umlauts = "ä".repeat(45);
        assert_eq!(short_form(&umlauts), format!("{}…", "ä".repeat(40)));
        let emoji = format!("{}🙂🙂🙂", "a".repeat(39));
        assert_eq!(short_form(&emoji), format!("{}🙂…", "a".repeat(39)));
    }

    #[test]
    fn blocked_keeps_or_derives_its_short_form() {
        let b = Blocked::new("short", "full");
        assert_eq!((b.short.as_ref(), b.full.as_ref()), ("short", "full"));
        let b = Blocked::from("It is locked. Unlock it first.");
        assert_eq!((b.short.as_ref(), b.full.as_ref()), ("It is locked", "It is locked. Unlock it first."));
        assert_eq!(Blocked::derived("Nothing to undo"), Blocked::new("Nothing to undo", "Nothing to undo"));
    }
}
