//! A file list that takes the keyboard focus as a whole, as one Tab stop: its focus target
//! (an id the list registers over its rows each frame), the keys it takes while it has the
//! focus (`state::list_keys::ListKey`, read before any binding by the shortcuts), and its
//! arrows kept from egui's own focus moves. egui reads the arrows for those moves at the start
//! of a frame, before the list can lock them, so on the first frame after the list took the
//! focus an arrow would also carry the focus away: the list takes it back on the next frame
//! (`restore`). For the WIP lists (`wip_keys.rs`) and any other list built the same way.

use crate::state::list_keys::{ListKey, ListMove};
use eframe::egui::{self, Event, EventFilter, Id, Key, Modifiers, WidgetInfo, WidgetType};

/// The arrows are the list's; Tab and Esc still move and drop the focus.
const FILTER: EventFilter = EventFilter { vertical_arrows: true, horizontal_arrows: true, tab: false, escape: false };

/// Registers the list's focus target `id` over `rect` (its rows, or its "none" line), named
/// `name`, with `page` rows in view (Page Up and Page Down move that many). Register it before
/// the rows: Tab reaches it after what comes before the list and before the rows' own buttons,
/// and as it senses no clicks a press on a row goes to the row.
pub(crate) fn target(ui: &mut egui::Ui, id: Id, rect: egui::Rect, name: &str, page: usize) -> egui::Response {
    let r = ui.interact(rect, id, egui::Sense::focusable_noninteractive());
    r.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, name));
    let ctx = ui.ctx();
    if r.has_focus() {
        ctx.memory_mut(|m| m.set_focus_lock_filter(id, FILTER));
        // egui keeps the filter only from a list's second frame with the focus on.
        if ctx.memory(|m| m.had_focus_last_frame(id)) {
            let pass = ctx.cumulative_pass_nr();
            ctx.data_mut(|d| d.insert_temp(id.with("locked"), pass));
        }
    }
    ctx.data_mut(|d| d.insert_temp(id.with("page"), page.max(1)));
    r
}

/// Gives the list `id` the keyboard focus (a click on one of its rows, or a key that goes to it).
pub(crate) fn request(ctx: &egui::Context, id: Id) {
    ctx.memory_mut(|m| m.request_focus(id));
}

fn restore_id() -> Id {
    Id::new("list-focus-restore")
}

/// At the start of a frame's input: gives the focus back to the list an arrow carried it away
/// from on its first frame, and draws one more frame so its lock is in place for the next key.
pub(crate) fn restore(ctx: &egui::Context) {
    // Held as an `Option`, which the temp store can default.
    let Some(Some(id)) = ctx.data_mut(|d| d.remove_temp::<Option<Id>>(restore_id())) else { return };
    if ctx.memory(|m| m.focused()) != Some(id) {
        request(ctx, id);
    }
    ctx.request_repaint();
}

/// Takes this frame's keys of the list `id` while it has the focus, in the order pressed: plain
/// Up/Down, Page Up/Page Down and Home/End (repeats too), the same with Shift alone, and plain
/// Space, Enter, Delete (or Mod+Backspace), Left and Right (not their repeats). Other keys and
/// modifiers are left for the bindings and egui.
pub(crate) fn take(ctx: &egui::Context, id: Id) -> Vec<ListKey> {
    if !ctx.memory(|m| m.has_focus(id)) {
        return Vec::new();
    }
    let page = ctx.data(|d| d.get_temp::<usize>(id.with("page"))).unwrap_or(1);
    let pass = ctx.cumulative_pass_nr();
    let locked = ctx.data(|d| d.get_temp::<u64>(id.with("locked"))).is_some_and(|p| p + 1 == pass);
    let mut keys = Vec::new();
    let mut arrow = false;
    ctx.input_mut(|i| {
        i.events.retain(|e| {
            let Event::Key { key, pressed: true, repeat, modifiers, .. } = e else { return true };
            let Some(k) = key_of(*key, *modifiers, page) else { return true };
            arrow |= matches!(key, Key::ArrowUp | Key::ArrowDown | Key::ArrowLeft | Key::ArrowRight);
            let once = matches!(k, ListKey::Toggle | ListKey::Open | ListKey::Delete | ListKey::Across { .. });
            if !(once && *repeat) {
                keys.push(k);
            }
            false
        });
    });
    if arrow && !locked {
        ctx.data_mut(|d| d.insert_temp(restore_id(), Some(id)));
        ctx.request_repaint();
    }
    keys
}

/// What `key` with `m` does in a focused list, if it is one of its keys.
fn key_of(key: Key, m: Modifiers, page: usize) -> Option<ListKey> {
    let plain = m.is_none();
    let shift = m.shift && !(m.alt || m.ctrl || m.command || m.mac_cmd);
    let mv = match key {
        Key::ArrowUp | Key::ArrowDown => Some(ListMove::Step { down: key == Key::ArrowDown }),
        Key::PageUp | Key::PageDown => Some(ListMove::Page { down: key == Key::PageDown, rows: page }),
        Key::Home | Key::End => Some(ListMove::Edge { end: key == Key::End }),
        _ => None,
    };
    match (mv, plain, shift) {
        (Some(mv), true, _) => return Some(ListKey::Move(mv)),
        (Some(mv), _, true) => return Some(ListKey::Extend(mv)),
        (Some(_), ..) => return None,
        _ => {}
    }
    match key {
        Key::Space if plain => Some(ListKey::Toggle),
        Key::Enter if plain => Some(ListKey::Open),
        Key::Delete if plain => Some(ListKey::Delete),
        Key::Backspace if m.command_only() => Some(ListKey::Delete),
        Key::ArrowLeft | Key::ArrowRight if plain => Some(ListKey::Across { right: key == Key::ArrowRight }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_their_modifiers() {
        let none = Modifiers::NONE;
        assert_eq!(key_of(Key::ArrowDown, none, 5), Some(ListKey::Move(ListMove::Step { down: true })));
        assert_eq!(key_of(Key::PageUp, Modifiers::SHIFT, 5), Some(ListKey::Extend(ListMove::Page { down: false, rows: 5 })));
        assert_eq!(key_of(Key::End, none, 5), Some(ListKey::Move(ListMove::Edge { end: true })));
        assert_eq!(key_of(Key::ArrowDown, Modifiers::ALT, 5), None, "Alt+Down is the next change");
        assert_eq!(key_of(Key::ArrowDown, Modifiers::COMMAND, 5), None);
        assert_eq!(key_of(Key::Space, none, 5), Some(ListKey::Toggle));
        assert_eq!(key_of(Key::Space, Modifiers::SHIFT, 5), None);
        assert_eq!(key_of(Key::Enter, none, 5), Some(ListKey::Open));
        assert_eq!(key_of(Key::Enter, Modifiers::COMMAND, 5), None, "Mod+Enter is Commit");
        assert_eq!(key_of(Key::Delete, none, 5), Some(ListKey::Delete));
        assert_eq!(key_of(Key::Backspace, Modifiers::COMMAND, 5), Some(ListKey::Delete));
        assert_eq!(key_of(Key::Backspace, none, 5), None);
        assert_eq!(key_of(Key::ArrowLeft, none, 5), Some(ListKey::Across { right: false }));
        assert_eq!(key_of(Key::ArrowRight, Modifiers::SHIFT, 5), None);
        assert_eq!(key_of(Key::S, none, 5), None, "S is Stage, a binding");
    }
}
