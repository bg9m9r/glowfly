//! Names inside menu labels ("Rebase *main* onto this branch", "Delete together with
//! *origin/x*…"): each name is cut in its middle to `NAME_MAX` characters, so a long branch
//! name never sets a menu's width and the label's verb stays in view (the menu review's spec
//! §8). The full label goes with the row (`menu_row::item_named`) as its tooltip and its
//! accessible name.

use glowfly_core::i18n::fill;
use std::fmt::Display;

/// The most characters a name inside a menu label keeps.
pub const NAME_MAX: usize = 28;

/// `name`, cut in its middle behind "…" to `NAME_MAX` characters when it is longer: its start
/// and end both matter (a prefix like `feature/`, a ticket number at the end).
pub fn cut(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    if chars.len() <= NAME_MAX {
        return name.to_owned();
    }
    let keep = NAME_MAX - 1;
    let head = keep.div_ceil(2);
    let tail = keep - head;
    chars[..head].iter().chain(&['…']).chain(&chars[chars.len() - tail..]).collect()
}

/// A label as shown (its names cut) and in full.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Named {
    pub shown: String,
    pub full: String,
}

/// The label `template` (a looked-up `t.tr(..)`, so the key scan sees it) with `names` filled
/// in: cut for showing, whole for the tooltip and screen readers.
pub fn named(template: &str, names: &[(&str, &str)]) -> Named {
    let cut_names: Vec<String> = names.iter().map(|(_, n)| cut(n)).collect();
    let shown: Vec<(&str, &dyn Display)> = names.iter().zip(&cut_names).map(|((slot, _), c)| (*slot, c as &dyn Display)).collect();
    let whole: Vec<(&str, &dyn Display)> = names.iter().map(|(slot, n)| (*slot, n as &dyn Display)).collect();
    Named { shown: fill(template, &shown), full: fill(template, &whole) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::i18n::Lang;

    #[test]
    fn long_names_lose_their_middle_and_keep_both_ends() {
        assert_eq!(cut("main"), "main");
        let long = "feature/very-long-branch-name-for-ticket-1234";
        let c = cut(long);
        assert_eq!(c.chars().count(), NAME_MAX);
        assert!(c.starts_with("feature/very-") && c.ends_with("ticket-1234") && c.contains('…'), "{c}");
        assert_eq!(cut(&"x".repeat(NAME_MAX)), "x".repeat(NAME_MAX));
    }

    #[test]
    fn the_verb_stays_and_the_full_label_is_kept() {
        let long = "release/2026-10-the-autumn-cleanup-branch";
        let n = named(Lang::En.tr("branch.menu.merge_into"), &[("branch", long)]);
        assert!(n.shown.starts_with("Merge into ") && n.shown.contains('…'), "{}", n.shown);
        assert_eq!(n.full, format!("Merge into {long}"));
        let short = named(Lang::En.tr("branch.menu.merge_into"), &[("branch", "main")]);
        assert_eq!(short.shown, short.full);
    }
}
