//! The narrow rails that stand in for side panels: the 16 px edge strip a hidden left or details
//! panel leaves (a click shows it again, saved like the menu's item), and the 36 px commit rail
//! the details panel collapses to while a tool view, the pull request panel or the rebase editor
//! fills the central area (expanding it lasts
//! for that tool view only; `state::layout::details_shape` decides which is drawn).

use super::access;
use super::backdrop::{self, Surface};
use super::theme::color;
use super::theme::sizes::ChipSize;
use super::widgets::chip::{Chip, Look};
use super::widgets::{self, Icon};
use eframe::egui::{self, Sense, Stroke, pos2};
use glowfly_model::keymap::{BindingId, KeyAction};
use glowfly_model::msg::{LayoutMsg, Msg};
use glowfly_model::state::AppState;
use glowfly_model::state::layout::{Panel, wip_change_count};

/// The edge strip's width.
const EDGE_W: f32 = 16.0;
/// The collapsed commit panel's width, and the room above and between its entries.
const RAIL_W: f32 = 36.0;
const RAIL_GAP: f32 = 8.0;

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
    let fill = if left { backdrop::fill(st.cues.backdrop, Surface::Side, color::panel()) } else { color::panel() };
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

/// The commit panel collapsed beside a tool view, the pull request panel or the rebase editor:
/// Show changes and commit (it expands), the working tree's change count as a badge (when there
/// are changes; it expands too) and Commit… (it expands and focuses the summary). Centred, 8 px
/// from the top and apart.
pub(super) fn details_rail(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let count = st.snap.as_ref().map_or(0, wip_change_count);
    egui::SidePanel::right("detail-rail")
        .exact_width(RAIL_W)
        .resizable(false)
        .frame(egui::Frame::new().fill(color::panel()).inner_margin(egui::Margin::symmetric(0, RAIL_GAP as i8)))
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = RAIL_GAP;
            ui.vertical_centered(|ui| {
                let expand = t.tr("details.rail.expand.tip");
                let r = widgets::icon_button(ui, Icon::ExpandLeft, &st.keymap.tip(expand, BindingId::FocusCommitMessage));
                access::name_button(&r, true, expand);
                if r.clicked() {
                    msgs.push(Msg::Layout(LayoutMsg::ExpandDetails));
                }
                if count > 0 && count_badge(ui, count, &t.trn("details.rail.changes", count, &[])).clicked() {
                    msgs.push(Msg::Layout(LayoutMsg::ExpandDetails));
                }
                let commit = t.tr("details.rail.commit.tip");
                let r = widgets::icon_button(ui, Icon::Commit, commit);
                access::name_button(&r, true, commit);
                if r.clicked() {
                    msgs.push(Msg::Shortcut(KeyAction::FocusMessage));
                }
            });
        });
}

/// The change count as an 18 px neutral badge, named and tipped `name`.
fn count_badge(ui: &mut egui::Ui, count: usize, name: &str) -> egui::Response {
    let resp = Chip::new(ChipSize::Badge, count.to_string()).look(Look::new(color::raised(), color::text())).button(ui);
    access::name_button(&resp, true, name);
    resp.on_hover_text(name)
}

#[cfg(test)]
#[path = "snapshot_tests/snapshot_tests_rails.rs"]
mod snapshot_tests;
