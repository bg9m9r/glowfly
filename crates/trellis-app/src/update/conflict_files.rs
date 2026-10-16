//! The Conflicted Files and Resolved Files lists: reading their marker counts and resolved
//! files, Mark resolved (checking the working files for markers first) and Mark conflicted.

use crate::effect::{ConflictRead, Effect, LocalOp};
use crate::msg::{ConflictMsg, Msg};
use crate::state::conflict_files::{ConflictFiles, conflicted_paths, markers_warning};
use crate::state::path_hazards::hazard_of;
use crate::state::{AppState, Modal};
use git2::RepositoryState;
use trellis_core::repo::{ConflictSummary, Hazard};

/// The status changed (or the tab was opened or activated): reads the lists again while an
/// operation is in progress or files are conflicted, and clears them once it is all over.
pub(super) fn want(st: &mut AppState, fx: &mut Vec<Effect>) {
    let generation = st.conflict_files.generation + 1;
    let (Some(snap), Some(workdir)) = (&st.snap, st.workdir()) else {
        st.conflict_files = ConflictFiles { generation, ..Default::default() };
        return;
    };
    let paths = conflicted_paths(snap);
    let conflicted = snap.unstaged.iter().any(|f| f.status == '!');
    if snap.state == RepositoryState::Clean && !conflicted {
        st.conflict_files = ConflictFiles { generation, ..Default::default() };
        return;
    }
    // Files conflicted again leave the resolved list now rather than when the read lands.
    let now: Vec<&str> = snap.unstaged.iter().filter(|f| f.status == '!').map(|f| f.path.as_str()).collect();
    let files = &mut st.conflict_files;
    files.generation = generation;
    files.resolved.retain(|r| !now.contains(&r.entry.path.as_str()));
    fx.push(Effect::ReadConflictFiles { workdir, paths, purpose: ConflictRead::Summary { generation } });
}

/// The paths of `paths` that are conflicted in the status on screen and can be named to git.
fn usable_conflicted(st: &AppState, paths: Vec<String>) -> Vec<String> {
    let Some(snap) = &st.snap else { return Vec::new() };
    let ok = |p: &String| snap.unstaged.iter().any(|f| f.status == '!' && f.path == *p && !matches!(f.hazard, Some(Hazard::NotUtf8(_))));
    paths.into_iter().filter(ok).collect()
}

/// Whether the merge tool has any of `paths` open (said in the log, see `conflict::in_tool`).
fn any_in_tool(st: &mut AppState, paths: &[String]) -> bool {
    paths.iter().any(|p| super::conflict::in_tool(st, p))
}

/// Mark resolved: the working files are checked for markers off the UI thread first.
/// Nothing is staged while the merge tool has any of them open.
pub(super) fn mark_resolved(st: &mut AppState, paths: Vec<String>, fx: &mut Vec<Effect>) {
    let paths = usable_conflicted(st, paths);
    if paths.is_empty() || any_in_tool(st, &paths) {
        return;
    }
    if let Some(workdir) = st.workdir() {
        fx.push(Effect::ReadConflictFiles { workdir, paths, purpose: ConflictRead::BeforeMarkResolved });
    }
}

/// Stages `paths` as they are (after the markers check or its warning). The open conflict
/// steps to the next file once its own path is resolved this way.
pub(super) fn confirmed(st: &mut AppState, paths: Vec<String>, fx: &mut Vec<Effect>) {
    let paths = usable_conflicted(st, paths);
    if paths.is_empty() || any_in_tool(st, &paths) {
        return;
    }
    if let Some(c) = st.conflict.as_mut().filter(|c| paths.contains(&c.path)) {
        c.step_on_resolve = true;
    }
    super::ops::local(st, LocalOp::MarkResolved(paths), fx);
}

/// Mark conflicted (confirmed in the view): only resolved files whose record has both sides,
/// with a name git can be given safely, and not open in the merge tool.
pub(super) fn mark_conflicted(st: &mut AppState, paths: Vec<String>, fx: &mut Vec<Effect>) {
    let restorable = |st: &AppState, p: &String| {
        st.conflict_files.resolved.iter().any(|r| r.both_sides && r.entry.hazard.is_none() && r.entry.path == *p)
    };
    let paths: Vec<String> = paths.into_iter().filter(|p| restorable(st, p) && hazard_of(st, p).is_none()).collect();
    let paths: Vec<String> = paths.into_iter().filter(|p| !super::conflict::in_tool(st, p)).collect();
    if !paths.is_empty() {
        super::ops::local(st, LocalOp::MarkConflicted(paths), fx);
    }
}

/// A read of the conflicted files came back.
pub(super) fn read_done(
    st: &mut AppState,
    purpose: ConflictRead,
    paths: Vec<String>,
    result: Result<ConflictSummary, String>,
    fx: &mut Vec<Effect>,
) {
    match purpose {
        ConflictRead::Summary { generation } => {
            if generation != st.conflict_files.generation {
                return;
            }
            match result {
                Ok(s) => {
                    st.conflict_files.counts = s.counts.into_iter().collect();
                    st.conflict_files.resolved = s.resolved;
                }
                Err(e) => st.log(false, e),
            }
        }
        ConflictRead::BeforeMarkResolved => match result {
            Err(e) => st.log(false, e),
            Ok(s) => {
                let left: Vec<(String, usize)> = s.counts.into_iter().filter_map(|(p, n)| n.filter(|&n| n > 0).map(|n| (p, n))).collect();
                if left.is_empty() {
                    confirmed(st, paths, fx);
                    return;
                }
                st.modal = Some(Modal::Confirm {
                    title: "Conflict markers left".into(),
                    body: markers_warning(&left),
                    button: "Mark resolved".into(),
                    then: Box::new(Msg::Conflict(ConflictMsg::MarkResolvedConfirmed(paths))),
                });
            }
        },
    }
}
