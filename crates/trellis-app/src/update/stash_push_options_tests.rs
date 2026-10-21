//! The Stash dialog's options: keeping the staged changes (`--keep-index`), which starts off each
//! time the dialog opens, and a chosen staged rename, which is stashed whole.

use super::stash_push_tests::{argv, dialog, form, git_run, job_label, run, wip_repo};
use super::tests::{drive, git, scratch_repo, state};
use crate::msg::{JobMsg, Msg, RepoMsg, StashMsg};
use crate::state::{AppState, Modal};
use std::path::PathBuf;
use trellis_core::i18n::Lang;
use trellis_core::repo::FileEntry;
use trellis_core::stash_ops::StashScope;

/// The Push message the dialog sends.
pub(super) fn push_with(scope: StashScope, message: &str, paths: &[&str], keep_index: bool) -> Msg {
    Msg::Stash(StashMsg::Push { scope, message: message.into(), paths: paths.iter().map(|p| p.to_string()).collect(), keep_index })
}

fn cmds(list: &[&[&str]]) -> Vec<Vec<String>> {
    list.iter().flat_map(|a| argv(a)).collect()
}

/// (status, path) of each row of a list.
fn rows(list: &[FileEntry]) -> Vec<(char, &str)> {
    list.iter().map(|f| (f.status, f.path.as_str())).collect()
}

#[test]
fn keep_index_is_passed_to_git() {
    let (mut st, dir) = wip_repo("stash-keep-index-argv");
    let fx = run(&mut st, push_with(StashScope::All, "m", &[], true));
    assert_eq!(git_run(&fx).0, argv(&["stash", "push", "-u", "--keep-index", "-m", "m"]));
    assert_eq!(job_label(&st), Some("Stash"));
    st.job = None;
    let fx = run(&mut st, push_with(StashScope::Files, "", &["b.txt"], true));
    assert_eq!(git_run(&fx).0, argv(&["--literal-pathspecs", "stash", "push", "-u", "--keep-index", "--", "b.txt"]));
    assert_eq!(job_label(&st), Some("Stash selected files"));
    st.job = None;
    let fx = run(&mut st, push_with(StashScope::Staged, "", &[], true));
    assert_eq!(git_run(&fx).0, argv(&["stash", "push", "--staged"]), "never with --staged");
    assert_eq!(job_label(&st), Some("Stash staged changes"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_option_starts_off_each_time() {
    let (mut st, dir) = wip_repo("stash-keep-index-reset");
    dialog(&mut st, StashScope::All);
    assert!(!form(&st).keep_index);
    if let Some(Modal::StashPush(f)) = &mut st.modal {
        f.keep_index = true;
    }
    run(&mut st, Msg::CloseModal);
    assert!(st.modal.is_none());
    dialog(&mut st, StashScope::All);
    assert!(!form(&st).keep_index, "not remembered");
    let _ = std::fs::remove_dir_all(&dir);
}

/// a.txt and b.txt committed, then a.txt renamed to moved.txt (staged) and an unstaged edit
/// to b.txt.
fn rename_repo(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    git(&dir, &["checkout", "--", "a.txt"]);
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    git(&dir, &["add", "b.txt"]);
    git(&dir, &["commit", "-q", "-m", "second"]);
    git(&dir, &["mv", "a.txt", "moved.txt"]);
    std::fs::write(dir.join("b.txt"), "b\nedit\n").unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(rows(&snap.staged), [('R', "moved.txt")]);
    assert_eq!(snap.staged[0].renamed_from.as_deref(), Some("a.txt"));
    assert_eq!(rows(&snap.unstaged), [('M', "b.txt")]);
    (st, dir)
}

#[test]
fn a_chosen_staged_rename_brings_its_old_path() {
    let (mut st, dir) = rename_repo("stash-rename-argv");
    let whole = cmds(&[
        &["--literal-pathspecs", "stash", "push", "-u", "--", "moved.txt"],
        &["--literal-pathspecs", "reset", "-q", "HEAD", "--", "a.txt"],
        &["checkout-index", "-u", "--", "a.txt"],
    ]);
    let fx = run(&mut st, push_with(StashScope::Files, "", &["moved.txt"], false));
    assert_eq!(git_run(&fx).0, whole);
    assert_eq!(job_label(&st), Some("Stash selected files"));
    let recorded = |fx: &[crate::effect::Effect]| match fx {
        [crate::effect::Effect::RunGit { record, .. }] => record.clone(),
        _ => None,
    };
    assert_eq!(recorded(&fx), Some(trellis_core::undo::UndoKind::StashPush { replay: Some(whole.clone()) }), "redo runs both");

    // a.txt has no row to choose, so naming it changes nothing.
    st.job = None;
    let fx = run(&mut st, push_with(StashScope::Files, "", &["moved.txt", "a.txt"], false));
    assert_eq!(git_run(&fx).0, whole);
    st.job = None;
    let fx = run(&mut st, push_with(StashScope::Files, "", &["b.txt"], false));
    assert_eq!(git_run(&fx).0, argv(&["--literal-pathspecs", "stash", "push", "-u", "--", "b.txt"]));
    // Keeping the index keeps the rename whole without help.
    st.job = None;
    let fx = run(&mut st, push_with(StashScope::Files, "", &["moved.txt"], true));
    assert_eq!(git_run(&fx).0, argv(&["--literal-pathspecs", "stash", "push", "-u", "--keep-index", "--", "moved.txt"]));
    // A case-only rename: its new name is its old one to a volume that folds case, so the
    // rename splits (the new file stays) rather than the old name being written over it.
    st.job = None;
    let case_only = FileEntry { renamed_from: Some("A.txt".into()), ..FileEntry::new("a.txt", 'R') };
    st.snap.as_mut().unwrap().staged.push(case_only);
    let fx = run(&mut st, push_with(StashScope::Files, "", &["a.txt"], false));
    assert_eq!(git_run(&fx).0, argv(&["--literal-pathspecs", "stash", "push", "-u", "--", "a.txt"]));
    st.snap.as_mut().unwrap().staged.pop();
    // A file at the old path now would be overwritten: the rename is stashed as before.
    st.job = None;
    st.snap.as_mut().unwrap().unstaged.push(FileEntry::new("a.txt", '?'));
    let fx = run(&mut st, push_with(StashScope::Files, "", &["moved.txt"], false));
    assert_eq!(git_run(&fx).0, argv(&["--literal-pathspecs", "stash", "push", "-u", "--", "moved.txt"]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rename_stash_end_to_end() {
    let (mut st, dir) = rename_repo("stash-rename-e2e");
    drive(&mut st, push_with(StashScope::Files, "", &["moved.txt"], false));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.stashes.len(), 1);
    assert!(snap.staged.is_empty(), "{:?}", snap.staged);
    assert_eq!(rows(&snap.unstaged), [('M', "b.txt")], "no row for moved.txt or a.txt");

    drive(&mut st, Msg::Job(JobMsg::Undo));
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.stashes.is_empty());
    assert_eq!(rows(&snap.staged), [('R', "moved.txt")], "the staged rename is back");
    drive(&mut st, Msg::Job(JobMsg::Redo));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.stashes.len(), 1);
    assert!(snap.staged.is_empty(), "redone whole: {:?}", snap.staged);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A file at the old path that status doesn't list under that name (here an ignored one; a
/// case twin on a volume that ignores case is the same) stops the stash before it runs.
#[test]
fn a_file_at_the_old_path_is_never_overwritten() {
    let (mut st, dir) = rename_repo("stash-rename-occupied");
    std::fs::write(dir.join(".git/info/exclude"), "a.txt\n").unwrap();
    std::fs::write(dir.join("a.txt"), "precious\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(!st.snap.as_ref().unwrap().unstaged.iter().any(|f| f.path == "a.txt"), "no row for the ignored file");
    drive(&mut st, push_with(StashScope::Files, "", &["moved.txt"], false));
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "precious\n");
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.stashes.is_empty(), "nothing was stashed");
    assert_eq!(rows(&snap.staged), [('R', "moved.txt")], "the rename is as it was");
    assert!(st.undo.undo.is_empty(), "nothing recorded");
    let headline = Lang::En.tr("stash.push.rename_blocked.toast");
    assert_eq!(st.log.last().expect("a log line").text, headline, "the translated headline");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Redo checks the old path again: a file made there after the undo is left alone.
#[test]
fn redo_never_overwrites_a_file_at_the_old_path() {
    let (mut st, dir) = rename_repo("stash-rename-redo-occupied");
    drive(&mut st, push_with(StashScope::Files, "", &["moved.txt"], false));
    drive(&mut st, Msg::Job(JobMsg::Undo));
    assert_eq!(rows(&st.snap.as_ref().unwrap().staged), [('R', "moved.txt")]);
    std::fs::write(dir.join("a.txt"), "mine\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Job(JobMsg::Redo));
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "mine\n");
    assert!(st.snap.as_ref().unwrap().stashes.is_empty(), "the redo was refused");
    // Once the file is gone, the redo is still there to run.
    std::fs::remove_file(dir.join("a.txt")).unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    drive(&mut st, Msg::Job(JobMsg::Redo));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.stashes.len(), 1);
    assert!(snap.staged.is_empty(), "redone whole: {:?}", snap.staged);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn keep_index_stash_end_to_end() {
    let (mut st, dir) = wip_repo("stash-keep-index-e2e");
    drive(&mut st, push_with(StashScope::All, "", &[], true));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.stashes.len(), 1);
    assert_eq!(rows(&snap.staged), [('M', "a.txt")]);
    assert!(!snap.unstaged.iter().any(|f| f.path == "b.txt" || f.path == "c.txt"), "{:?}", snap.unstaged);

    // Undo puts every part back where it was: pop --index finds the index unchanged.
    drive(&mut st, Msg::Job(JobMsg::Undo));
    let snap = st.snap.as_ref().unwrap();
    assert!(snap.stashes.is_empty(), "no stashes left");
    assert_eq!(rows(&snap.staged), [('M', "a.txt")]);
    assert_eq!(rows(&snap.unstaged), [('M', "b.txt"), ('?', "c.txt")]);
    assert_eq!(std::fs::read_to_string(dir.join("b.txt")).unwrap().replace("\r\n", "\n"), "b\nedit\n");
    // Redo stashes again, the staged change still staged.
    drive(&mut st, Msg::Job(JobMsg::Redo));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.stashes.len(), 1);
    assert_eq!(rows(&snap.staged), [('M', "a.txt")]);
    assert!(snap.unstaged.is_empty(), "{:?}", snap.unstaged);
    let _ = std::fs::remove_dir_all(&dir);
}
