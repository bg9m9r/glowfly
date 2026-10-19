//! Headless tests for applying a patch file: reading it for the Apply patch dialog, the jobs
//! each choice starts and their refusals, and end-to-end applies (three-way conflicts, .rej
//! files, a series as commits, and the Applying patches banner's buttons) on real repositories.

use super::super::test_mods::patch_tests::{beside, cleanup, commit, last_log, last_toast, run, three_commits};
use super::super::tests::{drive, git};
use super::super::update;
use crate::effect::Effect;
use crate::msg::{JobMsg, Msg, RepoMsg};
use crate::patch::{ApplyHow, ApplyRequest, PatchEffect, PatchMsg};
use crate::state::patch::apply::{ApplyForm, ApplyMode, STAGED_FIRST};
use crate::state::patch::{AM_LABEL, APPLY_LABEL, PendingApply};
use crate::state::{AppState, Modal};
use crate::ui::common::state_actions;
use git2::RepositoryState;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::patch_ops::{self, AM_ABORT, AM_CONTINUE, ApplyOptions, OnFail, PatchSummary};

fn request(st: &AppState, path: &Path, how: ApplyHow, commits: usize) -> ApplyRequest {
    ApplyRequest { workdir: st.workdir().unwrap(), path: path.to_owned(), how, commits }
}

fn changes(on_fail: OnFail, index: bool, reverse: bool) -> ApplyHow {
    ApplyHow::Changes(ApplyOptions { index, on_fail, reverse })
}

/// Sends the Apply patch dialog's `how` for `path`.
fn send(st: &mut AppState, path: &Path, how: ApplyHow, commits: usize) -> Vec<Effect> {
    let req = request(st, path, how, commits);
    run(st, PatchMsg::Apply(req))
}

/// The commands of the job `fx` starts.
fn job_cmds(fx: &[Effect]) -> Vec<Vec<String>> {
    match fx {
        [Effect::RunGit { cmds, .. }] => cmds.clone(),
        other => panic!("unexpected effects {other:?}"),
    }
}

/// The Apply patch dialog on `p`.
fn dialog(st: &AppState, p: &Path) -> Modal {
    Modal::ApplyPatch(Box::new(ApplyForm::new(st.workdir().unwrap(), p.to_owned(), PatchSummary::default())))
}

/// Ends the active job without running it, so the next one can start.
fn forget_job(st: &mut AppState) {
    st.job = None;
    st.patch_apply = None;
}

fn head(dir: &Path) -> git2::Oid {
    git2::Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

#[test]
fn apply_from_reads_the_file_before_any_job() {
    let (mut st, dir) = three_commits("patch-apply-inspect");
    let workdir = st.workdir().unwrap();
    let p = beside(&dir, "p.patch");
    assert!(run(&mut st, PatchMsg::ApplyFrom { workdir: workdir.clone(), path: None }).is_empty());
    let fx = run(&mut st, PatchMsg::ApplyFrom { workdir: workdir.clone(), path: Some(p.clone()) });
    assert!(matches!(&fx[..], [Effect::Patch(PatchEffect::Inspect { workdir: w, path })] if *w == workdir && *path == p), "{fx:?}");
    assert!(st.job.is_none() && st.modal.is_none());
    cleanup(&dir);
}

#[test]
fn inspected_opens_the_dialog_in_the_mode_the_file_suits() {
    let (mut st, dir) = three_commits("patch-apply-dialog");
    let workdir = st.workdir().unwrap();
    let mbox = beside(&dir, "series.patch");
    patch_ops::export(&dir, &[commit(&st, "Add the second line"), commit(&st, "Third: more (lines)")], &mbox).unwrap();
    drive(&mut st, Msg::Patch(PatchMsg::ApplyFrom { workdir: workdir.clone(), path: Some(mbox) }));
    let Some(Modal::ApplyPatch(form)) = &st.modal else { panic!("the dialog") };
    assert_eq!((form.mode, form.summary.commits, form.name.as_str()), (ApplyMode::Commits, 2, "series.patch"));
    assert_eq!(form.summary.files, ["a.txt", "b.txt"]);

    let diff = beside(&dir, "plain.diff");
    std::fs::write(&diff, "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-one\n+uno\n").unwrap();
    drive(&mut st, Msg::Patch(PatchMsg::ApplyFrom { workdir, path: Some(diff) }));
    let Some(Modal::ApplyPatch(form)) = &st.modal else { panic!("the dialog") };
    assert_eq!((form.mode, form.summary.commits), (ApplyMode::Changes, 0));
    assert!(st.job.is_none(), "nothing runs before Apply");
    cleanup(&dir);
}

#[test]
fn inspected_elsewhere_or_unreadable() {
    let (mut st, dir) = three_commits("patch-apply-inspected");
    let p = beside(&dir, "p.patch");
    let other = dir.with_file_name("someone-else");
    run(&mut st, PatchMsg::Inspected { workdir: other, path: p.clone(), result: Ok(PatchSummary::default()) });
    assert!(last_log(&st).contains("no longer the active tab"), "{}", last_log(&st));
    assert!(
        st.modal.is_none() && st.toasts.len() == 1 && crate::update::logs_tabs::tests::failure_with_details(&st),
        "announced with Details"
    );

    let workdir = st.workdir().unwrap();
    run(&mut st, PatchMsg::Inspected { workdir: workdir.clone(), path: p.clone(), result: Err("access denied".into()) });
    assert_eq!(last_toast(&st), (false, "Can't read p.patch: access denied".to_owned()));
    assert_eq!(last_log(&st), "Can't read p.patch: access denied");
    let big = "p.patch is larger than 64 MB; Trellis doesn't open patches that large.";
    run(&mut st, PatchMsg::Inspected { workdir, path: p, result: Err(big.into()) });
    assert_eq!(last_toast(&st), (false, big.to_owned()));
    assert!(st.modal.is_none());
    cleanup(&dir);
}

#[test]
fn apply_changes_starts_the_chosen_git_apply() {
    let (mut st, dir) = three_commits("patch-apply-jobs");
    let p = beside(&dir, "p.patch");
    let text = p.to_str().unwrap().to_owned();
    let cases = [
        (changes(OnFail::Refuse, false, false), vec![vec!["apply", "--check"], vec!["apply"]]),
        (changes(OnFail::ThreeWay, true, false), vec![vec!["apply", "--3way"]]),
        (changes(OnFail::Reject, false, true), vec![vec!["apply", "--reject", "-R"]]),
    ];
    for (how, want) in cases {
        st.modal = Some(dialog(&st, &p));
        let cmds = job_cmds(&send(&mut st, &p, how, 0));
        let want: Vec<Vec<String>> = want.into_iter().map(|c| c.into_iter().map(str::to_owned).chain([text.clone()]).collect()).collect();
        assert_eq!(cmds, want);
        assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some(APPLY_LABEL));
        assert_eq!(st.patch_apply, Some(PendingApply { label: APPLY_LABEL.into(), name: "p.patch".into(), how, commits: 0 }));
        assert!(st.modal.is_none(), "the dialog closes once the job starts");
        forget_job(&mut st);
    }

    // While another job runs the dialog stays open.
    assert!(!update(&mut st, Msg::git("Fetch", &["fetch"]), Instant::now()).is_empty());
    st.modal = Some(dialog(&st, &p));
    assert!(send(&mut st, &p, changes(OnFail::Refuse, false, false), 0).is_empty());
    assert_eq!(last_log(&st), "Another operation is still running.");
    assert!(matches!(st.modal, Some(Modal::ApplyPatch(_))));
    cleanup(&dir);
}

#[test]
fn apply_commits_starts_git_am_unless_blocked() {
    let (mut st, dir) = three_commits("patch-apply-am-job");
    let p = beside(&dir, "p.patch");
    let cmds = job_cmds(&send(&mut st, &p, ApplyHow::Commits { three_way: true }, 2));
    assert_eq!(cmds, [patch_ops::am_cmd(p.to_str().unwrap(), true)]);
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some(AM_LABEL));
    forget_job(&mut st);

    // Something staged.
    std::fs::write(
        dir.join("a.txt"),
        "staged
",
    )
    .unwrap();
    git(&dir, &["add", "a.txt"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(send(&mut st, &p, ApplyHow::Commits { three_way: false }, 2).is_empty());
    assert_eq!(last_log(&st), STAGED_FIRST);
    // An operation in progress.
    git(&dir, &["reset", "-q", "--hard"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    st.snap.as_mut().unwrap().state = RepositoryState::Merge;
    assert!(send(&mut st, &p, ApplyHow::Commits { three_way: true }, 2).is_empty());
    assert_eq!(last_log(&st), "Finish or abort the merge in progress first.");
    // A plain diff has no commits to apply.
    st.snap.as_mut().unwrap().state = RepositoryState::Clean;
    assert!(send(&mut st, &p, ApplyHow::Commits { three_way: true }, 0).is_empty());
    assert!(st.job.is_none());
    cleanup(&dir);
}

/// `three_commits`' later commits (`which`) saved as p.patch, then the branch reset to "first"
/// and, with `mine`, a commit that changes a.txt's second line.
fn patch_then_reset(name: &str, which: &[&str], mine: bool) -> (AppState, PathBuf, PathBuf) {
    let (mut st, dir) = three_commits(name);
    let p = beside(&dir, "p.patch");
    let oids: Vec<_> = which.iter().map(|s| commit(&st, s)).collect();
    patch_ops::export(&dir, &oids, &p).unwrap();
    git(&dir, &["reset", "-q", "--hard", "HEAD~2"]);
    if mine {
        std::fs::write(dir.join("a.txt"), "one\nmine\n").unwrap();
        git(&dir, &["commit", "-q", "-am", "Mine"]);
    }
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    (st, dir, p)
}

fn apply_now(st: &mut AppState, p: &Path, how: ApplyHow, commits: usize) {
    let req = request(st, p, how, commits);
    drive(st, Msg::Patch(PatchMsg::Apply(req)));
}

#[test]
fn apply_end_to_end_success_and_failure() {
    let (mut st, dir, p) = patch_then_reset("patch-apply-ok", &["Add the second line"], false);
    apply_now(&mut st, &p, changes(OnFail::Refuse, false, false), 1);
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "one\ntwo\n");
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.unstaged.iter().any(|f| f.path == "a.txt") && snap.staged.is_empty(), "the index is untouched");
    assert_eq!(last_toast(&st), (true, "Applied p.patch to the working tree.".to_owned()));
    assert!(st.job.is_none() && st.patch_apply.is_none());

    // Applied again, it no longer fits: nothing changes.
    apply_now(&mut st, &p, changes(OnFail::Refuse, false, false), 1);
    let (ok, toast) = last_toast(&st);
    assert!(!ok);
    assert!(toast.starts_with("p.patch doesn't apply: ") && toast.ends_with("Nothing was changed."), "{toast}");
    // Taken back out instead.
    apply_now(&mut st, &p, changes(OnFail::Refuse, false, true), 1);
    assert_eq!(last_toast(&st), (true, "Took p.patch's changes out of the working tree.".to_owned()));
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "one\n");
    cleanup(&dir);
}

#[test]
fn three_way_conflicts_end_to_end() {
    let (mut st, dir, p) = patch_then_reset("patch-apply-3way-e2e", &["Add the second line"], true);
    apply_now(&mut st, &p, changes(OnFail::ThreeWay, true, false), 1);
    let (ok, toast) = last_toast(&st);
    assert!(!ok);
    assert_eq!(toast, "Applied p.patch with conflicts in 1 file. Resolve them in the conflict view.");
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.unstaged.iter().any(|f| f.path == "a.txt" && f.status == '!'), "{:?}", snap.unstaged);
    cleanup(&dir);
}

#[test]
fn reject_end_to_end_names_the_rej_file() {
    let (mut st, dir) = three_commits("patch-apply-reject-e2e");
    let lines: Vec<String> = (1..=20).map(|i| format!("line {i}\n")).collect();
    std::fs::write(dir.join("a.txt"), lines.concat()).unwrap();
    git(&dir, &["commit", "-q", "-am", "Twenty lines"]);
    let mut edited = lines.clone();
    (edited[1], edited[18]) = ("line 2 changed\n".into(), "line 19 changed\n".into());
    std::fs::write(dir.join("a.txt"), edited.concat()).unwrap();
    git(&dir, &["commit", "-q", "-am", "Two hunks"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let p = beside(&dir, "p.patch");
    patch_ops::export(&dir, &[commit(&st, "Two hunks")], &p).unwrap();
    git(&dir, &["reset", "-q", "--hard", "HEAD~1"]);
    let mut mine = lines.clone();
    mine[18] = "line 19 mine\n".into();
    std::fs::write(dir.join("a.txt"), mine.concat()).unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));

    apply_now(&mut st, &p, changes(OnFail::Reject, false, false), 1);
    let (ok, toast) = last_toast(&st);
    assert!(!ok);
    assert_eq!(toast, "Applied part of p.patch: 1 hunk didn't fit and was saved to a.txt.rej.");
    assert!(dir.join("a.txt.rej").exists());
    cleanup(&dir);
}

#[test]
fn a_series_applies_as_commits() {
    let (mut st, dir, p) = patch_then_reset("patch-apply-am-e2e", &["Add the second line", "Third: more (lines)"], false);
    apply_now(&mut st, &p, ApplyHow::Commits { three_way: true }, 2);
    assert_eq!(last_toast(&st), (true, "Applied p.patch as 2 commits.".to_owned()));
    let snap = st.snap.as_ref().unwrap();
    let subjects: Vec<&str> = snap.commits.iter().take(3).map(|c| c.summary.as_str()).collect();
    assert_eq!(subjects, ["Third: more (lines)", "Add the second line", "first"]);
    assert_eq!(snap.state, RepositoryState::Clean);
    cleanup(&dir);
}

#[test]
fn a_stopped_series_then_abort_or_continue() {
    let (mut st, dir, p) = patch_then_reset("patch-apply-am-stop", &["Add the second line", "Third: more (lines)"], true);
    let mine = head(&dir);
    apply_now(&mut st, &p, ApplyHow::Commits { three_way: true }, 2);
    let (ok, toast) = last_toast(&st);
    assert!(!ok);
    assert_eq!(
        toast,
        "Applying p.patch stopped at patch 1 (Add the second line) with conflicts. Resolve them, then Continue; or Skip patch or Abort."
    );
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.state, RepositoryState::ApplyMailbox);
    assert_eq!(snap.am.as_ref().map(|a| (a.step, a.total, a.subject.as_str())), Some((1, 2, "Add the second line")));
    let (name, actions) = state_actions(snap.state).unwrap();
    assert_eq!(name, "Applying patches");
    let labels: Vec<(&str, &[&str])> = actions.to_vec();
    assert_eq!(labels, [("Continue", AM_CONTINUE), ("Skip patch", patch_ops::AM_SKIP), ("Abort", AM_ABORT)]);

    drive(&mut st, Msg::git("Abort", AM_ABORT));
    assert_eq!(last_toast(&st), (true, "Stopped applying patches. The branch is back where it was.".to_owned()));
    assert_eq!(head(&dir), mine);
    assert_eq!(st.snap.as_ref().unwrap().am, None);

    // Again, then resolve, stage and Continue.
    apply_now(&mut st, &p, ApplyHow::Commits { three_way: true }, 2);
    assert_eq!(st.snap.as_ref().unwrap().state, RepositoryState::ApplyMailbox);
    std::fs::write(dir.join("a.txt"), "one\nmine\ntwo\n").unwrap();
    git(&dir, &["add", "a.txt"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::git("Continue", AM_CONTINUE));
    assert_eq!(last_toast(&st), (true, "Applied the remaining patches.".to_owned()));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.state, RepositoryState::Clean);
    let subjects: Vec<&str> = snap.commits.iter().take(3).map(|c| c.summary.as_str()).collect();
    assert_eq!(subjects, ["Third: more (lines)", "Add the second line", "Mine"]);
    cleanup(&dir);
}

#[test]
fn other_job_ending_clears_pending_apply() {
    let (mut st, dir) = three_commits("patch-ui-stale");
    assert!(!update(&mut st, Msg::git("Fetch", &["fetch"]), Instant::now()).is_empty());
    let how = changes(OnFail::Refuse, false, false);
    st.patch_apply = Some(PendingApply { label: APPLY_LABEL.into(), name: "p.patch".into(), how, commits: 0 });
    update(&mut st, Msg::Job(JobMsg::JobDone { ok: false, output: "offline".into(), undo: None }), Instant::now());
    assert_eq!(last_toast(&st), (false, "Fetch failed: offline".to_owned()));
    assert!(st.patch_apply.is_none());
    cleanup(&dir);
}

#[test]
fn conflicts_block_the_index_and_commits_but_not_the_working_tree() {
    let (mut st, dir) = three_commits("patch-apply-blockers");
    let p = beside(&dir, "p.patch");
    st.snap.as_mut().unwrap().unstaged.push(trellis_core::repo::FileEntry::new("x.txt", '!'));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(crate::state::patch::apply::index_blocked(snap), Some(crate::state::patch::apply::RESOLVE_FIRST));
    assert_eq!(crate::state::patch::apply::am_blocked(snap).as_deref(), Some(crate::state::patch::apply::RESOLVE_FIRST));
    for how in [changes(OnFail::Refuse, true, false), changes(OnFail::ThreeWay, true, false), ApplyHow::Commits { three_way: true }] {
        assert!(send(&mut st, &p, how, 1).is_empty(), "{how:?}");
        assert_eq!(last_log(&st), "Resolve the conflicted files first.");
    }
    assert!(!send(&mut st, &p, changes(OnFail::Refuse, false, false), 1).is_empty(), "working tree only");
    cleanup(&dir);
}
