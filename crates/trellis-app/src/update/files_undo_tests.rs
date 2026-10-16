//! Headless tests for undoing a file restore end to end: the working copy it replaced comes
//! back, redo writes the restored version again, a file edited since is left alone, and a
//! result for a tab that is no longer active lands in that tab's history.

use super::tests::{drive, git, keys, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::FilesMsg;
use crate::msg::Inactive;
use crate::msg::JobMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::TabsMsg;
use crate::runner;
use crate::state::AppState;
use git2::{Oid, Repository};
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::undo::Action;

fn head(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn text(dir: &Path, f: &str) -> String {
    std::fs::read_to_string(dir.join(f)).unwrap()
}

fn restore(oid: Oid, path: &str) -> Msg {
    Msg::Files(FilesMsg::RestoreFile { oid, path: path.into(), deleted: false, confirmed: true, overwrite: true })
}

/// The scratch repository (a.txt committed as "one\n", edited to "one\ntwo\n"), opened.
fn opened(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    git(&dir, &["config", "core.autocrlf", "false"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    (st, dir)
}

fn top_is_restore_of(st: &AppState, want: &str) -> bool {
    matches!(st.undo.undo.last(), Some(Action::Restore { path, .. }) if path == want)
}

#[test]
fn restore_then_undo_and_redo() {
    let (mut st, dir) = opened("files-undo-restore");
    drive(&mut st, restore(head(&dir), "a.txt"));
    assert_eq!(text(&dir, "a.txt"), "one\n");
    assert!(top_is_restore_of(&st, "a.txt"), "{:?}", st.undo.undo);
    assert_eq!(st.undo.undo_tooltip(), format!("Undo restore of a.txt ({})", keys("Ctrl+Z")));

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(text(&dir, "a.txt"), "one\ntwo\n", "the edits are back");
    assert!(st.job.is_none());
    assert_eq!(st.undo.redo.len(), 1);
    let log: Vec<&str> = st.log.iter().map(|l| l.text.as_str()).collect();
    assert!(log.iter().any(|t| t.contains("Put back the earlier working copy of a.txt")), "{log:?}");

    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert_eq!(text(&dir, "a.txt"), "one\n");
    assert!(top_is_restore_of(&st, "a.txt"), "{:?}", st.undo.undo);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restore_of_deleted_file_undo_deletes_it_again() {
    let (mut st, dir) = opened("files-undo-gone");
    git(&dir, &["checkout", "-q", "--", "a.txt"]);
    std::fs::write(dir.join("gone.txt"), "v1\n").unwrap();
    git(&dir, &["add", "gone.txt"]);
    git(&dir, &["commit", "-q", "-m", "add gone"]);
    let older = head(&dir);
    git(&dir, &["rm", "-q", "gone.txt"]);
    git(&dir, &["commit", "-q", "-m", "delete gone"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));

    drive(&mut st, restore(older, "gone.txt"));
    assert_eq!(text(&dir, "gone.txt"), "v1\n");
    assert!(top_is_restore_of(&st, "gone.txt"), "{:?}", st.undo.undo);
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert!(!dir.join("gone.txt").exists(), "removed again");
    let snap = st.snap.as_ref().unwrap();
    assert!(!snap.unstaged.iter().any(|f| f.path == "gone.txt"), "{:?}", snap.unstaged);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undo_after_editing_the_restored_file_is_refused() {
    let (mut st, dir) = opened("files-undo-stale");
    drive(&mut st, restore(head(&dir), "a.txt"));
    assert!(top_is_restore_of(&st, "a.txt"));
    std::fs::write(dir.join("a.txt"), "newer\n").unwrap();

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(text(&dir, "a.txt"), "newer\n", "left alone");
    assert!(st.undo.undo.is_empty() && st.undo.redo.is_empty(), "history cleared");
    let said: Vec<&str> = st.toasts.iter().map(|t| t.text.as_str()).chain(st.log.iter().map(|l| l.text.as_str())).collect();
    assert!(said.iter().any(|t| t.contains("has changed since")), "{said:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn restore_result_for_inactive_tab_goes_to_its_history() {
    let (a, b) = (scratch_repo("files-undo-tab-a"), scratch_repo("files-undo-tab-b"));
    git(&a, &["config", "core.autocrlf", "false"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));

    let effect = update(&mut st, restore(head(&a), "a.txt"), Instant::now())
        .into_iter()
        .find(|e| matches!(e, Effect::RestoreFile { .. }))
        .expect("a restore");
    update(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)), Instant::now());
    let reply = runner::execute(effect).expect("a result");
    assert!(matches!(reply, Msg::Scoped { .. }), "scoped to tab A's repository");
    update(&mut st, reply, Instant::now());

    assert_eq!(text(&a, "a.txt"), "one\n");
    assert!(st.undo.undo.is_empty(), "tab B's history is untouched");
    let parked = st.tabs[0].parked.as_ref().expect("tab A is parked");
    assert_eq!(parked.undo.undo.len(), 1);
    assert!(matches!(parked.undo.undo.last(), Some(Action::Restore { path, .. }) if path == "a.txt"));
    for d in [&a, &b] {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// A restore that fails after git removed the file (a required smudge filter that fails) still
/// records an undo entry, and the log says so; undoing puts the edits back.
#[test]
fn failed_restore_that_removed_the_file_can_be_undone() {
    let (mut st, dir) = opened("files-undo-failed");
    for (k, v) in [("filter.x.clean", "cat"), ("filter.x.smudge", "cat"), ("filter.x.required", "true")] {
        git(&dir, &["config", k, v]);
    }
    std::fs::write(dir.join(".gitattributes"), "*.up filter=x\n").unwrap();
    std::fs::write(dir.join("b.up"), "one\n").unwrap();
    git(&dir, &["add", ".gitattributes", "b.up"]);
    git(&dir, &["commit", "-q", "-m", "add b"]);
    let base = head(&dir);
    std::fs::write(dir.join("b.up"), "two\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "edit b"]);
    std::fs::write(dir.join("b.up"), "local edits\n").unwrap();
    git(&dir, &["config", "filter.x.smudge", "false"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));

    drive(&mut st, restore(base, "b.up"));
    assert!(!dir.join("b.up").exists(), "git removed the file before the filter failed");
    assert!(top_is_restore_of(&st, "b.up"), "{:?}", st.undo.undo);
    let log: Vec<&str> = st.log.iter().map(|l| l.text.as_str()).collect();
    let said = log.iter().find(|t| t.starts_with("Could not restore b.up")).unwrap_or_else(|| panic!("{log:?}"));
    assert!(
        said.contains("git cat-file -p") && said.contains(&format!("Undo ({}) puts back what it held before.", keys("Ctrl+Z"))),
        "{said}"
    );

    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(text(&dir, "b.up"), "local edits\n", "the edits are back");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn failed_restore_with_undo_for_inactive_tab_logs_and_keeps_the_undo() {
    let oid = Oid::zero();
    let undo = Action::Restore { path: "a.txt".into(), oid, before: None, after: None };
    let msg = FilesMsg::FileRestored { path: "a.txt".into(), oid, result: Err("boom".into()), undo: Some(undo.clone()) };
    match msg.on_inactive() {
        Some(Inactive::LogAndUndo { text, undo: a }) => {
            assert!(text.starts_with("Could not restore a.txt: boom. Undo ("), "{text}");
            assert_eq!(a, undo);
        }
        _ => panic!("expected a log line and an undo entry"),
    }
}
