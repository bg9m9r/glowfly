//! The shell's side of the macOS system menu bar (`platform::native_menu`): before each frame's
//! input it runs the actions of the items chosen there, which do what the drawn items do; after
//! the frame's messages it hands over the menus as the state now has them.

use crate::msg::Msg;
use crate::platform::native_menu::NativeMenu;
use crate::state::AppState;
use crate::state::menu_model;

/// Runs the actions of the items chosen since the last frame.
pub(super) fn picked(menu: &mut NativeMenu, st: &mut AppState, msgs: &mut Vec<Msg>) {
    for action in menu.take_picked() {
        action.run(st, msgs);
    }
}

/// Shows the menus of `st` in the menu bar.
pub(super) fn sync(menu: &mut NativeMenu, st: &AppState) {
    menu.show(menu_model::build(st), st.settings.lang());
}
