//! The shortcut sheet's Menus group: the keys every menu answers to (`ui/menu_keys`). They are
//! fixed and no binding of the keymap table runs them, so the sheet lists them from here, after
//! the table's groups, filtered the same way.

use crate::keymap::{Chord, KeyName as K};

/// The group's title.
pub(super) const TITLE: &str = "Menus";

/// The text shown instead of key caps on the typeahead row.
pub(super) const ANY_LETTER: &str = "A letter";

/// One row: what the keys do, and the keys (none: `ANY_LETTER`).
pub(super) struct Row {
    pub label: &'static str,
    pub keys: &'static [Chord],
}

const fn row(label: &'static str, keys: &'static [Chord]) -> Row {
    Row { label, keys }
}

pub(super) const ROWS: [Row; 9] = [
    row("Open the focused or selected row's menu", &[Chord::shift(K::F(10))]),
    row("Open a menu button's menu", &[Chord::plain(K::Enter), Chord::plain(K::Space), Chord::alt(K::Down)]),
    row("Move to the row above or below, wrapping", &[Chord::plain(K::Up), Chord::plain(K::Down)]),
    row("Go to the first or last row", &[Chord::plain(K::Home), Chord::plain(K::End)]),
    row("Open a submenu, or reach a row's own button", &[Chord::plain(K::Right)]),
    row("Close a submenu, or go back to the row", &[Chord::plain(K::Left)]),
    row("Run the focused item", &[Chord::plain(K::Enter), Chord::plain(K::Space)]),
    row("Close one menu level", &[Chord::plain(K::Escape)]),
    row("Jump to the next row starting with a letter", &[]),
];

/// The keys of `r` as text, joined with " or ".
pub(super) fn keys_text(r: &Row) -> String {
    if r.keys.is_empty() { ANY_LETTER.to_owned() } else { r.keys.iter().map(Chord::text).collect::<Vec<_>>().join(" or ") }
}

/// The rows matching `query` as the table's rows do: every term in the label, the title or the
/// keys, ignoring case.
pub(super) fn filtered(query: &str) -> Vec<&'static Row> {
    let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    ROWS.iter()
        .filter(|r| {
            let hay = format!("{} {TITLE} {}", r.label, keys_text(r)).to_lowercase();
            terms.iter().all(|t| hay.contains(t.as_str()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_menu_rows_filter_by_label_title_and_key() {
        assert_eq!(filtered("").len(), ROWS.len());
        assert_eq!(filtered("menus").len(), ROWS.len(), "the title matches every row");
        assert_eq!(filtered("f10").len(), 1);
        assert_eq!(filtered("first last").len(), 1);
        assert!(filtered("nothing like this").is_empty());
    }
}
