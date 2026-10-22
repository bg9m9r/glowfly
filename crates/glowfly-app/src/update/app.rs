//! Window-level actions from the menu bar and a drawn title bar: quitting, the About dialog and its notices,
//! and moving, resizing, minimising or maximising the window, showing its system menu, or
//! entering or leaving full screen (which narrows what the others do, `WindowCmd::in_window`).
//! Every way of quitting (File > Quit, the Quit key, the title bar's Close and the OS asking to
//! close the window) goes through `quit`, which asks first while git work runs
//! (`state/quit_confirm.rs`).

use crate::effect::{Effect, UiEffect};
use crate::msg::AppMsg;
use crate::state::quit_confirm::{asking, quit_confirm};
use crate::state::{AppState, Modal};

pub(super) fn handle(st: &mut AppState, msg: AppMsg, fx: &mut Vec<Effect>) {
    match msg {
        AppMsg::Quit => quit(st, false, fx),
        AppMsg::CloseRequested => quit(st, true, fx),
        AppMsg::QuitConfirmed => {
            st.view.quitting = true;
            fx.push(Effect::Ui(UiEffect::CloseWindow));
        }
        // A full-screen window can't be moved or resized, and Maximize or Restore leaves full
        // screen.
        AppMsg::Window(cmd) => {
            if let Some(cmd) = cmd.in_window(st.view.window) {
                fx.push(Effect::Ui(UiEffect::Window(cmd)));
            }
        }
        // Like the shortcut sheet: not over another dialog, Preferences or a credential prompt,
        // and it replaces the palette. The notices' Back returns to it.
        AppMsg::ShowAbout => {
            if matches!(st.modal, None | Some(Modal::Notices)) && st.prefs.is_none() && !st.askpass.showing() {
                st.palette = None;
                st.modal = Some(Modal::About);
            }
        }
        // Only from About, which it replaces.
        AppMsg::ShowNotices => {
            if matches!(st.modal, Some(Modal::About)) {
                st.modal = Some(Modal::Notices);
            }
        }
    }
}

/// A quit from Glowfly (`os_request` false: it closes the window itself) or the OS asking to
/// close it (`os_request`: it closes after this pass unless cancelled). Tabs and settings are
/// saved as they change, so closing needs nothing else, but while git work runs the quit
/// question opens instead (over any other dialog, closing the palette) and the window stays.
fn quit(st: &mut AppState, os_request: bool, fx: &mut Vec<Effect>) {
    // Quit went ahead: the close that follows passes.
    if st.view.quitting {
        if !os_request {
            fx.push(Effect::Ui(UiEffect::CloseWindow));
        }
        return;
    }
    // Asked again while the question shows: it stays as it is.
    if asking(st) {
        if os_request {
            fx.push(Effect::Ui(UiEffect::CancelClose));
        }
        return;
    }
    if let Some(modal) = quit_confirm(st) {
        if os_request {
            fx.push(Effect::Ui(UiEffect::CancelClose));
        }
        st.palette = None;
        st.modal = Some(modal);
        return;
    }
    st.view.quitting = true;
    if !os_request {
        fx.push(Effect::Ui(UiEffect::CloseWindow));
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
