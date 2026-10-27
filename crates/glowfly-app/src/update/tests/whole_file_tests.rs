//! Headless tests for the whole-file diff view: switching modes, opening on the kept hunk,
//! stepping between change blocks, hunk actions on blocks and line operations being off.

use super::diff_tests::drive_holding_diffs;
use super::tests::{drive, finish, git, scratch_repo, state};
use super::update;
use crate::effect::{Effect, LocalOp};
use crate::msg::DiffMsg;
use crate::msg::HistoryMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::StageMsg;
use crate::msg::TabsMsg;
use crate::state::history::FileHistoryView;
use crate::state::lines::LineClick::Only;
use crate::state::{AppState, WHOLE_FILE_BLOCKS_LINE_OPS};
use glowfly_core::diff_rows::header_rows;
use glowfly_core::repo::{self, DiffLine, DiffTarget, FileDiff, FileHistory, HistoryEntry, Hunk, HunkOp, LoadOptions};
use glowfly_core::settings::{DiffLayout, DiffMode};
use glowfly_core::whole_file::{Expanded, WholeFile, block_rows};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

/// Lines 1..=30 with lines 2, 15 and 28 replaced: three hunks, and the whole file when `whole`.
pub(super) fn three_changes(whole: bool) -> FileDiff {
    let ctx = |n: u32| DiffLine { origin: ' ', old: Some(n), new: Some(n), text: format!("line {n}") };
    let del = |n: u32| DiffLine { origin: '-', old: Some(n), new: None, text: format!("line {n}") };
    let add = |n: u32| DiffLine { origin: '+', old: None, new: Some(n), text: format!("changed {n}") };
    let lines = |from: u32, to: u32| -> Vec<DiffLine> {
        (from..=to).flat_map(|n| if [2, 15, 28].contains(&n) { vec![del(n), add(n)] } else { vec![ctx(n)] }).collect()
    };
    let hunk = |from: u32, to: u32| Hunk { header: format!("@@ -{from} +{from} @@"), lines: lines(from, to) };
    let hunks = vec![hunk(1, 5), hunk(12, 18), hunk(25, 30)];
    let mut d = FileDiff { binary: false, hunks, highlight: None, intraline: Default::default(), whole: None };
    if whole {
        d.whole = WholeFile::new(&d.hunks, hunk(1, 30)).map(Box::new);
        assert_eq!(d.whole.as_ref().map(|w| w.blocks.len()), Some(3));
    }
    d
}

/// The block rows of `d`'s whole file with every fold closed.
pub(super) fn blocks(d: &FileDiff, layout: DiffLayout) -> Vec<usize> {
    block_rows(d.whole.as_deref().expect("whole file"), layout, &Expanded::default())
}

/// An open repository (so diffs can be requested) with `diff` on screen as f.txt's unstaged diff.
pub(super) fn with_diff(name: &str, diff: FileDiff) -> (AppState, PathBuf) {
    let dir = scratch_repo(name);
    let mut st = state();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(100)).unwrap());
    st.session.diff.view = Some((DiffTarget::Unstaged("f.txt".into()), diff.into()));
    (st, dir)
}

/// Delivers `d` as the result of the diff request in flight.
fn arrive(st: &mut AppState, target: DiffTarget, d: FileDiff) {
    let generation = st.session.diff.generation;
    run(st, Msg::Repo(RepoMsg::DiffLoaded { target, reload: false, generation, result: Ok(d), lfs: None }));
}

#[test]
fn set_diff_mode_whole_reloads_and_keeps_hunk() {
    let (mut st, dir) = with_diff("whole-switch", three_changes(false));
    run(&mut st, Msg::Diff(DiffMsg::SelectDiffLine { hunk: 1, line: 3, how: Only }));
    assert!(!st.view.diff_lines.is_empty());
    st.session.tab_view.diff_top_row = header_rows(&three_changes(false), DiffLayout::Inline)[1];

    let fx = run(&mut st, Msg::Diff(DiffMsg::SetDiffMode(DiffMode::WholeFile)));
    assert_eq!(st.settings.diff_mode, DiffMode::WholeFile);
    assert!(
        matches!(&fx[..], [Effect::SaveSettings(s), Effect::LoadDiff { whole_file: true, reload: false, .. }]
            if s.diff_mode == DiffMode::WholeFile),
        "{fx:?}"
    );
    assert_eq!(st.view.diff_open_hunk, Some(1));
    assert!(st.view.diff_lines.is_empty());
    assert_eq!(st.hunk_ops_gen(), None, "hunk buttons wait for the whole file");
    assert!(run(&mut st, Msg::Diff(DiffMsg::SetDiffMode(DiffMode::WholeFile))).is_empty(), "no change, nothing to do");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn diff_loaded_in_whole_mode_scrolls_to_kept_block() {
    let (mut st, dir) = with_diff("whole-open", three_changes(false));
    st.session.tab_view.diff_top_row = header_rows(&three_changes(false), DiffLayout::Inline)[1] + 2;
    run(&mut st, Msg::Diff(DiffMsg::SetDiffMode(DiffMode::WholeFile)));
    let target = DiffTarget::Unstaged("f.txt".into());
    arrive(&mut st, target, three_changes(true));
    let rows = blocks(&three_changes(true), DiffLayout::Inline);
    assert_eq!(st.view.diff_scroll_to_row, Some(rows[1]));
    assert_eq!(st.session.tab_view.diff_top_row, rows[1]);
    assert_eq!(st.view.diff_open_hunk, None, "used once");
    assert!(st.hunk_ops_gen().is_some());

    // Another file opens on its first change block.
    let other = DiffTarget::Unstaged("g.txt".into());
    let fx = run(&mut st, Msg::Repo(RepoMsg::OpenDiff(other.clone())));
    assert!(matches!(&fx[..], [Effect::LoadDiff { whole_file: true, .. }]), "{fx:?}");
    arrive(&mut st, other.clone(), three_changes(true));
    assert_eq!(st.view.diff_scroll_to_row, Some(rows[0]));

    // The hunk view still opens at the top.
    st.settings.diff_mode = DiffMode::Hunks;
    let fx = run(&mut st, Msg::Repo(RepoMsg::OpenDiff(other.clone())));
    assert!(matches!(&fx[..], [Effect::LoadDiff { whole_file: false, .. }]), "{fx:?}");
    arrive(&mut st, other, three_changes(false));
    assert_eq!(st.view.diff_scroll_to_row, Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn set_diff_mode_hunks_needs_no_reload() {
    for layout in [DiffLayout::Inline, DiffLayout::Split] {
        let (mut st, dir) = with_diff("whole-back", three_changes(true));
        st.settings.diff_mode = DiffMode::WholeFile;
        st.settings.diff_layout = layout;
        st.session.tab_view.diff_top_row = blocks(&three_changes(true), layout)[2] + 3;
        let shown = st.hunk_ops_gen();
        assert!(shown.is_some());

        let fx = run(&mut st, Msg::Diff(DiffMsg::SetDiffMode(DiffMode::Hunks)));
        assert!(matches!(&fx[..], [Effect::SaveSettings(s)] if s.diff_mode == DiffMode::Hunks), "{fx:?}");
        let (_, d) = st.session.diff.view.as_ref().unwrap();
        assert!(d.whole.is_none());
        let header = header_rows(&three_changes(false), layout)[2];
        assert_eq!((st.view.diff_scroll_to_row, st.session.tab_view.diff_top_row), (Some(header), header), "{layout:?}");
        assert_eq!(st.hunk_ops_gen(), shown, "the hunks didn't change");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn jump_to_change_steps_blocks_in_whole_mode() {
    for layout in [DiffLayout::Inline, DiffLayout::Split] {
        let mut st = state();
        st.session.diff.view = Some((DiffTarget::Unstaged("f.txt".into()), three_changes(true).into()));
        st.settings.diff_mode = DiffMode::WholeFile;
        st.settings.diff_layout = layout;
        let rows = blocks(&three_changes(true), layout);
        let step = |st: &mut AppState, n: isize| {
            assert!(run(st, Msg::Diff(DiffMsg::JumpToChange(n))).is_empty());
            st.view.diff_scroll_to_row.take()
        };
        assert_eq!(step(&mut st, 1), Some(rows[0]), "{layout:?}");
        assert_eq!(step(&mut st, 1), Some(rows[1]), "{layout:?}");
        assert_eq!(step(&mut st, 1), Some(rows[2]), "{layout:?}");
        assert_eq!(step(&mut st, 1), None, "{layout:?}: no wrap");
        assert_eq!(step(&mut st, -1), Some(rows[1]), "{layout:?}");
        assert_eq!(step(&mut st, -1), Some(rows[0]), "{layout:?}");
        assert_eq!(step(&mut st, -1), None, "{layout:?}: no wrap");
    }

    // Switching the layout keeps the same block at the top.
    let mut st = state();
    st.session.diff.view = Some((DiffTarget::Unstaged("f.txt".into()), three_changes(true).into()));
    st.settings.diff_mode = DiffMode::WholeFile;
    let (inline, split) = (blocks(&three_changes(true), DiffLayout::Inline), blocks(&three_changes(true), DiffLayout::Split));
    assert_ne!(inline[1], split[1]);
    st.session.tab_view.diff_top_row = inline[1] + 1;
    run(&mut st, Msg::Diff(DiffMsg::SetDiffLayout(DiffLayout::Split)));
    assert_eq!(st.view.diff_scroll_to_row, Some(split[1]));
}

#[test]
fn line_ops_blocked_in_whole_mode() {
    let (mut st, dir) = with_diff("whole-lines", three_changes(true));
    st.settings.diff_mode = DiffMode::WholeFile;
    assert_eq!(st.line_ops_blocked(), Some(WHOLE_FILE_BLOCKS_LINE_OPS));
    assert!(run(&mut st, Msg::Diff(DiffMsg::SelectDiffLine { hunk: 0, line: 1, how: Only })).is_empty());
    assert!(st.view.diff_lines.is_empty());

    // A selection made in the hunk view, applied after the setting changed (a stale message).
    st.settings.diff_mode = DiffMode::Hunks;
    run(&mut st, Msg::Diff(DiffMsg::SelectDiffLine { hunk: 0, line: 1, how: Only }));
    assert!(!st.view.diff_lines.is_empty());
    st.settings.diff_mode = DiffMode::WholeFile;
    assert!(run(&mut st, Msg::Stage(StageMsg::ApplyLines(HunkOp::Stage))).is_empty());
    assert!(st.log.iter().any(|l| !l.ok && l.text == WHOLE_FILE_BLOCKS_LINE_OPS));

    // A block's buttons send the normal hunk index.
    let generation = st.hunk_ops_gen().expect("enabled");
    let shown = st.session.diff.view.as_ref().unwrap().1.hunks[2].clone();
    let fx = run(&mut st, Msg::Stage(StageMsg::Hunk { op: HunkOp::Stage, path: "f.txt".into(), index: 2, generation }));
    // With the hunk as shown, so core can refuse it if the file changed since.
    assert!(matches!(&fx[..], [Effect::LocalOp { op: LocalOp::Hunk { index: 2, expected, .. }, .. }] if *expected == shown), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A Rust file of 30 lines with lines 3 and 27 edited, committed and changed in `dir`.
fn rust_repo(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    git(&dir, &["config", "core.autocrlf", "false"]);
    let body = |a: &str, b: &str| -> String {
        (1..=30)
            .map(|i| match i {
                3 => format!("    let a = {a};\n"),
                27 => format!("    let b = {b};\n"),
                _ => format!("    let x{i} = {i};\n"),
            })
            .collect()
    };
    std::fs::write(dir.join("main.rs"), format!("fn main() {{\n{}}}\n", body("1", "2"))).unwrap();
    git(&dir, &["add", "main.rs"]);
    git(&dir, &["commit", "-q", "-m", "rust"]);
    std::fs::write(dir.join("main.rs"), format!("fn main() {{\n{}}}\n", body("10", "\"two\""))).unwrap();
    dir
}

fn shape(d: &FileDiff) -> Vec<usize> {
    d.hunks.iter().map(|h| h.lines.len()).collect()
}

fn staged(dir: &Path) -> Vec<String> {
    let d = repo::load_diff(dir, &DiffTarget::Staged("main.rs".into())).unwrap();
    d.hunks.iter().flat_map(|h| &h.lines).filter(|l| matches!(l.origin, '+' | '-')).map(|l| format!("{}{}", l.origin, l.text)).collect()
}

#[test]
fn whole_file_stage_block_end_to_end() {
    let dir = rust_repo("whole-e2e");
    let mut st = state();
    st.settings.diff_mode = DiffMode::WholeFile;
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("main.rs".into()))));
    let (_, d) = st.session.diff.view.as_ref().expect("diff open");
    let w = d.whole.as_deref().expect("whole file loaded");
    assert_eq!(w.blocks.len(), 2);
    let hl = w.diff.highlight.as_ref().expect("the whole file is highlighted");
    assert_eq!(hl.hunk_lens(), shape(&w.diff));
    assert_eq!(w.diff.intraline.hunk_lens(), shape(&w.diff));
    assert_eq!(st.view.diff_scroll_to_row, Some(block_rows(w, DiffLayout::Inline, &Expanded::default())[0]));

    let index = w.blocks[0].hunk;
    let generation = st.hunk_ops_gen().expect("enabled");
    let held = drive_holding_diffs(&mut st, Msg::Stage(StageMsg::Hunk { op: HunkOp::Stage, path: "main.rs".into(), index, generation }));
    assert!(
        held.iter().any(|e| matches!(e, Effect::LoadDiff { reload: true, whole_file: true, previous: Some(p), .. } if p.whole.is_some())),
        "{held:?}"
    );
    for e in held {
        if let Some(msg) = crate::runner::execute(e) {
            drive(&mut st, msg);
        }
    }
    let (_, d) = st.session.diff.view.as_ref().expect("the second change is still unstaged");
    let w = d.whole.as_deref().expect("still the whole file");
    assert_eq!(w.blocks.len(), 1);
    assert!(w.diff.highlight.is_some());
    assert_eq!(staged(&dir), ["-    let a = 1;", "+    let a = 10;"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn whole_file_with_ignore_whitespace_stages_the_shown_hunk() {
    let dir = rust_repo("whole-ws");
    // A re-indent next to the first change, hidden in the diff.
    let text = std::fs::read_to_string(dir.join("main.rs")).unwrap();
    std::fs::write(dir.join("main.rs"), text.replace("    let x5 = 5;", "        let x5 = 5;")).unwrap();
    let mut st = state();
    st.settings.diff_mode = DiffMode::WholeFile;
    st.settings.diff_ignore_whitespace = true;
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("main.rs".into()))));
    assert!(st.session.diff.view.as_ref().and_then(|(_, d)| d.whole.as_ref()).is_some());
    let generation = st.hunk_ops_gen().expect("loaded");
    let fx = run(&mut st, Msg::Stage(StageMsg::Hunk { op: HunkOp::Stage, path: "main.rs".into(), index: 0, generation }));
    assert!(matches!(&fx[..], [Effect::LocalOp { op: LocalOp::Hunk { index: 0, ignore_whitespace: true, .. }, .. }]), "{fx:?}");
    finish(&mut st, fx);
    assert!(st.log.iter().all(|l| l.ok), "{:?}", st.log.last().map(|l| &l.text));
    assert_eq!(staged(&dir), ["-    let a = 1;", "+    let a = 10;"], "the re-indent stays unstaged");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_history_honours_whole_mode() {
    let dir = scratch_repo("whole-history");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    st.settings.diff_mode = DiffMode::WholeFile;
    let head = st.snap.as_ref().and_then(|s| s.head_oid).unwrap();
    let entry = HistoryEntry {
        oid: head,
        summary: "first".into(),
        author: "Test".into(),
        email: "test@example.com".into(),
        time: 1_700_000_000,
        path: "a.txt".into(),
        old_path: None,
        status: 'A',
        files: Vec::new(),
    };
    let mut view = FileHistoryView::loading("a.txt".into(), None);
    view.append(FileHistory { entries: vec![entry], from_head: true, ..Default::default() });
    st.session.history.view = Some(view);
    let fx = run(&mut st, Msg::History(HistoryMsg::SelectHistoryEntry(0)));
    assert!(matches!(&fx[..], [Effect::LoadDiff { target: DiffTarget::Commit(o, _), whole_file: true, .. }] if *o == head), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_parked_diff_follows_a_mode_change_made_in_another_tab() {
    let mut st = state();
    st.session.diff.view = Some((DiffTarget::Unstaged("f.txt".into()), three_changes(true).into()));
    st.settings.diff_mode = DiffMode::WholeFile;
    st.session.tab_view.diff_top_row = blocks(&three_changes(true), DiffLayout::Inline)[2];

    run(&mut st, Msg::Tabs(TabsMsg::NewTab));
    run(&mut st, Msg::Diff(DiffMsg::SetDiffMode(DiffMode::Hunks)));
    run(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    let (_, d) = st.session.diff.view.as_ref().unwrap();
    assert!(d.whole.is_none(), "the hunk view shows the normal hunks");
    let header = header_rows(&three_changes(false), DiffLayout::Inline)[2];
    assert_eq!((st.view.diff_scroll_to_row, st.session.tab_view.diff_top_row), (Some(header), header), "the same hunk stays at the top");
}
