//! Headless tests for undo and redo: what gets recorded, when the history is kept or cleared,
//! and undoing each kind of operation end to end against real repositories.

use super::tests::{clean_tree, drive, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, LoadKind};
use crate::msg::CommitMsg;
use crate::msg::JobMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::StageMsg;
use crate::msg::TabsMsg;
use crate::runner;
use crate::state::undo::{MAX_UNDO, UndoHistory};
use crate::state::{AppState, RunningJob};
use git2::Oid;
use glowfly_core::repo::{self, LoadOptions};
use glowfly_core::undo::{Action, DISCARD_STASH_MESSAGE, ResetMode, UndoKind};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn head_summary(st: &AppState) -> String {
    let snap = st.snap.as_ref().expect("a repository is open");
    let head = snap.head_oid.expect("HEAD");
    snap.commits[snap.row_of[&head]].summary.to_string()
}

fn summary_oid(st: &AppState, summary: &str) -> Oid {
    st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary == summary).expect("commit").oid
}

fn last_toast(st: &AppState) -> (bool, String) {
    st.toasts.last().map(|t| (t.ok, t.text.clone())).expect("a toast")
}

/// A scratch repository opened in `st` with a second commit, "Second line", made through the app.
fn committed(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())));
    st.session.draft.summary = "Second line".into();
    drive(&mut st, Msg::Commit(CommitMsg::Commit));
    (st, dir)
}

fn done(st: &mut AppState, ok: bool) {
    update(st, Msg::Job(JobMsg::JobDone { ok, output: String::new(), undo: None }), Instant::now());
}

fn dummy(i: usize) -> Action {
    Action::DeleteBranch { name: format!("b{i}"), oid: Oid::zero(), upstream: None }
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn commit_then_undo_and_redo_end_to_end() {
    let (mut st, dir) = committed("undo-commit");
    assert_eq!(st.undo.undo.len(), 1);
    assert!(
        st.undo.undo_entry(st.settings.lang(), true).0.starts_with("Undo commit 'Second line'"),
        "{}",
        st.undo.undo_entry(st.settings.lang(), true).0
    );

    st.session.draft = Default::default();
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(head_summary(&st), "first");
    assert!(st.snap.as_ref().unwrap().staged.iter().any(|f| f.path == "a.txt"), "the changes stay staged");
    assert_eq!(st.session.draft.summary, "Second line", "the message comes back into the empty draft");
    assert_eq!(st.undo.redo.len(), 1);
    assert_eq!(last_toast(&st), (true, "Undo commit 'Second line' succeeded".to_owned()));

    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert_eq!(head_summary(&st), "Second line");
    assert_eq!((st.undo.undo.len(), st.undo.redo.len()), (1, 0));
    assert!(st.session.draft.is_empty(), "the restored message leaves the draft once the commit is redone");

    // A draft edited after the undo is the user's: redo leaves it alone.
    drive(&mut st, Msg::Job(JobMsg::Undo));
    st.session.draft.summary = "Second line, reworded".into();
    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert_eq!(st.session.draft.summary, "Second line, reworded");
    cleanup(&[&dir]);
}

#[test]
fn git_jobs_record_their_kind() {
    let dir = scratch_repo("undo-kinds");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())));
    st.session.draft.summary = "Commit me".into();
    let fx = update(&mut st, Msg::Commit(CommitMsg::Commit), Instant::now());
    assert!(matches!(&fx[..], [Effect::RunGit { record: Some(UndoKind::Commit { amend: false }), .. }]), "{fx:?}");
    st.job = None;
    // The tree stays dirty: Always stash lets the guarded jobs through, still recorded as themselves.
    st.settings.auto_stash = true;
    let fx = update(&mut st, Msg::git("Merge", &["merge", "x"]), Instant::now());
    let merge = UndoKind::Move { op: glowfly_core::undo::MoveOp::Merge, detail: Some("x".into()) };
    assert!(matches!(&fx[..], [Effect::RunGit { record: Some(k), .. }] if *k == merge), "{fx:?}");
    st.job = None;
    let fx = update(&mut st, Msg::git("Rename branch", &["branch", "-m", "x", "y"]), Instant::now());
    assert!(matches!(&fx[..], [Effect::RunGit { record: None, .. }]), "{fx:?}");
    st.job = None;
    let fx = update(&mut st, Msg::git("Checkout", &["checkout", "x"]), Instant::now());
    assert!(matches!(&fx[..], [Effect::RunGit { record: Some(UndoKind::Checkout), .. }]), "{fx:?}");
    cleanup(&[&dir]);
}

#[test]
fn unrecorded_job_blocks_and_fetch_keeps_history() {
    let (mut st, dir) = committed("undo-blocked");
    update(&mut st, Msg::git("Fetch", &["fetch"]), Instant::now());
    done(&mut st, true);
    assert_eq!(st.undo.undo.len(), 1, "a fetch doesn't touch the branch or working tree");
    update(&mut st, Msg::git("Pull", &["pull"]), Instant::now());
    done(&mut st, false);
    assert_eq!(st.undo.undo.len(), 1, "a failed job changes nothing");

    update(&mut st, Msg::git("Rename branch", &["branch", "-m", "main", "trunk"]), Instant::now());
    done(&mut st, true);
    assert!(st.undo.undo.is_empty() && st.undo.redo.is_empty());
    assert_eq!(st.undo.blocked.as_deref(), Some("Rename branch"));
    let tip = st.undo.undo_entry(st.settings.lang(), true).1.unwrap_or_default();
    assert!(tip.starts_with("Can't undo Rename branch"), "{tip}");

    let fx = update(&mut st, Msg::Job(JobMsg::Undo), Instant::now());
    assert!(fx.is_empty());
    assert_eq!(last_toast(&st), (false, tip));
    cleanup(&[&dir]);
}

#[test]
fn hard_reset_undo_refused_while_dirty_then_allowed() {
    let (mut st, dir) = committed("undo-hard");
    let first = summary_oid(&st, "first").to_string();
    drive(&mut st, Msg::git("Reset (hard)", &["reset", "--hard", &first]));
    assert_eq!(head_summary(&st), "first");
    assert_eq!(st.undo.undo.len(), 2, "the commit, then the reset");
    assert!(matches!(st.undo.undo.last(), Some(Action::Reset { mode: ResetMode::Hard, .. })));

    std::fs::write(dir.join("a.txt"), "changed\n").unwrap();
    drive(&mut st, Msg::Job(JobMsg::Undo));
    let (ok, text) = last_toast(&st);
    assert!(!ok && text.contains("Commit or stash"), "{text}");
    assert_eq!(st.undo.undo.len(), 2, "the entry stays so the undo can be retried");
    assert!(st.job.is_none());

    git(&dir, &["checkout", "--", "."]);
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(head_summary(&st), "Second line");
    assert_eq!(st.undo.redo.len(), 1);
    cleanup(&[&dir]);
}

#[test]
fn stale_history_is_cleared() {
    let dir = scratch_repo("undo-stale");
    clean_tree(&dir);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    git(&dir, &["branch", "feature"]);
    drive(&mut st, Msg::git("Checkout", &["checkout", "feature"]));
    assert_eq!(st.undo.undo.len(), 1);

    git(&dir, &["checkout", "-q", "main"]); // outside the app
    drive(&mut st, Msg::Job(JobMsg::Undo));
    let (ok, text) = last_toast(&st);
    assert!(!ok && text.contains("undo history cleared"), "{text}");
    assert!(st.undo.undo.is_empty() && st.undo.redo.is_empty());
    cleanup(&[&dir]);
}

#[test]
fn discard_all_is_undoable_end_to_end() {
    let dir = scratch_repo("undo-discard");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::git("Discard all changes", &["stash", "push", "-u", "-m", DISCARD_STASH_MESSAGE]));
    let snap = st.snap.as_ref().unwrap();
    assert!(!snap.is_dirty());
    assert_eq!((snap.stashes.len(), st.undo.undo.len()), (1, 1));

    drive(&mut st, Msg::Job(JobMsg::Undo));
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.unstaged.iter().any(|f| f.path == "a.txt" && f.status == 'M'));
    assert!(snap.stashes.is_empty(), "the stash is used up");

    drive(&mut st, Msg::Job(JobMsg::Redo));
    let snap = st.snap.as_ref().unwrap();
    assert!(!snap.is_dirty());
    assert_eq!(snap.stashes.len(), 1);
    assert!(matches!(st.undo.undo.last(), Some(Action::DiscardAll { stash, .. }) if *stash == snap.stashes[0].oid));
    cleanup(&[&dir]);
}

#[test]
fn delete_branch_undo_restores_branch() {
    let dir = scratch_repo("undo-branch");
    let mut st = state();
    git(&dir, &["branch", "feature"]);
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let tip = st.snap.as_ref().unwrap().head_oid;
    drive(&mut st, Msg::git("Delete branch", &["branch", "-d", "feature"]));
    assert!(!st.snap.as_ref().unwrap().locals.iter().any(|b| b.name == "feature"));
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert!(st.snap.as_ref().unwrap().locals.iter().any(|b| b.name == "feature" && b.oid == tip));
    cleanup(&[&dir]);
}

#[test]
fn undo_result_lands_in_its_own_tab() {
    let (a, b) = (scratch_repo("undo-tab-a"), scratch_repo("undo-tab-b"));
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    drive(&mut st, Msg::Stage(StageMsg::StageFile("a.txt".into())));
    st.session.draft.summary = "Second line".into();
    drive(&mut st, Msg::Commit(CommitMsg::Commit));
    assert_eq!(st.undo.undo.len(), 1);

    let effect =
        update(&mut st, Msg::Job(JobMsg::Undo), Instant::now()).into_iter().find(|e| matches!(e, Effect::UndoRedo { .. })).expect("undo");
    update(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)), Instant::now());
    assert!(st.undo.undo.is_empty() && st.undo.blocked.is_none(), "tab B has its own history");
    let reply = runner::execute(effect).expect("a result");
    assert!(matches!(reply, Msg::Job(JobMsg::JobDone { .. })), "undo results are not scoped to the active tab");
    let fx = update(&mut st, reply, Instant::now());
    assert!(!fx.iter().any(|e| matches!(e, Effect::LoadRepo { .. })), "tab B is not refreshed");
    assert!(st.job.is_none());
    assert!(st.undo.undo.is_empty() && st.undo.redo.is_empty() && st.undo.blocked.is_none());
    let parked = st.tabs[0].parked.as_ref().expect("tab A is parked");
    assert_eq!((parked.undo.undo.len(), parked.undo.redo.len()), (0, 1));
    assert_eq!(parked.session.draft.summary, "Second line");

    update(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)), Instant::now());
    assert_eq!(st.undo.redo.len(), 1);
    cleanup(&[&a, &b]);
}

#[test]
fn undo_requires_idle_and_history() {
    let mut st = state();
    assert!(update(&mut st, Msg::Job(JobMsg::Undo), Instant::now()).is_empty());
    assert!(update(&mut st, Msg::Job(JobMsg::Redo), Instant::now()).is_empty());

    let dir = scratch_repo("undo-idle");
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    st.undo.record(dummy(0));
    st.job = Some(RunningJob {
        label: "Fetch".into(),
        workdir: st.workdir(),
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo: true,
        fetches: false,
    });
    assert!(update(&mut st, Msg::Job(JobMsg::Undo), Instant::now()).is_empty());
    assert!(st.log.last().is_some_and(|l| !l.ok && l.text.contains("still running")));
    st.job = None;

    // Opening a repository into a tab starts its history afresh.
    let snap = repo::load(&dir, &LoadOptions::all(100)).unwrap();
    let result = Ok(Box::new(snap));
    let generation = st.load_gen;
    update(&mut st, Msg::Repo(RepoMsg::RepoLoaded { generation, kind: LoadKind::Open, result }), Instant::now());
    assert!(st.undo.undo.is_empty());

    let mut h = UndoHistory { redo: vec![dummy(99)], blocked: Some("Merge".into()), ..Default::default() };
    for i in 0..25 {
        h.record(dummy(i));
    }
    assert_eq!(h.undo.len(), MAX_UNDO);
    assert_eq!(h.undo[0], dummy(5), "the oldest entries are dropped");
    assert!(h.redo.is_empty() && h.blocked.is_none());
    cleanup(&[&dir]);
}

#[test]
fn worktree_add_keeps_history_without_a_new_branch() {
    let keeps = |a: &[&str]| super::undo::keeps_history(&[glowfly_core::gitcli::args(a)]);
    assert!(keeps(&["worktree", "add", "C:/w/side", "side"]));
    assert!(keeps(&["worktree", "add", "--detach", "C:/w/abc", "abc1234"]));
    assert!(!keeps(&["worktree", "add", "-b", "x", "C:/w/x", "main"]));
}

#[test]
fn discarded_file_comes_back_with_undo() {
    let (mut st, dir) = committed("undo-discard-file");
    std::fs::write(dir.join("a.txt"), "work in progress\n").unwrap();
    drive(&mut st, Msg::Stage(StageMsg::DiscardFile("a.txt".into())));
    assert_ne!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "work in progress\n");
    assert!(matches!(st.undo.undo.last(), Some(Action::Discard { path, .. }) if path == "a.txt"), "{:?}", st.undo.undo);
    assert_eq!(st.undo.undo_entry(st.settings.lang(), true).0, "Undo discard in a.txt");

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "work in progress\n");
    assert!(st.job.is_none() && st.undo.redo.len() == 1, "{:?}", st.undo);
    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert_ne!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "work in progress\n");
    assert!(matches!(st.undo.undo.last(), Some(Action::Discard { .. })));
    cleanup(&[&dir]);
}

#[test]
fn hard_reset_stashes_uncommitted_work_and_undo_restores_it() {
    let (mut st, dir) = committed("undo-hard-dirty");
    let first = summary_oid(&st, "first").to_string();
    std::fs::write(dir.join("a.txt"), "not committed\n").unwrap();
    let fx = update(&mut st, Msg::git("Reset (hard)", &["reset", "--hard", &first]), Instant::now());
    assert!(
        matches!(&fx[..], [Effect::RunGit { cmds, .. }] if cmds[0][..2] == ["stash", "push"] && cmds[1] == ["reset", "--hard", first.as_str()]),
        "{fx:?}"
    );
    st.job = None;
    drive(&mut st, Msg::git("Reset (hard)", &["reset", "--hard", &first]));
    assert_eq!(head_summary(&st), "first");
    assert!(matches!(st.undo.undo.last(), Some(Action::Reset { stash: Some(_), .. })), "{:?}", st.undo.undo);

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(head_summary(&st), "Second line");
    // Line endings follow the user's core.autocrlf when git checks the stash out.
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap().replace("\r\n", "\n"), "not committed\n");
    cleanup(&[&dir]);
}

#[test]
fn git_options_before_the_subcommand_are_skipped() {
    use super::undo::keeps_history;
    let a = |v: &[&str]| glowfly_core::gitcli::args(v);
    let ids = [Oid::hash_object(git2::ObjectType::Blob, b"x").unwrap()];
    assert!(keeps_history(&glowfly_core::partial::fetch_cmds("origin", &ids)), "downloading a partial clone's contents");
    assert!(keeps_history(&[a(&["-c", "a=b", "--no-pager", "push", "origin"])]));
    assert!(!keeps_history(&[a(&["-c", "a=b", "fetch", ".", "a:b"])]), "a fetch into the repository itself");
    assert!(!keeps_history(&[a(&["-c", "a=b", "commit", "-m", "x"])]));
    assert!(!keeps_history(&[a(&["-c", "a=b"])]), "no subcommand");
}
