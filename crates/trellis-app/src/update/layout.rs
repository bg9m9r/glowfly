//! Zooming the window and showing or hiding its panels: both are settings, saved when they
//! change; the shell applies them on the next frame.

use crate::effect::Effect;
use crate::msg::LayoutMsg;
use crate::state::AppState;
use crate::state::layout::{Panel, set_shown, shown, zoom_toast_text};
use std::time::Instant;
use trellis_core::settings::{DEFAULT_ZOOM_PERCENT, zoom_in_from, zoom_out_from};

pub(super) fn handle(st: &mut AppState, msg: LayoutMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        LayoutMsg::ZoomIn => zoom(st, zoom_in_from(st.settings.zoom_percent), now, fx),
        LayoutMsg::ZoomOut => zoom(st, zoom_out_from(st.settings.zoom_percent), now, fx),
        LayoutMsg::ZoomReset => zoom(st, DEFAULT_ZOOM_PERCENT, now, fx),
        LayoutMsg::TogglePanel(p) => {
            let on = !shown(&st.settings, p);
            set(st, p, on, fx);
        }
        LayoutMsg::SetPanel { panel, shown } => set(st, panel, shown, fx),
    }
}

/// Shows the commit details panel if it is hidden (the commit box is in it).
pub(super) fn reveal_details(st: &mut AppState, fx: &mut Vec<Effect>) {
    set(st, Panel::Details, true, fx);
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

fn set(st: &mut AppState, p: Panel, on: bool, fx: &mut Vec<Effect>) {
    if set_shown(&mut st.settings, p, on) {
        fx.push(super::prefs::save_settings(st));
    }
}
