//! Headless tests for next/previous change across files: the list a diff came from, landing on
//! the first or last change, passing over files with nothing to land on, the ends, and what
//! cancels a pending step.

use super::diff_tests::three_hunks;
use super::tests::{drive, git, state};
use super::update;
use crate::effect::Effect;
use crate::msg::DiffMsg;
use crate::msg::HistoryMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::SelectionMsg;
use crate::msg::TabsMsg;
use crate::runner;
use crate::state::diff_nav::{self, Landing, Step, StepKind};
use crate::state::{AppState, Selection};
use git2::Oid;
use glowfly_core::diff_rows::header_rows;
use glowfly_core::repo::{DiffTarget, FileDiff, FileEntry};
use glowfly_core::settings::{DiffLayout, DiffMode};
use glowfly_core::whole_file::change_rows;
use std::path::PathBuf;
use std::time::Instant;

pub(super) fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

pub(super) fn unstaged(p: &str) -> DiffTarget {
    DiffTarget::Unstaged(p.into())
}

/// 20 lines, with the given line numbers changed.
fn lines(changed: &[usize]) -> String {
    (1..=20).map(|i| if changed.contains(&i) { format!("x{i}\n") } else { format!("{i}\n") }).collect()
}

/// a.txt, c.txt and d.txt (20 lines) and b.bin committed; then a.txt edited at lines 2 and 18
/// (two hunks), c.txt at line 5, b.bin rewritten, and d.txt edited at line 10 and staged.
/// Opened with the WIP selected: unstaged a.txt, b.bin, c.txt; staged d.txt.
pub(super) fn nav_repo(name: &str) -> (AppState, PathBuf) {
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-app-test-nav-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    for (key, value) in [("user.name", "Test"), ("user.email", "test@example.com"), ("commit.gpgsign", "false"), ("core.autocrlf", "false")]
    {
        git(&dir, &["config", key, value]);
    }
    let write = |f: &str, t: &[u8]| std::fs::write(dir.join(f), t).unwrap();
    for f in ["a.txt", "c.txt", "d.txt"] {
        write(f, lines(&[]).as_bytes());
    }
    write("b.bin", b"\x00\x01binary\x00");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    write("a.txt", lines(&[2, 18]).as_bytes());
    write("c.txt", lines(&[5]).as_bytes());
    write("b.bin", b"\x00\x02changed\x00");
    write("d.txt", lines(&[10]).as_bytes());
    git(&dir, &["add", "d.txt"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::SelectWip));
    let snap = st.snap.as_ref().unwrap();
    let paths = |fs: &[FileEntry]| fs.iter().map(|f| f.path.clone()).collect::<Vec<_>>();
    assert_eq!(paths(&snap.unstaged), ["a.txt", "b.bin", "c.txt"]);
    assert_eq!(paths(&snap.staged), ["d.txt"]);
    (st, dir)
}

pub(super) fn shown(st: &AppState) -> Option<&DiffTarget> {
    st.session.diff.view.as_ref().map(|(t, _)| t)
}

pub(super) fn diff(st: &AppState) -> &FileDiff {
    &st.session.diff.view.as_ref().expect("a diff is open").1
}

/// Runs every effect in `fx` and the messages they lead to, as the app would.
pub(super) fn finish(st: &mut AppState, fx: Vec<Effect>) {
    for e in fx {
        if let Some(m) = runner::testing::execute(e) {
            drive(st, m);
        }
    }
}

/// The one diff load in `fx` (line counts asked for beside it aside): its target and reload flag.
pub(super) fn one_load(fx: &[Effect]) -> (&DiffTarget, bool) {
    let fx: Vec<&Effect> = fx.iter().filter(|e| !matches!(e, Effect::LineCounts(_))).collect();
    match fx.as_slice() {
        [Effect::LoadDiff { target, reload, .. }] => (target, *reload),
        other => panic!("expected one diff load, got {other:?}"),
    }
}

/// Steps within the open file to `row`: no effects, and the cursor follows.
fn step_in_file(st: &mut AppState, dir: isize, row: usize) {
    assert!(run(st, Msg::Diff(DiffMsg::JumpToChange(dir))).is_empty(), "a step inside the file");
    assert_eq!(st.view.diff_scroll_to_row, Some(row));
    assert_eq!(st.view.diff_cursor, Some(row));
}

/// Opens `target` (a two-hunk a.txt) and steps to its last change; returns its header rows.
pub(super) fn at_end_of(st: &mut AppState, target: DiffTarget) -> Vec<usize> {
    drive(st, Msg::Repo(RepoMsg::OpenDiff(target)));
    let headers = header_rows(diff(st), st.settings.diff_layout);
    assert_eq!(headers.len(), 2);
    step_in_file(st, 1, headers[1]);
    headers
}

/// The commit at row `i` of the graph (0 is the newest).
fn commit(st: &AppState, i: usize) -> Oid {
    st.snap.as_ref().unwrap().commits[i].oid
}

#[test]
fn next_at_last_change_opens_next_file_on_first_change() {
    let (mut st, dir) = nav_repo("next");
    at_end_of(&mut st, unstaged("a.txt"));

    let fx = run(&mut st, Msg::Diff(DiffMsg::JumpToChange(1)));
    assert_eq!(one_load(&fx), (&unstaged("b.bin"), false));
    assert_eq!(st.view.diff_landing, Some((unstaged("b.bin"), Landing::First)));
    // b.bin has nothing to land on: c.txt is requested instead, and b.bin never shows.
    let [e] = <[Effect; 1]>::try_from(fx).unwrap();
    let fx = run(&mut st, runner::testing::execute(e).expect("diff result"));
    assert_eq!(one_load(&fx), (&unstaged("c.txt"), false));
    assert_eq!(shown(&st), Some(&unstaged("a.txt")), "the skipped file is never shown");
    assert_eq!(st.hunk_ops_gen(), None, "hunk operations wait for the new diff");
    finish(&mut st, fx);
    assert_eq!(shown(&st), Some(&unstaged("c.txt")));
    assert_eq!(st.view.diff_scroll_to_row, Some(0));
    assert_eq!(st.view.diff_landing, None);
    assert_eq!(st.view.diff_cursor, Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn last_unstaged_continues_to_first_staged() {
    let (mut st, dir) = nav_repo("staged");
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(unstaged("c.txt"))));
    drive(&mut st, Msg::Diff(DiffMsg::JumpToChange(1)));
    assert_eq!(shown(&st), Some(&DiffTarget::Staged("d.txt".into())));
    assert_eq!(st.view.diff_scroll_to_row, Some(0));

    drive(&mut st, Msg::Diff(DiffMsg::JumpToChange(-1)));
    assert_eq!(shown(&st), Some(&unstaged("c.txt")));
    let last = *header_rows(diff(&st), st.settings.diff_layout).last().unwrap();
    assert_eq!((st.view.diff_scroll_to_row, st.view.diff_cursor), (Some(last), Some(last)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn previous_lands_on_last_change() {
    for layout in [DiffLayout::Inline, DiffLayout::Split] {
        let (mut st, dir) = nav_repo(&format!("prev-{layout:?}"));
        st.settings.diff_layout = layout;
        drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(unstaged("c.txt"))));
        assert_eq!(st.session.tab_view.diff_top_row, 0);
        drive(&mut st, Msg::Diff(DiffMsg::JumpToChange(-1)));
        assert_eq!(shown(&st), Some(&unstaged("a.txt")), "{layout:?}: b.bin is passed over");
        let headers = header_rows(diff(&st), layout);
        assert_eq!(st.view.diff_scroll_to_row, Some(headers[1]), "{layout:?}");
        assert_eq!(st.view.diff_cursor, Some(headers[1]), "{layout:?}");
        step_in_file(&mut st, -1, headers[0]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn whole_file_mode_landing() {
    let (mut st, dir) = nav_repo("whole");
    st.settings.diff_mode = DiffMode::WholeFile;
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(unstaged("c.txt"))));
    assert!(diff(&st).whole.is_some());
    let c_rows = change_rows(diff(&st), DiffMode::WholeFile, st.settings.diff_layout, &st.session.tab_view.diff_expanded);

    drive(&mut st, Msg::Diff(DiffMsg::JumpToChange(-1)));
    assert_eq!(shown(&st), Some(&unstaged("a.txt")));
    let a_rows = change_rows(diff(&st), DiffMode::WholeFile, st.settings.diff_layout, &st.session.tab_view.diff_expanded);
    assert_eq!(a_rows.len(), 2);
    assert_eq!(st.view.diff_scroll_to_row, a_rows.last().copied());
    assert_ne!(a_rows.last(), header_rows(diff(&st), st.settings.diff_layout).last(), "change block rows, not hunk headers");

    drive(&mut st, Msg::Diff(DiffMsg::JumpToChange(1)));
    assert_eq!(shown(&st), Some(&unstaged("c.txt")));
    assert_eq!((st.view.diff_scroll_to_row, st.view.diff_cursor), (Some(c_rows[0]), Some(c_rows[0])));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_wrap_at_either_end() {
    let (mut st, dir) = nav_repo("ends");
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Staged("d.txt".into()))));
    let top = st.session.tab_view.diff_top_row;
    assert!(run(&mut st, Msg::Diff(DiffMsg::JumpToChange(1))).is_empty());
    assert_eq!(shown(&st), Some(&DiffTarget::Staged("d.txt".into())));
    assert_eq!(st.session.tab_view.diff_top_row, top);
    assert_eq!(diff_nav::step(&st, 1), Step::End);

    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(unstaged("a.txt"))));
    assert_eq!(st.session.tab_view.diff_top_row, 0);
    assert!(run(&mut st, Msg::Diff(DiffMsg::JumpToChange(-1))).is_empty());
    assert_eq!(diff_nav::step(&st, -1).kind(), StepKind::End);
    assert_eq!(diff_nav::step(&st, 1).kind(), StepKind::Change);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn conflicted_files_are_skipped() {
    let (mut st, dir) = nav_repo("conflicted");
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged.retain(|f| f.path != "b.bin");
    snap.unstaged.insert(1, FileEntry::new("b2.txt", '!'));
    assert_eq!(diff_nav::next_file(&st, &unstaged("a.txt"), 1), Some(unstaged("c.txt")));
    assert_eq!(diff_nav::next_file(&st, &unstaged("c.txt"), -1), Some(unstaged("a.txt")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn binary_last_in_direction_is_shown() {
    let (mut st, dir) = nav_repo("binary-last");
    let snap = st.snap.as_mut().unwrap();
    snap.unstaged.retain(|f| f.path == "a.txt" || f.path == "b.bin");
    snap.staged.clear();
    let headers = at_end_of(&mut st, unstaged("a.txt"));
    drive(&mut st, Msg::Diff(DiffMsg::JumpToChange(1)));
    assert_eq!(shown(&st), Some(&unstaged("b.bin")));
    assert!(diff(&st).binary);
    assert_eq!([diff_nav::step(&st, -1).kind(), diff_nav::step(&st, 1).kind()], [StepKind::File, StepKind::End]);
    drive(&mut st, Msg::Diff(DiffMsg::JumpToChange(-1)));
    assert_eq!(shown(&st), Some(&unstaged("a.txt")), "the arrows step back from the binary file");
    assert_eq!(st.view.diff_scroll_to_row, Some(headers[1]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn presses_while_loading_are_ignored() {
    let (mut st, dir) = nav_repo("loading");
    pending_step(&mut st);
    let (top, landing) = (st.session.tab_view.diff_top_row, st.view.diff_landing.clone());
    assert!(run(&mut st, Msg::Diff(DiffMsg::JumpToChange(1))).is_empty());
    assert!(run(&mut st, Msg::Diff(DiffMsg::JumpToChange(-1))).is_empty());
    assert_eq!((st.session.tab_view.diff_top_row, st.view.diff_landing.clone()), (top, landing));
    let _ = std::fs::remove_dir_all(&dir);
}

/// a.txt open on its last change, with a step to b.bin pending; returns the held load.
pub(super) fn pending_step(st: &mut AppState) -> Effect {
    at_end_of(st, unstaged("a.txt"));
    let fx = run(st, Msg::Diff(DiffMsg::JumpToChange(1)));
    assert!(st.view.diff_landing.is_some());
    let [e] = <[Effect; 1]>::try_from(fx).expect("one diff load");
    e
}

#[test]
fn landing_dropped_by_other_open_and_close() {
    let (mut st, dir) = nav_repo("cancel");
    let stale = pending_step(&mut st);
    let fx = run(&mut st, Msg::Repo(RepoMsg::OpenDiff(unstaged("c.txt"))));
    assert_eq!(st.view.diff_landing, None, "a click on another file cancels the step");
    run(&mut st, runner::testing::execute(stale).unwrap());
    assert_eq!(shown(&st), Some(&unstaged("a.txt")), "the stale b.bin result is ignored");
    finish(&mut st, fx);
    assert_eq!(shown(&st), Some(&unstaged("c.txt")));
    let row = super::diff::opening_row(diff(&st), &st.settings, None, &st.session.tab_view.diff_expanded);
    assert_eq!((st.view.diff_scroll_to_row, st.view.diff_cursor), (Some(row), None));

    pending_step(&mut st);
    run(&mut st, Msg::Repo(RepoMsg::CloseDiff));
    assert_eq!(st.view.diff_landing, None);

    pending_step(&mut st);
    let oid = commit(&st, 0);
    run(&mut st, Msg::Repo(RepoMsg::SelectCommit(oid)));
    assert_eq!(st.view.diff_landing, None);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The nav repo with a.txt, c.txt and d.txt's changes committed; returns that commit.
fn committed(name: &str) -> (AppState, PathBuf, Oid) {
    let (mut st, dir) = nav_repo(name);
    git(&dir, &["add", "a.txt", "c.txt", "d.txt"]);
    git(&dir, &["commit", "-q", "-m", "second"]);
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    let oid = commit(&st, 0);
    (st, dir, oid)
}

#[test]
fn commit_file_list() {
    let (mut st, dir, oid) = committed("commit-list");
    drive(&mut st, Msg::Repo(RepoMsg::SelectCommit(oid)));
    let files: Vec<&str> = st.session.detail.as_ref().expect("details loaded").files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(files, ["a.txt", "c.txt", "d.txt"]);
    at_end_of(&mut st, DiffTarget::Commit(oid, "a.txt".into()));
    let fx = run(&mut st, Msg::Diff(DiffMsg::JumpToChange(1)));
    assert_eq!(one_load(&fx).0, &DiffTarget::Commit(oid, "c.txt".into()));
    finish(&mut st, fx);
    assert_eq!(shown(&st), Some(&DiffTarget::Commit(oid, "c.txt".into())));

    // Details of another commit are not this diff's list.
    st.session.detail.as_mut().unwrap().oid = Oid::from_bytes(&[7; 20]).unwrap();
    assert!(run(&mut st, Msg::Diff(DiffMsg::JumpToChange(1))).is_empty());
    assert_eq!(diff_nav::step(&st, 1), Step::End);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn comparison_file_list() {
    let (mut st, dir, newer) = committed("compare-list");
    let older = commit(&st, 1);
    drive(&mut st, Msg::Repo(RepoMsg::SelectCommit(newer)));
    drive(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(older)));
    let Selection::Commits(m) = &st.session.selection else { panic!("two commits selected") };
    assert_eq!(m.compare_ends(), Some((older, newer)));
    assert!(matches!(&m.changes, Some(Ok(c)) if c.files.len() == 3), "a, c and d differ");
    at_end_of(&mut st, DiffTarget::Between(older, newer, "a.txt".into()));
    let fx = run(&mut st, Msg::Diff(DiffMsg::JumpToChange(1)));
    assert_eq!(one_load(&fx).0, &DiffTarget::Between(older, newer, "c.txt".into()));
    finish(&mut st, fx);
    assert_eq!(shown(&st), Some(&DiffTarget::Between(older, newer, "c.txt".into())));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_history_stays_single_file() {
    let (mut st, dir, oid) = committed("history");
    // The commit's details are loaded too, so only the history keeps the step in the file.
    drive(&mut st, Msg::Repo(RepoMsg::SelectCommit(oid)));
    drive(&mut st, Msg::History(HistoryMsg::OpenFileHistory { path: "a.txt".into(), at: None }));
    assert!(st.session.history.view.is_some());
    assert_eq!(shown(&st), Some(&DiffTarget::Commit(oid, "a.txt".into())));
    let headers = header_rows(diff(&st), st.settings.diff_layout);
    step_in_file(&mut st, 1, headers[1]);
    assert!(run(&mut st, Msg::Diff(DiffMsg::JumpToChange(1))).is_empty());
    assert_eq!(diff_nav::step(&st, 1), Step::End);
    let _ = std::fs::remove_dir_all(&dir);
}

fn synthetic() -> AppState {
    let mut st = state();
    st.session.diff.view = Some((unstaged("f.txt"), three_hunks().into()));
    st
}

#[test]
fn cursor_survives_clamped_scroll() {
    let mut st = synthetic();
    st.view.diff_visible_rows = 40;
    let headers = header_rows(&three_hunks(), DiffLayout::Inline);
    step_in_file(&mut st, 1, headers[1]);
    step_in_file(&mut st, 1, headers[2]);
    // The view couldn't scroll the last hunk to the top: the top row stays above it.
    st.session.tab_view.diff_top_row = headers[1] - 1;
    step_in_file(&mut st, -1, headers[1]);

    // Scrolled past the cursor: steps continue from the top row instead.
    st.view.diff_cursor = Some(headers[0]);
    st.session.tab_view.diff_top_row = headers[1];
    step_in_file(&mut st, 1, headers[2]);
}

#[test]
fn layout_switch_clears_cursor() {
    let mut st = synthetic();
    run(&mut st, Msg::Diff(DiffMsg::JumpToChange(1)));
    assert!(st.view.diff_cursor.is_some());
    run(&mut st, Msg::Diff(DiffMsg::SetDiffLayout(DiffLayout::Split)));
    assert_eq!(st.view.diff_cursor, None);
    run(&mut st, Msg::Diff(DiffMsg::JumpToChange(-1)));
    assert!(st.view.diff_cursor.is_some());
    run(&mut st, Msg::Diff(DiffMsg::SetDiffMode(DiffMode::WholeFile)));
    assert_eq!(st.view.diff_cursor, None);
    st.session.tab_view.diff_top_row = 0;
    run(&mut st, Msg::Diff(DiffMsg::JumpToChange(1)));
    assert!(st.view.diff_cursor.is_some());
    run(&mut st, Msg::Diff(DiffMsg::SetDiffMode(DiffMode::Hunks)));
    assert_eq!(st.view.diff_cursor, None);
}

#[test]
fn tab_park_clears_landing() {
    let (mut st, dir) = nav_repo("park");
    pending_step(&mut st);
    run(&mut st, Msg::Tabs(TabsMsg::NewTab));
    run(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    assert_eq!(st.view.diff_landing, None);
    assert_eq!(shown(&st), Some(&unstaged("a.txt")));
    let headers = header_rows(diff(&st), st.settings.diff_layout);
    step_in_file(&mut st, -1, headers[0]);
    let _ = std::fs::remove_dir_all(&dir);
}
