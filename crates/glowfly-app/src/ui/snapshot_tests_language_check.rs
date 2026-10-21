//! The English check the language snapshot tests share: every text a frame painted and every
//! accessibility node's text, compared with a list of the English an area used to show. A `*`
//! in a list entry stands for a name or number ("Merge into *").

use super::menu_testing::node_text;
use eframe::egui::Shape;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::i18n::Lang;

/// Every text the last frame painted (`Shape::Text` galleys, also inside `Shape::Vec`) and the
/// text of every accessibility node.
pub(super) fn shown_texts<S>(h: &Harness<'_, S>) -> Vec<String> {
    fn walk(shape: &Shape, out: &mut Vec<String>) {
        match shape {
            Shape::Text(t) => out.push(t.galley.text().to_owned()),
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for clipped in &h.output().shapes {
        walk(&clipped.shape, &mut out);
    }
    out.extend(h.query_all_by(|_| true).map(|n| node_text(&n)));
    out
}

/// `text` with each `{name}` placeholder made a `*`, to compare with a list's entries.
pub(super) fn starred(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        match rest[open..].find('}') {
            Some(close) => {
                out.push_str(&rest[..open]);
                out.push('*');
                rest = &rest[open + close + 1..];
            }
            None => break,
        }
    }
    out.push_str(rest);
    out
}

/// The entries of `list` that are not also German text (German keeps "Commit", "Graph",
/// "Blame").
pub(super) fn checkable(list: &[&'static str]) -> Vec<&'static str> {
    let german: Vec<String> = Lang::De.keys().map(|(_, v)| starred(v)).collect();
    list.iter().copied().filter(|e| !german.iter().any(|g| g == e)).collect()
}

/// Whether the shown `text` is the English `entry`: equal, or with each `*` standing for
/// something non-empty.
pub(super) fn matches(entry: &str, text: &str) -> bool {
    let parts: Vec<&str> = entry.split('*').collect();
    if parts.len() == 1 {
        return text == entry;
    }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if !text.starts_with(first) || !text.ends_with(last) || text.len() <= first.len() + last.len() {
        return false;
    }
    let mut at = first.len();
    let end = text.len() - last.len();
    for mid in &parts[1..parts.len() - 1] {
        match text[at..end].find(mid) {
            Some(i) if i > 0 => at += i + mid.len(),
            _ => return false,
        }
    }
    at < end
}

/// The shown strings that are one of `list`'s English texts; at least `min` of them must be
/// left to check once the ones German shares are dropped.
pub(super) fn english_shown<S>(h: &Harness<'_, S>, list: &[&'static str], min: usize) -> Vec<String> {
    let entries = checkable(list);
    assert!(entries.len() >= min, "only {} English texts are left to check", entries.len());
    let mut found: Vec<String> = shown_texts(h).into_iter().filter(|t| entries.iter().any(|e| matches(e, t.trim()))).collect();
    found.sort();
    found.dedup();
    found
}
