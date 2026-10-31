//! Headless tests for the clean-up after a merge from the pull request panel: the job's commands
//! with and without deleting the local branch, the checked-out branch (a clean working tree
//! checks out the target first; changes keep the branch), a branch checked out in another
//! worktree, a fork's same-named branch kept, a refusal for commits the target lacks (run for real
//! against a local remote), and the merge method remembered per repository.

use super::calls_tests::{calls, done, git_jobs, loaded, opened, reply_msg, run, view};
use crate::effect::Effect;
use crate::flows::{drive, finish_keeping, git};
use crate::msg::{Msg, RepoMsg};
use crate::pull_request::{PrCall, PrReply, PullRequestMsg};
use crate::state::AppState;
use crate::update::update;
use glowfly_core::forge::{MergeMethod, Merged};
use glowfly_core::undo::UndoKind;
use std::path::Path;
use std::time::Instant;

fn cmd(c: &[&str]) -> Vec<String> {
    c.iter().map(|s| s.to_string()).collect()
}

fn merged(method: MergeMethod, delete: bool, delete_local: bool) -> PrReply {
    let result = Ok(Merged { sha: "m1".into() });
    PrReply::Merged { method, remote: "origin".into(), source: "feature".into(), delete, delete_local, result }
}

/// The reply to a merge commit of #7, with the local branch to go too, as `update` handles it.
fn reply(st: &mut AppState, delete: bool) -> Vec<Effect> {
    let msg = reply_msg(st, merged(MergeMethod::Merge, delete, true));
    update(st, Msg::PullRequest(msg), Instant::now())
}

/// Reads the repository again after git ran outside the app, keeping the service the setup gave
/// (detection finds none for a local-path remote).
fn refresh(st: &mut AppState) {
    let forge = st.session.forge.take();
    drive(st, Msg::Repo(RepoMsg::Refresh));
    st.session.forge = forge;
}

fn logged(st: &AppState) -> Vec<&str> {
    st.log.iter().map(|l| l.text.as_str()).collect()
}

fn locals(work: &Path) -> String {
    glowfly_core::testkit::git_out(work, &["branch", "--format=%(refname:short)"])
}

/// What the service does on a merge commit: main on origin gets a merge of origin/feature; the
/// local main stays where it was.
fn merge_on_service(work: &Path, back_to: &str) {
    git(work, &["checkout", "-q", "-b", "svc", "origin/main"]);
    git(work, &["merge", "-q", "--no-ff", "-m", "Merge feature", "origin/feature"]);
    git(work, &["push", "-q", "origin", "svc:main"]);
    git(work, &["checkout", "-q", back_to]);
    git(work, &["branch", "-q", "-D", "svc"]);
}

const PUSH_DELETE: &[&str] = &["push", "--delete", "--", "origin", "feature"];
const FETCH: &[&str] = &["fetch", "--prune", "--", "origin"];
const CHECK: &[&str] = &["merge-base", "--is-ancestor", "refs/heads/feature", "refs/remotes/origin/main"];
const TRACK: &[&str] = &["config", "branch.feature.merge", "refs/heads/main"];
const DELETE: &[&str] = &["branch", "-d", "--", "feature"];

#[test]
fn the_form_sends_the_local_delete_only_when_it_can_happen() {
    let (mut st, _, dirs) = opened("prc-local-form");
    loaded(&mut st, |d| d.methods = vec![MergeMethod::Merge, MergeMethod::Squash]);
    let sent = |st: &mut AppState| {
        let fx = run(st, PullRequestMsg::Merge);
        let local = match calls(&fx).first().map(|j| &j.call) {
            Some(PrCall::Merge { delete_local, .. }) => *delete_local,
            other => panic!("{other:?}"),
        };
        st.session.pull_request.view.as_mut().unwrap().busy = None;
        local
    };
    assert!(!sent(&mut st), "not asked for");
    st.session.pull_request.view.as_mut().unwrap().forms.delete_local = true;
    assert!(sent(&mut st));
    // A squash gives the commits new ids: git could never tell the branch is merged.
    st.session.pull_request.view.as_mut().unwrap().forms.merge_method = Some(MergeMethod::Squash);
    assert!(!sent(&mut st));
    done(st, dirs);
}

#[test]
fn merged_with_local_delete_runs_remote_steps_then_the_checked_delete() {
    let (mut st, _, dirs) = opened("prc-local-delete");
    loaded(&mut st, |_| {});
    let fx = reply(&mut st, true);
    assert_eq!(git_jobs(&fx), [[PUSH_DELETE, FETCH, CHECK, TRACK, DELETE].map(cmd).to_vec()]);
    let record = fx.iter().find_map(|e| match e {
        Effect::RunGit { record, .. } => Some(record.clone()),
        _ => None,
    });
    assert_eq!(record, Some(Some(UndoKind::DeleteBranch("feature".into()))), "Undo brings the branch back");
    let label = "Delete feature on origin, fetch origin and delete local branch feature";
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some(label));
    assert_eq!(st.pr_cleanup.as_ref().map(|j| j.steps.len()), Some(5));

    // Without the remote delete, only the fetch comes first.
    let (mut st2, _, dirs2) = opened("prc-local-fetch");
    loaded(&mut st2, |_| {});
    let fx = reply(&mut st2, false);
    assert_eq!(git_jobs(&fx), [[FETCH, CHECK, TRACK, DELETE].map(cmd).to_vec()]);
    assert_eq!(st2.job.as_ref().map(|j| j.label.as_str()), Some("Fetch origin and delete local branch feature"));
    done(st, dirs);
    done(st2, dirs2);
}

#[test]
fn a_fork_listed_by_the_service_keeps_the_same_named_local_branch() {
    // The page says the same repository, but the service's list has the source on a fork: the
    // local `feature` tracking origin/feature is another branch, so it is never offered or deleted.
    let (mut st, _, dirs) = opened("prc-local-fork");
    loaded(&mut st, |_| {});
    let fork = glowfly_core::forge::HeadRepo::Fork { name: "bob/app".into() };
    st.session.pull_request.view.as_mut().unwrap().pr.head_repo = fork;
    st.session.pull_request.view.as_mut().unwrap().forms.delete_local = true;
    let fx = run(&mut st, PullRequestMsg::Merge);
    assert!(matches!(calls(&fx).first().map(|j| &j.call), Some(PrCall::Merge { delete_local: false, .. })));
    st.session.pull_request.view.as_mut().unwrap().busy = None;
    let fx = reply(&mut st, false);
    assert_eq!(git_jobs(&fx), [[FETCH].map(cmd).to_vec()]);
    assert!(logged(&st).iter().any(|l| l.contains("The source branch is on a fork")), "{:?}", logged(&st));
    done(st, dirs);
}

#[test]
fn a_checked_out_branch_goes_after_checking_out_the_target() {
    let (mut st, _, dirs) = opened("prc-local-head");
    // The fixture leaves a.txt changed.
    git(&dirs[0], &["checkout", "-q", "--", "a.txt"]);
    git(&dirs[0], &["checkout", "-q", "feature"]);
    refresh(&mut st);
    loaded(&mut st, |_| {});
    let fx = reply(&mut st, false);
    assert_eq!(git_jobs(&fx), [[FETCH, CHECK, &["checkout", "main", "--"], TRACK, DELETE].map(cmd).to_vec()]);
    done(st, dirs);
}

#[test]
fn a_checked_out_branch_with_changes_is_kept() {
    let (mut st, _, dirs) = opened("prc-local-dirty");
    git(&dirs[0], &["checkout", "-q", "feature"]);
    std::fs::write(dirs[0].join("f.txt"), "edited\n").unwrap();
    refresh(&mut st);
    loaded(&mut st, |_| {});
    let fx = reply(&mut st, true);
    assert_eq!(git_jobs(&fx), [[PUSH_DELETE, FETCH].map(cmd).to_vec()], "the rest still runs");
    let why = "Kept local branch feature. feature is checked out and the working tree has changes, so main can't be checked out first.";
    assert!(logged(&st).contains(&why), "{:?}", logged(&st));
    assert!(st.pr_cleanup.is_none());
    done(st, dirs);
}

#[test]
fn a_branch_checked_out_in_another_worktree_is_kept() {
    let (mut st, _, dirs) = opened("prc-local-worktree");
    let wt = dirs[0].with_file_name("prc-local-worktree-linked");
    let _ = std::fs::remove_dir_all(&wt);
    git(&dirs[0], &["worktree", "add", "-q", &wt.display().to_string(), "feature"]);
    refresh(&mut st);
    loaded(&mut st, |d| d.methods = vec![MergeMethod::Merge]);
    let v = view(&st);
    let snap = st.snap.as_ref().unwrap();
    let m = crate::state::pull_request_actions::merging(v, None).unwrap();
    let (branch, plan) = crate::state::pull_request_cleanup::local_delete(st.settings.lang(), snap, &m).unwrap();
    assert_eq!(branch, "feature");
    assert!(plan.is_err_and(|why| why.starts_with("feature is checked out in the worktree at")), "the checkbox is disabled");
    let fx = reply(&mut st, false);
    assert_eq!(git_jobs(&fx), [[FETCH].map(cmd).to_vec()]);
    assert!(logged(&st).iter().any(|l| l.starts_with("Kept local branch feature. feature is checked out in the worktree at")));
    drop(st);
    let _ = std::fs::remove_dir_all(&wt);
    super::tests::cleanup(&[&dirs[0], &dirs[1]]);
}

#[test]
fn the_merged_branch_is_deleted_for_real() {
    let (mut st, _, dirs) = opened("prc-local-real");
    git(&dirs[0], &["checkout", "-q", "--", "a.txt"]);
    git(&dirs[0], &["checkout", "-q", "feature"]);
    merge_on_service(&dirs[0], "feature");
    refresh(&mut st);
    loaded(&mut st, |_| {});
    let fx = reply(&mut st, true);
    finish_keeping(&mut st, fx);
    assert_eq!(locals(&dirs[0]).trim(), "main", "feature is gone and main is checked out");
    let label = "Delete feature on origin, fetch origin and delete local branch feature";
    assert!(
        st.toasts.iter().any(|t| t.ok && t.text == format!("{label} succeeded")),
        "{:?}",
        st.toasts.iter().map(|t| &t.text).collect::<Vec<_>>()
    );
    assert!(st.pr_cleanup.is_none());
    done(st, dirs);
}

#[test]
fn a_branch_with_commits_the_target_lacks_is_kept() {
    let (mut st, _, dirs) = opened("prc-local-unmerged");
    merge_on_service(&dirs[0], "main");
    // A commit made after the push: not on the service's main.
    git(&dirs[0], &["checkout", "-q", "feature"]);
    std::fs::write(dirs[0].join("late.txt"), "late\n").unwrap();
    git(&dirs[0], &["add", "late.txt"]);
    git(&dirs[0], &["commit", "-q", "-m", "late"]);
    git(&dirs[0], &["checkout", "-q", "main"]);
    refresh(&mut st);
    loaded(&mut st, |_| {});
    let fx = reply(&mut st, false);
    finish_keeping(&mut st, fx);
    assert!(locals(&dirs[0]).lines().any(|b| b == "feature"), "kept");
    let headline = "Kept local branch feature: it has commits that aren't on origin/main.";
    assert!(st.toasts.iter().any(|t| !t.ok && t.text == headline), "{:?}", st.toasts.iter().map(|t| &t.text).collect::<Vec<_>>());
    let upstream = glowfly_core::testkit::git_out(&dirs[0], &["rev-parse", "--abbrev-ref", "feature@{upstream}"]);
    assert_eq!(upstream.trim(), "origin/feature", "its upstream is left alone");
    done(st, dirs);
}

#[test]
fn a_refused_delete_puts_the_upstream_back() {
    let (mut st, _, dirs) = opened("prc-local-restore");
    merge_on_service(&dirs[0], "main");
    refresh(&mut st);
    loaded(&mut st, |_| {});
    let fx = reply(&mut st, false);
    // Checked out in a worktree added after the plan: git refuses the delete after the repoint.
    let wt = dirs[0].with_file_name("prc-local-restore-linked");
    let _ = std::fs::remove_dir_all(&wt);
    git(&dirs[0], &["worktree", "add", "-q", &wt.display().to_string(), "feature"]);
    finish_keeping(&mut st, fx);
    assert!(locals(&dirs[0]).lines().any(|b| b == "feature"), "kept");
    let merge = glowfly_core::testkit::git_out(&dirs[0], &["config", "branch.feature.merge"]);
    assert_eq!(merge.trim(), "refs/heads/feature", "it tracks the source again, not main");
    let toast = st.toasts.iter().find(|t| !t.ok).map(|t| t.text.as_str()).unwrap_or_default();
    assert!(toast.starts_with("Kept local branch feature: git didn't delete it."), "{toast}");
    assert!(st.pr_cleanup.is_none());
    drop(st);
    let _ = std::fs::remove_dir_all(&wt);
    super::tests::cleanup(&[&dirs[0], &dirs[1]]);
}

#[test]
fn the_merge_method_is_remembered_per_repository() {
    let (mut st, _, dirs) = opened("prc-method");
    loaded(&mut st, |d| d.methods = vec![MergeMethod::Merge, MergeMethod::Squash, MergeMethod::Rebase]);
    assert_eq!(view(&st).forms.merge_method, Some(MergeMethod::Merge), "the first allowed one at first");
    st.session.pull_request.view.as_mut().unwrap().forms.merge_method = Some(MergeMethod::Rebase);
    let fx = run(&mut st, PullRequestMsg::Merge);
    assert!(fx.iter().any(
        |e| matches!(e, Effect::SaveSettings(s) if s.merge_method_for(&st.snap.as_ref().unwrap().real_workdir) == Some(MergeMethod::Rebase))
    ));

    // Opened again: preselected, while the repository allows it.
    run(&mut st, PullRequestMsg::Close);
    run(&mut st, PullRequestMsg::Open(7));
    loaded(&mut st, |d| d.methods = vec![MergeMethod::Merge, MergeMethod::Rebase]);
    assert_eq!(view(&st).forms.merge_method, Some(MergeMethod::Rebase));
    run(&mut st, PullRequestMsg::Close);
    run(&mut st, PullRequestMsg::Open(7));
    loaded(&mut st, |d| d.methods = vec![MergeMethod::Squash, MergeMethod::Merge]);
    assert_eq!(view(&st).forms.merge_method, Some(MergeMethod::Squash), "rebasing turned off: the first allowed one");
    done(st, dirs);
}
