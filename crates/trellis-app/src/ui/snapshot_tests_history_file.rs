//! The history's File view while finding, with a long line wrapped and scrolled sideways, for a
//! commit that deletes the file and for a Git LFS pointer in an untrusted repository; and the
//! folder history's file buttons and a moved folder's row. Shares the fixtures of
//! `snapshot_tests_history`.

use super::file_menu::{NOT_IN_WORKDIR, OPEN_DEFAULT};
use super::find_bar::{FIND_HINT, LINE_HINT};
use super::history_file::WRAP_LINES;
use super::snapshot_tests::{git, harness, remove_fixture, with_repo};
use super::snapshot_tests_history::{commit, with_file_view, with_folder};
use crate::msg::{DiffMsg, HistoryMsg, Msg};
use crate::state::AppState;
use crate::state::blame_find::search;
use crate::state::history::{FileHistoryView, FileText, FileTextData, HistoryKind, HistoryPane, TextState};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{By, Queryable};
use std::path::PathBuf;
use std::sync::Arc;
use trellis_core::highlight::{self, SyntaxHint};
use trellis_core::repo::{self, FileAtOptions, HistoryStart, LfsState, LoadOptions};
use trellis_core::settings::ThemeChoice;

/// The File view's text replaced by `lines` (highlighted as Rust) with `lfs`.
pub(super) fn set_text(st: &mut AppState, lines: Vec<String>, lfs: LfsState) {
    let view = st.history.as_mut().unwrap();
    let text = view.text.as_mut().unwrap();
    let spans = highlight::highlight_file(SyntaxHint { path: "lib.rs", language: None }, &lines);
    text.state = TextState::Ready(Arc::new(FileTextData { lines, spans, lfs, preview: None }));
}

/// The opened version with a long comment line in the middle.
fn with_long_line(name: &str, wrap: bool) -> (AppState, PathBuf) {
    let (mut st, dir) = with_file_view(name, ThemeChoice::Dark);
    let TextState::Ready(d) = &st.history.as_ref().unwrap().text.as_ref().unwrap().state else { panic!("text ready") };
    let mut lines = d.lines.clone();
    let long = "    // Words are split on any run of whitespace, so tabs, several spaces and a final newline all \
                count as one break between two words, and a line made only of spaces has no words at all.";
    lines.insert(4, long.to_owned());
    set_text(&mut st, lines, LfsState::None);
    st.settings.diff_word_wrap = wrap;
    (st, dir)
}

#[test]
fn file_history_file_find() {
    let (mut st, dir) = with_file_view("history-file-find", ThemeChoice::Dark);
    let nav = &mut st.history.as_mut().unwrap().nav;
    nav.find.query = "words".into();
    let TextState::Ready(d) = &st.history.as_ref().unwrap().text.as_ref().unwrap().state else { panic!("text ready") };
    let lines = d.lines.clone();
    assert_eq!(search(&lines, "words").len(), 4);
    let nav = &mut st.history.as_mut().unwrap().nav;
    let line = nav.find.refresh(&lines, 2).unwrap();
    nav.land_on(line);
    let mut h = harness(st, true);
    h.run();
    h.run();
    h.check_snapshot("file_history_file_find");
    assert!(h.query_by_label("3 of 4").is_some());
    assert!(h.query_by_label(FIND_HINT).is_some() && h.query_by_label(LINE_HINT).is_some());
    h.get(By::new().role(Role::TextInput).value("words")).type_text("x");
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::History(HistoryMsg::FileFindChanged))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn file_history_file_wrap() {
    let (st, dir) = with_long_line("history-file-wrap", true);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("file_history_file_wrap");
    h.get_by_label(WRAP_LINES).click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Diff(DiffMsg::SetWordWrap(false)))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn file_history_file_hscroll() {
    let (st, dir) = with_long_line("history-file-hscroll", false);
    let mut h = harness(st, true);
    h.run();
    // Over the lines, so the scroll bars show.
    h.input_mut().events.push(egui::Event::PointerMoved(egui::pos2(820.0, 600.0)));
    h.run();
    h.check_snapshot("file_history_file_hscroll");
    h.get_by_label(WRAP_LINES).click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Diff(DiffMsg::SetWordWrap(true)))));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn file_history_file_deleted() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "history-file-deleted");
    let text = "pub fn words(line: &str) -> Vec<&str> {\n    line.split_whitespace().collect()\n}\n";
    commit(&dir, &[("words.rs", text)], "Add word splitting", "2026-01-11T10:00:00+00:00");
    let d = "2026-01-12T10:00:00+00:00";
    git(&dir, &["rm", "-q", "words.rs"], d);
    git(&dir, &["commit", "-q", "-m", "Drop word splitting"], d);
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let h = repo::file_history(&dir, "words.rs", &HistoryStart::Head, 500).unwrap();
    let mut view = FileHistoryView::loading("words.rs".into(), None);
    view.append(h);
    view.selected = Some(0);
    view.pane = HistoryPane::File;
    let (oid, _, status) = view.shown().unwrap();
    assert_eq!(status, 'D');
    let f = repo::file_at_with(&dir, oid, "words.rs", FileAtOptions { before: true, smudge: false }).unwrap();
    let spans = highlight::highlight_file(SyntaxHint { path: "words.rs", language: None }, &f.lines);
    let data = Arc::new(FileTextData { lines: f.lines, spans, lfs: f.lfs, preview: None });
    view.text = Some(FileText { oid, path: "words.rs".into(), before: true, state: TextState::Ready(data) });
    st.history = Some(view);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("file_history_file_deleted");
    assert!(h.get_by_label("Blame").is_disabled(), "nothing to blame in a deleting commit");
    assert!(!h.get_by_label("Diff").is_disabled() && !h.get_by_label("File").is_disabled(), "only Blame is off");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn file_history_file_lfs_pointer_light() {
    let (mut st, dir) = with_file_view("history-file-lfs", ThemeChoice::Light);
    let pointer = [
        "version https://git-lfs.github.com/spec/v1",
        "oid sha256:4d7a214614ab2935c943f9e0ff69d22eadbb8f32b1258daaa5e2ca24d17e2393",
        "size 18204",
    ];
    set_text(&mut st, pointer.map(String::from).to_vec(), LfsState::Pointer);
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("file_history_file_lfs_pointer_light");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn folder_history_file_buttons() {
    let (mut st, dir) = with_folder("history-folder-buttons", ThemeChoice::Dark);
    st.history.as_mut().unwrap().absent.insert("src/lib.rs".into());
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("folder_history_file_buttons");
    assert!(h.get_by_role_and_label(Role::Button, OPEN_DEFAULT).is_disabled(), "{NOT_IN_WORKDIR}");
    assert!(!h.get_by_role_and_label(Role::Button, "Restore this version…").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn folder_history_moved() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "history-folder-moved");
    let a = "pub fn trim(s: &str) -> &str {\n    s.trim()\n}\n";
    let b = "pub enum Token {\n    Word(String),\n    Space,\n}\n";
    commit(&dir, &[("src/util.rs", a), ("src/lexer.rs", b)], "Add the lexer", "2026-01-11T10:00:00+00:00");
    std::fs::create_dir_all(dir.join("crates")).unwrap();
    let d = "2026-01-12T10:00:00+00:00";
    git(&dir, &["mv", "src", "crates/text"], d);
    git(&dir, &["commit", "-q", "-m", "Move the lexer into a crate"], d);
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let h = repo::folder_history(&dir, "crates/text", &HistoryStart::Head, 500).unwrap();
    let mut view = FileHistoryView::loading_kind("crates/text".into(), None, HistoryKind::Folder);
    view.append(h);
    assert_eq!(view.entries.iter().map(|e| e.status).collect::<String>(), "RA");
    view.selected = Some(0);
    st.history = Some(view);
    let mut h = harness(st, true);
    h.run_steps(4);
    h.check_snapshot("folder_history_moved");
    drop(h);
    remove_fixture(&dir);
}
