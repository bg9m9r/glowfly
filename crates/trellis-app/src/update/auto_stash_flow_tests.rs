//! Stash and carry on, end to end on fixture repositories: the stash is made in the
//! operation's job and put back once the operation is over, whether it finished at once, stopped
//! and was continued, aborted or committed, or putting it back conflicted; and the branch
//! checkouts a double-click on a graph chip sends.

use super::auto_stash_tests::{ask, git_cmds, up};
use super::cherry_pick_tests::feature_repo;
use super::tests::{accept_stash, drive, finish, git};
use crate::effect::Effect;
use crate::msg::{AutoStashMsg, BranchMsg, CommitMsg, JobMsg, Msg, RebaseMsg, RepoMsg, SelectionMsg};
use crate::state::{AppState, Modal, remote_branch};
use git2::{Oid, RepositoryState};
use std::path::Path;
use trellis_core::gitcli::args;
use trellis_core::rebase::Resume;
use trellis_core::stash_ops::AutoStashFor;
use trellis_core::undo::UndoKind;

/// a.txt edited, so the tree has one changed file Trellis can stash.
fn dirty(st: &mut AppState, dir: &Path) {
    std::fs::write(dir.join("a.txt"), "one\nmine\n").unwrap();
    drive(st, Msg::Repo(RepoMsg::Refresh));
}

fn mine_is_back(st: &AppState, dir: &Path) {
    let text = std::fs::read_to_string(dir.join("a.txt")).unwrap().replace("\r\n", "\n");
    assert_eq!(text, "one\nmine\n", "the changes are back");
    assert!(stash_count(dir) == 0, "the stash is gone");
    let toast = st.toasts.iter().rev().find(|t| t.text.starts_with("Your changes from")).expect("the back toast");
    assert!(toast.ok && toast.text.ends_with("are back."), "{}", toast.text);
    assert!(st.auto_stash.pending.is_empty() && st.auto_stash.making.is_none());
}

fn stash_count(dir: &Path) -> usize {
    trellis_core::stash_ops::list(dir).unwrap().len()
}

fn state_of(st: &AppState) -> RepositoryState {
    st.snap.as_ref().unwrap().state
}

#[test]
fn stash_and_checkout_puts_changes_back() {
    let (mut st, dir, _) = feature_repo("asf-checkout", &[("feature work", "feature\n")], None);
    dirty(&mut st, &dir);
    assert!(up(&mut st, Msg::git("Checkout", &["checkout", "feature", "--"])).is_empty());
    assert_eq!(ask(&st).files, 1);
    let fx = up(&mut st, Msg::AutoStash(AutoStashMsg::Stash));
    assert_eq!(
        git_cmds(&fx),
        vec![args(&["stash", "push", "-m", "Auto-stash before checking out feature"]), args(&["checkout", "feature", "--"])]
    );
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Checkout"), "the job keeps its label");
    let record = Some(trellis_core::undo::UndoKind::Checkout);
    assert!(fx.iter().any(|e| matches!(e, Effect::RunGit { record: r, .. } if *r == record)), "undo records the checkout");
    finish(&mut st, fx);
    assert_eq!(st.snap.as_ref().unwrap().head_branch.as_deref(), Some("feature"));
    mine_is_back(&st, &dir);
    assert_eq!(st.undo.undo.len(), 2, "the checkout and putting the changes back are both undoable");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn always_stash_saves_and_skips_prompt() {
    let (mut st, dir, _) = feature_repo("asf-always", &[("feature work", "feature\n")], None);
    dirty(&mut st, &dir);
    up(&mut st, Msg::git("Checkout", &["checkout", "feature", "--"]));
    let Some(Modal::AutoStash(a)) = st.modal.as_mut() else { panic!("the prompt") };
    a.always = true;
    let fx = up(&mut st, Msg::AutoStash(AutoStashMsg::Stash));
    assert!(st.settings.auto_stash, "Always stash is on");
    assert!(fx.iter().any(|e| matches!(e, Effect::SaveSettings(s) if s.auto_stash)), "and saved");
    finish(&mut st, fx);
    mine_is_back(&st, &dir);

    // From now on the checkout goes straight to stashing.
    dirty(&mut st, &dir);
    let fx = up(&mut st, Msg::git("Checkout", &["checkout", "main", "--"]));
    assert!(st.modal.is_none());
    assert_eq!(git_cmds(&fx)[0], args(&["stash", "push", "-m", "Auto-stash before checking out main"]));
    finish(&mut st, fx);
    assert_eq!(st.snap.as_ref().unwrap().head_branch.as_deref(), Some("main"));
    mine_is_back(&st, &dir);
    let _ = std::fs::remove_dir_all(&dir);
}

fn head_branch(st: &AppState) -> Option<&str> {
    st.snap.as_ref().unwrap().head_branch.as_deref()
}

fn checkout_op(target: &str) -> AutoStashFor {
    AutoStashFor::Checkout { target: target.into() }
}

fn checkout_remote(remote: &str, name: &str) -> Msg {
    Msg::Branch(BranchMsg::CheckoutRemote { remote: remote.into(), name: name.into() })
}

#[test]
fn double_click_checkout_asks_then_stashes() {
    let (mut st, dir, _) = feature_repo(
        "asf-dbl-local",
        &[(
            "feature work",
            "feature
",
        )],
        None,
    );
    dirty(&mut st, &dir);
    let feature = st.snap.as_ref().unwrap().locals.iter().find(|b| b.name == "feature").and_then(|b| b.oid).unwrap();
    // The double-click's first click selects the row: no modal, so the guard isn't blocked.
    drive(&mut st, Msg::Repo(RepoMsg::SelectCommit(feature)));
    assert!(st.modal.is_none());
    assert!(up(&mut st, Msg::Branch(BranchMsg::Checkout("feature".into()))).is_empty());
    assert_eq!(ask(&st).op, checkout_op("feature"));
    assert_eq!(ask(&st).files, 1);
    assert!(matches!(&*ask(&st).retry, Msg::Branch(BranchMsg::Checkout(n)) if n == "feature"), "retried as sent");
    let fx = up(&mut st, Msg::AutoStash(AutoStashMsg::Stash));
    assert_eq!(
        git_cmds(&fx),
        vec![args(&["stash", "push", "-m", "Auto-stash before checking out feature"]), args(&["checkout", "feature", "--"])]
    );
    let record = Some(UndoKind::Checkout);
    assert!(fx.iter().any(|e| matches!(e, Effect::RunGit { record: r, .. } if *r == record)), "undo records the checkout");
    finish(&mut st, fx);
    assert_eq!(head_branch(&st), Some("feature"));
    mine_is_back(&st, &dir);

    // With Always stash on, the double-click never stops at the prompt.
    st.settings.auto_stash = true;
    dirty(&mut st, &dir);
    let fx = up(&mut st, Msg::Branch(BranchMsg::Checkout("main".into())));
    assert!(st.modal.is_none());
    assert_eq!(
        git_cmds(&fx),
        vec![args(&["stash", "push", "-m", "Auto-stash before checking out main"]), args(&["checkout", "main", "--"])]
    );
    finish(&mut st, fx);
    assert_eq!(head_branch(&st), Some("main"));
    mine_is_back(&st, &dir);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn double_click_remote_with_tracking_branch_asks_for_it() {
    let (mut st, work, bare) = super::branch_tests::with_origin("asf-dbl-tracked");
    dirty(&mut st, &work);
    assert!(up(&mut st, checkout_remote("origin", "feature")).is_empty());
    assert_eq!(ask(&st).op, checkout_op("feature"));
    assert!(matches!(&*ask(&st).retry, Msg::Branch(BranchMsg::Checkout(n)) if n == "feature"), "the tracking branch's checkout");
    accept_stash(&mut st);
    assert_eq!(head_branch(&st), Some("feature"));
    mine_is_back(&st, &work);
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

#[test]
fn double_click_remote_without_local_creates_it_after_stashing() {
    let (mut st, work, bare) = super::branch_tests::with_origin("asf-dbl-new");
    git(&work, &["push", "-q", "origin", "main:other"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    dirty(&mut st, &work);
    assert!(up(&mut st, checkout_remote("origin", "other")).is_empty());
    assert_eq!(ask(&st).op, checkout_op("origin/other"));
    let retry =
        matches!(&*ask(&st).retry, Msg::Branch(BranchMsg::CheckoutRemote { remote, name }) if remote == "origin" && name == "other");
    assert!(retry, "retried as sent");
    let stash = ask(&st).op.stash_command();
    let fx = up(&mut st, Msg::AutoStash(AutoStashMsg::Stash));
    assert_eq!(git_cmds(&fx), vec![stash, args(&["checkout", "--track", "origin/other", "--"])]);
    finish(&mut st, fx);
    assert_eq!(head_branch(&st), Some("other"));
    let snap = st.snap.as_ref().unwrap();
    let other = snap.locals.iter().find(|b| b.name == "other").expect("the new branch");
    assert!(remote_branch::tracks(other, "origin", "other"), "it tracks origin/other");
    mine_is_back(&st, &work);
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&bare);
}

/// A merge of `feature` that stops on f.txt, with the changes in the stash.
fn stopped_merge(name: &str) -> (AppState, std::path::PathBuf) {
    let (mut st, dir, _) = feature_repo(name, &[("feature edit", "feature\n")], Some("main\n"));
    dirty(&mut st, &dir);
    up(&mut st, Msg::git("Merge", &["merge", "feature"]));
    accept_stash(&mut st);
    assert_eq!(state_of(&st), RepositoryState::Merge);
    assert_eq!(stash_count(&dir), 1, "the changes wait in the stash");
    let made = st.toasts.iter().find(|t| t.text.contains("Auto-stash before merging feature into main")).expect("the made toast");
    assert!(made.action.is_some(), "with Show stash");
    let note = crate::state::auto_stash::banner_note(&st).expect("the banner says so");
    assert!(note.contains("Auto-stash before merging feature into main"), "{note}");
    (st, dir)
}

#[test]
fn merge_that_stops_puts_back_after_abort() {
    let (mut st, dir) = stopped_merge("asf-merge-abort");
    drive(&mut st, Msg::Job(JobMsg::Abort { label: "Abort merge".into(), args: args(&["merge", "--abort"]), keep: false }));
    assert_eq!(state_of(&st), RepositoryState::Clean);
    mine_is_back(&st, &dir);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn merge_that_stops_puts_back_after_commit() {
    let (mut st, dir) = stopped_merge("asf-merge-commit");
    std::fs::write(dir.join("f.txt"), "merged\n").unwrap();
    git(&dir, &["add", "f.txt"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(stash_count(&dir), 1, "still a merge in progress");
    drive(&mut st, Msg::Commit(CommitMsg::CommitMerge));
    assert_eq!(state_of(&st), RepositoryState::Clean);
    mine_is_back(&st, &dir);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn plain_rebase_stops_then_continue_puts_back() {
    let (mut st, dir, _) = feature_repo("asf-rebase", &[("feature edit", "feature\n")], Some("main\n"));
    git(&dir, &["checkout", "-q", "feature"]);
    dirty(&mut st, &dir);
    up(&mut st, Msg::git("Rebase", &["rebase", "main"]));
    assert!(ask(&st).op.can_stop());
    accept_stash(&mut st);
    assert_ne!(state_of(&st), RepositoryState::Clean, "stopped on f.txt");
    assert_eq!(stash_count(&dir), 1);
    std::fs::write(dir.join("f.txt"), "both\n").unwrap();
    git(&dir, &["add", "f.txt"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Rebase(RebaseMsg::Resume(Resume::Continue)));
    assert_eq!(state_of(&st), RepositoryState::Clean);
    mine_is_back(&st, &dir);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cherry_pick_and_revert_stash_and_put_back() {
    let (mut st, dir, picks) = feature_repo("asf-pick", &[("feature work", "feature\n")], None);
    dirty(&mut st, &dir);
    assert!(up(&mut st, Msg::Selection(SelectionMsg::CherryPick(vec![picks[0]]))).is_empty());
    let body = crate::state::auto_stash::body(trellis_core::i18n::Lang::En, &ask(&st).op, 1);
    let short = &picks[0].to_string()[..7];
    assert!(body.contains(&format!("cherry-picks {short} onto main")), "{body}");
    accept_stash(&mut st);
    let head: Oid = st.snap.as_ref().unwrap().head_oid.unwrap();
    assert_ne!(head, picks[0]);
    assert_eq!(std::fs::read_to_string(dir.join("f.txt")).unwrap().replace("\r\n", "\n"), "feature\n");
    mine_is_back(&st, &dir);

    dirty(&mut st, &dir);
    assert!(up(&mut st, Msg::Selection(SelectionMsg::Revert(vec![head]))).is_empty());
    accept_stash(&mut st);
    assert_eq!(std::fs::read_to_string(dir.join("f.txt")).unwrap().replace("\r\n", "\n"), "base\n");
    mine_is_back(&st, &dir);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn conflicting_put_back_says_so_and_keeps_stash() {
    // feature changes a.txt too, so the stashed edit collides with it.
    let (mut st, dir, _) = feature_repo("asf-conflict", &[("feature work", "feature\n")], None);
    git(&dir, &["checkout", "-q", "feature"]);
    std::fs::write(dir.join("a.txt"), "one\nfeature\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "a on feature"]);
    git(&dir, &["checkout", "-q", "main"]);
    dirty(&mut st, &dir);
    up(&mut st, Msg::git("Checkout", &["checkout", "feature", "--"]));
    accept_stash(&mut st);
    assert_eq!(st.snap.as_ref().unwrap().head_branch.as_deref(), Some("feature"), "the checkout ran");
    assert_eq!(stash_count(&dir), 1, "the stash is kept");
    assert!(std::fs::read_to_string(dir.join("a.txt")).unwrap().contains("<<<<<<<"));
    let toast = st.toasts.last().expect("a toast");
    assert!(!toast.ok);
    assert!(toast.text.starts_with("Putting back \"Auto-stash before checking out feature\" conflicted in 1 file."), "{}", toast.text);
    assert!(toast.action.is_some(), "with Show stash");
    assert!(st.auto_stash.pending.is_empty(), "nothing more to put back");
    let snap = st.snap.as_ref().unwrap();
    let oid = trellis_core::stash_ops::list(&dir).unwrap()[0].oid;
    assert!(snap.stashes.iter().any(|x| x.oid == oid), "the list shows the kept stash");
    assert!(snap.unstaged.iter().chain(&snap.staged).any(|f| f.path == "a.txt" && f.status == '!'), "a.txt is conflicted");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_merge_commit_asks_for_the_mainline_then_the_guard() {
    use crate::state::pick::PickOp;
    let (mut st, dir, _) = feature_repo("asf-mainline", &[("feature work", "feature\n")], None);
    git(&dir, &["merge", "-q", "--no-ff", "-m", "merge feature", "feature"]);
    dirty(&mut st, &dir);
    let merge = st.snap.as_ref().unwrap().head_oid.unwrap();
    assert!(up(&mut st, Msg::Selection(SelectionMsg::Revert(vec![merge]))).is_empty());
    assert!(matches!(st.modal, Some(Modal::Mainline(_))), "the mainline first");
    st.modal = None;
    let pick = Msg::Selection(SelectionMsg::PickWithMainline { op: PickOp::Revert, oids: vec![merge], mainline: 1 });
    assert!(up(&mut st, pick).is_empty());
    assert!(matches!(*ask(&st).retry, Msg::Selection(SelectionMsg::PickWithMainline { mainline: 1, .. })), "then the guard");
    accept_stash(&mut st);
    assert_ne!(st.snap.as_ref().unwrap().head_oid, Some(merge), "the revert ran");
    assert_eq!(std::fs::read_to_string(dir.join("f.txt")).unwrap().replace("\r\n", "\n"), "base\n");
    mine_is_back(&st, &dir);
    let _ = std::fs::remove_dir_all(&dir);
}
