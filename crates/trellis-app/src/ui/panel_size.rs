//! Hands the drawers' fitted sizes (`state::panel_layout`) to egui. egui keeps a panel's last
//! width in its memory and clamps it to whatever room the window had, so a window that
//! shrank, or a drawer dragged too small, would stay that way; instead each frame the drawer
//! is set to its fitted size, except while its edge is dragged (egui then follows the pointer
//! within the fitted range) and on the frame the drag ends, when the new size is reported.

use crate::msg::{LayoutMsg, Msg};
use crate::state::AppState;
use crate::state::layout::{DetailsShape, details_shape};
use crate::state::panel_layout::{self, DETAILS, Fit, MAIN_MIN_WIDTH, SIDEBAR, Spec};
use eframe::egui::{self, Id, Pos2, Rect, Vec2, containers::panel::PanelState};
use trellis_core::settings::SizedPanel;

/// The egui ids of the drawers sized here.
pub const SIDEBAR_ID: &str = "refs";
pub const DETAILS_ID: &str = "detail";
pub const ACTIVITY_ID: &str = "activity";

/// Whether the edge of panel `id` is being dragged.
pub fn dragging(ctx: &egui::Context, id: Id) -> bool {
    ctx.is_being_dragged(id.with("__resize"))
}

/// Before drawing panel `id`: sets its size to `size` unless its edge is held, or was held
/// last frame (`was_dragging`: the frame the drag ends keeps the dragged size until it is saved).
pub fn pin(ctx: &egui::Context, id: Id, size: f32, was_dragging: bool) {
    if was_dragging || dragging(ctx, id) {
        return;
    }
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(size.max(0.0)));
    ctx.data_mut(|d| d.insert_persisted(id, PanelState { rect }));
}

/// For a panel not drawn this frame: forgets the rect egui keeps for it, so nothing reads the
/// last frame's panel as still there (a drawer standing aside for its rail, say). Its size
/// comes back from the settings through `pin`.
pub fn forget(ctx: &egui::Context, id: &str) {
    ctx.data_mut(|d| d.remove::<PanelState>(Id::new(id)));
}

/// After drawing a drawer whose size is saved in the settings: tracks the drag of its edge
/// and, when one ends, reports the size it was dragged to (`size`, the drawn length).
pub fn track(ctx: &egui::Context, st: &mut AppState, p: SizedPanel, id: Id, size: f32, msgs: &mut Vec<Msg>) {
    let now = dragging(ctx, id);
    let was = st.view.panel_drag == Some(p);
    if was && !now {
        msgs.push(Msg::Layout(LayoutMsg::PanelSize { panel: p, size }));
    }
    if now {
        st.view.panel_drag = Some(p);
    } else if was {
        st.view.panel_drag = None;
    }
}

/// The left and details drawers' fits for this frame (for the ones drawn in full), from the
/// width left beside the panels drawn so far.
pub fn side_fits(ctx: &egui::Context, st: &AppState) -> (Option<Fit>, Option<Fit>) {
    let room = ctx.available_rect().width();
    let sizes = &st.settings.panel_sizes;
    let (left_on, details_on) = (st.settings.show_left_panel, details_shape(st) == DetailsShape::Full);
    let mut drawers = Vec::with_capacity(2);
    if left_on {
        drawers.push((SIDEBAR, sizes.get(SizedPanel::Sidebar)));
    }
    if details_on {
        drawers.push((DETAILS, sizes.get(SizedPanel::Details)));
    }
    let mut fits = panel_layout::fit(room, MAIN_MIN_WIDTH, &drawers).into_iter();
    let left = left_on.then(|| fits.next()).flatten();
    let details = details_on.then(|| fits.next()).flatten();
    (left, details)
}

/// A resizable side panel at `fit`'s size (after `pin`).
pub fn side(panel: egui::SidePanel, fit: Fit) -> egui::SidePanel {
    panel.resizable(true).default_width(fit.size).width_range(fit.lo..=fit.hi)
}

/// First thing inside a side drawer: makes its content span the drawer. egui stores a panel's
/// width as its content's, so a drawer of short labels would otherwise narrow itself.
pub fn fill(ui: &mut egui::Ui) {
    ui.set_min_width(ui.available_width());
}

/// A resizable side panel inside a view `room` px wide, sized by egui within `spec`'s limits
/// for that room (it isn't saved, so egui's memory of it is enough).
pub fn inner_side(panel: egui::SidePanel, spec: Spec, room: f32) -> egui::SidePanel {
    let (lo, hi) = spec.range_for(room);
    panel.resizable(true).default_width(spec.default_for(room).clamp(lo, hi)).width_range(lo..=hi)
}

/// `inner_side` for a bottom panel inside a view `room` px tall.
pub fn inner_bottom(panel: egui::TopBottomPanel, spec: Spec, room: f32) -> egui::TopBottomPanel {
    let (lo, hi) = spec.range_for(room);
    panel.resizable(true).default_height(spec.default_for(room).clamp(lo, hi)).height_range(lo..=hi)
}

#[cfg(test)]
#[path = "panel_size_tests.rs"]
mod tests;
