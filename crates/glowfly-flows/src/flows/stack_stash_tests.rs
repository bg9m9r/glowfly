//! Restacking a three-layer stack (main <- low <- mid <- top) with uncommitted changes, end to
//! end: one stash for the whole restack, held (and saved for a restart) while a stop waits, and
//! put back, staged as it was, on the branch checked out before after the last step, after
//! Continue and after Abort; an abort midway goes back to that branch and records the finished
//! part as one undo entry.

use super::stack_tests::{above_low, commit, head_name, last_toast, opened, stacked, tip, undo_label};
use super::tests::{drive, git};
use crate::msg::{JobMsg, Msg, RebaseMsg, RepoMsg};
use crate::state::AppState;
use crate::state::auto_stash::back;
use git2::RepositoryState;
use glowfly_core::i18n::Lang;
use glowfly_core::rebase::Resume;
use glowfly_core::stash_ops::{self, AutoStashFor};
use std::path::{Path, PathBuf};

const STASH: &str = "Auto-stash before restacking onto low";

/// A staged change to a.txt and an unstaged one to l.txt, which the stack's later commits
/// don't touch; then a refresh, and Always stash so the restack doesn't ask.
fn dirty(st: &mut AppState, dir: &Path) {
    std::fs::write(dir.join("a.txt"), "staged\n").unwrap();
    git(dir, &["add", "a.txt"]);
    std::fs::write(dir.join("l.txt"), "unstaged\n").unwrap();
    st.settings.auto_stash = true;
    drive(st, Msg::Repo(RepoMsg::Refresh));
}

#[track_caller]
fn changes_back(dir: &Path) {
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "staged\n");
    assert_eq!(std::fs::read_to_string(dir.join("l.txt")).unwrap(), "unstaged\n");
    let (ok, staged) = glowfly_core::gitcli::run(dir, &glowfly_core::gitcli::args(&["diff", "--cached", "--name-only"]));
    assert!(ok);
    assert_eq!(staged.trim(), "a.txt", "what was staged is staged again");
    assert!(stash_ops::list(dir).unwrap().is_empty(), "the stash is gone");
}

/// `stacked` with mid's second commit (m2) and top's second (t2) both changing g.txt's last
/// line, top built on m1, and low amended: two steps, the second of which stops.
fn second_step_conflicts(name: &str) -> PathBuf {
    let dir = stacked(name, false);
    git(&dir, &["checkout", "-q", "top"]);
    commit(&dir, "g.txt", "one\n2\ntop\n", "t2");
    git(&dir, &["checkout", "-q", "mid"]);
    commit(&dir, "g.txt", "one\n2\nmid\n", "m2");
    git(&dir, &["checkout", "-q", "low"]);
    git(&dir, &["commit", "-q", "--amend", "-m", "l1, amended"]);
    dir
}

/// Restacks the branches above low in `second_step_conflicts`'s repository with dirty changes,
/// which stops in the second step; with mid's tip from before.
fn stopped(name: &str) -> (AppState, PathBuf, git2::Oid) {
    let dir = second_step_conflicts(name);
    let old_mid = tip(&dir, "mid");
    let mut st = opened(&dir);
    dirty(&mut st, &dir);
    drive(&mut st, above_low());
    let state = st.snap.as_ref().unwrap().state;
    assert!(matches!(state, RepositoryState::RebaseMerge | RepositoryState::RebaseInteractive), "{state:?}");
    assert_eq!(st.restacks.len(), 1);
    (st, dir, old_mid)
}

#[test]
fn the_changes_come_back_staged_on_the_original_branch() {
    let dir = stacked("stack-stash-run", true);
    let mut st = opened(&dir);
    dirty(&mut st, &dir);
    drive(&mut st, above_low());
    assert!(st.toasts.iter().any(|t| t.ok && t.text == "Restack succeeded"));
    assert_eq!(last_toast(&st), (true, back(Lang::En, STASH)));
    assert_eq!(head_name(&dir), "low");
    assert_eq!(tip(&dir, "mid^"), tip(&dir, "low"));
    changes_back(&dir);
    assert!(st.auto_stash.pending.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stop_holds_the_stash_and_continue_puts_it_back() {
    let (mut st, dir, _) = stopped("stack-stash-continue");
    let oid = stash_ops::list(&dir).unwrap()[0].oid;
    let p = &st.auto_stash.pending[0];
    assert_eq!((p.oid, &p.job.op), (Some(oid), &AutoStashFor::Restack { onto: "low".into() }));
    assert!(p.saved, "a restart still offers it");
    assert_eq!(stash_ops::read_saved(&dir).len(), 1);

    std::fs::write(dir.join("g.txt"), "one\n2\nboth\n").unwrap();
    git(&dir, &["add", "g.txt"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert_eq!(st.auto_stash.pending.len(), 1, "still held while the restack stops");
    drive(&mut st, Msg::Rebase(RebaseMsg::Resume(Resume::Continue)));
    assert_eq!(st.snap.as_ref().unwrap().state, RepositoryState::Clean);
    assert!(st.restacks.is_empty());
    assert_eq!(head_name(&dir), "low");
    assert_eq!(tip(&dir, "top~2"), tip(&dir, "mid"));
    changes_back(&dir);
    assert!(st.auto_stash.pending.is_empty());
    assert!(stash_ops::read_saved(&dir).is_empty(), "the saved record is gone");
    assert_eq!(st.session.undo.undo.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_abort_goes_back_puts_the_stash_back_and_undoes_the_finished_part() {
    let (mut st, dir, old_mid) = stopped("stack-stash-abort");
    let (old_top, new_mid) = (tip(&dir, "top"), tip(&dir, "mid"));
    assert_ne!(new_mid, old_mid, "the first step finished");
    drive(&mut st, Msg::git("Abort rebase", &["rebase", "--abort"]));
    assert_eq!(st.snap.as_ref().unwrap().state, RepositoryState::Clean);
    assert!(st.restacks.is_empty() && st.job.is_none());
    assert_eq!(head_name(&dir), "low", "the branch checked out before is checked out again");
    changes_back(&dir);
    assert!(st.auto_stash.pending.is_empty());
    assert!(stash_ops::read_saved(&dir).is_empty());
    assert_eq!(tip(&dir, "top"), old_top, "the aborted step moved nothing");

    assert_eq!(st.session.undo.undo.len(), 1, "the finished part is one entry");
    assert!(undo_label(&st).contains("restack of the branches above low"), "{}", undo_label(&st));
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(tip(&dir, "mid"), old_mid, "undo moves mid back");
    let _ = std::fs::remove_dir_all(&dir);
}
