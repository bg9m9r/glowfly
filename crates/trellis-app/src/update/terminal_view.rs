//! What the terminal grid asks of its active tab: scrolling the history, copying and selecting
//! all, clearing, pasting from the clipboard, and remembering the panel's height.

use super::terminal::active_mut;
use crate::effect::{Effect, UiEffect};
use crate::state::AppState;
use crate::state::terminal::Selection;
use crate::terminal::TerminalEffect as E;

/// The panel's smallest height, as the view allows it.
const MIN_PANEL_HEIGHT: f32 = 90.0;

pub(super) fn scroll(st: &mut AppState, rows: i32) {
    if let Some(s) = active_mut(st) {
        s.screen.scroll_by(rows);
    }
}

/// Copies the active tab's selection; `keep` leaves it selected.
pub(super) fn copy(st: &mut AppState, keep: bool, fx: &mut Vec<Effect>) {
    let Some(s) = active_mut(st) else { return };
    if let Some((start, end)) = s.selection.and_then(|sel| sel.range(&s.screen)) {
        let text = s.screen.text_between(start, end);
        if !text.is_empty() {
            fx.push(Effect::Ui(UiEffect::CopyText(text)));
        }
    }
    if !keep {
        s.selection = None;
    }
    st.terminal.focus_requested = true;
}

pub(super) fn select_all(st: &mut AppState) {
    let Some(s) = active_mut(st) else { return };
    let (anchor, head) = s.screen.select_all();
    s.selection = Some(Selection { anchor, head, epoch: s.screen.epoch() });
    st.terminal.focus_requested = true;
}

/// Drops the history. Shells that redraw on Ctrl+L are sent it, so they clear the screen in
/// step with Windows' pseudoconsole; cmd doesn't, so its screen is cleared here, keeping the
/// prompt's row. Nothing happens while a full-screen program has the alternate screen.
pub(super) fn clear(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(s) = active_mut(st) else { return };
    if s.screen.alternate() {
        return;
    }
    s.screen.clear_history();
    s.selection = None;
    if s.program == "cmd" {
        s.screen.clear_screen_keep_cursor_line();
    } else if s.alive() {
        fx.push(Effect::Terminal(E::Write { id: s.id, bytes: vec![0x0c] }));
    }
    st.terminal.focus_requested = true;
}

/// Asks the window for the clipboard, which comes back to the focused grid as a paste.
pub(super) fn paste(st: &mut AppState, fx: &mut Vec<Effect>) {
    if active_mut(st).is_some_and(|s| s.alive()) {
        st.terminal.focus_requested = true;
        fx.push(Effect::Ui(UiEffect::RequestPaste));
    }
}

/// Saves the height a drag of the panel's edge left it at.
pub(super) fn panel_height(st: &mut AppState, h: f32, fx: &mut Vec<Effect>) {
    if !h.is_finite() {
        return;
    }
    let h = h.round().max(MIN_PANEL_HEIGHT);
    if st.settings.terminal.panel_height != Some(h) {
        st.settings.terminal.panel_height = Some(h);
        fx.push(super::prefs::save_settings(st));
    }
}
