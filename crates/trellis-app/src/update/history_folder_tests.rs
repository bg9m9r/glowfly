//! Headless tests for folder history: the request, selecting an entry and one of its files,
//! keeping the selected file through a re-run, and a real folder's history.

use super::history_tests::{entry, oid, opened};
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, LoadKind};
use crate::msg::{HistoryMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::state::history::{FileHistoryView, HistoryKind};
use std::time::Instant;
use trellis_core::repo::{self, DiffTarget, FileEntry, FileHistory, HistoryEntry, HistoryStart, LoadOptions};

fn send(st: &mut AppState, m: HistoryMsg) -> Vec<Effect> {
    update(st, Msg::History(m), Instant::now())
}

/// A folder entry of "src" changing `files`.
fn folder_entry(n: u8, files: &[&str]) -> HistoryEntry {
    HistoryEntry { files: files.iter().map(|f| FileEntry::new(*f, 'M')).collect(), ..entry(n, "src") }
}

/// An open repository with a loaded folder history of src: entry 1 (two files) and entry 2
/// (none).
fn with_folder_view(name: &str) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = opened(name);
    let mut view = FileHistoryView::loading_kind("src".into(), None, HistoryKind::Folder);
    let entries = vec![folder_entry(1, &["src/a.rs", "src/b.rs"]), folder_entry(2, &[])];
    view.append(FileHistory { entries, from_head: true, ..Default::default() });
    view.batches = 1;
    st.history = Some(view);
    (st, dir)
}

fn diff_targets(fx: &[Effect]) -> Vec<DiffTarget> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::LoadDiff { target, .. } => Some(target.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn opening_folder_history_requests_a_folder_walk() {
    let (mut st, dir) = opened("folder-open");
    st.diff = Some((DiffTarget::Unstaged("a.txt".into()), super::history_tests::empty_diff().into()));
    let fx = send(&mut st, HistoryMsg::OpenFolderHistory { path: "src".into(), at: None });
    assert!(matches!(&fx[..], [Effect::LoadFileHistory { path, start: HistoryStart::Head, folder: true, .. }] if path == "src"), "{fx:?}");
    let v = st.history.as_ref().unwrap();
    assert_eq!(v.kind, HistoryKind::Folder);
    assert!(v.loading && st.diff.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn selecting_a_folder_entry_opens_its_first_file() {
    let (mut st, dir) = with_folder_view("folder-select");
    let fx = send(&mut st, HistoryMsg::SelectHistoryEntry(0));
    assert_eq!(diff_targets(&fx), [DiffTarget::Commit(oid(1), "src/a.rs".into())]);
    let fx = send(&mut st, HistoryMsg::SelectHistoryFile(1));
    assert_eq!(diff_targets(&fx), [DiffTarget::Commit(oid(1), "src/b.rs".into())]);
    assert_eq!(st.history.as_ref().unwrap().file, 1);
    assert_eq!(super::history::missing_diff(&st), Some(DiffTarget::Commit(oid(1), "src/b.rs".into())));
    assert!(send(&mut st, HistoryMsg::SelectHistoryFile(5)).is_empty(), "out of range");
    assert_eq!(st.history.as_ref().unwrap().file, 1);
    // Picking another entry starts at its first file again.
    send(&mut st, HistoryMsg::SelectHistoryEntry(0));
    assert_eq!(st.history.as_ref().unwrap().file, 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn folder_entry_without_files_opens_no_diff() {
    let (mut st, dir) = with_folder_view("folder-no-files");
    st.diff = Some((DiffTarget::Commit(oid(1), "src/a.rs".into()), super::history_tests::empty_diff().into()));
    let fx = send(&mut st, HistoryMsg::SelectHistoryEntry(1));
    assert!(diff_targets(&fx).is_empty(), "{fx:?}");
    assert!(st.diff.is_none());
    assert!(super::history::missing_diff(&st).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn folder_rerun_keeps_the_selected_file() {
    let (mut st, dir) = with_folder_view("folder-rerun");
    send(&mut st, HistoryMsg::SelectHistoryEntry(0));
    send(&mut st, HistoryMsg::SelectHistoryFile(1));
    let workdir = st.workdir().unwrap();
    let mut snap = repo::load(&workdir, &LoadOptions::all(100)).unwrap();
    snap.head_oid = Some(oid(9));
    let m = Msg::Repo(RepoMsg::RepoLoaded { generation: st.load_gen, kind: LoadKind::Refresh, result: Ok(Box::new(snap)) });
    let fx = update(&mut st, m, Instant::now());
    assert!(
        fx.iter().any(|e| matches!(e, Effect::LoadFileHistory { start: HistoryStart::Head, folder: true, .. })),
        "a folder re-runs from HEAD: {fx:?}"
    );
    let entries = vec![folder_entry(1, &["src/new.rs", "src/a.rs", "src/b.rs"]), folder_entry(2, &[])];
    let m = Msg::History(HistoryMsg::FileHistoryLoaded {
        path: "src".into(),
        generation: st.history_gen,
        result: Ok(FileHistory { entries, from_head: true, ..Default::default() }),
    });
    update(&mut st, m, Instant::now());
    let v = st.history.as_ref().unwrap();
    assert_eq!((v.selected, v.file), (Some(0), 2));
    assert_eq!(v.target(), Some(DiffTarget::Commit(oid(1), "src/b.rs".into())));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn folder_history_e2e() {
    let dir = scratch_repo("folder-e2e");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/x.txt"), "x\n").unwrap();
    std::fs::write(dir.join("src/y.txt"), "y\n").unwrap();
    git(&dir, &["add", "src"]);
    git(&dir, &["commit", "-q", "-m", "add src"]);
    std::fs::write(dir.join("src/y.txt"), "y2\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "edit y and a"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::History(HistoryMsg::OpenFolderHistory { path: "src".into(), at: None }));
    let v = st.history.as_ref().unwrap();
    let summaries: Vec<&str> = v.entries.iter().map(|e| e.summary.as_str()).collect();
    assert_eq!(summaries, ["edit y and a", "add src"]);
    let files: Vec<&str> = v.entries[1].files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(files, ["src/x.txt", "src/y.txt"]);
    assert_eq!(v.entries[0].files.len(), 1, "a.txt is outside the folder");
    assert_eq!(st.diff.as_ref().map(|(t, _)| t.clone()), Some(DiffTarget::Commit(v.entries[0].oid, "src/y.txt".into())));
    let _ = std::fs::remove_dir_all(&dir);
}
