//! The window's panels in drawing order: the status bar, the top chrome (the drawn title bar
//! with the tabs, or with the system frame the menu bar and the tab bar; then the toolbar,
//! operation strip and banners), the side panels with their rails, the central view (or the
//! maximised terminal panel in its place) and the tour. egui gives each panel the room the ones before it left, so the order is the layout.

use super::toasts::ToastBounds;
use super::{
    UiCache, activity_log, central, details, git_banner, lfs_banner, menu_bar, op_strip, panel_rails, panel_size, prefs, sidebar,
    sign_in_banner, status, tabs, terminal, title_bar, toolbar, tour,
};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::layout::{DetailsShape, Panel, details_shape};
use eframe::egui;

/// Draws every panel of the window (Preferences in place of all but the status bar and a drawn
/// title bar while open); returns where the chrome ends, for the toasts.
pub(super) fn show(ctx: &egui::Context, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) -> ToastBounds {
    status::status_bar(ctx, st, msgs);
    let bottom = ctx.available_rect().bottom();
    let drawn = st.view.chrome.drawn();
    if drawn {
        title_bar::show(ctx, st, msgs);
    }
    if st.prefs.is_some() {
        // The terminal isn't drawn, so it can't hold the keyboard (and Esc closes Preferences).
        st.terminal.focused = false;
        let top = ctx.available_rect().top();
        prefs::show(ctx, st, msgs);
        return ToastBounds { top, bottom };
    }
    if !drawn {
        menu_bar::show(ctx, st, msgs);
        tabs::show(ctx, st, msgs);
    }
    if st.settings.show_toolbar {
        toolbar::show(ctx, st, msgs);
    }
    op_strip::show(ctx, st, msgs);
    git_banner::show(ctx, st, msgs);
    if st.snap.is_some() && !st.dashboard.open {
        sign_in_banner::show(ctx, st, msgs);
        lfs_banner::show(ctx, st, msgs);
    }
    let top = ctx.available_rect().top();
    activity_log::show(ctx, st, msgs);
    side_panels(ctx, st, cache, msgs);
    let central = if crate::state::terminal::maximized_shown(st) {
        terminal::show_in_main(ctx, st, msgs)
    } else {
        central::show(ctx, st, cache, msgs)
    };
    tour::show(ctx, st, central, msgs);
    ToastBounds { top, bottom }
}

/// The left and details drawers, or the rails standing in for them, and the terminal. The rails
/// go first, so the drawers are fitted to the room they leave.
fn side_panels(ctx: &egui::Context, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    if st.snap.is_none() || st.dashboard.open {
        st.terminal.focused = false;
        return;
    }
    if !st.settings.show_left_panel {
        panel_rails::edge(ctx, st, Panel::Left, msgs);
    }
    let shape = details_shape(st);
    match shape {
        DetailsShape::Hidden => panel_rails::edge(ctx, st, Panel::Details, msgs),
        DetailsShape::Rail => panel_rails::details_rail(ctx, st, msgs),
        DetailsShape::Full => {}
    }
    if shape != DetailsShape::Full {
        panel_size::forget(ctx, panel_size::DETAILS_ID);
    }
    let (left, right) = panel_size::side_fits(ctx, st);
    if let Some(fit) = left {
        sidebar::show(ctx, st, fit, msgs);
    }
    if let Some(fit) = right {
        details::show(ctx, st, cache, fit, msgs);
    }
    terminal::show(ctx, st, msgs);
}
