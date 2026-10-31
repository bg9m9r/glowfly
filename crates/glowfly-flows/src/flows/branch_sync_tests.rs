//! Headless tests for keeping a branch's remote copy in step: Rename with the remote (pushed
//! before the old name is deleted, a push or delete that fails halfway, the lease on the old tip,
//! the refusals) and the Delete remote branch dialog's warning with Unset upstream. End to end
//! against a repository with a bare `origin`.

use super::branch_tests::{remote_commit, with_origin};
use super::tests::{drive, git};
use super::update;
use crate::effect::Effect;
use crate::msg::{BranchMsg, JobMsg, Msg, RepoMsg};
use crate::state::branch_sync::RenameForm;
use crate::state::{AppState, Modal};
use glowfly_core::gitcli::{args, run as git_run};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Tracking;
use std::path::Path;
use std::time::Instant;

const T: Lang = Lang::En;

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

fn rename(old: &str, new: &str) -> Msg {
    Msg::Branch(BranchMsg::RenameBranch { old: old.into(), new: new.into(), remote: true })
}

fn last_toast(st: &AppState) -> String {
    st.toasts.last().map(|t| t.text.clone()).unwrap_or_default()
}

/// Whether `a` is Try again (the toast holds a proxy for the run's fix, under its label).
fn retries(a: Option<&crate::state::ToastAction>) -> bool {
    a.is_some_and(|a| a.label == T.tr("job_retry.button"))
}

fn last_log(st: &AppState) -> String {
    st.log.last().map(|l| l.text.clone()).unwrap_or_default()
}

/// Whether `bare` has branch `name`.
fn on_remote(bare: &Path, name: &str) -> bool {
    git_run(bare, &args(&["rev-parse", "--verify", "--quiet", &format!("refs/heads/{name}")])).0
}

fn tracking(st: &AppState, name: &str) -> Option<Tracking> {
    st.snap.as_ref().unwrap().locals.iter().find(|b| b.name == name).expect("branch").tracking.clone()
}

fn upstream(merge: &str) -> Option<Tracking> {
    Some(Tracking { remote: "origin".into(), merge: merge.into() })
}

#[test]
fn rename_with_remote_pushes_then_deletes_the_old_name() {
    let (mut st, work, bare) = with_origin("sync-rename");
    st.modal = Some(Modal::RenameBranch(RenameForm { name: "feature-2".into(), remote: true, ..RenameForm::new("feature".into()) }));
    let tip = crate::state::remote_branch::exists(st.snap.as_ref().unwrap(), "origin", "feature").unwrap();
    let fx = update(&mut st, rename("feature", "feature-2"), Instant::now());
    let [Effect::RunGit { cmds, .. }] = &fx[..] else { panic!("one job: {fx:?}") };
    assert_eq!(cmds, &glowfly_core::branch_ops::rename_with_remote_cmds("feature", "feature-2", "origin", "feature", tip));
    assert!(st.modal.is_none(), "the dialog closes");
    super::tests::finish(&mut st, fx);
    assert_eq!(tracking(&st, "feature-2"), upstream("feature-2"), "the upstream moved");
    assert!(on_remote(&bare, "feature-2") && !on_remote(&bare, "feature"));
    assert_eq!(last_toast(&st), "Renamed feature to feature-2 here and on origin.");
    assert!(st.rename_job.is_none());
    cleanup(&[&work, &bare]);
}

#[test]
fn a_failed_push_keeps_the_old_remote_branch_and_upstream() {
    let (mut st, work, bare) = with_origin("sync-push-fails");
    let nowhere = work.with_file_name("sync-push-fails-nowhere.git");
    git(&work, &["remote", "set-url", "--push", "origin", &nowhere.display().to_string()]);
    drive(&mut st, rename("feature", "feature-2"));
    assert_eq!(tracking(&st, "feature-2"), upstream("feature"), "renamed here, still tracking the old name");
    assert!(on_remote(&bare, "feature") && !on_remote(&bare, "feature-2"));
    let toast = st.toasts.last().expect("a toast");
    assert!(toast.text.starts_with("Renamed feature to feature-2 here, but pushing it to origin failed: "), "{}", toast.text);
    assert!(toast.text.ends_with(". origin/feature is kept, and feature-2 still tracks it."), "{}", toast.text);
    assert!(!retries(toast.action.as_ref()) && !retries(toast.alt.as_ref()), "no Try again: the rename already ran");
    cleanup(&[&work, &bare]);
}

#[test]
fn a_failed_delete_says_the_old_name_is_still_there() {
    let (mut st, work, bare) = with_origin("sync-delete-fails");
    git(&bare, &["config", "receive.denyDeletes", "true"]);
    drive(&mut st, rename("feature", "feature-2"));
    assert_eq!(tracking(&st, "feature-2"), upstream("feature-2"), "the push moved the upstream");
    assert!(on_remote(&bare, "feature") && on_remote(&bare, "feature-2"));
    let text = last_toast(&st);
    assert!(text.starts_with("Renamed feature to feature-2 and pushed it to origin as its upstream, but deleting origin/feature failed: "));
    assert!(text.ends_with(". The old name is still on origin."), "{text}");
    cleanup(&[&work, &bare]);
}

/// Commits pushed to the old name since the last fetch are never deleted with it.
#[test]
fn the_delete_is_leased_on_the_tip_last_fetched() {
    let (mut st, work, bare) = with_origin("sync-lease");
    remote_commit(&bare, "feature", "theirs.txt");
    let theirs = git_run(&bare, &args(&["rev-parse", "refs/heads/feature"])).1;
    drive(&mut st, rename("feature", "feature-2"));
    assert!(last_toast(&st).ends_with(". The old name is still on origin."), "{}", last_toast(&st));
    assert_eq!(git_run(&bare, &args(&["rev-parse", "refs/heads/feature"])).1, theirs, "their commit stays");
    cleanup(&[&work, &bare]);
}

#[test]
fn a_failed_rename_keeps_the_pending_job_for_try_again() {
    let (mut st, work, bare) = with_origin("sync-rename-fails");
    let fx = update(&mut st, rename("feature", "feature-2"), Instant::now());
    assert!(matches!(&fx[..], [Effect::RunGit { .. }]));
    let output = "$ git branch -m feature feature-2\nfatal: Unable to create '.git/refs/heads/feature-2.lock': File exists.".to_owned();
    update(&mut st, Msg::Job(JobMsg::JobDone { ok: false, output, undo: None }), Instant::now());
    assert!(st.rename_job.is_some(), "kept for a run again");
    assert!(st.toasts.last().is_some_and(|t| retries(t.action.as_ref()) || retries(t.alt.as_ref())), "Try again is offered");
    cleanup(&[&work, &bare]);
}

#[test]
fn renaming_on_the_remote_is_refused_with_a_reason() {
    let (mut st, work, bare) = with_origin("sync-refused");
    git(&work, &["push", "-q", "origin", "main:taken", "main:group/x"]);
    git(&work, &["config", "gitflow.branch.master", "main"]);
    git(&work, &["config", "gitflow.branch.develop", "develop"]);
    for kind in ["feature", "release", "hotfix"] {
        git(&work, &["config", &format!("gitflow.prefix.{kind}"), &format!("{kind}/")]);
    }
    git(&work, &["branch", "--track", "develop", "origin/feature"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let snap = st.snap.as_ref().unwrap();
    let b = |n: &str| snap.locals.iter().find(|b| b.name == n).unwrap().clone();
    let why = |n: &str, new: &str| crate::state::branch_sync::remote_rename(T, snap, &b(n), new).map(|r| r.err());
    assert_eq!(
        why("main", "trunk"),
        Some(Some("main is the default branch of origin, so it is renamed only here: others build on it.".into()))
    );
    assert!(snap.gitflow.base_role("develop").is_some(), "Git Flow is set up");
    {
        assert_eq!(
            why("develop", "dev"),
            Some(Some("Git Flow uses develop as its development branch, so it is renamed only here.".into()))
        );
    }
    assert_eq!(why("feature", "taken"), Some(Some("origin/taken already exists, so the new name can't be pushed there.".into())));
    assert_eq!(why("feature", "group"), Some(Some("group clashes with origin/group/x on the remote.".into())));
    assert_eq!(why("feature", "fine"), Some(None));

    // A branch without an upstream offers nothing; refused in update, the dialog stays open.
    git(&work, &["branch", "solo"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    st.modal = Some(Modal::RenameBranch(RenameForm::new("solo".into())));
    let fx = update(&mut st, rename("solo", "solo-2"), Instant::now());
    assert!(fx.is_empty() && matches!(st.modal, Some(Modal::RenameBranch(_))));
    assert_eq!(last_log(&st), "solo has no upstream on a remote to rename.");

    // Behind its upstream: deleting the old name would lose the commits it lacks.
    st.snap.as_mut().unwrap().locals.iter_mut().find(|b| b.name == "feature").unwrap().behind = 2;
    assert!(update(&mut st, rename("feature", "feature-2"), Instant::now()).is_empty());
    assert_eq!(
        last_log(&st),
        "origin/feature has 2 commits that feature doesn't have. Pull them first, or deleting origin/feature would lose them."
    );
    cleanup(&[&work, &bare]);
}

#[test]
fn delete_remote_warns_of_tracking_branches_and_unsets_them() {
    let (mut st, work, bare) = with_origin("sync-delete-remote");
    git(&work, &["branch", "--track", "copy", "origin/feature"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    update(&mut st, Msg::Branch(BranchMsg::DeleteRemoteDialog { remote: "origin".into(), name: "feature".into() }), Instant::now());
    let Some(Modal::DeleteRemoteBranch(form)) = &st.modal else { panic!("the dialog") };
    assert_eq!(form.tracking, ["copy", "feature"]);
    assert!(form.unset, "offered ticked");
    assert_eq!(
        form.tracking_note(T).as_deref(),
        Some("2 local branches track origin/feature: copy, feature. Their upstreams would point at a branch that no longer exists.")
    );
    assert_eq!(form.unset_label(T), "Unset the upstreams of all 2");
    let delete = form.delete_msg();
    let fx = update(&mut st, delete, Instant::now());
    let [Effect::RunGit { cmds, .. }] = &fx[..] else { panic!("one job") };
    assert_eq!(
        cmds,
        &[
            args(&["push", "origin", "--delete", "feature"]),
            args(&["branch", "--unset-upstream", "copy"]),
            args(&["branch", "--unset-upstream", "feature"])
        ]
    );
    assert!(st.modal.is_none());
    super::tests::finish(&mut st, fx);
    assert!(!on_remote(&bare, "feature"));
    assert_eq!((tracking(&st, "copy"), tracking(&st, "feature")), (None, None));
    cleanup(&[&work, &bare]);
}

#[test]
fn delete_remote_keeps_upstreams_when_untick_or_when_it_fails() {
    let (mut st, work, bare) = with_origin("sync-delete-keep");
    let msg = |unset: &[&str]| {
        let unset = unset.iter().map(|s| s.to_string()).collect();
        Msg::Branch(BranchMsg::DeleteRemote { remote: "origin".into(), name: "feature".into(), unset })
    };
    // The delete fails: the upstream stays, as it still names a branch that exists.
    git(&bare, &["config", "receive.denyDeletes", "true"]);
    drive(&mut st, msg(&["feature"]));
    assert!(on_remote(&bare, "feature"));
    assert_eq!(tracking(&st, "feature"), upstream("feature"));
    // Unticked: only the delete runs.
    git(&bare, &["config", "receive.denyDeletes", "false"]);
    drive(&mut st, msg(&[]));
    assert!(!on_remote(&bare, "feature"));
    assert_eq!(tracking(&st, "feature"), upstream("feature"), "left tracking the deleted branch");
    // Gone by the time it is confirmed.
    update(&mut st, Msg::Branch(BranchMsg::DeleteRemoteDialog { remote: "origin".into(), name: "feature".into() }), Instant::now());
    assert!(st.modal.is_none());
    assert_eq!(last_log(&st), "origin/feature no longer exists.");
    cleanup(&[&work, &bare]);
}
