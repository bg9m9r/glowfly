//! Headless tests for checking out a remote branch through a local branch tracking it: the
//! fast-forward in the same job, the diverged branch left where it is, a chosen one of several
//! trackers, the refusals (a tracker another worktree has opens that worktree), the palette's
//! route, and a tracking branch under another name from the New branch dialog. Commands are
//! checked as emitted, and end to end against a repository with a bare `origin`.

use super::branch_tests::{remote_commit, with_origin};
use super::tests::{clean_tree, drive, finish, git};
use super::update;
use git2::Oid;
use glowfly_core::branch_ops::create_tracking_cmd;
use glowfly_core::gitcli::args;
use glowfly_core::repo::Tracking;
use glowfly_core::undo::UndoKind;
use glowfly_model::effect::Effect;
use glowfly_model::msg::{BranchMsg, JobMsg, Msg, PaletteMsg, RepoMsg};
use glowfly_model::state::palette::PaletteCommand;
use glowfly_model::state::{AppState, Modal};
use std::path::Path;
use std::time::Instant;

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// The commands `msg` starts and how they are recorded for undo, or nothing. Clears the job so
/// the next message can start one.
fn run_git(st: &mut AppState, msg: Msg) -> (Vec<Vec<String>>, Option<UndoKind>) {
    let fx = update(st, msg, Instant::now());
    st.job = None;
    match &fx[..] {
        [Effect::Job(glowfly_model::effect::JobEffect::RunGit { cmds, record, .. })] => (cmds.clone(), record.clone()),
        [] => (Vec::new(), None),
        other => panic!("unexpected effects {other:?}"),
    }
}

fn last_log(st: &AppState) -> String {
    st.log.last().map(|l| l.text.clone()).unwrap_or_default()
}

fn checkout(remote: &str, name: &str) -> Msg {
    Msg::Branch(BranchMsg::CheckoutRemote { remote: remote.into(), name: name.into() })
}

fn tracking(local: &str) -> Msg {
    Msg::Branch(BranchMsg::CheckoutTracking { remote: "origin".into(), name: "feature".into(), local: local.into() })
}

fn tip(dir: &Path, refname: &str) -> Oid {
    git2::Repository::open(dir).unwrap().refname_to_id(refname).unwrap()
}

/// Adds a commit on top of local `branch` without checking it out.
fn local_commit(dir: &Path, branch: &str) {
    let oid = glowfly_core::testkit::git_out(dir, &["commit-tree", &format!("{branch}^{{tree}}"), "-p", branch, "-m", "local"]);
    git(dir, &["update-ref", &format!("refs/heads/{branch}"), oid.trim()]);
}

/// `with_origin` with `origin/feature` one commit ahead of the local `feature` tracking it.
fn behind(name: &str) -> (AppState, std::path::PathBuf, std::path::PathBuf) {
    let (mut st, work, bare) = with_origin(name);
    clean_tree(&work);
    remote_commit(&bare, "feature", "f.txt");
    git(&work, &["fetch", "-q", "origin"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    (st, work, bare)
}

#[test]
fn checkout_fast_forwards_a_tracking_branch_that_is_behind() {
    let (mut st, work, bare) = behind("rco-ff");
    let (from, to) = (tip(&work, "refs/heads/feature"), tip(&work, "refs/remotes/origin/feature"));
    let (cmds, record) = run_git(&mut st, checkout("origin", "feature"));
    assert_eq!(
        cmds,
        vec![
            args(&["merge-base", "--is-ancestor", &from.to_string(), &to.to_string()]),
            args(&["update-ref", "-m", "fast-forward to origin/feature", "refs/heads/feature", &to.to_string(), &from.to_string()]),
            args(&["checkout", "feature", "--"]),
        ]
    );
    assert_eq!(record, Some(UndoKind::Checkout));
    drive(&mut st, checkout("origin", "feature"));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.head_branch.as_deref(), Some("feature"));
    assert_eq!(snap.head_oid, Some(to), "checked out at the remote branch's tip");
    cleanup(&[&work, &bare]);
}

#[test]
fn checkout_leaves_a_diverged_tracking_branch_where_it_is() {
    let (mut st, work, bare) = behind("rco-diverged");
    local_commit(&work, "feature");
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let before = tip(&work, "refs/heads/feature");
    let (cmds, _) = run_git(&mut st, checkout("origin", "feature"));
    assert_eq!(cmds, vec![args(&["checkout", "feature", "--"])], "no fast-forward");
    assert_eq!(
        last_log(&st),
        "feature and origin/feature have diverged (1 ahead, 1 behind), so feature is checked out without moving it. \
         Merge or rebase to bring them together."
    );
    drive(&mut st, checkout("origin", "feature"));
    assert_eq!(st.snap.as_ref().unwrap().head_branch.as_deref(), Some("feature"));
    assert_eq!(tip(&work, "refs/heads/feature"), before, "not moved");
    cleanup(&[&work, &bare]);
}

#[test]
fn checkout_of_one_of_several_trackers() {
    let (mut st, work, bare) = behind("rco-several");
    git(&work, &["branch", "-q", "--track", "x", "origin/feature"]);
    git(&work, &["update-ref", "refs/heads/x", "refs/heads/feature"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let snap = st.snap.as_ref().unwrap();
    let names: Vec<&str> =
        glowfly_model::state::remote_branch::trackers(snap, "origin", "feature").iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, ["feature", "x"], "the same name first");
    let (cmds, _) = run_git(&mut st, tracking("x"));
    assert_eq!(cmds.last(), Some(&args(&["checkout", "x", "--"])));
    assert!(cmds[1].contains(&"refs/heads/x".to_owned()), "x is fast-forwarded, not feature: {cmds:?}");
    drive(&mut st, tracking("x"));
    assert_eq!(st.snap.as_ref().unwrap().head_branch.as_deref(), Some("x"));
    assert_eq!(tip(&work, "refs/heads/x"), tip(&work, "refs/remotes/origin/feature"));
    assert_ne!(tip(&work, "refs/heads/feature"), tip(&work, "refs/heads/x"), "feature stays behind");
    cleanup(&[&work, &bare]);
}

#[test]
fn checkout_tracking_refusals() {
    let (mut st, work, bare) = behind("rco-refused");
    let wt = work.with_file_name(format!("{}-wt", work.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&wt);
    git(&work, &["worktree", "add", "-q", &wt.display().to_string(), "feature"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let before = tip(&work, "refs/heads/feature");
    // Another worktree has feature: that worktree opens, and feature isn't moved.
    drive(&mut st, tracking("feature"));
    assert_eq!((st.tabs.len(), st.active_tab), (2, 1), "{:?}", st.tabs[0].log.last().map(|l| &l.text));
    assert!(super::tests::same_place(&st.snap.as_ref().unwrap().workdir, &wt));
    let note = st.tabs[0].log.last().map(|l| l.text.clone()).unwrap_or_default();
    assert!(note.starts_with("Opened the worktree at ") && note.ends_with(", where feature is checked out."), "{note}");
    assert_eq!(tip(&work, "refs/heads/feature"), before, "not moved");
    drive(&mut st, Msg::Tabs(glowfly_model::msg::TabsMsg::SelectTab(0)));
    assert!(run_git(&mut st, tracking("main")).0.is_empty());
    assert_eq!(last_log(&st), "main no longer tracks origin/feature.");
    git(&work, &["worktree", "remove", "--force", &wt.display().to_string()]);
    cleanup(&[&work, &bare, &wt]);
}

/// The checked-out tracker is fast-forwarded in place, as Fast-forward does.
#[test]
fn checkout_of_the_checked_out_tracker_fast_forwards_it() {
    let (mut st, work, bare) = behind("rco-head");
    let snap = st.snap.as_mut().unwrap();
    let main = snap.locals.iter_mut().find(|b| b.name == "main").unwrap();
    assert!(main.is_head);
    main.tracking = Some(Tracking { remote: "origin".into(), merge: "feature".into() });
    main.upstream_ref = Some("refs/remotes/origin/feature".into());
    main.behind = 1;
    let (cmds, _) = run_git(&mut st, tracking("main"));
    assert_eq!(cmds, vec![args(&["merge", "--ff-only", "refs/remotes/origin/feature"])]);
    cleanup(&[&work, &bare]);
}

#[test]
fn palette_remote_checkout_goes_through_the_branch_message() {
    let (mut st, work, bare) = with_origin("rco-palette");
    clean_tree(&work);
    git(&work, &["push", "-q", "origin", "main:other"]);
    git(&work, &["fetch", "-q", "origin"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let run = |remote: &str, branch: &str| {
        Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::CheckoutRemote { remote: remote.into(), branch: branch.into() }))
    };
    assert_eq!(
        run_git(&mut st, run("origin", "other")),
        (vec![args(&["checkout", "--track", "origin/other", "--"])], Some(UndoKind::Checkout))
    );
    // A palette entry gone stale is refused as the menu's is.
    assert!(run_git(&mut st, run("origin", "gone")).0.is_empty());
    assert_eq!(last_log(&st), "origin/gone no longer exists.");
    cleanup(&[&work, &bare]);
}

#[test]
fn new_branch_from_a_remote_branch_tracks_it_under_another_name() {
    let (mut st, work, bare) = with_origin("rco-track-as");
    clean_tree(&work);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let fx = update(&mut st, Msg::Branch(BranchMsg::NewTrackingBranch { remote: "origin".into(), name: "feature".into() }), Instant::now());
    assert!(fx.is_empty());
    let tip = tip(&work, "refs/remotes/origin/feature").to_string();
    assert!(
        matches!(&st.modal, Some(Modal::NewBranch { start: Some(s), track: Some((r, n)), checkout: true, .. }) if *s == tip && r == "origin" && n == "feature")
    );
    st.modal = None;
    // What the dialog's Create sends (ui/branch_dialogs.rs).
    let create =
        Msg::Job(JobMsg::Git { label: "Create branch".into(), cmds: vec![create_tracking_cmd("mine", "origin", "feature", true)] });
    drive(&mut st, create);
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.head_branch.as_deref(), Some("mine"));
    let mine = snap.head_info().unwrap();
    assert_eq!(mine.tracking, Some(Tracking { remote: "origin".into(), merge: "feature".into() }));
    let trackers = glowfly_model::state::remote_branch::trackers(snap, "origin", "feature");
    assert_eq!(trackers.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), ["mine", "feature"], "checked-out first");

    assert!(
        update(&mut st, Msg::Branch(BranchMsg::NewTrackingBranch { remote: "origin".into(), name: "gone".into() }), Instant::now())
            .is_empty()
    );
    assert_eq!(last_log(&st), "origin/gone no longer exists.");
    cleanup(&[&work, &bare]);
}

fn short(o: Oid) -> String {
    o.to_string()[..7].to_owned()
}

/// A commit on top of `parent` with the tree of `branch`, not on any branch.
fn child_of(dir: &Path, branch: &str, parent: Oid, msg: &str) -> Oid {
    let out = glowfly_core::testkit::git_out(dir, &["commit-tree", &format!("{branch}^{{tree}}"), "-p", &parent.to_string(), "-m", msg]);
    Oid::from_str(out.trim()).unwrap()
}

#[test]
fn checkout_failing_after_the_fast_forward_moves_the_branch_back() {
    let (mut st, work, bare) = behind("rco-back");
    let (from, to) = (tip(&work, "refs/heads/feature"), tip(&work, "refs/remotes/origin/feature"));
    // The remote's new commit adds f.txt, which is in the way as an untracked file.
    std::fs::write(work.join("f.txt"), "mine\n").unwrap();
    drive(&mut st, checkout("origin", "feature"));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.head_branch.as_deref(), Some("main"));
    assert_eq!(tip(&work, "refs/heads/feature"), from, "moved back");
    let feature = snap.locals.iter().find(|b| b.name == "feature").unwrap();
    assert_eq!(feature.tracking, Some(Tracking { remote: "origin".into(), merge: "feature".into() }));
    assert!(st.toasts.iter().any(|t| !t.ok), "the checkout's failure is toasted");
    let said = format!(
        "The checkout failed after feature was fast-forwarded to origin/feature ({}), so feature was moved back to {}.",
        short(to),
        short(from)
    );
    assert!(st.log.iter().any(|l| l.ok && l.text == said), "{:?}", st.log.iter().map(|l| &l.text).collect::<Vec<_>>());
    assert!(st.toasts.iter().all(|t| !t.text.contains("moved back")));

    std::fs::remove_file(work.join("f.txt")).unwrap();
    drive(&mut st, checkout("origin", "feature"));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!((snap.head_branch.as_deref(), snap.head_oid), (Some("feature"), Some(to)));
    cleanup(&[&work, &bare]);
}

#[test]
fn checkout_failing_after_the_fast_forward_keeps_a_branch_that_moved_meanwhile() {
    let (mut st, work, bare) = behind("rco-moved");
    let (from, to) = (tip(&work, "refs/heads/feature"), tip(&work, "refs/remotes/origin/feature"));
    std::fs::write(work.join("f.txt"), "mine\n").unwrap();
    // Run the checkout job and keep what its result leads to, without running it yet.
    let mut queue: Vec<Effect> = update(&mut st, checkout("origin", "feature"), Instant::now());
    let mut collected = Vec::new();
    while let Some(e) = queue.pop() {
        if is_move_back(&e) {
            collected.push(e);
            continue;
        }
        if super::tests::skipped(&e) {
            continue;
        }
        for m in glowfly_runner::testing::execute_all(e) {
            queue.extend(update(&mut st, m, Instant::now()));
        }
    }
    let back = collected.iter().find_map(|e| match e {
        Effect::Job(glowfly_model::effect::JobEffect::RunGit { cmds, expect, .. }) => Some((cmds.clone(), expect.clone())),
        _ => None,
    });
    assert_eq!(
        back,
        Some((vec![args(&["branch", "-f", "--", "feature", &from.to_string()])], Some(("refs/heads/feature".to_owned(), to))))
    );
    // The branch moves again before the move-back runs.
    let third = child_of(&work, "feature", to, "elsewhere");
    git(&work, &["update-ref", "refs/heads/feature", &third.to_string()]);
    finish(&mut st, collected);
    assert_eq!(tip(&work, "refs/heads/feature"), third, "left alone");
    let line = st.log.iter().find(|l| l.text.contains("could not be moved back")).expect("failure line");
    assert!(!line.ok && line.text.contains(&short(from)) && line.text.contains(&short(to)), "{}", line.text);
    let toast = st.toasts.iter().find(|t| t.text.contains("couldn't be moved back")).expect("toast");
    assert_eq!(toast.text, format!("The checkout failed after fast-forwarding feature, and it couldn't be moved back to {}.", short(from)));
    cleanup(&[&work, &bare]);
}

fn is_move_back(e: &Effect) -> bool {
    matches!(e, Effect::Job(glowfly_model::effect::JobEffect::RunGit { cmds, .. }) if cmds.first().is_some_and(|c| c.starts_with(&["branch".to_owned(), "-f".to_owned()])))
}

#[test]
fn checkout_failing_before_the_fast_forward_claims_nothing() {
    let (mut st, work, bare) = behind("rco-stale");
    let from = tip(&work, "refs/heads/feature");
    // The branch moves after the snapshot, so the update-ref's lease refuses.
    let moved = child_of(&work, "feature", from, "meanwhile");
    git(&work, &["update-ref", "refs/heads/feature", &moved.to_string()]);
    drive(&mut st, checkout("origin", "feature"));
    assert_eq!(tip(&work, "refs/heads/feature"), moved);
    assert_eq!(st.snap.as_ref().unwrap().head_branch.as_deref(), Some("main"));
    let said: Vec<&String> = st.log.iter().map(|l| &l.text).collect();
    assert!(said.iter().all(|l| !l.contains("moved back") && !l.contains("fast-forwarded from")), "{said:?}");
    assert!(st.ff_checkout.is_none());
    cleanup(&[&work, &bare]);
}
