//! Headless tests for the branch menu's operations: rename, upstream, fast-forward, push and
//! delete with the remote branch. Commands are checked as emitted, and end to end against a
//! repository with a bare `origin`.

use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::BranchMsg;
use crate::msg::JobMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::state::{AppState, Modal};
use git2::{Oid, RepositoryState};
use glowfly_core::gitcli::args;
use glowfly_core::repo::{BranchInfo, Tracking};
use glowfly_core::undo::{Action, UndoKind};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// A repository opened in `st` with a bare `origin` next to it; `main` and `feature` are pushed
/// with their upstreams set. Returns (state, working repository, bare repository).
pub(crate) fn with_origin(name: &str) -> (AppState, PathBuf, PathBuf) {
    let work = scratch_repo(name);
    let bare = work.with_file_name(format!("{}-origin.git", work.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&bare);
    std::fs::create_dir_all(&bare).unwrap();
    git(&bare, &["init", "-q", "--bare", "-b", "main"]);
    git(&work, &["remote", "add", "origin", &bare.display().to_string()]);
    git(&work, &["push", "-q", "-u", "origin", "main"]);
    git(&work, &["branch", "feature"]);
    git(&work, &["push", "-q", "-u", "origin", "feature"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(work.clone())));
    (st, work, bare)
}

/// Commits a file on `branch` in `bare` through a temporary clone.
pub(crate) fn remote_commit(bare: &Path, branch: &str, file: &str) {
    let tmp = bare.with_file_name(format!("{}-clone", bare.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&tmp);
    git(tmp.parent().unwrap(), &["clone", "-q", "-b", branch, &bare.display().to_string(), &tmp.display().to_string()]);
    git(&tmp, &["config", "user.name", "Test"]);
    git(&tmp, &["config", "user.email", "test@example.com"]);
    git(&tmp, &["config", "commit.gpgsign", "false"]);
    std::fs::write(tmp.join(file), "remote\n").unwrap();
    git(&tmp, &["add", "."]);
    git(&tmp, &["commit", "-q", "-m", file]);
    git(&tmp, &["push", "-q", "origin", branch]);
    let _ = std::fs::remove_dir_all(&tmp);
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// The commands `msg` starts, or nothing. Clears the job so the next message can start one.
fn cmds(st: &mut AppState, msg: Msg) -> Vec<Vec<String>> {
    let fx = update(st, msg, Instant::now());
    st.job = None;
    match &fx[..] {
        [Effect::RunGit { cmds, .. }] => cmds.clone(),
        [] => Vec::new(),
        other => panic!("unexpected effects {other:?}"),
    }
}

fn last_log(st: &AppState) -> String {
    st.log.last().map(|l| l.text.clone()).unwrap_or_default()
}

fn local<'a>(st: &'a mut AppState, name: &str) -> &'a mut BranchInfo {
    st.snap.as_mut().unwrap().locals.iter_mut().find(|b| b.name == name).expect("branch")
}

/// Adds a branch without an upstream to the snapshot (as if loaded).
fn add_local(st: &mut AppState, name: &str) {
    let b =
        BranchInfo { name: name.into(), is_head: false, tracking: None, upstream: None, upstream_ref: None, ..local(st, "main").clone() };
    st.snap.as_mut().unwrap().locals.push(b);
}

fn done(st: &mut AppState, ok: bool) {
    update(st, Msg::Job(JobMsg::JobDone { ok, output: String::new(), undo: None }), Instant::now());
}

fn dummy() -> Action {
    Action::DeleteBranch { name: "old".into(), oid: Oid::zero(), upstream: None }
}

fn rename(old: &str, new: &str) -> Msg {
    Msg::Branch(BranchMsg::RenameBranch { old: old.into(), new: new.into() })
}

#[test]
fn rename_dialog_and_checks() {
    let (mut st, work, bare) = with_origin("br-rename");
    update(&mut st, Msg::Branch(BranchMsg::RenameBranchDialog("feature".into())), Instant::now());
    assert!(matches!(&st.modal, Some(Modal::RenameBranch { old, name, .. }) if old == "feature" && name == "feature"));
    assert_eq!(cmds(&mut st, rename("feature", " feature-2 ")), vec![args(&["branch", "-m", "feature", "feature-2"])]);
    assert!(st.modal.is_none(), "the dialog closes");

    add_local(&mut st, "group/x");
    for (new, why) in [
        ("a b", "Not a valid branch name."),
        ("Main", "A branch named main already exists."),
        ("group", "group clashes with branch group/x."),
        ("feature", "feature already has that name."),
    ] {
        assert!(cmds(&mut st, rename("feature", new)).is_empty(), "{new}");
        assert_eq!(last_log(&st), why);
    }
    assert!(cmds(&mut st, rename("gone", "x")).is_empty());
    assert_eq!(last_log(&st), "Branch gone no longer exists.");
    cleanup(&[&work, &bare]);
}

#[test]
fn rename_end_to_end_moves_tracking_and_invalidates_undo() {
    let (mut st, work, bare) = with_origin("br-rename-e2e");
    st.undo.record(dummy());
    drive(&mut st, rename("feature", "feature-2"));
    let snap = st.snap.as_ref().unwrap();
    assert!(!snap.locals.iter().any(|b| b.name == "feature"));
    let b = snap.locals.iter().find(|b| b.name == "feature-2").expect("renamed");
    assert_eq!(b.tracking, Some(Tracking { remote: "origin".into(), merge: "feature".into() }));
    assert!(st.undo.undo.is_empty());
    assert_eq!(st.undo.blocked.as_deref(), Some("Rename branch"));
    cleanup(&[&work, &bare]);
}

#[test]
fn upstream_dialog_and_commands() {
    let (mut st, work, bare) = with_origin("br-upstream");
    git(&work, &["branch", "x"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    update(&mut st, Msg::Branch(BranchMsg::SetUpstreamDialog("x".into())), Instant::now());
    assert!(
        matches!(&st.modal, Some(Modal::SetUpstream { branch, remote, name, .. }) if branch == "x" && remote == "origin" && name == "x")
    );
    local(&mut st, "feature").tracking = Some(Tracking { remote: "origin".into(), merge: "other".into() });
    update(&mut st, Msg::Branch(BranchMsg::SetUpstreamDialog("feature".into())), Instant::now());
    assert!(matches!(&st.modal, Some(Modal::SetUpstream { remote, name, .. }) if remote == "origin" && name == "other"));
    st.modal = None;

    let set =
        |name: &str, remote: &str| Msg::Branch(BranchMsg::SetUpstream { branch: "x".into(), remote: remote.into(), name: name.into() });
    assert_eq!(cmds(&mut st, set("feature", "origin")), vec![args(&["branch", "--set-upstream-to=origin/feature", "x"])]);
    assert_eq!(
        cmds(&mut st, set("nothere", "origin")),
        vec![args(&["config", "branch.x.remote", "origin"]), args(&["config", "branch.x.merge", "refs/heads/nothere"])]
    );
    assert!(cmds(&mut st, set("feature", "upstream")).is_empty());
    assert_eq!(last_log(&st), "There is no remote named upstream.");
    assert!(cmds(&mut st, set("a..b", "origin")).is_empty());
    assert_eq!(last_log(&st), "Not a valid branch name.");

    // Setting an upstream keeps the undo history.
    st.undo.record(dummy());
    update(&mut st, set("feature", "origin"), Instant::now());
    done(&mut st, true);
    assert_eq!(st.undo.undo.len(), 1);

    assert_eq!(
        cmds(&mut st, Msg::Branch(BranchMsg::UnsetUpstream("feature".into()))),
        vec![args(&["branch", "--unset-upstream", "feature"])]
    );
    update(&mut st, Msg::Branch(BranchMsg::UnsetUpstream("feature".into())), Instant::now());
    done(&mut st, true);
    assert_eq!(st.undo.undo.len(), 1, "unset keeps it too");
    cleanup(&[&work, &bare]);
}

#[test]
fn upstream_dialog_needs_a_remote() {
    let dir = scratch_repo("br-noremote");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    update(&mut st, Msg::Branch(BranchMsg::SetUpstreamDialog("main".into())), Instant::now());
    assert!(st.modal.is_none());
    assert_eq!(last_log(&st), "This repository has no remotes.");
    assert!(cmds(&mut st, Msg::Branch(BranchMsg::PushBranch { branch: "main".into(), remote: None })).is_empty());
    assert_eq!(last_log(&st), "This repository has no remotes.");
    cleanup(&[&dir]);
}

#[test]
fn fast_forward_commands_and_refusals() {
    let (mut st, work, bare) = with_origin("br-ff");
    local(&mut st, "feature").behind = 2;
    let ff = |n: &str| Msg::Branch(BranchMsg::FastForwardBranch(n.into()));
    assert_eq!(
        cmds(&mut st, ff("feature")),
        vec![args(&["fetch", "--no-recurse-submodules", ".", "refs/remotes/origin/feature:refs/heads/feature"])]
    );
    local(&mut st, "main").behind = 1;
    assert_eq!(cmds(&mut st, ff("main")), vec![args(&["merge", "--ff-only", "refs/remotes/origin/main"])]);

    // The fetch into the repository moves a branch, so it clears the undo history.
    st.undo.record(dummy());
    update(&mut st, ff("feature"), Instant::now());
    done(&mut st, true);
    assert!(st.undo.undo.is_empty());
    assert_eq!(st.undo.blocked.as_deref(), Some("Fast-forward"));

    // The job's refresh reloaded the snapshot; set the counts up again.
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    local(&mut st, "main").behind = 1;
    st.snap.as_mut().unwrap().state = RepositoryState::Merge;
    assert!(cmds(&mut st, ff("main")).is_empty());
    assert_eq!(last_log(&st), "Finish or abort the merge/rebase in progress first.");
    st.snap.as_mut().unwrap().state = RepositoryState::Clean;

    add_local(&mut st, "x");
    let refusals = [
        ((0, 0), "feature is already up to date with origin/feature."),
        ((2, 0), "feature is 2 commits ahead of origin/feature. There is nothing to fast-forward."),
        ((1, 2), "feature and origin/feature have diverged (1 ahead, 2 behind). Merge or rebase instead."),
    ];
    for ((ahead, behind), why) in refusals {
        let b = local(&mut st, "feature");
        (b.ahead, b.behind) = (ahead, behind);
        assert!(cmds(&mut st, ff("feature")).is_empty());
        assert_eq!(last_log(&st), why);
    }
    assert!(cmds(&mut st, ff("x")).is_empty());
    assert_eq!(last_log(&st), "x has no upstream. Use Set upstream… first.");
    cleanup(&[&work, &bare]);
}

#[test]
fn fast_forward_end_to_end_without_checkout() {
    let (mut st, work, bare) = with_origin("br-ff-e2e");
    remote_commit(&bare, "feature", "f1.txt");
    remote_commit(&bare, "feature", "f2.txt");
    drive(&mut st, Msg::git("Fetch", &["fetch", "origin"]));
    assert_eq!(local(&mut st, "feature").behind, 2);
    drive(&mut st, Msg::Branch(BranchMsg::FastForwardBranch("feature".into())));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.head_branch.as_deref(), Some("main"));
    let b = snap.locals.iter().find(|b| b.name == "feature").unwrap();
    assert_eq!((b.ahead, b.behind), (0, 0));
    let remote_tip = snap.remotes[0].1.iter().find(|(n, _)| n == "feature").map(|(_, o)| *o);
    assert_eq!(b.oid, remote_tip);
    assert_eq!(std::fs::read_to_string(work.join("a.txt")).unwrap(), "one\ntwo\n", "the working tree is untouched");
    cleanup(&[&work, &bare]);
}

#[test]
fn delete_with_remote_commands_and_refusals() {
    let (mut st, work, bare) = with_origin("br-delete");
    let fx = update(&mut st, Msg::Branch(BranchMsg::DeleteBranchAndRemote("feature".into())), Instant::now());
    st.job = None;
    match &fx[..] {
        [Effect::RunGit { cmds, record, .. }] => {
            assert_eq!(
                cmds,
                &vec![
                    args(&["merge-base", "--is-ancestor", "refs/heads/feature", "HEAD"]),
                    args(&["branch", "-d", "feature"]),
                    args(&["push", "origin", "--delete", "feature"])
                ]
            );
            assert!(matches!(record, Some(UndoKind::DeleteBranch(n)) if n == "feature"));
        }
        other => panic!("unexpected {other:?}"),
    }
    let b = local(&mut st, "feature");
    b.tracking = Some(Tracking { remote: "origin".into(), merge: "other".into() });
    b.upstream_ref = Some("refs/remotes/origin/other".into());
    assert_eq!(
        cmds(&mut st, Msg::Branch(BranchMsg::DeleteBranchAndRemote("feature".into())))[2],
        args(&["push", "origin", "--delete", "other"])
    );

    // Not merged into HEAD: refused, since both deletes would leave its commits on no branch.
    local(&mut st, "feature").merged = false;
    assert!(cmds(&mut st, Msg::Branch(BranchMsg::DeleteBranchAndRemote("feature".into()))).is_empty(), "unmerged");
    assert_eq!(
        last_log(&st),
        "feature has commits that aren't merged into main. Deleting it and origin/other would leave them on no branch."
    );
    local(&mut st, "feature").merged = true;

    assert!(cmds(&mut st, Msg::Branch(BranchMsg::DeleteBranchAndRemote("main".into()))).is_empty(), "checked out");
    let b = local(&mut st, "feature");
    b.tracking = Some(Tracking { remote: ".".into(), merge: "main".into() });
    b.upstream_ref = Some("refs/heads/main".into());
    assert!(cmds(&mut st, Msg::Branch(BranchMsg::DeleteBranchAndRemote("feature".into()))).is_empty(), "local upstream");
    assert_eq!(last_log(&st), "feature has no upstream branch on a remote to delete.");

    // Through the confirmation.
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let then = Msg::Branch(BranchMsg::DeleteBranchAndRemote("feature".into()));
    update(&mut st, Msg::confirm("Delete local and remote branch", "…".into(), "Delete both", then), Instant::now());
    let fx = update(&mut st, Msg::ConfirmAccepted, Instant::now());
    assert!(matches!(&fx[..], [Effect::RunGit { cmds, .. }] if cmds.len() == 3));
    st.job = None;
    cleanup(&[&work, &bare]);
}

#[test]
fn delete_with_remote_end_to_end_then_undo() {
    let (mut st, work, bare) = with_origin("br-delete-e2e");
    let tip = local(&mut st, "feature").oid;
    drive(&mut st, Msg::Branch(BranchMsg::DeleteBranchAndRemote("feature".into())));
    let snap = st.snap.as_ref().unwrap();
    assert!(!snap.locals.iter().any(|b| b.name == "feature"));
    assert!(!snap.remotes[0].1.iter().any(|(n, _)| n == "feature"), "gone from origin");
    assert_eq!(st.undo.undo.len(), 1);

    drive(&mut st, Msg::Job(JobMsg::Undo));
    let b = local(&mut st, "feature").clone();
    assert_eq!(b.oid, tip);
    assert_eq!((b.tracking, b.upstream), (None, None), "the remote branch is not restored");
    cleanup(&[&work, &bare]);
}

#[test]
fn delete_with_remote_keeps_a_pushed_unmerged_branch() {
    let (mut st, work, bare) = with_origin("br-delete-unmerged");
    git(&work, &["checkout", "-q", "feature"]);
    std::fs::write(
        work.join("b.txt"),
        "feature only
",
    )
    .unwrap();
    git(&work, &["add", "b.txt"]);
    git(&work, &["commit", "-q", "-m", "feature only"]);
    git(&work, &["push", "-q", "origin", "feature"]);
    git(&work, &["checkout", "-q", "main"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let b = local(&mut st, "feature").clone();
    assert_eq!((b.ahead, b.behind, b.merged), (0, 0, false), "pushed, not merged into main");

    assert!(cmds(&mut st, Msg::Branch(BranchMsg::DeleteBranchAndRemote("feature".into()))).is_empty());
    // Even when the snapshot is stale and says merged, the command sequence refuses.
    local(&mut st, "feature").merged = true;
    drive(&mut st, Msg::Branch(BranchMsg::DeleteBranchAndRemote("feature".into())));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.locals.iter().find(|l| l.name == "feature").and_then(|l| l.oid), b.oid, "the local branch stays");
    assert!(snap.remotes[0].1.iter().any(|(n, o)| n == "feature" && Some(*o) == b.oid), "origin/feature stays");
    let on_origin = git2::Repository::open(&bare).unwrap().refname_to_id("refs/heads/feature").ok();
    assert_eq!(on_origin, b.oid, "the branch stays on the remote");
    cleanup(&[&work, &bare]);
}

#[test]
fn push_targets_and_refusals() {
    let (mut st, work, bare) = with_origin("br-push");
    let push = |b: &str, r: Option<&str>| Msg::Branch(BranchMsg::PushBranch { branch: b.into(), remote: r.map(str::to_owned) });
    local(&mut st, "feature").ahead = 1;
    assert_eq!(cmds(&mut st, push("feature", None)), vec![args(&["push", "origin", "refs/heads/feature:refs/heads/feature"])]);

    add_local(&mut st, "x");
    assert_eq!(cmds(&mut st, push("x", None)), vec![args(&["push", "-u", "origin", "refs/heads/x:refs/heads/x"])]);
    let x = local(&mut st, "x");
    x.tracking = Some(Tracking { remote: ".".into(), merge: "main".into() });
    x.upstream_ref = Some("refs/heads/main".into());
    assert_eq!(cmds(&mut st, push("x", None)), vec![args(&["push", "origin", "refs/heads/x:refs/heads/x"])], "no -u");

    st.snap.as_mut().unwrap().remotes.push(("backup".into(), Vec::new()));
    let b = local(&mut st, "feature");
    b.tracking = Some(Tracking { remote: "origin".into(), merge: "other".into() });
    assert_eq!(cmds(&mut st, push("feature", Some("backup"))), vec![args(&["push", "backup", "refs/heads/feature:refs/heads/feature"])]);
    assert_eq!(cmds(&mut st, push("feature", Some("origin"))), vec![args(&["push", "origin", "refs/heads/feature:refs/heads/other"])]);

    local(&mut st, "feature").behind = 2;
    assert!(cmds(&mut st, push("feature", None)).is_empty());
    assert_eq!(last_log(&st), "feature is behind origin/feature. Fast-forward or pull first.");
    assert!(!cmds(&mut st, push("feature", Some("backup"))).is_empty(), "another remote's state is unknown");
    let b = local(&mut st, "feature");
    (b.ahead, b.behind) = (0, 0);
    assert!(cmds(&mut st, push("feature", Some("origin"))).is_empty());
    assert_eq!(last_log(&st), "feature is up to date with origin/feature.");
    cleanup(&[&work, &bare]);
}

#[test]
fn which_jobs_keep_the_undo_history() {
    use super::undo::keeps_history;
    let keeps = |a: &[&str]| keeps_history(&[args(a)]);
    assert!(keeps(&["config", "branch.x.remote", "origin"]));
    assert!(keeps(&["branch", "--set-upstream-to=origin/x", "x"]));
    assert!(keeps(&["branch", "--unset-upstream", "x"]));
    assert!(keeps(&["fetch", "--all"]));
    assert!(keeps(&["push", "-u", "origin", "refs/heads/x:refs/heads/x"]));
    assert!(!keeps(&["fetch", "--no-recurse-submodules", ".", "a:b"]));
    assert!(!keeps(&["branch", "-m", "a", "b"]));
}
