//! Zooming the window, showing or hiding its panels and the sizes its drawers were dragged to:
//! all are settings, saved when they change; the shell applies them on the next frame. Expanding
//! the commit panel collapsed beside a tool view is view state only (`view.details_expanded_for`),
//! never saved, and lasts while that tool view stays open. Full screen is the window's own
//! state, read each frame into `view.window` and never saved: the toggle only asks the shell.

use crate::effect::{Effect, UiEffect};
use crate::msg::{LayoutMsg, Msg};
use crate::state::AppState;
use crate::state::layout::{Panel, set_shown, shown, tool_view, zoom_toast_text};
use crate::state::panel_layout;
use crate::state::toasts::ToastAction;
use crate::state::window_chrome::WindowCmd;
use std::time::Instant;
use trellis_core::settings::{DEFAULT_ZOOM_PERCENT, zoom_in_from, zoom_out_from};

pub(super) fn handle(st: &mut AppState, msg: LayoutMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        LayoutMsg::ZoomIn => zoom(st, zoom_in_from(st.settings.zoom_percent), now, fx),
        LayoutMsg::ZoomOut => zoom(st, zoom_out_from(st.settings.zoom_percent), now, fx),
        LayoutMsg::ZoomReset => zoom(st, DEFAULT_ZOOM_PERCENT, now, fx),
        LayoutMsg::ToggleFullscreen => fx.push(Effect::Ui(UiEffect::Window(WindowCmd::SetFullscreen(!st.view.window.fullscreen)))),
        LayoutMsg::TogglePanel(p) => {
            let on = !shown(&st.settings, p);
            set_and_note(st, p, on, now, fx);
        }
        LayoutMsg::SetPanel { panel, shown } => set_and_note(st, panel, shown, now, fx),
        LayoutMsg::PanelSize { panel, size } => {
            if let Some(v) = panel_layout::spec(panel).sanitize(size)
                && st.settings.panel_sizes.set(panel, v)
            {
                fx.push(super::prefs::save_settings(st));
            }
        }
        LayoutMsg::ExpandDetails => st.view.details_expanded_for = tool_view(st),
    }
}

/// `set`, and when that hid the menu bar, a toast that brings it back (it has no key, and the
/// View menu that showed it is gone).
fn set_and_note(st: &mut AppState, p: Panel, on: bool, now: Instant, fx: &mut Vec<Effect>) {
    if set(st, p, on, fx) && p == Panel::MenuBar && !on {
        let back = Msg::Layout(LayoutMsg::SetPanel { panel: Panel::MenuBar, shown: true });
        let action = ToastAction { label: "Show menu bar".into(), msg: Box::new(back), workdir: None };
        st.toast_action(true, "Menu bar hidden", action, now);
    }
}

/// Shows the commit details panel if it is hidden, and in full if it is collapsed beside a
/// tool view (the commit box is in it).
pub(super) fn reveal_details(st: &mut AppState, fx: &mut Vec<Effect>) {
    set(st, Panel::Details, true, fx);
    st.view.details_expanded_for = tool_view(st);
}

/// Moves to zoom level `new` (saving it when it differs) and shows the level in the zoom
/// toast, updating the one still up from the last step rather than stacking another.
fn zoom(st: &mut AppState, new: u16, now: Instant, fx: &mut Vec<Effect>) {
    if new != st.settings.zoom_percent {
        st.settings.zoom_percent = new;
        fx.push(super::prefs::save_settings(st));
    }
    let text = zoom_toast_text(st.settings.zoom_level());
    st.view.zoom_toast = Some(st.toast_replacing(st.view.zoom_toast, true, text, now));
}

/// Shows or hides `p`, saving when that changed it; returns whether it did.
fn set(st: &mut AppState, p: Panel, on: bool, fx: &mut Vec<Effect>) -> bool {
    let changed = set_shown(&mut st.settings, p, on);
    if changed {
        fx.push(super::prefs::save_settings(st));
    }
    changed
}
