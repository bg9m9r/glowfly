//! Hiding and soloing branches and tags in the graph from the left panel: the branch and tag
//! menu items, the markers on hidden and soloed rows, and the banner above the lists. Views only
//! push `GraphMsg::GraphFilter`; `update` saves the filter and reloads.

use super::common::{menu_item, menu_item_if};
use super::theme::color;
use crate::msg::GraphMsg;
use crate::msg::Msg;
use crate::state::branches::HEAD_ALWAYS_SHOWN;
use crate::ui::theme::type_scale;
use eframe::egui::{
    self, Align, Color32, CursorIcon, Layout, Painter, Pos2, Rect, Response, RichText, Sense, Shape, Stroke, UiBuilder, pos2, vec2,
};
use trellis_core::i18n::Lang;
use trellis_core::settings::{FilterOp, GraphFilter};

const HIDDEN_TIP: &str = "Hidden from the graph. Click to show it.";
const SOLO_TIP: &str = "Soloed. Click to stop soloing it.";

fn filter_msg(op: FilterOp) -> Msg {
    Msg::Graph(GraphMsg::GraphFilter(op))
}

/// "Hide/Show in graph" and "Solo/Stop soloing" for the ref `full`, in a menu headed by its
/// name. Hiding is disabled with the reason on hover for the checked-out branch (`head`).
pub fn menu_items(ui: &mut egui::Ui, t: Lang, full: &str, filter: &GraphFilter, head: bool, msgs: &mut Vec<Msg>) {
    let r = full.to_owned();
    if filter.is_hidden(full) {
        menu_item(msgs, ui, t.tr("refs.menu.show_in_graph"), filter_msg(FilterOp::Show(r.clone())));
    } else {
        let blocked = head.then_some(HEAD_ALWAYS_SHOWN);
        menu_item_if(msgs, ui, t.tr("refs.menu.hide_in_graph"), blocked, filter_msg(FilterOp::Hide(r.clone())));
    }
    if filter.is_soloed(full) {
        menu_item(msgs, ui, t.tr("refs.menu.stop_soloing"), filter_msg(FilterOp::Unsolo(r)));
    } else {
        menu_item(msgs, ui, t.tr("refs.menu.solo"), filter_msg(FilterOp::Solo(r)));
    }
}

/// For a hidden or soloed sidebar `row`: dims a hidden row's text and draws a clickable marker
/// in its left gutter (`gutter_x` from the row's left edge) that shows the ref again or stops
/// soloing it. Call it after the row, so the marker is on top for clicks.
pub fn row_marker(ui: &mut egui::Ui, row: &Response, gutter_x: f32, full: &str, label: &str, filter: &GraphFilter, msgs: &mut Vec<Msg>) {
    let hidden = filter.is_hidden(full);
    if !hidden && !filter.is_soloed(full) {
        return;
    }
    let c = pos2(row.rect.left() + gutter_x, row.rect.center().y);
    if hidden {
        // Everything right of the marker: the row's text and its right-hand counts.
        let text = Rect::from_min_max(pos2(c.x + 9.0, row.rect.top()), row.rect.max);
        ui.painter().rect_filled(text, 0.0, color::panel().gamma_multiply(0.55));
    }
    let rect = Rect::from_center_size(c, vec2(18.0, 18.0));
    let r = ui.interact(rect, row.id.with("vis"), Sense::click());
    let (name, tip, op) = if hidden {
        (format!("Show {label} in graph"), HIDDEN_TIP, FilterOp::Show(full.to_owned()))
    } else {
        (format!("Stop soloing {label}"), SOLO_TIP, FilterOp::Unsolo(full.to_owned()))
    };
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
    let r = r.on_hover_text(tip).on_hover_cursor(CursorIcon::PointingHand);
    let col = if r.hovered() { color::text() } else { color::weak() };
    if hidden {
        paint_eye(ui.painter(), c, true, col);
    } else {
        paint_solo(ui.painter(), c, col);
    }
    if r.clicked() {
        msgs.push(filter_msg(op));
    }
}

/// What the graph is filtered by, with a button to undo it: one row while soloing, one while
/// refs are hidden. Nothing when the filter is empty.
pub fn banner(ui: &mut egui::Ui, filter: &GraphFilter, msgs: &mut Vec<Msg>) {
    let names = |refs: &[String]| refs.iter().map(|r| GraphFilter::short(r)).collect::<Vec<_>>().join("\n");
    if !filter.solo.is_empty() {
        let text = solo_text(&filter.solo);
        let tip = format!("Soloing:\n{}\n\nThe checked-out branch is always shown.", names(&filter.solo));
        let icon = |p: &Painter, c: Pos2| paint_solo(p, c, color::accent());
        banner_row(ui, icon, &text, &tip, ("Stop soloing", FilterOp::StopSoloing), msgs);
    }
    if !filter.hidden.is_empty() {
        let text = format!("{} hidden from the graph", filter.hidden.len());
        let tip = format!("Hidden:\n{}", names(&filter.hidden));
        let icon = |p: &Painter, c: Pos2| paint_eye(p, c, true, color::weak());
        banner_row(ui, icon, &text, &tip, ("Show all", FilterOp::ShowAll), msgs);
    }
    if !filter.is_empty() {
        ui.add_space(6.0);
    }
}

/// The banner's solo line: the ref's short name for one, else how many and of which kind.
fn solo_text(solo: &[String]) -> String {
    if let [one] = solo {
        return format!("Soloing {}", GraphFilter::short(one));
    }
    let tags = solo.iter().filter(|r| r.starts_with("refs/tags/")).count();
    let kind = match tags {
        0 => "branches",
        t if t == solo.len() => "tags",
        _ => "branches and tags",
    };
    format!("Soloing {} {kind}", solo.len())
}

fn banner_row(ui: &mut egui::Ui, icon: impl Fn(&Painter, Pos2), text: &str, tip: &str, button: (&str, FilterOp), msgs: &mut Vec<Msg>) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
    let inner = rect.shrink2(vec2(8.0, 0.0));
    icon(ui.painter(), pos2(inner.left() + 8.0, inner.center().y));
    let area = Rect::from_min_max(pos2(inner.left() + 22.0, inner.top()), inner.max);
    ui.allocate_new_ui(UiBuilder::new().max_rect(area).layout(Layout::right_to_left(Align::Center)), |ui| {
        let (label, op) = button;
        if ui.small_button(label).clicked() {
            msgs.push(filter_msg(op));
        }
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            let l = egui::Label::new(RichText::new(text).size(type_scale::SMALL).color(color::text())).truncate().selectable(false);
            ui.add(l).on_hover_text(tip);
        });
    });
}

/// An eye drawn with lines (the bundled fonts have no eye glyph); `crossed` strikes it through.
fn paint_eye(p: &Painter, c: Pos2, crossed: bool, col: Color32) {
    let s = Stroke::new(1.4_f32, col);
    for sign in [-1.0_f32, 1.0] {
        let pts = (0..=12).map(|i| {
            let x = -6.0 + i as f32;
            pos2(c.x + x, c.y + sign * 3.5 * (1.0 - (x / 6.0).powi(2)))
        });
        p.add(Shape::line(pts.collect(), s));
    }
    p.circle_filled(c, 1.8, col);
    if crossed {
        p.line_segment([pos2(c.x - 5.5, c.y + 5.5), pos2(c.x + 5.5, c.y - 5.5)], s);
    }
}

/// A ring with a filled center: the solo marker.
fn paint_solo(p: &Painter, c: Pos2, col: Color32) {
    p.circle_stroke(c, 5.0, Stroke::new(1.4_f32, col));
    p.circle_filled(c, 2.0, col);
}

#[cfg(test)]
mod tests {
    use super::solo_text;

    fn refs(r: &[&str]) -> Vec<String> {
        r.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn solo_text_names_one_ref_and_counts_by_kind() {
        assert_eq!(solo_text(&refs(&["refs/heads/main"])), "Soloing main");
        assert_eq!(solo_text(&refs(&["refs/tags/v1.0"])), "Soloing v1.0");
        assert_eq!(solo_text(&refs(&["refs/heads/a", "refs/remotes/origin/b"])), "Soloing 2 branches");
        assert_eq!(solo_text(&refs(&["refs/tags/a", "refs/tags/b"])), "Soloing 2 tags");
        assert_eq!(solo_text(&refs(&["refs/heads/a", "refs/tags/b", "refs/tags/c"])), "Soloing 3 branches and tags");
    }
}
