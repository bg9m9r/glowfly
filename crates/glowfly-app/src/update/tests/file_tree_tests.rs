//! Headless tests for the changed-file lists' tree: the List / Tree setting, collapsing folders
//! (kept per tab, cleared with another repository), the WIP keys and next/previous change in
//! tree order, staging and unstaging a folder, and a folder row's history.

use super::history_tests::{empty_diff, opened};
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, LocalOp};
use crate::keymap::KeyAction;
use crate::msg::{DiffMsg, HistoryMsg, LayoutMsg, Msg, RepoMsg, StageMsg, TabsMsg};
use crate::state::AppState;
use crate::state::diff_nav;
use crate::state::file_folds::FileListId;
use glowfly_core::repo::{self, DiffTarget, FileEntry, Hazard, HistoryStart, LoadOptions};
use glowfly_core::settings::FileListMode;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn toggle(st: &mut AppState, list: FileListId, path: &str) -> Vec<Effect> {
    run(st, Msg::Layout(LayoutMsg::ToggleFolder { list, path: path.into() }))
}

fn un(path: &str) -> DiffTarget {
    DiffTarget::Unstaged(path.into())
}

fn write(dir: &Path, path: &str, text: &str) {
    let p = dir.join(path);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

fn reload(st: &mut AppState, dir: &Path) {
    st.snap = Some(repo::load(dir, &LoadOptions::all(100)).unwrap());
}

/// The scratch repository (a.txt modified) with new src/b.txt and src/c.txt, loaded, in Tree
/// mode. Flat order: a.txt, src/b.txt, src/c.txt; tree order: src/b.txt, src/c.txt, a.txt.
fn tree_repo(name: &str) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    write(&dir, "src/b.txt", "b\n");
    write(&dir, "src/c.txt", "c\n");
    let mut st = state();
    st.settings.file_list_mode = FileListMode::Tree;
    reload(&mut st, &dir);
    (st, dir)
}

/// The one diff `fx` asks for.
fn loaded(fx: &[Effect]) -> Option<&DiffTarget> {
    fx.iter().find_map(|e| match e {
        Effect::LoadDiff { target, .. } => Some(target),
        _ => None,
    })
}

fn step(st: &mut AppState, down: bool) -> Option<DiffTarget> {
    let fx = run(st, Msg::Shortcut(KeyAction::StepFile { staged: false, down }));
    loaded(&fx).cloned()
}

fn local_op(fx: &[Effect]) -> Option<&LocalOp> {
    fx.iter().find_map(|e| match e {
        Effect::LocalOp { op, .. } => Some(op),
        _ => None,
    })
}

#[test]
fn the_list_mode_is_saved_once() {
    let mut st = state();
    let fx = run(&mut st, Msg::Layout(LayoutMsg::SetFileListMode(FileListMode::Tree)));
    assert_eq!(st.settings.file_list_mode, FileListMode::Tree);
    assert!(matches!(&fx[..], [Effect::SaveSettings(_)]), "{fx:?}");
    assert!(run(&mut st, Msg::Layout(LayoutMsg::SetFileListMode(FileListMode::Tree))).is_empty());
}

#[test]
fn folders_collapse_and_expand() {
    let mut st = state();
    assert!(toggle(&mut st, FileListId::Commit, "src").is_empty(), "view state: nothing saved");
    assert!(st.view.file_folds.is_closed(FileListId::Commit, "src"));
    toggle(&mut st, FileListId::Commit, "src");
    assert!(!st.view.file_folds.is_closed(FileListId::Commit, "src"));
    assert!(LayoutMsg::SetFileListMode(FileListMode::Tree).on_inactive().is_none());
    assert!(LayoutMsg::ToggleFolder { list: FileListId::Commit, path: "src".into() }.on_inactive().is_none());
}

#[test]
fn folds_are_kept_per_tab_and_cleared_by_another_repository() {
    let (mut st, dir) = opened("tree-folds-tabs");
    toggle(&mut st, FileListId::Unstaged, "src");
    drive(&mut st, Msg::Tabs(TabsMsg::NewTab));
    assert!(!st.view.file_folds.is_closed(FileListId::Unstaged, "src"), "a new tab has no folds");
    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    assert!(st.view.file_folds.is_closed(FileListId::Unstaged, "src"), "back in the first tab");
    // Opening a repository in the tab (LoadKind::Open) starts without folds.
    let mut fresh = state();
    toggle(&mut fresh, FileListId::Commit, "docs");
    drive(&mut fresh, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert!(fresh.snap.is_some());
    assert!(!fresh.view.file_folds.is_closed(FileListId::Commit, "docs"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn up_and_down_follow_the_tree_and_skip_collapsed_folders() {
    let (mut st, dir) = tree_repo("tree-keys");
    assert_eq!(step(&mut st, true), Some(un("src/b.txt")), "the first file in tree order");
    assert_eq!(step(&mut st, true), Some(un("src/c.txt")));
    assert_eq!(step(&mut st, true), Some(un("a.txt")), "root files after the folder's");
    assert_eq!(step(&mut st, true), None);
    // From a file hidden by collapsing its folder: on to the next visible one.
    run(&mut st, Msg::Repo(RepoMsg::OpenDiff(un("src/b.txt"))));
    toggle(&mut st, FileListId::Unstaged, "src");
    assert_eq!(step(&mut st, true), Some(un("a.txt")));
    assert_eq!(step(&mut st, false), None, "src's files are skipped");
    // List mode keeps git's order.
    st.settings.file_list_mode = FileListMode::List;
    assert_eq!(step(&mut st, true), Some(un("src/b.txt")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn next_change_walks_the_tree_and_opens_collapsed_folders() {
    let (mut st, dir) = tree_repo("tree-nav");
    git(&dir, &["add", "src/c.txt"]);
    reload(&mut st, &dir);
    st.selection = crate::state::Selection::Wip;
    // Unstaged in tree order (src/b.txt, a.txt), then Staged (src/c.txt).
    assert_eq!(diff_nav::next_file(&st, &un("src/b.txt"), 1), Some(un("a.txt")));
    assert_eq!(diff_nav::next_file(&st, &un("a.txt"), 1), Some(DiffTarget::Staged("src/c.txt".into())));
    assert_eq!(diff_nav::next_file(&st, &un("a.txt"), -1), Some(un("src/b.txt")));
    // A step into a file of a collapsed folder opens the folder.
    toggle(&mut st, FileListId::Unstaged, "src");
    assert_eq!(diff_nav::next_file(&st, &un("a.txt"), -1), Some(un("src/b.txt")), "folds don't hide files from stepping");
    st.diff = Some((un("a.txt"), empty_diff().into()));
    let fx = run(&mut st, Msg::Diff(DiffMsg::JumpToChange(-1)));
    assert_eq!(loaded(&fx), Some(&un("src/b.txt")));
    assert!(!st.view.file_folds.is_closed(FileListId::Unstaged, "src"));
    // List mode keeps git's order.
    st.settings.file_list_mode = FileListMode::List;
    assert_eq!(diff_nav::next_file(&st, &un("a.txt"), 1), Some(un("src/b.txt")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn commit_details_walk_in_tree_order() {
    let dir = scratch_repo("tree-commit");
    write(&dir, "a/b.txt", "b\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "second"]);
    let mut st = state();
    st.settings.file_list_mode = FileListMode::Tree;
    reload(&mut st, &dir);
    let head = st.snap.as_ref().unwrap().head_oid.unwrap();
    let detail = repo::commit_detail(&dir, head).unwrap();
    let flat: Vec<&str> = detail.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(flat, ["a.txt", "a/b.txt"], "git's order");
    st.selection = crate::state::Selection::Commit(head);
    st.detail = Some(detail);
    let c = |p: &str| DiffTarget::Commit(head, p.into());
    assert_eq!(diff_nav::next_file(&st, &c("a/b.txt"), 1), Some(c("a.txt")), "the folder first");
    assert_eq!(crate::state::file_folds::list_of(&st, &c("a.txt")), Some(FileListId::Commit));
    st.settings.file_list_mode = FileListMode::List;
    assert_eq!(diff_nav::next_file(&st, &c("a.txt"), 1), Some(c("a/b.txt")));
    assert_eq!(crate::state::file_folds::list_of(&st, &c("a.txt")), None, "no tree in List mode");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn s_moves_on_to_the_tree_neighbour() {
    let dir = scratch_repo("tree-s-moves-on");
    write(&dir, "src/b.txt", "b\n");
    write(&dir, "src/c.txt", "c\n");
    let mut st = state();
    st.settings.file_list_mode = FileListMode::Tree;
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(un("src/c.txt"))));
    drive(&mut st, Msg::Shortcut(KeyAction::Stage));
    assert!(st.snap.as_ref().unwrap().staged.iter().any(|f| f.path == "src/c.txt"));
    // In git's order src/c.txt is last (src/b.txt above it); in the tree a.txt follows it.
    assert_eq!(st.diff.as_ref().map(|(t, _)| t), Some(&un("a.txt")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stage_folder_stages_only_that_folder() {
    let dir = scratch_repo("tree-stage-folder");
    for f in ["src/a.txt", "src/b.txt", "src2/c.txt", "src.txt"] {
        write(&dir, f, "new\n");
    }
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let fx = run(&mut st, Msg::Stage(StageMsg::StageFolder("src".into())));
    assert_eq!(local_op(&fx), Some(&LocalOp::StageFile("src".into())), "{fx:?}");
    drive(&mut st, Msg::Stage(StageMsg::StageFolder("src".into())));
    let snap = st.snap.as_ref().unwrap();
    let mut staged: Vec<&str> = snap.staged.iter().map(|f| f.path.as_str()).collect();
    staged.sort();
    assert_eq!(staged, ["src/a.txt", "src/b.txt"]);
    assert!(snap.unstaged.iter().any(|f| f.path == "src2/c.txt") && snap.unstaged.iter().any(|f| f.path == "src.txt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stage_folder_waits_for_its_conflicts() {
    let (mut st, dir) = tree_repo("tree-stage-conflict");
    st.snap.as_mut().unwrap().unstaged.push(FileEntry::new("src/x.txt", '!'));
    let fx = run(&mut st, Msg::Stage(StageMsg::StageFolder("src".into())));
    assert!(local_op(&fx).is_none(), "{fx:?}");
    assert!(st.toasts.last().is_some_and(|t| !t.ok && t.text.contains("conflicted")));
    // Another folder's conflict doesn't block it.
    st.snap.as_mut().unwrap().unstaged.retain(|f| f.status != '!');
    st.snap.as_mut().unwrap().unstaged.push(FileEntry::new("srcx/x.txt", '!'));
    let fx = run(&mut st, Msg::Stage(StageMsg::StageFolder("src".into())));
    assert_eq!(local_op(&fx), Some(&LocalOp::StageFile("src".into())));
    assert!(run(&mut st, Msg::Stage(StageMsg::StageFolder("none".into()))).is_empty(), "nothing under it");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unstage_folder_unstages_its_staged_files() {
    let (mut st, dir) = tree_repo("tree-unstage-folder");
    let snap = st.snap.as_mut().unwrap();
    snap.staged = ["src/a.txt", "src/sub/b.txt", "src2/c.txt", "src.txt"].iter().map(|p| FileEntry::new(*p, 'M')).collect();
    let odd = FileEntry { hazard: Some(Hazard::NotUtf8(b"src/\xff".to_vec())), ..FileEntry::new("src/\\xff", 'A') };
    snap.staged.push(odd);
    let fx = run(&mut st, Msg::Stage(StageMsg::UnstageFolder("src".into())));
    assert_eq!(local_op(&fx), Some(&LocalOp::UnstagePaths(vec!["src/a.txt".into(), "src/sub/b.txt".into()])), "{fx:?}");
    assert!(run(&mut st, Msg::Stage(StageMsg::UnstageFolder("docs".into()))).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_folder_rows_history_starts_at_its_commit() {
    let (mut st, dir) = opened("tree-folder-history");
    let head = st.snap.as_ref().unwrap().head_oid.unwrap();
    let fx = run(&mut st, Msg::History(HistoryMsg::OpenFolderHistory { path: "src/search".into(), at: Some(head) }));
    assert!(
        matches!(&fx[..], [Effect::LoadFileHistory { path, start: HistoryStart::At(o), folder: true, .. }] if path == "src/search" && *o == head),
        "{fx:?}"
    );
    let v = st.session.history.view.as_ref().unwrap();
    assert_eq!(v.kind, crate::state::history::HistoryKind::Folder);
    let _ = std::fs::remove_dir_all(&dir);
}
