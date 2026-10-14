//! The newer keyboard shortcuts. `ui/shortcuts.rs` matches the keys; this file decides whether
//! each applies now and sends the message the buttons already use: S/U stage or unstage the
//! open working-tree file (its selected lines, when a selection is visible), Ctrl+Shift+S/U
//! stage or unstage everything, Ctrl+Shift+M goes to the commit message, and Ctrl+/ opens or
//! closes the shortcut sheet.

use crate::effect::Effect;
use crate::keymap::KeyAction;
use crate::msg::Msg;
use crate::state::{AppState, Modal, Selection};
use std::time::Instant;
use trellis_core::repo::{DiffTarget, HunkOp};

pub(super) fn handle(st: &mut AppState, action: KeyAction, now: Instant, fx: &mut Vec<Effect>) {
    let msg = match action {
        KeyAction::Stage => stage_msg(st, true),
        KeyAction::Unstage => stage_msg(st, false),
        KeyAction::StageAll => st.snap.as_ref().is_some_and(|s| !s.unstaged.is_empty()).then_some(Msg::StageAll),
        KeyAction::UnstageAll => st.snap.as_ref().is_some_and(|s| !s.staged.is_empty()).then_some(Msg::UnstageAll),
        KeyAction::FocusMessage => {
            focus_message(st, now, fx);
            None
        }
        KeyAction::ToggleSheet => {
            toggle_sheet(st);
            None
        }
    };
    if let Some(m) = msg {
        fx.extend(super::update(st, m, now));
    }
}

/// What S (`stage`) or U does on the open diff: the selected lines when the selection is
/// visible, else the whole file. Nothing on a diff of the other side, a commit or comparison
/// diff, a conflicted file, or under the conflict view, the file history or Preferences.
fn stage_msg(st: &AppState, stage: bool) -> Option<Msg> {
    if st.prefs.is_some() || st.conflict.is_some() || st.history.is_some() {
        return None;
    }
    let (target, _) = st.diff.as_ref()?;
    // A conflicted file is marked resolved in the conflict view, never by a stray key.
    let conflicted = st.snap.as_ref().is_some_and(|s| s.unstaged.iter().any(|f| f.path == target.path() && f.status == '!'));
    if conflicted {
        return None;
    }
    let sel = &st.view.diff_lines;
    let lines = !sel.is_empty() && sel.target.as_ref() == Some(target) && st.line_ops_blocked().is_none();
    match (target, stage) {
        (DiffTarget::Unstaged(p), true) => Some(if lines { Msg::ApplyLines(HunkOp::Stage) } else { Msg::StageFile(p.clone()) }),
        (DiffTarget::Staged(p), false) => Some(if lines { Msg::ApplyLines(HunkOp::Unstage) } else { Msg::UnstageFile(p.clone()) }),
        _ => None,
    }
}

/// Selects the WIP panel (when a repository is open) and has the commit box focus its summary
/// on the next frame.
fn focus_message(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    if st.snap.is_none() || st.prefs.is_some() {
        return;
    }
    if st.selection != Selection::Wip {
        fx.extend(super::update(st, Msg::SelectWip, now));
    }
    st.view.focus_message = true;
}

/// Closes the open sheet, or opens it (closing the palette) unless a dialog, Preferences or a
/// credential prompt is in the way.
fn toggle_sheet(st: &mut AppState) {
    if matches!(st.modal, Some(Modal::Shortcuts { .. })) {
        st.modal = None;
    } else if st.modal.is_none() && st.prefs.is_none() && !st.askpass.showing() {
        st.palette = None;
        st.modal = Some(Modal::Shortcuts { filter: String::new() });
    }
}
