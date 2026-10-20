//! A menu that drops below a custom-drawn button, like a menu bar's: a click opens and closes
//! it, hovering another button of the same `id` while one is open moves to that one, Esc and a
//! click elsewhere close it, and code can open or close it too (the title bar's Alt tap).
//! Returns whether it is open, so the caller can paint the button's open look.

use eframe::egui::{self, Id, Response};
use egui::menu::{BarState, MenuRoot};

/// Shows `contents` in a menu below `button` while it is open; `toggle` opens it (or closes it
/// when open). Buttons sharing `id` behave as one menu bar. Returns whether `button`'s menu is
/// open after this frame.
pub(super) fn menu_below(ui: &egui::Ui, id: Id, button: &Response, toggle: bool, contents: impl FnOnce(&mut egui::Ui)) -> bool {
    let mut bar = BarState::load(ui.ctx(), id);
    if toggle {
        **bar = if bar.is_some() { None } else { Some(MenuRoot::new(button.rect.left_bottom(), button.id)) };
    }
    bar.bar_menu(button, contents);
    let open = bar.as_ref().is_some_and(|r| r.id == button.id);
    bar.store(ui.ctx(), id);
    open
}
