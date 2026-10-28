//! Headless tests for the undo history across a plain abort: a merge, cherry-pick, revert or
//! rebase that Glowfly started and that stopped for conflicts, aborted back to exactly where it
//! started (HEAD, its branch and the index), keeps the entries older than it, which still undo;
//! what its own steps recorded goes. An abort that ends anywhere else clears the history.

use super::tests::{drive, git};
use super::undo_continue_tests::merge_repo;
use crate::msg::{JobMsg, Msg, RepoMsg, SelectionMsg, StageMsg};
use crate::state::AppState;
use git2::RepositoryState;
use glowfly_core::gitcli::args;
use glowfly_core::undo::Action;

fn head_summary(st: &AppState) -> String {
    let snap = st.snap.as_ref().expect("a repository is open");
    snap.commits[snap.row_of[&snap.head_oid.expect("HEAD")]].summary.to_string()
}

fn repo_state(st: &AppState) -> RepositoryState {
    st.snap.as_ref().unwrap().state
}

/// The banner's plain abort (`label`, `git <op> --abort`), as its confirmation sends it.
fn abort(st: &mut AppState, label: &str, op: &str) {
    drive(st, Msg::Job(JobMsg::Abort { label: label.into(), args: args(&[op, "--abort"]), keep: false }));
}

/// From `merge_repo` (main at "add b" with one Commit entry; "ours" and "theirs" both change
/// a.txt's second line), runs `start` so it stops for conflicts, stages a.txt (an entry of the
/// operation's own) and aborts with `git <op> --abort`: the commit entry alone is left, and
/// still undoes.
fn earlier_entries_survive(name: &str, start: impl FnOnce(&AppState) -> Msg, stopped: RepositoryState, label: &str, op: &str) {
    let (mut st, dir) = merge_repo(name);
    let before = st.session.undo.undo.clone();
    let start = start(&st);
    drive(&mut st, start);
    assert_eq!(repo_state(&st), stopped);
    std::fs::write(dir.join("a.txt"), "one\nmerged\n").unwrap();
    drive(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())));
    assert!(st.session.undo.undo.len() > before.len(), "staging it is recorded: {:?}", st.session.undo.undo);

    abort(&mut st, label, op);
    assert_eq!(repo_state(&st), RepositoryState::Clean);
    assert_eq!(head_summary(&st), "add b");
    assert_eq!(st.session.undo.undo, before, "the earlier entry stays; the operation's own goes");
    assert!(st.session.undo.blocked.is_none() && st.session.undo.stopped.is_none());

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(head_summary(&st), "ours", "the commit is undone");
    assert!(matches!(st.session.undo.redo.last(), Some(Action::Commit { .. })), "{:?}", st.session.undo);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn merge_abort_keeps_the_earlier_entries() {
    let start = |_: &AppState| Msg::git("Merge", &["merge", "other"]);
    earlier_entries_survive("undo-abort-merge", start, RepositoryState::Merge, "Abort merge", "merge");
}

#[test]
fn cherry_pick_abort_keeps_the_earlier_entries() {
    // From the graph, as its own job (`Effect::Pick`).
    let start = |st: &AppState| {
        let snap = st.snap.as_ref().unwrap();
        let theirs = snap.commits.iter().find(|c| c.summary == "theirs").unwrap().oid;
        Msg::Selection(SelectionMsg::CherryPick(vec![theirs]))
    };
    earlier_entries_survive("undo-abort-pick", start, RepositoryState::CherryPick, "Abort cherry-pick", "cherry-pick");
}

#[test]
fn revert_abort_keeps_the_earlier_entries() {
    let start = |_: &AppState| Msg::git("Revert", &["revert", "--no-edit", "other"]);
    earlier_entries_survive("undo-abort-revert", start, RepositoryState::Revert, "Abort revert", "revert");
}

#[test]
fn rebase_abort_keeps_the_earlier_entries() {
    let start = |_: &AppState| Msg::git("Rebase", &["rebase", "other"]);
    earlier_entries_survive("undo-abort-rebase", start, RepositoryState::RebaseInteractive, "Abort rebase", "rebase");
}

#[test]
fn an_abort_that_ends_elsewhere_clears_the_history() {
    let (mut st, dir) = merge_repo("undo-abort-elsewhere");
    drive(&mut st, Msg::git("Merge", &["merge", "other"]));
    assert_eq!(repo_state(&st), RepositoryState::Merge);
    // The branch moves under the merge, outside Glowfly; the abort then ends on "ours".
    git(&dir, &["update-ref", "refs/heads/main", "HEAD~1"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    abort(&mut st, "Abort merge", "merge");
    assert_eq!(repo_state(&st), RepositoryState::Clean);
    assert_eq!(head_summary(&st), "ours");
    assert!(st.session.undo.undo.is_empty(), "{:?}", st.session.undo.undo);
    assert_eq!(st.session.undo.blocked.as_deref(), Some("Abort merge"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_abort_of_an_operation_started_elsewhere_clears_the_history() {
    let (mut st, dir) = merge_repo("undo-abort-outside");
    let (ok, _) = glowfly_core::gitcli::run(&dir, &args(&["merge", "other"]));
    assert!(!ok, "the merge stops");
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    abort(&mut st, "Abort merge", "merge");
    assert_eq!(head_summary(&st), "add b");
    assert!(st.session.undo.undo.is_empty(), "nothing says where it started: {:?}", st.session.undo.undo);
    assert_eq!(st.session.undo.blocked.as_deref(), Some("Abort merge"));
    let _ = std::fs::remove_dir_all(&dir);
}
