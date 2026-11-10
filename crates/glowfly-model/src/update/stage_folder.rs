//! Staging and unstaging a folder from a folder row of the WIP lists' tree: the folder as one
//! pathspec for `git add` (taken literally), and its staged files by exact path for unstage;
//! the selected files of those lists (picked with Shift or Ctrl/Cmd), staged or unstaged
//! together; and Unstage all.

use super::jobs::Started;
use crate::effect::{Effect, LocalOp};
use crate::state::AppState;
use crate::state::file_tree::under;
use glowfly_core::repo::{FileEntry, Hazard};
use std::time::Instant;

/// Stage folder: refused while a conflicted file lies under it (`git add` would mark it
/// resolved, markers and all), and nothing to do when no unstaged file does.
pub(super) fn stage(st: &mut AppState, folder: String, now: Instant, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref() else { return };
    let inside: Vec<_> = snap.unstaged.iter().filter(|f| under(&folder, &f.path)).collect();
    if inside.iter().any(|f| f.status == '!') {
        let text = st.settings.lang().tr("files.folder_menu.stage_conflicted");
        st.toast(false, text, now);
        return;
    }
    if !inside.is_empty() {
        let _ = super::ops::local(st, LocalOp::StageFile(folder), fx);
    }
}

/// Unstage folder: the staged files under it, by exact path. Names that aren't UTF-8 name no
/// file and stay staged, as their own Unstage button can't unstage them either; a toast says so.
pub(super) fn unstage(st: &mut AppState, folder: String, now: Instant, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref() else { return };
    let inside: Vec<&FileEntry> = snap.staged.iter().filter(|f| under(&folder, &f.path)).collect();
    let paths: Vec<String> = inside.iter().filter(|f| !not_utf8(f)).map(|f| f.path.clone()).collect();
    let left = inside.len() - paths.len();
    if !paths.is_empty() {
        let _ = super::ops::local(st, LocalOp::UnstagePaths(paths), fx);
    }
    left_staged(st, left, now);
}

/// Unstage all: every staged file, by exact path. Names that aren't UTF-8 name no file and stay
/// staged, as for a folder; a toast says so, rather than the button seeming to have worked.
pub(super) fn unstage_all(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref() else { return };
    let paths: Vec<String> = snap.staged.iter().filter(|f| !not_utf8(f)).map(|f| f.path.clone()).collect();
    let left = snap.staged.len() - paths.len();
    if !paths.is_empty() {
        let _ = super::ops::local(st, LocalOp::UnstagePaths(paths), fx);
    }
    left_staged(st, left, now);
}

/// Says, as a failure toast, that `n` files the user asked to unstage stay staged because their
/// names aren't UTF-8. Nothing when `n` is 0.
fn left_staged(st: &mut AppState, n: usize, now: Instant) {
    if n > 0 {
        let text = st.settings.lang().trn("files.hazard.not_utf8.left_staged", n, &[]);
        st.toast(false, text, now);
    }
}

/// Whether `f`'s name isn't UTF-8, so its shown path names no file.
fn not_utf8(f: &FileEntry) -> bool {
    matches!(f.hazard, Some(Hazard::NotUtf8(_)))
}

/// Whether `f` is one of `paths` and its name can be given to git (one that isn't UTF-8 names
/// no file, as its own buttons can't act on it either).
fn named(paths: &[String], f: &FileEntry) -> bool {
    !not_utf8(f) && paths.contains(&f.path)
}

/// Stages the selected files of Unstaged Files that are still there, in one operation. A
/// conflicted file is left alone: staging it would mark it resolved, markers and all.
pub(super) fn stage_paths(st: &mut AppState, paths: Vec<String>, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref() else { return };
    let paths: Vec<String> = snap.unstaged.iter().filter(|f| f.status != '!' && named(&paths, f)).map(|f| f.path.clone()).collect();
    if !paths.is_empty() && super::ops::local(st, LocalOp::StagePaths(paths.clone()), fx) == Started::Now {
        super::wip_list::stage_sent(st, false, &paths);
    }
}

/// Unstages the selected files of Staged Files that are still there, in one operation. Picked
/// names that aren't UTF-8 stay staged, and a toast says so.
pub(super) fn unstage_paths(st: &mut AppState, picked: Vec<String>, now: Instant, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref() else { return };
    let paths: Vec<String> = snap.staged.iter().filter(|f| named(&picked, f)).map(|f| f.path.clone()).collect();
    let left = snap.staged.iter().filter(|f| not_utf8(f) && picked.contains(&f.path)).count();
    if !paths.is_empty() {
        let _ = super::ops::local(st, LocalOp::UnstagePaths(paths.clone()), fx);
        super::wip_list::stage_sent(st, true, &paths);
    }
    left_staged(st, left, now);
}
