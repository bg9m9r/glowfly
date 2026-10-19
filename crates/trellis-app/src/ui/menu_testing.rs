//! Opening a menu's submenu and finding its items in UI tests. A submenu's entries can share a
//! label with something else on screen (a REMOTE group row named "origin", the graph's "SHA"
//! column header), so an item is found by where it sits: right of its parent, nearest to it.

use crate::msg::Msg;
use crate::state::AppState;
use egui_kittest::Harness;
use egui_kittest::kittest::Node;
use egui_kittest::kittest::Queryable;

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// Hovers the submenu button `parent` so its submenu opens.
pub(super) fn open_submenu(h: &mut H, parent: &str) {
    h.get_by_label(parent).hover();
    // A loading pane's spinner keeps repainting; stop at the step limit then.
    h.run_ok();
}

/// A node's text: a label widget keeps it as its value, a button as its label.
pub(super) fn node_text(n: &Node) -> String {
    n.value().filter(|v| !v.is_empty()).or_else(|| n.label()).unwrap_or_default()
}

/// The text of every node whose label contains `text`.
pub(super) fn texts_containing(h: &H, text: &str) -> Vec<String> {
    h.query_all_by_label_contains(text).map(|n| node_text(&n)).collect()
}

/// The entry `label` of the open submenu `parent`: the match beside the parent's button whose
/// centre is nearest the parent's. Panics when there is none.
pub(super) fn submenu_item<'a>(h: &'a H, parent: &str, label: &'a str) -> Node<'a> {
    let p = h.get_by_label(parent).bounding_box().expect("the submenu button");
    let (px, py) = ((p.x0 + p.x1) / 2.0, (p.y0 + p.y1) / 2.0);
    let dist = |n: &Node| {
        // Out past the menu's edge: right of it (egui shifts a submenu without room left, over
        // the menu), or left of it.
        n.bounding_box().filter(|b| b.x1 > p.x1 + 4.0 || b.x1 < p.x0 + 4.0).map(|b| {
            let (x, y) = ((b.x0 + b.x1) / 2.0 - px, (b.y0 + b.y1) / 2.0 - py);
            x * x + y * y
        })
    };
    h.get_all_by_label(label)
        .filter_map(|n| dist(&n).map(|d| (d, n)))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, n)| n)
        .unwrap_or_else(|| panic!("no {label} in the {parent} submenu"))
}
