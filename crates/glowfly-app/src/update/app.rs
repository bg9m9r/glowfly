//! Window-level actions from the menu bar and a drawn title bar: quitting, the About dialog and its notices,
//! and moving, resizing, minimising or maximising the window, showing its system menu, or
//! entering or leaving full screen (which narrows what the others do, `WindowCmd::in_window`).

use crate::effect::{Effect, UiEffect};
use crate::msg::AppMsg;
use crate::state::{AppState, Modal};

pub(super) fn handle(st: &mut AppState, msg: AppMsg, fx: &mut Vec<Effect>) {
    match msg {
        // Tabs and settings are saved as they change, so closing needs nothing else.
        AppMsg::Quit => fx.push(Effect::Ui(UiEffect::CloseWindow)),
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

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
