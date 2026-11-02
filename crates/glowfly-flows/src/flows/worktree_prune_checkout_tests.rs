//! Headless tests for Prune and check out (`BranchMsg::PruneThen`) of a branch whose worktree
//! folder is missing: on a dirty tree it asks through the auto-stash guard like any checkout,
//! and stashing runs the stash, the prune and the checkout as one job recorded for undo; a pull
//! request's source branch is pruned and checked out as an undoable checkout too.

use super::auto_stash_guard_tests::pull_request;
use super::auto_stash_tests::{ask, git_cmds, up};
use super::push_pr_tests::with_host;
use super::tests::{clean_tree, drive, git, scratch_repo, state};
use crate::effect::Effect;
use crate::msg::{AutoStashMsg, BranchMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::state::branches::checkout_local;
use glowfly_core::stash_ops::AutoStashFor;
use glowfly_core::undo::UndoKind;
use glowfly_core::worktree_ops::prune_cmd;
use std::path::{Path, PathBuf};

/// Adds a worktree for `branch` in `<work>-wt`, deletes its folder and refreshes `st`. Returns
/// the folder.
fn missing_worktree(st: &mut AppState, work: &Path, branch: &str, new: bool) -> PathBuf {
    let wt = work.with_file_name(format!("{}-wt", work.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&wt);
    let wt_arg = wt.display().to_string();
    let add: Vec<&str> =
        if new { vec!["worktree", "add", "-q", "-b", branch, &wt_arg] } else { vec!["worktree", "add", "-q", &wt_arg, branch] };
    git(work, &add);
    std::fs::remove_dir_all(&wt).unwrap();
    drive(st, Msg::Repo(RepoMsg::Refresh));
    assert!(crate::state::worktree_branches::elsewhere(st.snap.as_ref().unwrap(), branch).is_some_and(|w| w.missing));
    wt
}

/// The undo kind of the one RunGit in `fx`.
fn record(fx: &[Effect]) -> Option<UndoKind> {
    fx.iter().find_map(|e| match e {
        Effect::RunGit { record, .. } => Some(record.clone()),
        _ => None,
    })?
}

fn prune_then(m: BranchMsg) -> Msg {
    Msg::Branch(BranchMsg::PruneThen(Box::new(m)))
}

#[test]
fn prune_and_check_out_on_a_dirty_tree_asks_first() {
    let dir = scratch_repo("wpc-dirty");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    missing_worktree(&mut st, &dir, "side", true);
    std::fs::write(dir.join("a.txt"), "edited\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));

    let again = prune_then(BranchMsg::Checkout("side".into()));
    assert!(up(&mut st, again).iter().all(|e| !matches!(e, Effect::RunGit { .. })), "asked, nothing ran");
    let asked = ask(&st);
    assert_eq!(asked.op, AutoStashFor::Checkout { target: "side".into() });
    assert!(matches!(&*asked.retry, Msg::Branch(BranchMsg::PruneThen(m)) if matches!(&**m, BranchMsg::Checkout(b) if b == "side")));

    let fx = up(&mut st, Msg::AutoStash(AutoStashMsg::Stash));
    let cmds = git_cmds(&fx);
    assert_eq!(cmds.len(), 3, "{cmds:?}");
    assert_eq!(cmds[0][..2], ["stash".to_owned(), "push".to_owned()]);
    assert_eq!(cmds[1..], [prune_cmd(), checkout_local("side")]);
    assert_eq!(record(&fx), Some(UndoKind::Checkout));
    st.job = None;
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_pull_requests_prune_and_check_out_is_recorded_for_undo() {
    let (mut st, work, bare) = with_host("wpc-pr");
    let wt = missing_worktree(&mut st, &work, "feature", false);
    clean_tree(&work);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    st.session.forge.as_mut().unwrap().prs.push(pull_request(12, "feature"));

    let fx = up(&mut st, prune_then(BranchMsg::CheckoutPullRequest { id: 12 }));
    assert_eq!(git_cmds(&fx), vec![prune_cmd(), checkout_local("feature")]);
    assert_eq!(record(&fx), Some(UndoKind::Checkout));
    st.job = None;
    for d in [&work, &bare, &wt] {
        let _ = std::fs::remove_dir_all(d);
    }
}
