//! Staging and unstaging a folder from a folder row of the WIP lists' tree: the folder as one
//! pathspec for `git add` (taken literally), and its staged files by exact path for unstage.

use crate::effect::{Effect, LocalOp};
use crate::state::AppState;
use crate::state::file_tree::under;
use glowfly_core::repo::Hazard;
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
        super::ops::local(st, LocalOp::StageFile(folder), fx);
    }
}

/// Unstage folder: the staged files under it, by exact path. Names that aren't UTF-8 name no
/// file and stay staged, as their own Unstage button can't unstage them either.
pub(super) fn unstage(st: &mut AppState, folder: String, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref() else { return };
    let paths: Vec<String> = snap
        .staged
        .iter()
        .filter(|f| under(&folder, &f.path) && !matches!(f.hazard, Some(Hazard::NotUtf8(_))))
        .map(|f| f.path.clone())
        .collect();
    if !paths.is_empty() {
        super::ops::local(st, LocalOp::UnstagePaths(paths), fx);
    }
}
