//! The in-window menu bar (system frame): the five titles as buttons, each opening its menu
//! below it as egui's menu bar does (click, and hover while another is open), plus a menu's
//! mnemonic opening it with its first item focused, and Left and Right in an open menu moving
//! to the menu before or after it (wrapping), its first item focused.

use super::menu_anchor::{self, MenuCmd};
use super::{BAR, contents, keys};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::menu_keys::TopMenu;
use crate::ui::menu_keys;
use eframe::egui::{self, Button, Id, Key};

/// Draws the titles inside `egui::menu::bar` (which gives them the bar's look) and the open
/// menu.
pub(super) fn draw(ui: &mut egui::Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let ctx = ui.ctx().clone();
    let id = Id::new(BAR);
    super::keep_inside(&ctx);
    let t = st.settings.lang();
    let marks = keys::marks(st);
    let show = keys::shown(&ctx, st);
    let mut buttons = Vec::with_capacity(TopMenu::ALL.len());
    for m in TopMenu::ALL {
        let r = ui.add(Button::new(keys::title(ui, t, m, &marks, show)));
        let cmd = if st.view.menu_keys.request == Some(m) {
            let k = &mut st.view.menu_keys;
            k.request = None;
            k.keyboard = true;
            k.focus_first = true;
            MenuCmd::Open
        } else {
            MenuCmd::None
        };
        menu_anchor::menu_below(ui, id, &r, cmd, |ui| contents(m, ui, st, msgs));
        buttons.push(r);
    }
    let open = menu_anchor::open_id(&ctx, id).and_then(|open| buttons.iter().position(|b| b.id == open));
    let Some(at) = open else {
        st.view.menu_keys.closed();
        return;
    };
    if !menu_keys::may_step(&ctx) {
        return;
    }
    // The open menu's keys are set aside for it (`menu_keys`): take them from there.
    let d = if menu_keys::take_key(&ctx, Key::ArrowLeft) {
        -1
    } else if menu_keys::take_key(&ctx, Key::ArrowRight) {
        1
    } else {
        0
    };
    if d != 0 {
        let next = TopMenu::ALL[at].step(d);
        let to = TopMenu::ALL.iter().position(|&m| m == next).unwrap_or(at);
        menu_anchor::open(&ctx, id, &buttons[to]);
        st.view.menu_keys.keyboard = true;
        st.view.menu_keys.focus_first = true;
        ctx.request_repaint();
    }
}
