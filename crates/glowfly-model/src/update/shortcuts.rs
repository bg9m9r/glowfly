//! The staging, revert, commit-message and sheet keys. `ui/shortcuts/global.rs` matches the
//! keys; this file decides whether each applies now and sends the message the buttons already
//! use: S/U stage or unstage the open working-tree file (its selected lines, when a selection is
//! visible), R reverts the selected lines or the current hunk of a commit or comparison diff,
//! Ctrl+Shift+S/U stage or unstage everything, Ctrl+Shift+M goes to the commit message, and
//! Ctrl+/ opens or closes the shortcut sheet (the default keys). The keys of a focused Unstaged
//! or Staged Files list go to `wip_keys.rs`, the moves of the other file lists to
//! `file_list_keys.rs`; after the whole open file was staged or unstaged the view moves on to
//! its neighbour (`wip_list.rs`). The pane keys, F6 and Enter in the graph ask the view to
//! move the keyboard focus to a pane (`focus_pane`), showing its panel first.

use crate::effect::Effect;
use crate::keymap::KeyAction;
use crate::msg::RepoMsg;
use crate::msg::StageMsg;
use crate::msg::{LayoutMsg, Msg};
use crate::state::layout::{Pane, Panel};
use crate::state::{AppState, Modal, Selection};
use glowfly_core::repo::{DiffTarget, HunkOp};
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, action: KeyAction, now: Instant, fx: &mut Vec<Effect>) {
    // Preferences and a credential prompt cover the main window: only the sheet toggle (which
    // checks for itself) gets through, whatever the key matching let past.
    if action != KeyAction::ToggleSheet && (st.prefs.is_some() || st.askpass.showing()) {
        return;
    }
    let msg = match action {
        KeyAction::Stage => stage_msg(st, true),
        KeyAction::Unstage => stage_msg(st, false),
        KeyAction::RevertHunk => revert_msg(st),
        KeyAction::StageAll => st.snap.as_ref().is_some_and(|s| !s.unstaged.is_empty()).then_some(Msg::Stage(StageMsg::StageAll)),
        KeyAction::UnstageAll => st.snap.as_ref().is_some_and(|s| !s.staged.is_empty()).then_some(Msg::Stage(StageMsg::UnstageAll)),
        KeyAction::FocusMessage => {
            focus_message(st, now, fx);
            None
        }
        KeyAction::ToggleSheet => {
            toggle_sheet(st);
            None
        }
        KeyAction::FocusPane(pane) => {
            focus_pane(st, pane, now, fx);
            None
        }
        KeyAction::OpenDetails => {
            if st.session.selection != Selection::None {
                focus_pane(st, Pane::Files, now, fx);
            }
            None
        }
        KeyAction::WipList { staged, key } => {
            super::wip_keys::handle(st, staged, key, now, fx);
            None
        }
        KeyAction::FileList { list, mv } => {
            super::file_list_keys::handle(st, list, mv, now, fx);
            None
        }
    };
    if let Some(m) = msg {
        fx.extend(super::update_sealed(st, m, now));
    }
}

/// What S (`stage`) or U does on the open diff (or the working-tree file still opening): the
/// selected lines when the selection is visible on it, else the whole file. Nothing on a diff of the other side, a commit or comparison
/// diff, a conflicted file, or under the conflict view or the file history.
fn stage_msg(st: &AppState, stage: bool) -> Option<Msg> {
    if st.session.conflict.view.is_some() || st.session.history.view.is_some() {
        return None;
    }
    // The file still opening, when there is one: the line selection is on the diff on screen.
    let target = super::wip_list::cursor(st)?;
    // A conflicted file is marked resolved in the conflict view, never by a stray key.
    let conflicted = st.snap.as_ref().is_some_and(|s| s.unstaged.iter().any(|f| f.path == target.path() && f.status == '!'));
    if conflicted {
        return None;
    }
    let sel = &st.view.diff_lines;
    let lines = !sel.is_empty() && sel.target.as_ref() == Some(target) && st.line_ops_blocked().is_none();
    match (target, stage) {
        (DiffTarget::Unstaged(p), true) => {
            Some(if lines { Msg::Stage(StageMsg::ApplyLines(HunkOp::Stage)) } else { Msg::Stage(StageMsg::StageFile(p.clone())) })
        }
        (DiffTarget::Staged(p), false) => {
            Some(if lines { Msg::Stage(StageMsg::ApplyLines(HunkOp::Unstage)) } else { Msg::Stage(StageMsg::UnstageFile(p.clone())) })
        }
        _ => None,
    }
}

/// What R does on the open commit or comparison diff: Revert lines when a selection is visible
/// on it, else Revert Hunk on the current hunk (`diff_nav::current_hunk`). Nothing on a
/// working-tree diff or under the conflict or blame view. Refusals (a reload on its way) are
/// logged by the handlers.
fn revert_msg(st: &mut AppState) -> Option<Msg> {
    if st.session.conflict.view.is_some() || st.session.blame.view.is_some() {
        return None;
    }
    let (target, _) = st.session.diff.view.as_ref()?;
    if !target.reverts_hunks() {
        return None;
    }
    let sel = &st.view.diff_lines;
    if !sel.is_empty() && sel.target.as_ref() == Some(target) && st.line_ops_blocked().is_none() {
        return Some(Msg::Stage(StageMsg::RevertLines));
    }
    let index = crate::state::diff_nav::current_hunk(st)?;
    let Some(generation) = st.hunk_ops_gen() else {
        st.log(false, "The diff changed before the revert ran, so nothing was changed. Try again on the updated diff.");
        return None;
    };
    Some(Msg::Stage(StageMsg::RevertHunk { index, generation }))
}

/// Selects the WIP panel (when a repository is open) and has the commit box focus its summary
/// on the next frame.
fn focus_message(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    if st.snap.is_none() {
        return;
    }
    super::layout::reveal_details(st, fx);
    if st.session.selection != Selection::Wip {
        fx.extend(super::update_sealed(st, Msg::Repo(RepoMsg::SelectWip), now));
    }
    st.view.focus_message = true;
}

/// Asks the view to give `pane` the keyboard focus, showing the left or details panel first
/// when it is hidden; the commit message goes as Go to the commit message does. The diff only
/// while one is open.
fn focus_pane(st: &mut AppState, pane: Pane, now: Instant, fx: &mut Vec<Effect>) {
    match pane {
        Pane::Message => return focus_message(st, now, fx),
        Pane::Sidebar if !crate::state::layout::shown(&st.settings, Panel::Left) => {
            fx.extend(super::update_sealed(st, Msg::Layout(LayoutMsg::SetPanel { panel: Panel::Left, shown: true }), now));
        }
        Pane::Files => super::layout::reveal_details(st, fx),
        Pane::Diff if !st.diff_or_download() => return,
        _ => {}
    }
    st.view.pane_focus = Some(pane);
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
