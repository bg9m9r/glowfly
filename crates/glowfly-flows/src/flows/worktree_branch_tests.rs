//! Headless tests for local branches checked out in another worktree: a checkout opens that
//! worktree instead (switching to its tab when one is open, noted in the activity log), also
//! when it is rebasing or bisecting the branch there with HEAD detached; Delete refused with the
//! words the menus show; a free branch checked out as an undoable job; the current worktree's own
//! branch never counted; the palette's Switch branch listing such a branch; and one whose
//! worktree folder is missing refused once (however often it is clicked) with Prune and check
//! out, which prunes and checks it out in one job. A branch held by a rebase or bisect there is
//! still refused for delete, rename, fast-forward and force push.

use super::tests::{clean_tree, drive, finish, git, same_place, scratch_repo, state};
use super::update;
use glowfly_core::i18n::Lang;
use glowfly_core::native_path::shown;
use glowfly_model::effect::Effect;
use glowfly_model::msg::{BranchMsg, Msg, RepoMsg, TabsMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::branches::push::force_blocked;
use glowfly_model::state::palette::{Pick, entries};
use glowfly_model::state::worktree_branches::{Elsewhere, checkout_elsewhere, delete_blocked, elsewhere, held_blocked, local_tip_line};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// A repository with branch `free` and a linked worktree on branch `side` in
/// `<repo>-worktrees/side`, opened in `st`. Returns (state, repository, worktree folder).
fn with_side(name: &str) -> (AppState, PathBuf, PathBuf) {
    let dir = scratch_repo(name);
    let root = dir.with_file_name(format!("{}-worktrees", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&root);
    let side = root.join("side");
    git(&dir, &["branch", "free"]);
    git(&dir, &["worktree", "add", "-q", "-b", "side", &side.display().to_string()]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir, side)
}

fn cleanup(dir: &Path, side: &Path) {
    let _ = std::fs::remove_dir_all(side.parent().unwrap());
    let _ = std::fs::remove_dir_all(dir);
}

fn last_log(st: &AppState) -> (bool, String) {
    st.log.last().map(|l| (l.ok, l.text.clone())).expect("a log entry")
}

fn branch(m: BranchMsg) -> Msg {
    Msg::Branch(m)
}

/// The listed folder of the worktree that has `side`, as the words show it.
fn side_path(st: &AppState) -> String {
    shown(&elsewhere(st.snap.as_ref().unwrap(), "side").expect("side is elsewhere").path)
}

/// Checks out `side` from the first tab: the worktree opens in the second, and the first tab's
/// log ends with `note`. Then back to the first tab.
fn opens_side(st: &mut AppState, side: &Path, note: &str) {
    drive(st, branch(BranchMsg::Checkout("side".into())));
    assert_eq!((st.tabs.len(), st.active_tab), (2, 1), "opened in a second tab");
    assert!(same_place(&st.snap.as_ref().unwrap().workdir, side));
    let first = &st.tabs[0].log;
    assert_eq!(first.last().map(|l| (l.ok, l.text.as_str())), Some((true, note)));
    drive(st, Msg::Tabs(TabsMsg::SelectTab(0)));
}

#[test]
fn checkout_opens_the_worktree_that_has_the_branch() {
    let (mut st, dir, side) = with_side("wtb-opens");
    let snap = st.snap.as_ref().unwrap();
    let path = side_path(&st);
    assert_eq!(checkout_elsewhere(Lang::En, snap, "side"), Some(Elsewhere::Open { tip: format!("Opens the worktree at {path}") }));
    assert_eq!(checkout_elsewhere(Lang::En, snap, "free"), None);
    let delete = delete_blocked(Lang::En, snap, "side").unwrap();
    assert!(delete.ends_with("Remove that worktree or check out another branch there first."), "{delete}");
    assert!(local_tip_line(Lang::En, elsewhere(snap, "side").unwrap(), "side").starts_with("Checked out in the worktree at "));

    let note = format!("Opened the worktree at {path}, where side is checked out.");
    opens_side(&mut st, &side, &note);
    // Again: its tab is switched to, not opened twice (`opens_side` checks there are two).
    opens_side(&mut st, &side, &note);
    assert!(st.toasts.iter().all(|t| t.ok), "no error toast: {:?}", st.toasts.iter().map(|t| &t.text).collect::<Vec<_>>());

    let fx = update(&mut st, branch(BranchMsg::DeleteBranchAndRemote("side".into())), Instant::now());
    assert!(fx.is_empty() && st.job.is_none());
    assert_eq!(last_log(&st), (false, delete));

    let before = st.log.len();
    let fx = update(&mut st, branch(BranchMsg::Checkout("main".into())), Instant::now());
    assert!(fx.is_empty());
    assert_eq!(st.log.len(), before, "nothing said for the branch already checked out");
    cleanup(&dir, &side);
}

#[test]
fn a_free_branch_is_checked_out_as_an_undoable_job() {
    let (mut st, dir, side) = with_side("wtb-free");
    super::tests::clean_tree(&dir);
    super::tests::drive(&mut st, Msg::Repo(glowfly_model::msg::RepoMsg::Refresh));
    let fx = update(&mut st, branch(BranchMsg::Checkout("free".into())), Instant::now());
    match &fx[..] {
        [Effect::RunGit { cmds, record, .. }] => {
            assert_eq!(cmds, &[glowfly_model::state::branches::checkout_local("free")]);
            assert!(record.is_some(), "recorded for undo");
        }
        other => panic!("expected one RunGit, got {other:?}"),
    }
    super::tests::finish(&mut st, fx);
    assert_eq!(st.snap.as_ref().unwrap().head_branch.as_deref(), Some("free"));
    assert_eq!(st.session.undo.undo.len(), 1);
    cleanup(&dir, &side);
}

#[test]
fn only_other_worktrees_count_and_switch_branch_lists_the_branch() {
    let (st, dir, side) = with_side("wtb-palette");
    let snap = st.snap.as_ref().unwrap();
    assert!(elsewhere(snap, "main").is_none(), "this worktree's own branch");
    assert!(elsewhere(snap, "free").is_none());

    let labels: Vec<String> = entries(&st).into_iter().map(|e| e.label).collect();
    assert!(labels.iter().any(|l| l == "Checkout free"), "{labels:?}");
    assert!(!labels.iter().any(|l| l == "Checkout side"), "the main list offers Open worktree: {labels:?}");
    assert!(labels.iter().any(|l| l == "Open worktree side (side)"), "{labels:?}");

    let pick = Pick::checkouts(snap);
    assert_eq!((pick.label(0), pick.label(1)), ("free", "side"), "after the plain local checkouts");
    assert_eq!(pick.detail(1).as_deref(), Some("opens its worktree"));
    cleanup(&dir, &side);
}

#[test]
fn a_missing_worktree_is_refused_once_then_pruned_and_checked_out() {
    let (mut st, dir, side) = with_side("wtb-missing");
    let path = side_path(&st);
    std::fs::remove_dir_all(&side).unwrap();
    clean_tree(&dir);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let snap = st.snap.as_ref().unwrap();
    assert!(elsewhere(snap, "side").is_some_and(|w| w.missing), "a missing worktree still counts");
    let tip = format!("The worktree at {path} is gone. Prunes it, then checks out side.");
    assert_eq!(checkout_elsewhere(Lang::En, snap, "side"), Some(Elsewhere::Prune { tip }));
    assert!(delete_blocked(Lang::En, snap, "side").unwrap().ends_with(", whose folder is missing. Prune it first."));
    assert!(local_tip_line(Lang::En, elsewhere(snap, "side").unwrap(), "side").ends_with(" (folder missing)"));
    assert!(!entries(&st).into_iter().any(|e| e.label.starts_with("Open worktree side")));
    assert!(Pick::checkouts(snap).command(1).is_none(), "only free: nothing to open");

    // Refused, offering Prune and check out: twice adds no second line or toast.
    for _ in 0..2 {
        let fx = update(&mut st, branch(BranchMsg::Checkout("side".into())), Instant::now());
        assert!(fx.is_empty() && st.job.is_none(), "{fx:?}");
    }
    // The refusal's line, then the toast's own line (every toast is logged), each once.
    let line = st.log.iter().rev().find(|l| !l.ok).map(|l| l.text.clone());
    assert_eq!(line, Some(format!("side is still checked out in a missing worktree ({path}).")));
    assert_eq!(st.log.iter().filter(|l| l.text.contains("missing worktree")).count(), 1);
    assert_eq!(st.log.iter().filter(|l| l.text.contains("whose folder is gone")).count(), 1);
    let toasts: Vec<_> = st.toasts.iter().filter(|t| t.text.contains("whose folder is gone")).collect();
    assert_eq!(toasts.len(), 1, "one toast, restarted");
    assert!(toasts[0].ok, "informational, not an error");
    assert_eq!(toasts[0].text, format!("side is still checked out in a worktree whose folder is gone ({path})."));
    let action = toasts[0].action.as_ref().expect("Prune and check out");
    assert_eq!(action.label, "Prune and check out");
    let Msg::Branch(BranchMsg::PruneThen(m)) = &*action.msg else { panic!("PruneThen") };
    assert!(matches!(&**m, BranchMsg::Checkout(b) if b == "side"));

    let fx = update(&mut st, branch(BranchMsg::PruneThen(Box::new(BranchMsg::Checkout("side".into())))), Instant::now());
    match &fx[..] {
        [Effect::RunGit { cmds, record, .. }] => {
            assert_eq!(cmds, &[glowfly_core::worktree_ops::prune_cmd(), glowfly_model::state::branches::checkout_local("side")]);
            assert!(record.is_some(), "recorded for undo as the checkout");
        }
        other => panic!("expected one RunGit, got {other:?}"),
    }
    finish(&mut st, fx);
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.head_branch.as_deref(), Some("side"));
    assert!(elsewhere(snap, "side").is_none(), "pruned");
    cleanup(&dir, &side);
}

/// `with_side`, then `side` stopped mid-rebase onto main in its worktree (both edit a.txt).
fn rebasing_side(name: &str) -> (AppState, PathBuf, PathBuf) {
    let (mut st, dir, side) = with_side(name);
    std::fs::write(side.join("a.txt"), "one\nside\n").unwrap();
    git(&side, &["commit", "-q", "-am", "side edit"]);
    git(&dir, &["commit", "-q", "-am", "main edit"]);
    let (ok, out) = glowfly_core::gitcli::run(&side, &glowfly_core::gitcli::args(&["rebase", "main"]));
    assert!(!ok, "the rebase stops for the conflict: {out}");
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    (st, dir, side)
}

/// `what` is refused, logging `why`, with nothing started.
fn refused(st: &mut AppState, what: &str, m: BranchMsg, why: &str) {
    let fx = update(st, branch(m), Instant::now());
    assert!(fx.iter().all(|e| !matches!(e, Effect::RunGit { .. })) && st.job.is_none(), "{what}: {fx:?}");
    assert_eq!(last_log(st), (false, why.to_owned()), "{what}");
}

#[test]
fn a_branch_mid_rebase_in_another_worktree_is_held_there() {
    let (mut st, dir, side) = rebasing_side("wtb-rebasing");
    let snap = st.snap.as_ref().unwrap();
    let wt = elsewhere(snap, "side").expect("the LOCAL mark: held by the rebasing worktree");
    assert_eq!(wt.branch, None, "its HEAD is detached");
    assert!(local_tip_line(Lang::En, wt, "side").starts_with("Mid-rebase in the worktree at "));
    let path = side_path(&st);
    let tip = format!("Opens the worktree at {path}, where side is mid-rebase");
    assert_eq!(checkout_elsewhere(Lang::En, snap, "side"), Some(Elsewhere::Open { tip }));
    let why = delete_blocked(Lang::En, snap, "side").unwrap();
    assert!(
        why.starts_with("side is mid-rebase in the worktree at ") && why.ends_with(". Continue or abort the rebase there first."),
        "{why}"
    );
    let b = snap.locals.iter().find(|b| b.name == "side").unwrap().clone();
    assert_eq!(force_blocked(Lang::En, snap, &b).as_deref(), Some(why.as_str()), "force push");
    assert!(held_blocked(Lang::En, snap, "free").is_none());

    // Opened where the rebase is continued or aborted.
    opens_side(&mut st, &side, &format!("Opened the worktree at {path}, where side is mid-rebase."));
    refused(&mut st, "delete", BranchMsg::DeleteBranchAndRemote("side".into()), &why);
    refused(&mut st, "rename", BranchMsg::RenameBranch { old: "side".into(), new: "other".into(), remote: false }, &why);
    refused(&mut st, "fast-forward", BranchMsg::FastForwardBranch("side".into()), &why);
    git(&side, &["rebase", "--abort"]);
    cleanup(&dir, &side);
}

#[test]
fn a_branch_mid_bisect_in_another_worktree_is_held_but_one_merely_checked_out_renames() {
    let (mut st, dir, side) = with_side("wtb-bisecting");
    let snap = st.snap.as_ref().unwrap();
    assert!(held_blocked(Lang::En, snap, "side").is_none(), "git renames a branch only checked out elsewhere");
    for n in 0..3 {
        std::fs::write(side.join("b.txt"), format!("{n}\n")).unwrap();
        git(&side, &["add", "b.txt"]);
        git(&side, &["commit", "-q", "-m", &format!("b{n}")]);
    }
    git(&side, &["bisect", "start", "side", "side~3"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let snap = st.snap.as_ref().unwrap();
    let wt = elsewhere(snap, "side").expect("held by the bisecting worktree");
    assert_eq!(wt.branch, None, "bisect checked out a commit");
    assert!(local_tip_line(Lang::En, wt, "side").starts_with("Mid-bisect in the worktree at "));
    let why = held_blocked(Lang::En, snap, "side").expect("rename refused");
    assert!(why.starts_with("side is mid-bisect in the worktree at ") && why.ends_with(". End the bisect there first."), "{why}");
    assert_eq!(delete_blocked(Lang::En, snap, "side").as_deref(), Some(why.as_str()));
    let path = side_path(&st);
    let tip = format!("Opens the worktree at {path}, where side is mid-bisect");
    assert_eq!(checkout_elsewhere(Lang::En, snap, "side"), Some(Elsewhere::Open { tip }));
    opens_side(&mut st, &side, &format!("Opened the worktree at {path}, where side is mid-bisect."));
    refused(&mut st, "rename", BranchMsg::RenameBranch { old: "side".into(), new: "other".into(), remote: false }, &why);
    git(&side, &["bisect", "reset"]);
    cleanup(&dir, &side);
}
