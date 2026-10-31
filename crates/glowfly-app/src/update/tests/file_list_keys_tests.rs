//! Headless tests for the moves of a focused list that only opens files: Up/Down (and Page,
//! Home and End) in the commit, comparison, stacked, working-directory comparison, pull request,
//! folder history, Conflicted and Resolved Files lists, Tree mode skipping collapsed folders,
//! a key never leaving a conflict's unsaved picks, and Mark resolved from Conflicted Files
//! moving on to the next conflicted file.

use super::conflict_files_tests::both_resolved;
use super::conflict_tests::opened_two;
use super::selection_tests::fixture;
use super::tests::drive;
use super::update;
use crate::effect::Effect;
use crate::keymap::KeyAction;
use crate::msg::{ConflictMsg, HistoryMsg, LayoutMsg, Msg, RepoMsg, SelectionMsg};
use crate::state::AppState;
use crate::state::file_folds::FileListId;
use crate::state::list_keys::{FileList, ListMove};
use glowfly_core::repo::{DiffTarget, Side};
use glowfly_core::settings::FileListMode;
use std::time::Instant;

const DOWN: ListMove = ListMove::Step { down: true };
const UP: ListMove = ListMove::Step { down: false };

fn key(st: &mut AppState, list: FileList, mv: ListMove) -> Vec<Effect> {
    update(st, Msg::Shortcut(KeyAction::FileList { list, mv }), Instant::now())
}

/// Sends the move and runs what it starts, as the app would.
fn press(st: &mut AppState, list: FileList, mv: ListMove) {
    drive(st, Msg::Shortcut(KeyAction::FileList { list, mv }));
}

/// The diff `fx` opens.
fn opened(fx: &[Effect]) -> Option<&DiffTarget> {
    fx.iter().find_map(|e| match e {
        Effect::LoadDiff { target, reload: false, .. } => Some(target),
        _ => None,
    })
}

fn shown(st: &AppState) -> Option<&DiffTarget> {
    st.session.diff.view.as_ref().map(|(t, _)| t)
}

#[test]
fn the_commit_list_opens_the_file_above_or_below() {
    let (mut st, f) = fixture("file-keys-commit");
    drive(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(3))));
    let at = |p: &str| DiffTarget::Commit(f.c(3), p.into());
    // No file open yet: Down starts at the first file.
    let fx = key(&mut st, FileList::Commit, DOWN);
    assert_eq!(opened(&fx), Some(&at("f.txt")));
    assert_eq!(st.view.file_reveal.as_ref().map(|(t, _)| t), Some(&at("f.txt")), "the list scrolls to it");
    press(&mut st, FileList::Commit, DOWN);
    press(&mut st, FileList::Commit, DOWN);
    assert_eq!(shown(&st), Some(&at("g.txt")), "the second press opened g.txt, the third stopped at the end");
    assert!(key(&mut st, FileList::Commit, DOWN).is_empty(), "nothing past the last file");
    press(&mut st, FileList::Commit, UP);
    assert_eq!(shown(&st), Some(&at("f.txt")));
    press(&mut st, FileList::Commit, ListMove::Edge { end: true });
    assert_eq!(shown(&st), Some(&at("g.txt")));
    press(&mut st, FileList::Commit, ListMove::Page { down: false, rows: 5 });
    assert_eq!(shown(&st), Some(&at("f.txt")));
    // Another list's keys don't reach this one's files.
    assert!(key(&mut st, FileList::Compare, DOWN).is_empty());
    assert!(key(&mut st, FileList::Workdir, DOWN).is_empty());
    let _ = std::fs::remove_dir_all(&f.dir);
}

/// The fixture with one more commit adding src/a.txt, src/b.txt and z.txt; returns it.
fn with_folder(st: &mut AppState, dir: &std::path::Path) -> git2::Oid {
    std::fs::create_dir_all(dir.join("src")).unwrap();
    for p in ["src/a.txt", "src/b.txt", "z.txt"] {
        std::fs::write(
            dir.join(p),
            "x
",
        )
        .unwrap();
    }
    let oid = super::selection_tests::commit_on(dir, "tree", 9);
    drive(st, Msg::Repo(RepoMsg::Refresh));
    oid
}

#[test]
fn tree_mode_follows_the_tree_and_skips_collapsed_folders() {
    let (mut st, f) = fixture("file-keys-tree");
    let oid = with_folder(&mut st, &f.dir);
    drive(&mut st, Msg::Repo(RepoMsg::SelectCommit(oid)));
    st.settings.file_list_mode = FileListMode::Tree;
    let at = |p: &str| DiffTarget::Commit(oid, p.into());
    press(&mut st, FileList::Commit, DOWN);
    assert_eq!(shown(&st), Some(&at("src/a.txt")), "the folder's files come first in the tree");
    drive(&mut st, Msg::Layout(LayoutMsg::ToggleFolder { list: FileListId::Commit, path: "src".into() }));
    press(&mut st, FileList::Commit, DOWN);
    assert_eq!(shown(&st), Some(&at("z.txt")), "from a collapsed folder's file to the next shown one");
    assert!(key(&mut st, FileList::Commit, UP).is_empty(), "src's files are hidden");
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn the_comparison_and_stacked_lists_step_through_their_files() {
    let (mut st, f) = fixture("file-keys-compare");
    drive(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(3))));
    drive(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(2))));
    let between = |p: &str| DiffTarget::Between(f.c(2), f.c(3), p.into());
    press(&mut st, FileList::Compare, DOWN);
    assert_eq!(shown(&st), Some(&between("f.txt")));
    press(&mut st, FileList::Compare, DOWN);
    assert_eq!(shown(&st), Some(&between("g.txt")));
    assert!(key(&mut st, FileList::Stacked, DOWN).is_empty(), "a pair has no stacked list");

    // c5, c4 and c2, without c3 between them: each commit's own f.txt, newest first.
    drive(&mut st, Msg::Repo(RepoMsg::SelectCommit(f.c(5))));
    drive(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(4))));
    drive(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(f.c(2))));
    let commit = |n: usize| DiffTarget::Commit(f.c(n), "f.txt".into());
    press(&mut st, FileList::Stacked, DOWN);
    assert_eq!(shown(&st), Some(&commit(5)));
    press(&mut st, FileList::Stacked, DOWN);
    assert_eq!(shown(&st), Some(&commit(4)), "on into the next commit's section");
    press(&mut st, FileList::Stacked, ListMove::Edge { end: true });
    assert_eq!(shown(&st), Some(&commit(2)));
    press(&mut st, FileList::Stacked, UP);
    assert_eq!(shown(&st), Some(&commit(4)));
    assert!(key(&mut st, FileList::Compare, DOWN).is_empty(), "no combined diff to step through");
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn the_working_directory_comparison_steps_through_its_files() {
    let (mut st, f) = fixture("file-keys-workdir");
    drive(&mut st, Msg::Selection(SelectionMsg::CompareWithWorkdir(f.c(1))));
    let crate::state::Selection::VsWorkdir(w) = &st.session.selection else { panic!("compared") };
    let files: Vec<String> = w.changes.as_ref().unwrap().as_ref().unwrap().files.iter().map(|f| f.path.clone()).collect();
    assert_eq!(files, ["f.txt", "g.txt"]);
    let (first, second) = (w.target("f.txt".into()), w.target("g.txt".into()));
    press(&mut st, FileList::Workdir, DOWN);
    assert_eq!(shown(&st), Some(&first));
    press(&mut st, FileList::Workdir, DOWN);
    assert_eq!(shown(&st), Some(&second));
    press(&mut st, FileList::Workdir, ListMove::Edge { end: false });
    assert_eq!(shown(&st), Some(&first));
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn the_pull_request_list_steps_through_its_files() {
    let (mut st, work, bare, _) = crate::update::pull_request::tests::setup("file-keys-pr", &["g.txt"]);
    crate::update::pull_request::tests::open(&mut st);
    let target = |st: &AppState, p: &str| st.session.pull_request.view.as_ref().unwrap().diff_target(p).unwrap();
    let (f, g) = (target(&st, "f.txt"), target(&st, "g.txt"));
    press(&mut st, FileList::PullRequest, DOWN);
    assert_eq!(shown(&st), Some(&f));
    assert!(st.session.pull_request.view.is_some(), "the panel stays open");
    press(&mut st, FileList::PullRequest, DOWN);
    assert_eq!(shown(&st), Some(&g));
    press(&mut st, FileList::PullRequest, UP);
    assert_eq!(shown(&st), Some(&f));
    for d in [&work, &bare] {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn the_folder_history_list_selects_the_file_above_or_below() {
    let (mut st, f) = fixture("file-keys-folder");
    let oid = with_folder(&mut st, &f.dir);
    drive(&mut st, Msg::History(HistoryMsg::OpenFolderHistory { path: "src".into(), at: None }));
    let Some(view) = st.session.history.view.as_ref() else { panic!("a folder history") };
    let i = view.entries.iter().position(|e| e.oid == oid).expect("the commit is listed");
    drive(&mut st, Msg::History(HistoryMsg::SelectHistoryEntry(i)));
    let file = |st: &AppState| st.session.history.view.as_ref().map(|v| v.file);
    assert_eq!(file(&st), Some(0));
    press(&mut st, FileList::FolderHistory, DOWN);
    assert_eq!(file(&st), Some(1));
    assert_eq!(st.view.history_file_scroll, Some(1), "the list scrolls to it");
    assert_eq!(shown(&st), Some(&DiffTarget::Commit(oid, "src/b.txt".into())));
    assert!(key(&mut st, FileList::FolderHistory, DOWN).is_empty());
    press(&mut st, FileList::FolderHistory, UP);
    assert_eq!(file(&st), Some(0));
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[test]
fn the_conflicted_list_opens_the_conflict_above_or_below() {
    let (mut st, dir) = opened_two("file-keys-conflicted");
    let open = |st: &AppState| st.session.conflict.view.as_ref().map(|c| c.path.clone());
    press(&mut st, FileList::Conflicted, DOWN);
    assert_eq!(open(&st).as_deref(), Some("b.txt"));
    let reveal = st.view.file_reveal.as_ref().map(|(t, _)| t);
    assert_eq!(reveal, Some(&DiffTarget::Unstaged("b.txt".into())), "Conflicted Files scrolls to it");
    assert!(key(&mut st, FileList::Conflicted, DOWN).is_empty(), "b.txt is the last");
    press(&mut st, FileList::Conflicted, UP);
    assert_eq!(open(&st).as_deref(), Some("a.txt"));

    // A pick not saved yet keeps the key from leaving the file.
    drive(&mut st, Msg::Conflict(ConflictMsg::ConflictToggle { chunk: 0, side: Side::Ours }));
    assert!(key(&mut st, FileList::Conflicted, DOWN).is_empty());
    assert_eq!(open(&st).as_deref(), Some("a.txt"));
    let said = st.log.last().map(|l| l.text.clone()).unwrap_or_default();
    assert!(said.starts_with("a.txt has picks that aren't saved"), "{said}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mark_resolved_from_the_list_opens_the_next_conflicted_file() {
    let (mut st, dir) = opened_two("file-keys-mark-resolved");
    std::fs::write(dir.join("a.txt"), "one\nours\n").unwrap();
    // The row's Mark resolved on the open file, through the markers check.
    drive(&mut st, Msg::Conflict(ConflictMsg::MarkResolved(vec!["a.txt".into()])));
    let c = st.session.conflict.view.as_ref().expect("still open");
    assert_eq!(c.path, "b.txt", "moved on to the next conflicted file");
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Resolved a.txt. Opened b.txt (1 conflicted file left)"));
    let conflicted: Vec<&str> = st.snap.as_ref().unwrap().unstaged.iter().filter(|f| f.status == '!').map(|f| f.path.as_str()).collect();
    assert_eq!(conflicted, ["b.txt"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_resolved_list_opens_the_staged_file_above_or_below() {
    let (mut st, dir) = both_resolved("file-keys-resolved");
    let staged = |p: &str| DiffTarget::Staged(p.into());
    press(&mut st, FileList::Resolved, DOWN);
    assert_eq!(shown(&st), Some(&staged("a.txt")));
    press(&mut st, FileList::Resolved, DOWN);
    assert_eq!(shown(&st), Some(&staged("b.txt")));
    press(&mut st, FileList::Resolved, ListMove::Edge { end: false });
    assert_eq!(shown(&st), Some(&staged("a.txt")));
    assert!(key(&mut st, FileList::Conflicted, DOWN).is_empty(), "nothing is conflicted");
    let _ = std::fs::remove_dir_all(&dir);
}
