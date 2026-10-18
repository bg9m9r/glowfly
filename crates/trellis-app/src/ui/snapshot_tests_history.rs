//! The history's File view (a file renamed after the commit it was opened at, in syntax colors
//! with line numbers, and a binary file's note) and the folder history (dark and light). Shares
//! the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{git, harness, remove_fixture, with_repo};
use crate::msg::{HistoryMsg, Msg};
use crate::state::AppState;
use crate::state::history::{FileHistoryView, FileText, FileTextData, HistoryKind, HistoryPane, TextState};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use git2::Oid;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use trellis_core::highlight::{self, SyntaxHint};
use trellis_core::intraline;
use trellis_core::repo::{self, DiffTarget, HistoryStart, LoadOptions};
use trellis_core::settings::ThemeChoice;

const LIB_1: &str = "/// Splits a line into words.
pub fn words(line: &str) -> Vec<&str> {
    line.split(' ').collect()
}
";

const LIB_2: &str = "/// Splits a line into words, skipping runs of spaces.
pub fn words(line: &str) -> Vec<&str> {
    if line.is_empty() {
        return Vec::new();
    }
    line.split_whitespace().collect()
}

/// How many words a line has.
pub fn count(line: &str) -> usize {
    words(line).len()
}
";

const LIB_3: &str = "/// Splits a line into words, skipping runs of spaces.
pub fn words(line: &str) -> Result<Vec<&str>, String> {
    if line.is_empty() {
        return Err(\"empty line\".to_owned());
    }
    Ok(line.split_whitespace().collect())
}
";

fn head(dir: &Path) -> Oid {
    git2::Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

/// Writes `files` (making folders) and commits them on `date`.
fn commit(dir: &Path, files: &[(&str, &str)], message: &str, date: &str) -> Oid {
    for (f, text) in files {
        let path = dir.join(f);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    let mut args = vec!["add", "--"];
    args.extend(files.iter().map(|(f, _)| *f));
    git(dir, &args, date);
    git(dir, &["commit", "-q", "-m", message], date);
    head(dir)
}

/// lib.rs in three versions, then moved to src/lib.rs; the history is opened at the second
/// version, in the File view.
fn with_file_view(name: &str, theme: ThemeChoice) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    commit(&dir, &[("lib.rs", LIB_1)], "Add word splitting", "2026-01-11T10:00:00+00:00");
    let at = commit(&dir, &[("lib.rs", LIB_2)], "Skip runs of spaces and count words", "2026-01-12T10:00:00+00:00");
    commit(&dir, &[("lib.rs", LIB_3)], "Report an empty line as an error", "2026-01-13T10:00:00+00:00");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    let d = "2026-01-14T10:00:00+00:00";
    git(&dir, &["mv", "lib.rs", "src/lib.rs"], d);
    git(&dir, &["commit", "-q", "-m", "Move the library under src"], d);
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());

    let h = repo::file_history(&dir, "lib.rs", &HistoryStart::At(at), 500).unwrap();
    assert_eq!(h.head_path.as_deref(), Some("src/lib.rs"));
    let mut view = FileHistoryView::loading("lib.rs".into(), Some(at));
    view.head_path = h.head_path.clone();
    view.append(h);
    view.batches = 1;
    view.selected = view.entries.iter().position(|e| e.oid == at);
    assert_eq!(view.selected, Some(2), "the rename and the later edit are listed above it");
    view.pane = HistoryPane::File;
    let f = repo::file_at(&dir, at, "lib.rs").unwrap();
    let spans = highlight::highlight_file(SyntaxHint { path: "lib.rs", language: None }, &f.lines);
    assert!(spans.is_some(), "Rust is highlighted");
    let data = Arc::new(FileTextData { lines: f.lines, spans });
    view.text = Some(FileText { oid: at, path: "lib.rs".into(), state: TextState::Ready(data) });
    st.history = Some(view);
    (st, dir)
}

#[test]
fn file_history_file_view() {
    let (st, dir) = with_file_view("history-file-view", ThemeChoice::Dark);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("file_history_file_view");
    assert!(h.query_by_label("now src/lib.rs").is_some());
    h.get_by_label("Diff").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::History(HistoryMsg::SetHistoryPane(HistoryPane::Diff)))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn file_history_file_binary_light() {
    let (mut st, dir) = with_file_view("history-file-binary", ThemeChoice::Light);
    st.settings.diff_line_numbers = false;
    let text = st.history.as_mut().unwrap().text.as_mut().unwrap();
    text.state = TextState::Failed("Binary file not shown".into());
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("file_history_file_binary_light");
    drop(h);
    remove_fixture(&dir);
}

/// Three commits under src/ (and one outside it); the folder history is open on src with the
/// newest entry's second file selected and its diff loaded.
fn with_folder(name: &str, theme: ThemeChoice) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let util = "pub fn trim(s: &str) -> &str {\n    s.trim()\n}\n";
    commit(&dir, &[("src/lib.rs", LIB_1), ("src/util.rs", util)], "Add word splitting", "2026-01-11T10:00:00+00:00");
    commit(&dir, &[("docs/notes.md", "# Notes\n")], "Write notes", "2026-01-12T10:00:00+00:00");
    let util2 = "/// Trims both ends.\npub fn trim(s: &str) -> &str {\n    s.trim()\n}\n";
    commit(&dir, &[("src/util.rs", util2)], "Document trim", "2026-01-13T10:00:00+00:00");
    let lexer = "pub enum Token {\n    Word(String),\n    Space,\n}\n";
    commit(&dir, &[("src/lexer.rs", lexer), ("src/lib.rs", LIB_2)], "Add a lexer and count words", "2026-01-14T10:00:00+00:00");
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());

    let h = repo::folder_history(&dir, "src", &HistoryStart::Head, 500).unwrap();
    let mut view = FileHistoryView::loading_kind("src".into(), None, HistoryKind::Folder);
    view.append(h);
    view.batches = 1;
    let counts: Vec<usize> = view.entries.iter().map(|e| e.files.len()).collect();
    assert_eq!(counts, [2, 1, 2]);
    view.selected = Some(0);
    view.file = 1;
    let target = view.target().unwrap();
    assert_eq!(target, DiffTarget::Commit(view.entries[0].oid, "src/lib.rs".into()));
    let mut diff = repo::load_diff(&dir, &target).unwrap();
    diff.highlight = highlight::highlight_diff(target.path(), &diff, None);
    diff.intraline = intraline::intraline(&diff);
    st.diff = Some((target, diff.into()));
    st.history = Some(view);
    (st, dir)
}

#[test]
fn folder_history() {
    let (st, dir) = with_folder("history-folder", ThemeChoice::Dark);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("folder_history");
    h.get_by_label("lexer.rs").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::History(HistoryMsg::SelectHistoryFile(0)))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn folder_history_light() {
    let (st, dir) = with_folder("history-folder-light", ThemeChoice::Light);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("folder_history_light");
    drop(h);
    remove_fixture(&dir);
}
