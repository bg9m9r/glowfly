//! What the terminal grid asks of its active tab: scrolling the history (by rows, or a screen
//! at a time from the keyboard), copying, a finished mouse selection, a word or line picked by a
//! double or triple click and selecting all (each
//! offered as the PRIMARY selection, and copied too with Copy on select), clearing, pasting from
//! the clipboard or from PRIMARY (a middle click), and remembering the panel's height.

use crate::effect::{Effect, UiEffect};
use crate::state::AppState;
use crate::state::terminal::{Selection, TermSession};
use crate::terminal::{ScrollStep, TerminalEffect as E};
use crate::update::terminal::active_mut;
use glowfly_core::terminal::TermPoint;

pub(in crate::update) fn scroll(st: &mut AppState, rows: i32) {
    if let Some(s) = active_mut(st) {
        s.screen.scroll_by(rows);
    }
}

/// A screen of rows back or forward, or to the start or end of the history. The alternate
/// screen has no history, so nothing moves there.
pub(in crate::update) fn scroll_page(st: &mut AppState, step: ScrollStep) {
    let Some(s) = active_mut(st) else { return };
    if s.screen.alternate() {
        return;
    }
    let rows = i32::from(s.screen.size().0);
    match step {
        ScrollStep::PageUp => s.screen.scroll_by(rows),
        ScrollStep::PageDown => s.screen.scroll_by(-rows),
        ScrollStep::Top => s.screen.scroll_by(i32::try_from(s.screen.history_len()).unwrap_or(i32::MAX)),
        ScrollStep::Bottom => s.screen.scroll_to_bottom(),
    }
}

/// The text of `s`'s selection, when it has any.
fn copy_text(s: &mut TermSession) -> Option<String> {
    let (start, end) = s.selection.and_then(|sel| sel.range(&s.screen))?;
    Some(s.screen.text_between(start, end)).filter(|t| !t.is_empty())
}

/// Copies the active tab's selection; `keep` leaves it selected.
pub(in crate::update) fn copy(st: &mut AppState, keep: bool, fx: &mut Vec<Effect>) {
    let Some(s) = active_mut(st) else { return };
    if let Some(text) = copy_text(s) {
        fx.push(Effect::Ui(UiEffect::CopyText(text)));
    }
    if !keep {
        s.selection = None;
    }
    st.view.terminal.focus_requested = true;
}

/// A mouse selection ended: it becomes the PRIMARY selection, and with Copy on select is
/// copied too. It stays selected.
pub(in crate::update) fn selected(st: &mut AppState, fx: &mut Vec<Effect>) {
    let copy_on_select = st.settings.terminal.copy_on_select;
    let Some(s) = active_mut(st) else { return };
    offer(copy_text(s), copy_on_select, fx);
    st.view.terminal.focus_requested = true;
}

/// Selects the word at `at` (or with `line` its whole line) in session `id`, a double or triple
/// click, and offers it as `selected` does; an empty pick offers nothing.
pub(in crate::update) fn select_at(st: &mut AppState, id: u64, at: TermPoint, line: bool, fx: &mut Vec<Effect>) {
    let copy_on_select = st.settings.terminal.copy_on_select;
    let Some(s) = st.terminal.sessions.iter_mut().find(|s| s.id == id) else { return };
    let (anchor, head) = if line { s.screen.line_at(at.line) } else { s.screen.word_at(at) };
    let sel = Selection { anchor, head, epoch: s.screen.epoch() };
    s.selection = Some(sel);
    if sel.range(&s.screen).is_some() {
        offer(copy_text(s), copy_on_select, fx);
        st.view.terminal.focus_requested = true;
    }
}

/// Selects the whole history and screen, which becomes the PRIMARY selection; with Copy on
/// select it is copied too.
pub(in crate::update) fn select_all(st: &mut AppState, fx: &mut Vec<Effect>) {
    let copy_on_select = st.settings.terminal.copy_on_select;
    let Some(s) = active_mut(st) else { return };
    let (anchor, head) = s.screen.select_all();
    s.selection = Some(Selection { anchor, head, epoch: s.screen.epoch() });
    offer(copy_text(s), copy_on_select, fx);
    st.view.terminal.focus_requested = true;
}

/// A new selection's `text` as the PRIMARY selection (which the shell ignores where there is
/// none), and on the clipboard too with `copy_on_select`.
fn offer(text: Option<String>, copy_on_select: bool, fx: &mut Vec<Effect>) {
    let Some(text) = text else { return };
    if copy_on_select {
        fx.push(Effect::Ui(UiEffect::CopyText(text.clone())));
    }
    fx.push(Effect::Ui(UiEffect::SetPrimary(text)));
}

/// Drops the history. Shells that redraw on Ctrl+L are sent it, so they clear the screen in
/// step with Windows' pseudoconsole; cmd doesn't, so its screen is cleared here, keeping the
/// prompt's row. Nothing happens while a full-screen program has the alternate screen.
pub(in crate::update) fn clear(st: &mut AppState, fx: &mut Vec<Effect>) {
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
    st.view.terminal.focus_requested = true;
}

/// Asks the window for the clipboard, which comes back to the focused grid as a paste.
pub(in crate::update) fn paste(st: &mut AppState, fx: &mut Vec<Effect>) {
    if active_mut(st).is_some_and(|s| s.alive()) {
        st.view.terminal.focus_requested = true;
        fx.push(Effect::Ui(UiEffect::RequestPaste));
    }
}

/// Asks the shell for the PRIMARY selection (a middle click), which it pastes into the active
/// tab, or for the clipboard where there is no PRIMARY.
pub(in crate::update) fn paste_primary(st: &mut AppState, fx: &mut Vec<Effect>) {
    if active_mut(st).is_some_and(|s| s.alive()) {
        st.view.terminal.focus_requested = true;
        fx.push(Effect::Ui(UiEffect::PastePrimary));
    }
}

/// Saves the height a drag of the panel's edge left it at.
pub(in crate::update) fn panel_height(st: &mut AppState, h: f32, fx: &mut Vec<Effect>) {
    let Some(h) = crate::state::panel_layout::TERMINAL.sanitize(h) else { return };
    if st.settings.terminal.panel_height != Some(h) {
        st.settings.terminal.panel_height = Some(h);
        fx.extend(crate::update::prefs::save_settings(st));
    }
}
