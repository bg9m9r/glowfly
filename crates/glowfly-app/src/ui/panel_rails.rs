//! The narrow rails that stand in for side panels: the 16 px edge strip a hidden left or details
//! panel leaves (a click shows it again, saved like the menu's item), and the 44 px commit rail
//! the details panel collapses to while a tool view fills the central area (expanding it lasts
//! for that tool view only; `state::layout::details_shape` decides which is drawn).

use super::access;
use super::backdrop::{self, Surface};
use super::theme::{color, spacing, type_scale};
use super::widgets::{self, Icon};
use crate::keymap::{BindingId, KeyAction};
use crate::msg::{LayoutMsg, Msg};
use crate::state::AppState;
use crate::state::layout::{Panel, wip_change_count};
use crate::ui::theme::tokens::radius;
use eframe::egui::{self, CornerRadius, Sense, Stroke, StrokeKind, pos2, vec2};

/// The edge strip's width.
pub const EDGE_W: f32 = 16.0;
/// The collapsed commit panel's width.
pub const RAIL_W: f32 = 44.0;

/// The strip a hidden `panel` (Left or Details) leaves at its edge of the window.
pub(super) fn edge(ctx: &egui::Context, st: &AppState, panel: Panel, msgs: &mut Vec<Msg>) {
    debug_assert!(matches!(panel, Panel::Left | Panel::Details), "only side panels leave a strip");
    let t = st.settings.lang();
    let left = panel == Panel::Left;
    let (side, name, tip, key, icon) = if left {
        let (name, tip) = (t.tr("layout.edge.left.button"), t.tr("layout.edge.left.tip"));
        (egui::SidePanel::left("refs-edge"), name, tip, BindingId::ToggleLeftPanel, Icon::ExpandRight)
    } else {
        let (name, tip) = (t.tr("layout.edge.details.button"), t.tr("layout.edge.details.tip"));
        (egui::SidePanel::right("detail-edge"), name, tip, BindingId::ToggleDetailsPanel, Icon::ExpandLeft)
    };
    // The left strip stands in for the sidebar, so it sits on Mica like it.
    let fill = if left { backdrop::fill(st.view.backdrop, Surface::Side, color::panel()) } else { color::panel() };
    side.exact_width(EDGE_W).resizable(false).show_separator_line(false).frame(egui::Frame::new().fill(fill)).show(ctx, |ui| {
        let rect = ui.max_rect();
        let resp = ui.allocate_rect(rect, Sense::click());
        let p = ui.painter();
        if resp.hovered() {
            p.rect_filled(rect, 0.0, color::hover());
        }
        // The 1 px line on the side facing the central area.
        let x = if left { rect.right() - 0.5 } else { rect.left() + 0.5 };
        p.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(1.0_f32, color::border()));
        let col = if resp.hovered() { color::text() } else { color::weak() };
        widgets::paint_icon_sized(p, rect.center(), icon, col, widgets::icon::SIZE_SM);
        access::name_button(&resp, true, name);
        let resp = resp.on_hover_text(st.keymap.tip(tip, key)).on_hover_cursor(egui::CursorIcon::PointingHand);
        if resp.clicked() {
            msgs.push(Msg::Layout(LayoutMsg::SetPanel { panel, shown: true }));
        }
    });
}

/// The commit panel collapsed beside a tool view: expand, the working tree's change count (when
/// there are changes) and Go to the commit message.
pub(super) fn details_rail(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let count = st.snap.as_ref().map_or(0, wip_change_count);
    egui::SidePanel::right("detail-rail")
        .exact_width(RAIL_W)
        .resizable(false)
        .frame(egui::Frame::new().fill(color::panel()).inner_margin(egui::Margin::symmetric(0, spacing::PANEL_PAD)))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                let expand_tip = t.tr("details.rail.expand.tip");
                let r = widgets::icon_button(ui, Icon::ExpandLeft, expand_tip);
                access::name_button(&r, true, expand_tip);
                if r.clicked() {
                    msgs.push(Msg::Layout(LayoutMsg::ExpandDetails));
                }
                if count > 0 {
                    ui.add_space(8.0);
                    if change_pill(ui, count, &t.trn("details.rail.changes", count, &[])).clicked() {
                        msgs.push(Msg::Layout(LayoutMsg::ExpandDetails));
                    }
                }
                ui.add_space(8.0);
                let r = widgets::icon_button(ui, Icon::Commit, &st.keymap.tip(t.tr("details.rail.commit.tip"), BindingId::FocusMessage));
                access::name_button(&r, true, t.tr("details.rail.commit.tip"));
                if r.clicked() {
                    msgs.push(Msg::Shortcut(KeyAction::FocusMessage));
                }
            });
        });
}

/// A small accent pill with `count`, named and tipped `name`.
fn change_pill(ui: &mut egui::Ui, count: usize, name: &str) -> egui::Response {
    let accent = color::accent();
    let galley = ui.painter().layout_no_wrap(count.to_string(), type_scale::font(type_scale::SMALL), color::text());
    let size = vec2((galley.size().x + 12.0).max(22.0), 20.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    let radius = CornerRadius::same(radius::pill(rect.height()));
    let fill = color::wash(accent, if resp.hovered() { 0.3 } else { 0.18 });
    p.rect(rect, radius, fill, Stroke::new(1.0_f32, accent), StrokeKind::Inside);
    p.galley(rect.center() - galley.size() / 2.0, galley, color::text());
    access::name_button(&resp, true, name);
    resp.on_hover_text(name).on_hover_cursor(egui::CursorIcon::PointingHand)
}

#[cfg(test)]
#[path = "snapshot_tests/snapshot_tests_rails.rs"]
mod snapshot_tests;
