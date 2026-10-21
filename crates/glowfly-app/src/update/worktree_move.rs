//! Moving a worktree (from its WORKTREES row, its WIP row in the graph or the palette): the Move
//! dialog, the request checked again against the current snapshot, `Effect::MoveWorktree` as the
//! active job, and what follows a move: the tabs, the recent list, the closed tabs, favorites and
//! the known folders point at the new folder. A target found taken reopens the dialog with the
//! error under the field.

use super::ops;
use crate::effect::Effect;
use crate::state::tabs::ClosedTab;
use crate::state::worktree_manage::gone_text;
use crate::state::worktree_move::{MoveForm, move_refused, move_request, moved_headline};
use crate::state::worktrees::find;
use crate::state::{AppState, Modal};
use crate::worktree::MoveRequest;
use glowfly_core::paths::{self, FolderId, same_spelling};
use glowfly_core::repo::WorktreeInfo;
use glowfly_core::worktree_ops::MoveOutcome;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// The listed worktree at `path` that can be moved, or None after logging why not.
fn movable(st: &mut AppState, path: &Path) -> Option<WorktreeInfo> {
    let found = find(st.snap.as_ref()?, path).cloned();
    let Some(wt) = found else {
        st.log(false, gone_text(path));
        return None;
    };
    if let Some(why) = move_refused(&wt) {
        st.log(false, format!("Can't move {}: {why}", wt.folder_name()));
        return None;
    }
    Some(wt)
}

pub(super) fn move_dialog(st: &mut AppState, path: PathBuf) {
    if let Some(wt) = movable(st, &path) {
        st.modal = Some(Modal::MoveWorktree(MoveForm::new(&wt)));
    }
}

/// The Move dialog's button.
pub(super) fn submit(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(Modal::MoveWorktree(form)) = st.modal.as_ref() else { return };
    let form = form.clone();
    if movable(st, &form.path).is_none() {
        if st.snap.as_ref().is_some_and(|s| find(s, &form.path).is_none()) {
            st.modal = None;
        }
        return;
    }
    let Some(snap) = st.snap.as_ref() else { return };
    let req = match move_request(snap, &form) {
        Ok(r) => r,
        Err(e) => return st.log(false, e),
    };
    let effect = move |repo| Effect::MoveWorktree(MoveRequest { repo, ..req });
    if ops::begin_job(st, "Move worktree", true, effect, fx) == ops::Started::Now {
        st.modal = None;
    }
}

/// The move finished: one toast and log entry; a moved worktree's tabs and lists follow it, and a
/// taken target reopens the dialog with the error.
pub(super) fn moved(st: &mut AppState, r: MoveRequest, outcome: MoveOutcome, output: String, now: Instant, fx: &mut Vec<Effect>) {
    let Some(job) = &st.job else { return };
    let here = match (job.workdir.as_deref(), st.workdir()) {
        (Some(a), Some(b)) => same_spelling(a, &b),
        _ => false,
    };
    let ok = outcome == MoveOutcome::Moved;
    if ok {
        retarget(st, &r.from, &r.to, r.folder, fx);
    }
    let headline = ok.then(|| moved_headline(&r.name, &r.to));
    ops::finish_job(st, ok, output.clone(), headline, None, now, fx);
    if outcome == MoveOutcome::Taken
        && here
        && st.modal.is_none()
        && let Some(wt) = st.snap.as_ref().and_then(|s| find(s, &r.from))
    {
        let folder = r.to.display().to_string();
        let form = MoveForm { error: Some((folder.clone(), output)), folder, ..MoveForm::new(wt) };
        st.modal = Some(Modal::MoveWorktree(form));
    }
}

/// `list` with every path `is_from` matches replaced by `to`, keeping the first of each spelling.
/// None when nothing matched.
fn replaced(list: &[PathBuf], is_from: impl Fn(&Path) -> bool, to: &Path) -> Option<Vec<PathBuf>> {
    if !list.iter().any(|p| is_from(p)) {
        return None;
    }
    let mut out: Vec<PathBuf> = Vec::new();
    for p in list {
        let p = if is_from(p) { to.to_path_buf() } else { p.clone() };
        if !out.iter().any(|q| same_spelling(q, &p)) {
            out.push(p);
        }
    }
    Some(out)
}

/// Points everything that named the folder `from` (whose identity is `folder`) at `to`: the
/// tabs (a parked snapshot is dropped so the tab loads again when selected), the recent list,
/// the closed tabs, favorites and the known folders, then saves them.
fn retarget(st: &mut AppState, from: &Path, to: &Path, folder: Option<FolderId>, fx: &mut Vec<Effect>) {
    let name = to.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| to.display().to_string());
    let id = folder.or_else(|| st.known.id_of(from));
    for tab in st.tabs.iter_mut().filter(|t| t.path.is_some() && t.place().is(from, id)) {
        tab.path = Some(to.to_path_buf());
        tab.name = name.clone();
        tab.real = None;
        if let Some(p) = tab.parked.as_mut() {
            p.snap = None;
        }
    }
    if let Some(recent) = replaced(&st.recent, |p| st.same_folder(p, from), to) {
        st.recent = recent;
        fx.push(Effect::SaveRecent(st.recent.clone()));
    }
    if st.closed_tabs.iter().any(|c| st.same_folder(&c.path, from)) {
        // Each keeps its label; a closed tab that already named `to` is merged into it.
        let mut closed: Vec<ClosedTab> = Vec::new();
        for mut c in std::mem::take(&mut st.closed_tabs) {
            if st.same_folder(&c.path, from) {
                c.path = to.to_path_buf();
            }
            if !closed.iter().any(|q| same_spelling(&q.path, &c.path)) {
                closed.push(c);
            }
        }
        st.closed_tabs = closed;
    }
    let favorites: Vec<PathBuf> = st.settings.favorite_repos.iter().map(|f| paths::decode(f)).collect();
    if let Some(favorites) = replaced(&favorites, |p| st.same_folder(p, from), to) {
        st.settings.favorite_repos = favorites.iter().map(|p| paths::encode(p)).collect();
        fx.push(super::prefs::save_settings(st));
    }
    st.known.forget(from);
    if let Some(id) = id {
        st.known.learn(to, id);
    }
    fx.push(super::tabs::save(st));
}
