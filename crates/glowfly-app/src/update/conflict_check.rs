//! Predicting merge conflicts of the checked-out branch: the automatic check after loads, tab
//! switches and turning the preference on (one at a time per tab, skipped when its inputs are
//! unchanged), and the checks the branch menus ask for, whose results open a dialog.

use crate::conflict_check::{CheckJob, CheckRequest, ConflictCheckMsg};
use crate::effect::Effect;
use crate::state::conflict_check::{ConflictReport, current_request, headline};
use crate::state::{AppState, Modal};
use glowfly_core::conflict_check::{MergeTarget, TargetKind};
use std::time::Instant;

/// Starts the automatic check when the preference is on, git may run here unasked (merge-tree
/// runs the merge drivers the repository's config names), the checked-out branch has a
/// target, nothing is known for these inputs yet and no check is running (otherwise one is
/// queued behind it).
pub(super) fn want(st: &mut AppState, fx: &mut Vec<Effect>) {
    // An old git can't (the banner says so); the branch menus' checks still explain it.
    if !st.settings.conflict_check || !st.runs_git_unasked() || !st.git.has(glowfly_core::git_version::Feature::MergeTree) {
        return;
    }
    let Some(snap) = &st.snap else { return };
    let Some(request) = current_request(snap) else { return };
    if !st.conflict_checks.wants(&request) {
        return;
    }
    let workdir = snap.workdir.clone();
    let Some(generation) = st.conflict_checks.start() else { return };
    fx.push(Effect::CheckConflicts(CheckJob { workdir, request, manual: false, generation }));
}

pub(super) fn handle(st: &mut AppState, msg: ConflictCheckMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        // Asked for by the user: no trust or preference gate, like a Pull they start.
        ConflictCheckMsg::CheckNow => match st.snap.as_ref().map(|s| (s.workdir.clone(), current_request(s))) {
            Some((workdir, Some(request))) => manual(workdir, request, fx),
            Some((_, None)) => st.log(false, no_request_reason(st)),
            None => {}
        },
        ConflictCheckMsg::CheckBranch(name) => check_branch(st, &name, fx),
        ConflictCheckMsg::Checked { request, manual: false, generation, result } => {
            let report = ConflictReport { request, result };
            let line = headline(st.settings.lang(), &report);
            let done = st.conflict_checks.finish(generation, report);
            if done.accepted && done.new_error.is_some() {
                st.log_quiet(false, line);
            }
            if done.queued {
                want(st, fx);
            }
        }
        ConflictCheckMsg::Checked { request, manual: true, generation: _, result } => {
            let report = ConflictReport { request, result };
            let current = st.snap.as_ref().and_then(current_request);
            if current.as_ref() == Some(&report.request) {
                st.conflict_checks.store_manual(report.clone());
            }
            let ok = report.result.is_ok();
            let line = headline(st.settings.lang(), &report);
            st.log(ok, line.clone());
            if st.modal.is_none() {
                st.modal = Some(Modal::ConflictReport(report));
                st.palette = None;
            } else {
                st.toast(ok, line, now);
            }
        }
    }
}

/// Why the checked-out branch can't be checked (CheckNow without a request).
fn no_request_reason(st: &AppState) -> &'static str {
    let t = st.settings.lang();
    match &st.snap {
        Some(s) if s.head_branch.is_none() || s.head_oid.is_none() => t.tr("conflict_check.no_branch"),
        Some(s) if s.state != git2::RepositoryState::Clean => t.tr("conflict_check.in_progress"),
        _ => t.tr("conflict_check.no_target"),
    }
}

/// Checks merging local or remote-tracking branch `name` (origin/x) into the checked-out branch.
fn check_branch(st: &mut AppState, name: &str, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    let (Some(head), Some(ours)) = (snap.head_branch.clone(), snap.head_oid) else {
        st.log(false, "Check out a branch with commits to check it for conflicts.");
        return;
    };
    if head == name {
        st.log(false, format!("{name} is the checked-out branch."));
        return;
    }
    let local = snap.locals.iter().find(|b| b.name == name).and_then(|b| b.oid);
    let remote =
        || snap.remotes.iter().flat_map(|(r, bs)| bs.iter().map(move |(n, o)| (r, n, *o))).find(|(r, n, _)| format!("{r}/{n}") == name);
    let Some(oid) = local.or_else(|| remote().map(|(_, _, o)| o)) else {
        st.log(false, format!("There is no branch {name}."));
        return;
    };
    let workdir = snap.workdir.clone();
    let target = MergeTarget { name: name.to_owned(), oid, kind: TargetKind::Chosen };
    manual(workdir, CheckRequest { branch: head, ours, target }, fx);
}

fn manual(workdir: std::path::PathBuf, request: CheckRequest, fx: &mut Vec<Effect>) {
    fx.push(Effect::CheckConflicts(CheckJob { workdir, request, manual: true, generation: 0 }));
}
