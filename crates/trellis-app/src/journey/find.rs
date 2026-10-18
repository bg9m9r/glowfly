//! Finding widgets by their accessible names, the way a user finds them by their visible
//! labels: inside one region of the window, and picking one when several share a label.

use eframe::egui::accesskit::Role;
use eframe::egui::{self, Id, Pos2, Rect};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Node, Queryable};

/// A part of the window, from the panels' rectangles of the last frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Region {
    Anywhere,
    Toolbar,
    /// The left panel: branches, remotes, tags, stashes.
    Sidebar,
    /// The right panel: the commit or working-tree details.
    Details,
    /// What no panel covers: the graph, a diff, the conflict view, the welcome; never the
    /// open command palette.
    Central,
    /// The open command palette: its field and rows.
    Palette,
}

/// Which of several matching widgets to take.
#[derive(Clone, Copy, Debug)]
pub(super) enum Pick<'a> {
    /// Exactly one must match.
    Only,
    /// The first in the tree's order (drawing order).
    First,
    Last,
    Leftmost,
    Rightmost,
    /// The one on the same line as the widget labelled with this text, nearest to it.
    InRowOf(&'a str),
}

/// Every panel the regions are cut from, by its egui id.
const PANELS: [&str; 6] = ["toolbar", "refs", "detail", "tabs", "status", "activity"];

fn panel(ctx: &egui::Context, id: &str) -> Option<Rect> {
    egui::containers::panel::PanelState::load(ctx, Id::new(id)).map(|p| p.rect)
}

/// The centre of `node`'s bounds, in points.
pub(super) fn centre(node: &Node<'_>) -> Option<Pos2> {
    let b = node.raw_bounds()?;
    Some(Pos2::new(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32))
}

fn in_region(ctx: &egui::Context, region: Region, node: &Node<'_>) -> bool {
    let Some(c) = centre(node) else { return false };
    let inside = |id: &str| panel(ctx, id).is_some_and(|r| r.contains(c));
    let in_palette = || crate::ui::palette_rect(ctx).is_some_and(|r| r.contains(c));
    match region {
        Region::Anywhere => true,
        Region::Toolbar => inside("toolbar"),
        Region::Sidebar => inside("refs"),
        Region::Details => inside("detail"),
        Region::Central => !PANELS.iter().any(|id| inside(id)) && !in_palette(),
        Region::Palette => in_palette(),
    }
}

/// The text a node shows: a label's value, any other widget's label.
pub(super) fn text_of(node: &Node<'_>) -> Option<String> {
    if node.role() == Role::Label { node.value() } else { node.label() }
}

/// The nodes in `region` whose name is `label` (or contains it when `exact` is false).
fn candidates<'t, S>(h: &'t Harness<'_, S>, label: &'t str, exact: bool, region: Region) -> Vec<Node<'t>> {
    let by = if exact { By::new().label(label) } else { By::new().label_contains(label) };
    h.query_all(by).filter(|n| in_region(&h.ctx, region, n)).collect()
}

/// The widget labelled `label` in `region`, picked by `pick`; None when nothing (or, for
/// `Pick::Only`, more than one widget) matches.
pub(super) fn find<'t, S>(h: &'t Harness<'_, S>, label: &'t str, exact: bool, region: Region, pick: Pick<'t>) -> Option<Node<'t>> {
    let mut nodes = candidates(h, label, exact, region);
    let x = |n: &Node<'_>| centre(n).map_or(0.0, |c| c.x);
    match pick {
        Pick::Only => (nodes.len() == 1).then(|| nodes.remove(0)),
        Pick::First => nodes.into_iter().next(),
        Pick::Last => nodes.pop(),
        Pick::Leftmost => nodes.into_iter().min_by(|a, b| x(a).total_cmp(&x(b))),
        Pick::Rightmost => nodes.into_iter().max_by(|a, b| x(a).total_cmp(&x(b))),
        Pick::InRowOf(anchor) => {
            let a = find(h, anchor, true, region, Pick::First)?;
            let ab = a.raw_bounds()?;
            let ax = x(&a);
            let aid = a.id();
            nodes
                .into_iter()
                .filter(|n| n.id() != aid && centre(n).is_some_and(|c| f64::from(c.y) >= ab.y0 && f64::from(c.y) <= ab.y1))
                .min_by(|p, q| (x(p) - ax).abs().total_cmp(&(x(q) - ax).abs()))
        }
    }
}

/// How many widgets in `region` are labelled `label`.
pub(super) fn count<S>(h: &Harness<'_, S>, label: &str, exact: bool, region: Region) -> usize {
    candidates(h, label, exact, region).len()
}

/// Every name shown in `region`, in tree order, for failure messages.
pub(super) fn labels_in<S>(h: &Harness<'_, S>, region: Region) -> Vec<String> {
    h.query_all(By::new()).filter(|n| in_region(&h.ctx, region, n)).filter_map(|n| text_of(&n)).filter(|t| !t.trim().is_empty()).collect()
}

/// The first text field in `region`.
pub(super) fn field_in<'t, S>(h: &'t Harness<'_, S>, region: Region) -> Option<Node<'t>> {
    h.query_all(By::new()).find(|n| matches!(n.role(), Role::TextInput | Role::MultilineTextInput) && in_region(&h.ctx, region, n))
}

/// The text field named `name`.
pub(super) fn field<'t, S>(h: &'t Harness<'_, S>, name: &'t str) -> Option<Node<'t>> {
    h.query_all(By::new().label(name)).find(|n| matches!(n.role(), Role::TextInput | Role::MultilineTextInput))
}
