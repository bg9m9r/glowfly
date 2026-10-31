//! A menu that drops below a custom-drawn button, like a menu bar's: a click opens and closes
//! it (and Enter, Space or Alt+Down on the focused button, `widgets/menu_open.rs`), hovering
//! another button of the same `id` while one is open moves to that one, Esc and a click
//! elsewhere close it, and code can open or close it too (the title bar's Alt tap, a
//! mnemonic, Left and Right between the bar's menus). Returns whether it is open, so the
//! caller can paint the button's open look. Its contents get the shared menu style
//! (`widgets/menu.rs`).

use eframe::egui::{self, Id, Pos2, Response};
use egui::menu::{BarState, MenuRoot};

/// What code does to a button's menu this frame, before the pointer has its say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MenuCmd {
    None,
    /// Open it, or close it when a menu of the bar is open.
    Toggle,
    /// Open it (in place of another open menu of the bar); an open one stays as it is.
    Open,
}

/// Shows `contents` in a menu below `button` while it is open, after applying `cmd`. Buttons
/// sharing `id` behave as one menu bar. Returns whether `button`'s menu is open after this
/// frame.
pub(super) fn menu_below(ui: &egui::Ui, id: Id, button: &Response, cmd: MenuCmd, contents: impl FnOnce(&mut egui::Ui)) -> bool {
    let mut bar = BarState::load(ui.ctx(), id);
    match cmd {
        MenuCmd::None => {}
        MenuCmd::Toggle => **bar = if bar.is_some() { None } else { Some(MenuRoot::new(button.rect.left_bottom(), button.id)) },
        MenuCmd::Open => {
            if bar.as_ref().is_none_or(|r| r.id != button.id) {
                **bar = Some(MenuRoot::new(below(button), button.id));
            }
        }
    }
    super::widgets::menu_open::dropdown(&mut bar, button, false, |ui| super::widgets::menu::styled(ui, contents));
    let open = bar.as_ref().is_some_and(|r| r.id == button.id);
    bar.store(ui.ctx(), id);
    open
}

/// Opens `button`'s menu of the bar `id` in place of the open one, from code; it shows from
/// the next frame.
pub(super) fn open(ctx: &egui::Context, id: Id, button: &Response) {
    let mut bar = BarState::load(ctx, id);
    **bar = Some(MenuRoot::new(below(button), button.id));
    bar.store(ctx, id);
}

/// Closes the open menu of the bar `id`, if any.
pub(super) fn close(ctx: &egui::Context, id: Id) {
    let mut bar = BarState::load(ctx, id);
    if bar.is_some() {
        **bar = None;
        bar.store(ctx, id);
    }
}

/// The button whose menu of the bar `id` is open.
pub(super) fn open_id(ctx: &egui::Context, id: Id) -> Option<Id> {
    BarState::load(ctx, id).as_ref().map(|r| r.id)
}

/// Where a click opens `button`'s menu: below it, its first item lined up with the button.
fn below(button: &Response) -> Pos2 {
    super::widgets::menu_open::below_pos(button)
}
