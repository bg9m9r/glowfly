//! The keyboard focus of the lists that only open files (`state::list_keys::FileList`: a
//! commit's, a comparison's, the pull request's, the folder history's, Conflicted and Resolved
//! Files) on `list_focus.rs`: one stable focus target per list, its plain moves taken by the
//! shortcuts before any binding as `KeyAction::FileList` messages (Shift, Space, Enter, Delete,
//! Left and Right are left for the bindings: Delete is still Drop commit), a click on a row
//! giving the list the focus, and the focus ring on its current row (or, with none, the list).

use crate::keymap::KeyAction;
use crate::msg::Msg;
use crate::state::list_keys::{FileList, KeySet, ListKey};
use eframe::egui;

/// The focus target of `list`.
pub(crate) fn list_id(list: FileList) -> egui::Id {
    egui::Id::new(("file-list-focus", list))
}

/// Which of these lists has the keyboard focus, if one does.
pub(crate) fn focused(ctx: &egui::Context) -> Option<FileList> {
    let focused = ctx.memory(|m| m.focused())?;
    FileList::ALL.into_iter().find(|&l| list_id(l) == focused)
}

/// This frame's moves in the focused list as messages, before the bindings see them; nothing
/// while a dialog, Preferences, the palette or a credential prompt is `covered` over it.
pub(crate) fn input(ctx: &egui::Context, covered: bool, msgs: &mut Vec<Msg>) {
    let Some(list) = focused(ctx).filter(|_| !covered) else { return };
    for key in super::list_focus::take(ctx, list_id(list), KeySet::MOVES) {
        if let ListKey::Move(mv) = key {
            msgs.push(Msg::Shortcut(KeyAction::FileList { list, mv }));
        }
    }
}

/// A list's focus this frame, from `target`.
pub(super) struct Focus {
    list: FileList,
    pub focused: bool,
}

/// Registers `list`'s focus target over the `rows` rows it is about to draw, at most `max_h`
/// high, named `name` (its heading). Register it before the rows.
pub(super) fn target(ui: &mut egui::Ui, list: FileList, rows: usize, max_h: f32, name: &str) -> Focus {
    let pitch = super::widgets::LIST_ROW_H + ui.spacing().item_spacing.y;
    let h = (rows.max(1) as f32 * pitch).min(max_h.max(pitch));
    let rect = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), h));
    // A page keeps one row of the last in view.
    let page = ((max_h / pitch).floor() as usize).saturating_sub(1);
    let r = super::list_focus::target(ui, list_id(list), rect, name, page);
    Focus { list, focused: r.has_focus() }
}

impl Focus {
    /// After drawing a row (`r`): a click on it gives the list the focus, and the list's current
    /// row (`current`) is ringed while it has it.
    pub(super) fn row(&self, ui: &egui::Ui, r: &egui::Response, current: bool) {
        if r.clicked() {
            super::list_focus::request(ui.ctx(), list_id(self.list));
        }
        if self.focused && current {
            super::widgets::file_row_focus(ui, r.rect);
        }
    }

    /// After the rows: rings the whole list (the part its rows fill) while it has the focus and
    /// none of its files is current.
    pub(super) fn rest<R>(&self, ui: &egui::Ui, shown: &egui::scroll_area::ScrollAreaOutput<R>, current: bool) {
        if self.focused && !current {
            super::widgets::file_row_focus(ui, shown_rect(shown));
        }
    }
}

/// The part of a scroll area its content fills (the viewport can be taller).
pub(super) fn shown_rect<R>(out: &egui::scroll_area::ScrollAreaOutput<R>) -> egui::Rect {
    let r = out.inner_rect;
    egui::Rect::from_min_size(r.min, egui::vec2(r.width(), out.content_size.y.min(r.height())))
}
