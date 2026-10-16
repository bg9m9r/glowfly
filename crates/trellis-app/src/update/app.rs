//! Window-level actions from the menu bar: quitting and the About dialog.

use crate::effect::{Effect, UiEffect};
use crate::msg::AppMsg;
use crate::state::{AppState, Modal};

pub(super) fn handle(st: &mut AppState, msg: AppMsg, fx: &mut Vec<Effect>) {
    match msg {
        // Tabs and settings are saved as they change, so closing needs nothing else.
        AppMsg::Quit => fx.push(Effect::Ui(UiEffect::CloseWindow)),
        // Like the shortcut sheet: not over another dialog, Preferences or a credential prompt,
        // and it replaces the palette.
        AppMsg::ShowAbout => {
            if st.modal.is_none() && st.prefs.is_none() && !st.askpass.showing() {
                st.palette = None;
                st.modal = Some(Modal::About);
            }
        }
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
