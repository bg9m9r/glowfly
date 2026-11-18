//! The texts of a chapter row: its commit count, authors, date span and accessible names.

use glowfly_core::chapters::Chapter;
use glowfly_core::i18n::Lang;
use glowfly_core::settings::Settings;

/// "3 commits", or "12+ commits" when the merged-in history goes on below what is loaded.
pub fn count_text(t: Lang, ch: &Chapter) -> String {
    if ch.partial { t.trn("chapters.count_partial", ch.count(), &[]) } else { t.trn("chapters.count", ch.count(), &[]) }
}

/// The authors' names, then how many more there are.
pub fn authors_text(t: Lang, ch: &Chapter) -> String {
    let names = ch.authors.iter().map(|a| a.name()).collect::<Vec<_>>().join(", ");
    if ch.more_authors == 0 { names } else { t.trf("chapters.authors.more", &[("names", &names), ("count", &ch.more_authors)]) }
}

/// The dates of the chapter's oldest and newest commit: one date when both read the same.
pub fn span_text(s: &Settings, ch: &Chapter, now: i64, offset: i32) -> String {
    let (from, to) = (s.format_graph_time(ch.oldest, now, offset), s.format_graph_time(ch.newest, now, offset));
    if from == to { from } else { s.lang().trf("chapters.span", &[("from", &from), ("to", &to)]) }
}

/// The chapter row's accessible name.
pub fn row_name(t: Lang, ch: &Chapter) -> String {
    t.trf("chapters.row.name", &[("title", &ch.title), ("count", &count_text(t, ch))])
}

/// The release heading's accessible name.
pub fn release_name(t: Lang, ch: &Chapter) -> String {
    t.trf("chapters.release.name", &[("tags", &ch.releases.join(", "))])
}
