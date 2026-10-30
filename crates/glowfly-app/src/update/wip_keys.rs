//! The keys of a focused Unstaged or Staged Files list (`ui/wip_keys.rs` reads them): Up/Down,
//! Page Up/Page Down and Home/End open the file they land on, with Shift they also select every
//! file from where the selection started; Space stages or unstages the file or the selected
//! files, Enter opens the list's current file, Delete discards the current unstaged file through
//! the menu's confirmation (Undo puts it back), and Left/Right go to the Unstaged or Staged list
//! keeping the place. Each sends the message its button or menu item sends.

use super::wip_list::{cursor, landing, open, still_listed};
use crate::effect::Effect;
use crate::msg::{Msg, StageMsg};
use crate::state::AppState;
use crate::state::list_keys::{ListKey, ListMove};
use crate::state::wip_lists::{self, WipSpan, files, path_in, target};
use glowfly_core::repo::DiffTarget;
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, staged: bool, key: ListKey, now: Instant, fx: &mut Vec<Effect>) {
    match key {
        ListKey::Move(m) => {
            st.view.wip_span = None;
            super::wip_list::step(st, staged, m, fx);
        }
        ListKey::Extend(m) => extend(st, staged, m, fx),
        ListKey::Toggle => toggle(st, staged, now, fx),
        ListKey::Open => open_current(st, staged, fx),
        ListKey::Delete => discard(st, staged, now, fx),
        ListKey::Across { right } => across(st, staged, right, fx),
    }
}

/// The path of the file the keys act on, when it is in that list and no conflict view or file
/// history covers its diff (as for S and U).
fn current(st: &AppState, staged: bool) -> Option<String> {
    if st.session.conflict.view.is_some() || st.session.history.view.is_some() {
        return None;
    }
    cursor(st).and_then(|t| path_in(staged, t)).map(str::to_owned)
}

/// Shift with a move: opens the file it lands on and selects from the selection's start (the
/// file the keys acted on, when no selection was going) to it.
fn extend(st: &mut AppState, staged: bool, m: ListMove, fx: &mut Vec<Effect>) {
    let anchor = match wip_lists::span(st, staged) {
        Some(s) => Some(s.anchor.clone()),
        None => cursor(st).and_then(|t| path_in(staged, t)).map(str::to_owned),
    };
    let Some(t) = landing(st, staged, m) else { return };
    let head = t.path().to_owned();
    let anchor = anchor.unwrap_or_else(|| head.clone());
    open(st, t, fx);
    st.view.wip_span = Some(WipSpan { staged, anchor, head });
}

/// Space: stages (Unstaged Files) or unstages the selected files, or the current file alone.
fn toggle(st: &mut AppState, staged: bool, now: Instant, fx: &mut Vec<Effect>) {
    let span = wip_lists::span_paths(st, staged);
    let msg = match (span.len() > 1, staged) {
        (true, false) => StageMsg::StagePaths(span),
        (true, true) => StageMsg::UnstagePaths(span),
        (false, _) => {
            let Some(p) = current(st, staged) else { return };
            if staged { StageMsg::UnstageFile(p) } else { StageMsg::StageFile(p) }
        }
    };
    st.view.wip_span = None;
    fx.extend(super::update_sealed(st, Msg::Stage(msg), now));
}

/// Enter: opens the list's current file when another one (or none) is open: where the list
/// was left, else its first file.
fn open_current(st: &mut AppState, staged: bool, fx: &mut Vec<Effect>) {
    st.view.wip_span = None;
    if cursor(st).and_then(|t| path_in(staged, t)).is_some() {
        return;
    }
    if let Some(t) = left_at(st, staged).or_else(|| landing(st, staged, ListMove::Step { down: true })) {
        open(st, t, fx);
    }
}

/// The file that list last opened, while it is still there.
fn left_at(st: &AppState, staged: bool) -> Option<DiffTarget> {
    let t = target(staged, st.view.wip_place[usize::from(staged)].as_deref()?);
    still_listed(st, &t).then_some(t)
}

/// Delete in Unstaged Files: asks to discard the current file's changes, as its menu item does.
fn discard(st: &mut AppState, staged: bool, now: Instant, fx: &mut Vec<Effect>) {
    if staged {
        return;
    }
    let Some(path) = current(st, false) else { return };
    let Some(f) = st.snap.as_ref().and_then(|s| files(s, false).into_iter().find(|f| f.path == path)) else { return };
    let msg = crate::state::files::discard_msg(st.settings.lang(), f);
    st.view.wip_span = None;
    fx.extend(super::update_sealed(st, msg, now));
}

/// Left (to Unstaged Files) or Right (to Staged Files) from the other list: gives that list the
/// keyboard and opens the same file when it has it too (a partly staged file), else where the
/// list was left, else its first file. An empty list just takes the keyboard.
fn across(st: &mut AppState, staged: bool, right: bool, fx: &mut Vec<Effect>) {
    if right == staged {
        return;
    }
    let to = right;
    st.view.wip_span = None;
    st.view.wip_focus = Some(to);
    let same = cursor(st).and_then(|t| path_in(staged, t)).map(|p| target(to, p)).filter(|t| still_listed(st, t));
    let t = same.or_else(|| left_at(st, to)).or_else(|| landing(st, to, ListMove::Step { down: true }));
    if let Some(t) = t.filter(|t| cursor(st) != Some(t)) {
        open(st, t, fx);
    }
}
