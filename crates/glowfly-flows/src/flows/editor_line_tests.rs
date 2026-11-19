//! Open in external editor at the line the diff or the file history shows: the header's line,
//! selected lines coming first, diffs without rows, the whole-file and staged diffs, a file row
//! opening at the open diff's line, the file history's panes and the row menu's working file.

use super::diff_header_tests::{fixture, open};
use super::tests::{drive, scratch_repo, state};
use super::tools_tests::{launch, open_file, run};
use glowfly_core::native_path::native;
use glowfly_core::repo::DiffTarget;
use glowfly_core::settings::DiffMode;
use glowfly_model::msg::{DiffMsg, HistoryMsg, Msg, RepoMsg};
use glowfly_model::state::diff_header::{self, EditorAction};
use glowfly_model::state::diff_text::{LineRef, TextPos, TextSel, TextSide, Unit};
use glowfly_model::state::history::HistoryPane;
use glowfly_model::state::lines::LineClick;
use glowfly_model::state::tools::editor_line;
use glowfly_model::state::tools::editor_setup;
use glowfly_model::state::{AppState, diff_nav};
use glowfly_model::tools::ToolKind;
use std::ffi::OsString;

fn editor_line_of(st: &AppState) -> Option<u32> {
    diff_header::model(st).and_then(|m| m.editor).and_then(|e| e.line)
}

/// The index of the first line of `hunk` in the open diff with origin `origin`.
fn index_of(st: &AppState, hunk: usize, origin: char) -> usize {
    let (_, d) = st.session.diff.view.as_ref().expect("a diff is open");
    d.hunks[hunk].lines.iter().position(|l| l.origin == origin).expect("such a line")
}

fn text_in(st: &AppState, hunk: usize, line: usize) -> TextSel {
    let key = (st.session.diff.view.as_ref().unwrap().0.clone(), st.session.diff.shown_gen);
    let at = TextPos { line: LineRef::Hunk { hunk, line }, col: 0 };
    TextSel { anchor: at, head: TextPos { col: 2, ..at }, unit: Unit::Char, side: TextSide::Inline, key }
}

#[test]
fn the_header_opens_at_the_change_on_screen() {
    let (mut st, dir) = fixture("editor-header");
    let m = open(&mut st, DiffTarget::Unstaged("src/app.txt".into()));
    assert_eq!(m.editor, Some(EditorAction { setup: Some(editor_setup(st.settings.lang())), line: Some(2) }));
    st.settings.external_tools.editor = "ed".into();
    assert_eq!(diff_header::model(&st).unwrap().editor, Some(EditorAction { setup: None, line: Some(2) }));

    let rows = diff_nav::shown_change_rows(&st);
    assert_eq!(rows.len(), 2);
    st.view.tab.diff_top_row = rows[1];
    st.view.diff_visible_rows = 40;
    assert_eq!(editor_line_of(&st), Some(18), "the change at the top of the screen");

    // The change last stepped to, while it is on screen, counts instead of the top row.
    st.view.tab.diff_top_row = 0;
    st.cues.diff_cursor = Some(rows[1]);
    assert_eq!(editor_line_of(&st), Some(18));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn selected_lines_come_first_in_the_header() {
    let (mut st, dir) = fixture("editor-selected");
    open(&mut st, DiffTarget::Unstaged("src/app.txt".into()));
    st.view.diff_visible_rows = 40;
    let added = index_of(&st, 1, '+');
    drive(&mut st, Msg::Diff(DiffMsg::SelectDiffLine { hunk: 1, line: added, how: LineClick::Only }));
    assert_eq!(editor_line_of(&st), Some(18), "the top row is 0, the selection is in the second hunk");

    // Text selected in the first hunk's second added line (line 3) is above the gutter's line.
    let second_added = index_of(&st, 0, '+') + 1;
    st.view.diff_text = Some(text_in(&st, 0, second_added));
    assert_eq!(editor_line_of(&st), Some(3));
    // Text of an earlier load of the diff is ignored.
    let mut stale = text_in(&st, 0, second_added);
    stale.key.1 += 1;
    st.view.diff_text = Some(stale);
    assert_eq!(editor_line_of(&st), Some(18));

    drive(&mut st, Msg::Diff(DiffMsg::ClearLineSelection));
    st.view.diff_text = None;
    assert_eq!(editor_line_of(&st), Some(2));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn views_without_rows_give_no_line() {
    let (mut st, dir) = fixture("editor-no-rows");
    let m = open(&mut st, DiffTarget::Unstaged("gone.txt".into()));
    assert_eq!(m.editor.map(|e| e.line), Some(None), "a file deleted outright has no line");
    assert!(m.actions.open.unwrap().missing.is_some());

    let head = st.snap.as_ref().unwrap().head_oid.unwrap();
    let m = open(&mut st, DiffTarget::Commit(head, "logo.png".into()));
    assert_eq!(m.editor.and_then(|e| e.line), None, "a binary file");

    let parent = st.snap.as_ref().unwrap().commits[1].oid;
    let m = open(&mut st, DiffTarget::Between(parent, head, "staged.txt".into()));
    assert_eq!(m.editor, None, "a comparison of two commits has no working file");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn whole_file_and_staged_diffs_give_the_new_side() {
    let (mut st, dir) = fixture("editor-whole");
    let m = open(&mut st, DiffTarget::Staged("staged.txt".into()));
    assert_eq!(m.editor.and_then(|e| e.line), Some(3), "the added line, by the index's numbers");

    st.settings.diff_mode = DiffMode::WholeFile;
    let m = open(&mut st, DiffTarget::Unstaged("src/app.txt".into()));
    assert_eq!(m.editor.and_then(|e| e.line), Some(2), "the first change of the whole file");
    // The third line of the file is its second added line, at index 4 of the whole file.
    let (_, d) = st.session.diff.view.as_ref().unwrap();
    let at = d.whole.as_ref().expect("the whole file loaded").diff.hunks[0].lines.iter().rposition(|l| l.new == Some(3) && l.origin == '+');
    let key = (st.session.diff.view.as_ref().unwrap().0.clone(), st.session.diff.shown_gen);
    let pos = TextPos { line: LineRef::Whole { line: at.unwrap() }, col: 0 };
    st.view.diff_text = Some(TextSel { anchor: pos, head: TextPos { col: 2, ..pos }, unit: Unit::Char, side: TextSide::Inline, key });
    assert_eq!(editor_line_of(&st), Some(3));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_row_opens_at_the_open_diffs_line() {
    let (mut st, dir) = fixture("editor-row");
    st.settings.external_tools.editor = "ed".into();
    st.settings.external_tools.editor_file_args = "{path}:{line}".into();
    let w = st.workdir().unwrap();
    let at = |file: &str, line: u32| vec![OsString::from(format!("{}:{line}", native(&w.join(file)).display()))];
    let args = |st: &mut AppState, file: &str, line: Option<u32>| launch(run(st, open_file(file, line)), ToolKind::Editor).0.args;

    open(&mut st, DiffTarget::Unstaged("src/app.txt".into()));
    assert_eq!(args(&mut st, "src/app.txt", None), at("src/app.txt", 2), "the file on screen opens at its line");
    assert_eq!(args(&mut st, "staged.txt", None), at("staged.txt", 1), "another file opens at the top");
    assert_eq!(args(&mut st, "src/app.txt", Some(7)), at("src/app.txt", 7), "an explicit line wins");

    drive(&mut st, Msg::Repo(RepoMsg::CloseDiff));
    assert_eq!(args(&mut st, "src/app.txt", None), at("src/app.txt", 1), "no diff is open");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_file_history_gives_its_panes_line() {
    let dir = scratch_repo("editor-history");
    glowfly_core::testkit::git(&dir, &["commit", "-q", "-am", "edit a"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    st.settings.external_tools.editor = "ed".into();
    st.settings.external_tools.editor_file_args = "{path}:{line}".into();
    drive(&mut st, Msg::History(HistoryMsg::OpenFileHistory { path: "a.txt".into(), at: None }));
    let w = st.workdir().unwrap();
    let at = |file: &str, line: u32| vec![OsString::from(format!("{}:{line}", native(&w.join(file)).display()))];
    let args = |st: &mut AppState, file: &str| launch(run(st, open_file(file, None)), ToolKind::Editor).0.args;

    drive(&mut st, Msg::History(HistoryMsg::SetHistoryPane(HistoryPane::File)));
    let hv = |st: &mut AppState| st.session.history.view.as_mut().unwrap().nav.clone();
    let mut nav = hv(&mut st);
    nav.cursor = Some(1);
    st.session.history.view.as_mut().unwrap().nav = nav.clone();
    assert_eq!(args(&mut st, "a.txt"), at("a.txt", 2), "the cursor line");
    nav.cursor = None;
    nav.top = 0;
    st.session.history.view.as_mut().unwrap().nav = nav;
    assert_eq!(args(&mut st, "a.txt"), at("a.txt", 1), "the first line on screen");
    assert_eq!(args(&mut st, "b.txt"), at("b.txt", 1), "another file than the history's");

    drive(&mut st, Msg::History(HistoryMsg::SetHistoryPane(HistoryPane::Diff)));
    assert_eq!(args(&mut st, "a.txt"), at("a.txt", 2), "the diff's first changed line");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_row_menus_working_file() {
    let (mut st, dir) = fixture("editor-working-file");
    assert!(editor_line::working_file(&st).is_none(), "no diff open");
    open(&mut st, DiffTarget::Unstaged("src/app.txt".into()));
    let f = editor_line::working_file(&st).unwrap();
    assert_eq!((f.path.as_str(), f.missing), ("src/app.txt", None));
    assert!(f.setup.is_some(), "no editor is set");
    open(&mut st, DiffTarget::Unstaged("gone.txt".into()));
    let f = editor_line::working_file(&st).unwrap();
    assert_eq!((f.path.as_str(), f.missing.is_some()), ("gone.txt", true));

    let head = st.snap.as_ref().unwrap().head_oid.unwrap();
    let parent = st.snap.as_ref().unwrap().commits[1].oid;
    open(&mut st, DiffTarget::Between(parent, head, "staged.txt".into()));
    assert!(editor_line::working_file(&st).is_none());
    let _ = std::fs::remove_dir_all(&dir);

    let dir = scratch_repo("editor-history-file");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::History(HistoryMsg::OpenFileHistory { path: "a.txt".into(), at: None }));
    let f = editor_line::working_file(&st).unwrap();
    assert_eq!((f.path.as_str(), f.missing), ("a.txt", None), "the file's name today");
    let _ = std::fs::remove_dir_all(&dir);
}
