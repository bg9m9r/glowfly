//! A menu's focusable items, found after its contents are drawn, in rows: the widgets drawn
//! straight into the menu's `Ui` (walked by their auto ids, the way a `Ui` counts them up) and
//! the ones that registered themselves (`register`: menu rows, submenu rows, split buttons,
//! repository rows and their star and ×, the filter field; widgets with ids of their own or in
//! nested scopes the walk can't see). Items sharing a line form one row, left to right: a
//! split row and its button, a repository row and its star and ×.

use crate::ui::widgets::menu_submenu::row_at;
use eframe::egui::{Id, Rect, Response, Ui};

/// One focusable item.
#[derive(Clone, Debug)]
pub(super) struct Item {
    pub id: Id,
    pub rect: Rect,
    /// The label typeahead matches, in the active language.
    pub label: Option<String>,
    /// Whether it opens a submenu (activating it doesn't close a popup).
    pub opens: bool,
}

#[derive(Clone, Default)]
struct Registry {
    pass: u64,
    items: Vec<(Id, Option<String>, bool)>,
}

fn key(ui: &Ui) -> Id {
    Id::new(("menu-keys-items", ui.layer_id()))
}

/// Makes `resp` an item of the menu `ui` is in, labelled `label` for typeahead; `opens` for a
/// submenu row.
pub(in crate::ui) fn register(ui: &Ui, resp: &Response, label: Option<&str>, opens: bool) {
    let pass = ui.ctx().cumulative_pass_nr();
    ui.data_mut(|d| {
        let r = d.get_temp_mut_or_default::<Registry>(key(ui));
        if r.pass != pass {
            *r = Registry { pass, items: Vec::new() };
        }
        r.items.push((resp.id, label.map(str::to_owned), opens));
    });
}

/// Where the walk starts: the auto-id counter of `ui`'s next widget.
pub(super) fn start(ui: &Ui) -> Option<u64> {
    let next = ui.next_auto_id();
    let base = ui.unique_id().value().wrapping_add(1);
    (0..256u64).map(|j| base.wrapping_add(j)).find(|&s| Id::new(s) == next)
}

/// The menu's items after drawing, in rows from the top, each left to right.
pub(super) fn rows(ui: &Ui, start: Option<u64>) -> Vec<Vec<Item>> {
    let ctx = ui.ctx();
    let layer = ui.layer_id();
    let pass = ctx.cumulative_pass_nr();
    let next = ui.next_auto_id();
    let walked: Vec<Id> =
        start.map(|s| (0..1024u64).map(|k| Id::new(s.wrapping_add(k))).take_while(|&id| id != next).collect()).unwrap_or_default();
    let registered = ui.data(|d| d.get_temp::<Registry>(key(ui))).filter(|r| r.pass == pass).map(|r| r.items).unwrap_or_default();
    let mut items: Vec<Item> = Vec::new();
    let candidates = walked.into_iter().map(|id| (row_at(ctx, id), None, false)).chain(registered);
    for (id, label, opens) in candidates {
        let Some(r) = ctx.read_response(id).filter(|r| r.layer_id == layer && r.sense.is_focusable() && r.enabled()) else { continue };
        match items.iter_mut().find(|i| i.id == id) {
            // Registered after the walk found it: the registration knows its label.
            Some(it) => {
                it.label = it.label.take().or(label);
                it.opens |= opens;
            }
            None => items.push(Item { id, rect: r.rect, label, opens }),
        }
    }
    items.sort_by(|a, b| a.rect.top().total_cmp(&b.rect.top()).then(a.rect.left().total_cmp(&b.rect.left())));
    let mut rows: Vec<Vec<Item>> = Vec::new();
    for it in items {
        match rows.last_mut() {
            Some(row) if row[0].rect.y_range().contains(it.rect.center().y) => row.push(it),
            _ => rows.push(vec![it]),
        }
    }
    for row in &mut rows {
        row.sort_by(|a, b| a.rect.left().total_cmp(&b.rect.left()));
    }
    rows
}

/// Where `id` is in `rows`: its row and its place in the row.
pub(super) fn find(rows: &[Vec<Item>], id: Id) -> Option<(usize, usize)> {
    rows.iter().enumerate().find_map(|(r, row)| row.iter().position(|i| i.id == id).map(|p| (r, p)))
}

/// The next item after `from` (or from the top) whose label starts with `c`, case-folded,
/// wrapping past the last.
pub(super) fn typeahead(rows: &[Vec<Item>], from: Option<Id>, c: char) -> Option<Id> {
    let flat: Vec<&Item> = rows.iter().flatten().collect();
    let at = from.and_then(|f| flat.iter().position(|i| i.id == f));
    let n = flat.len();
    let starts = |i: &Item| {
        i.label.as_deref().and_then(|l| l.trim_start().chars().next()).is_some_and(|first| first.to_lowercase().eq(c.to_lowercase()))
    };
    (1..=n).map(|k| flat[(at.unwrap_or(n - 1) + k) % n]).find(|i| starts(i)).map(|i| i.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{pos2, vec2};

    fn item(id: &str, label: &str, y: f32) -> Item {
        Item { id: Id::new(id), rect: Rect::from_min_size(pos2(0.0, y), vec2(100.0, 28.0)), label: Some(label.into()), opens: false }
    }

    #[test]
    fn typeahead_finds_the_next_label_case_folded_and_wraps() {
        let rows = vec![vec![item("a", "Copy", 0.0)], vec![item("b", "cherry-pick", 30.0)], vec![item("c", "Revert", 60.0)]];
        assert_eq!(typeahead(&rows, None, 'c'), Some(Id::new("a")));
        assert_eq!(typeahead(&rows, Some(Id::new("a")), 'C'), Some(Id::new("b")));
        assert_eq!(typeahead(&rows, Some(Id::new("b")), 'c'), Some(Id::new("a")), "wraps");
        assert_eq!(typeahead(&rows, Some(Id::new("a")), 'x'), None);
        let rows = vec![vec![item("e", "Éditer", 0.0)]];
        assert_eq!(typeahead(&rows, None, 'é'), Some(Id::new("e")), "beyond ASCII");
    }
}
