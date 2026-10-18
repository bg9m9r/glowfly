//! The rebase panel's keys, while it can be edited, no editor is open, nothing has the keyboard
//! focus and nothing covers the window: the arrows select a row (with Alt they move it), the
//! action letters set a commit's action, X and B add an exec or break step, Delete or
//! Backspace removes one, and Enter opens the selected row's editor. Keep these in step with
//! the fixed "in the interactive rebase panel" rows in `keymap/table.rs`. Also scrolls a newly
//! selected row into view.

use super::rows::Slot;
use crate::msg::{Msg, RebaseMsg};
use crate::state::rebase::RebasePanel;
use crate::state::rebase_steps::NewStep;
use eframe::egui::{Event, Id, Key, Modifiers, Ui};
use trellis_core::rebase::{RebaseAction, TodoStep};

/// Handles this frame's panel keys; `covered`: a dialog, the palette, Preferences or a
/// credential prompt is open.
pub(super) fn keys(ui: &Ui, covered: bool, panel: &RebasePanel, slots: &[Slot], msgs: &mut Vec<Msg>) {
    reveal(ui, panel, slots);
    let focused = ui.ctx().memory(|m| m.focused().is_some());
    if covered || focused || !panel.editable() || panel.editing.is_some() {
        return;
    }
    ui.ctx().input_mut(|i| {
        let mut n = 0;
        while n < i.events.len() {
            let msg = match &i.events[n] {
                Event::Key { key, pressed: true, modifiers, .. } => action(*key, *modifiers, panel),
                _ => None,
            };
            match msg {
                Some(m) => {
                    msgs.push(Msg::Rebase(m));
                    i.events.remove(n);
                }
                None => n += 1,
            }
        }
    });
}

/// What a press of `key` with `m` does to the panel's list, if anything. Alt is checked
/// exactly: egui's own key matching ignores an extra Alt or Shift.
fn action(key: Key, m: Modifiers, panel: &RebasePanel) -> Option<RebaseMsg> {
    let n = panel.items.len();
    let sel = panel.selected.filter(|&s| s < n);
    let alt_only = m.alt && !m.command && !m.ctrl && !m.mac_cmd && !m.shift;
    if alt_only {
        let s = sel?;
        return match key {
            Key::ArrowUp if s > 0 => Some(RebaseMsg::RebaseMove { from: s, to: s - 1 }),
            Key::ArrowDown if s + 1 < n => Some(RebaseMsg::RebaseMove { from: s, to: s + 1 }),
            _ => None,
        };
    }
    if !m.is_none() {
        return None;
    }
    let commit = sel.filter(|&s| panel.items[s].commit().is_some());
    let set = |action: RebaseAction| commit.map(|index| RebaseMsg::RebaseSetAction { index, action });
    match key {
        Key::ArrowUp | Key::ArrowDown => {
            let next = panel.next_selection(if key == Key::ArrowDown { 1 } else { -1 })?;
            (sel != Some(next)).then_some(RebaseMsg::RebaseSelect(next))
        }
        Key::P => set(RebaseAction::Pick),
        Key::R => set(RebaseAction::Reword),
        Key::S => set(RebaseAction::Squash),
        Key::F => set(RebaseAction::Fixup),
        Key::D => set(RebaseAction::Drop),
        Key::E => set(RebaseAction::Edit),
        Key::X => Some(RebaseMsg::RebaseAddStep { after: sel, step: NewStep::Exec }),
        Key::B => Some(RebaseMsg::RebaseAddStep { after: sel, step: NewStep::Break }),
        Key::Delete | Key::Backspace => {
            sel.filter(|&s| matches!(panel.items[s], TodoStep::Exec(_) | TodoStep::Break)).map(RebaseMsg::RebaseRemoveStep)
        }
        Key::Enter => sel.filter(|&s| !matches!(panel.items[s], TodoStep::Break)).map(RebaseMsg::RebaseEditMessage),
        _ => None,
    }
}

/// Scrolls the selected row into view when the selection changed.
fn reveal(ui: &Ui, panel: &RebasePanel, slots: &[Slot]) {
    let id = Id::new("rebase-selected-shown");
    let shown = ui.data(|d| d.get_temp::<Option<usize>>(id)).flatten();
    if shown == panel.selected {
        return;
    }
    ui.data_mut(|d| d.insert_temp(id, panel.selected));
    if let Some(slot) = panel.selected.and_then(|s| slots.get(s)) {
        ui.scroll_to_rect(slot.rect, None);
    }
}
