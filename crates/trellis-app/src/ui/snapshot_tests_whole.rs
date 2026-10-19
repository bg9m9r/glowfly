//! Whole-file diff snapshots: every line with changes in place and an action row above each
//! change block, in both layouts, for a commit and with whitespace hidden, and long unchanged
//! runs folded into "N unchanged lines" rows, an opened run's fold-back control and the
//! header's Expand all / Collapse all. Shares the fixture repository and renderer with
//! `snapshot_tests`.

use super::snapshot_tests::{git, harness, harness_sized, remove_fixture, render, with_repo};
use crate::msg::DiffMsg;
use crate::msg::Msg;
use crate::msg::StageMsg;
use crate::state::AppState;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use trellis_core::repo::{self, DiffTarget, LoadDiffOptions, LoadOptions};
use trellis_core::settings::{DiffLayout, DiffMode, ThemeChoice};
use trellis_core::whole_file::{Expanded, folds};

const BEFORE: &str = "/// Shapes and their areas.
struct Rect {
    w: u32,
    h: u32,
}

impl Rect {
    fn area(&self) -> u32 {
        self.w * self.h
    }

    fn perimeter(&self) -> u32 {
        2 * (self.w + self.h)
    }
}
";

/// Two edits far enough apart to be two hunks: the doc comment and the perimeter formula.
fn after() -> String {
    BEFORE.replace("Shapes and their", "Rectangles and their").replace("2 * (self.w + self.h)", "2 * self.w + 2 * self.h")
}

/// shapes.rs committed as `BEFORE` and changed to `after`, then opened in the whole-file view as
/// the runner loads it (highlighted and word-marked). With `commit` the change is committed
/// and the commit's diff is shown; with `ignore_ws` it also re-spaces the area formula and
/// loads with whitespace hidden. Every fold is open, so the whole file shows.
fn with_whole(name: &str, theme: ThemeChoice, layout: DiffLayout, commit: bool, ignore_ws: bool) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let d = "2026-01-11T10:00:00+00:00";
    std::fs::write(dir.join("shapes.rs"), BEFORE).unwrap();
    git(&dir, &["add", "shapes.rs"], d);
    git(&dir, &["commit", "-q", "-m", "Add shapes.rs"], d);
    let changed = if ignore_ws { after().replace("self.w * self.h", "self.w  *  self.h") } else { after() };
    std::fs::write(dir.join("shapes.rs"), changed).unwrap();
    if commit {
        git(&dir, &["commit", "-q", "-am", "Rename shapes"], d);
    }
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let target = match st.snap.as_ref().and_then(|s| s.head_oid) {
        Some(head) if commit => DiffTarget::Commit(head, "shapes.rs".into()),
        _ => DiffTarget::Unstaged("shapes.rs".into()),
    };
    let opts = LoadDiffOptions { ignore_whitespace: ignore_ws, whole_file: true };
    let mut diff = repo::load_diff_with(&dir, &target, opts).unwrap();
    crate::runner::decorate(target.path(), &mut diff, None);
    let w = diff.whole.as_deref().expect("the whole file loads");
    assert_eq!(w.blocks.len(), 2);
    assert!(w.diff.highlight.is_some(), "shapes.rs is highlighted as Rust");
    st.view.diff_expanded = Expanded::all(w);
    st.diff = Some((target, diff.into()));
    st.settings.diff_layout = layout;
    st.settings.diff_mode = DiffMode::WholeFile;
    st.settings.diff_ignore_whitespace = ignore_ws;
    (st, dir)
}

#[test]
fn diff_whole_inline_and_split() {
    let (st, dir) = with_whole("whole-inline", ThemeChoice::Dark, DiffLayout::Inline, false, false);
    render("diff_whole_inline", st, true);
    remove_fixture(&dir);
    let (st, dir) = with_whole("whole-split", ThemeChoice::Light, DiffLayout::Split, false, false);
    render("diff_whole_split_light", st, false);
    remove_fixture(&dir);
}

#[test]
fn diff_whole_commit() {
    let (st, dir) = with_whole("whole-commit", ThemeChoice::Dark, DiffLayout::Inline, true, false);
    render("diff_whole_commit", st, true);
    remove_fixture(&dir);
}

#[test]
fn diff_whole_ignore_whitespace() {
    let (st, dir) = with_whole("whole-ws", ThemeChoice::Dark, DiffLayout::Inline, false, true);
    let lines = &st.diff.as_ref().unwrap().1.whole.as_ref().unwrap().diff.hunks[0].lines;
    assert!(lines.iter().any(|l| l.origin == ' ' && l.text.contains("self.w  *  self.h")), "the re-spaced line is context");
    assert!(st.hunk_ops_gen().is_some(), "the block buttons stay on");
    render("diff_whole_ignore_whitespace", st, true);
    remove_fixture(&dir);
}

#[test]
fn whole_file_revert_targets_the_blocks_hunk() {
    let (st, dir) = with_whole("whole-revert", ThemeChoice::Dark, DiffLayout::Inline, true, false);
    let generation = st.hunk_ops_gen().expect("buttons enabled");
    let mut h = harness(st, true);
    h.run();
    let buttons: Vec<_> = h.get_all_by_label("Revert Hunk").collect();
    assert_eq!(buttons.len(), 2, "one per change block");
    buttons[1].click();
    h.run();
    let sent: Vec<(usize, u64)> = h
        .state()
        .1
        .iter()
        .filter_map(|m| if let Msg::Stage(StageMsg::RevertHunk { index, generation }) = m { Some((*index, *generation)) } else { None })
        .collect();
    assert_eq!(sent, [(1, generation)]);
    drop(h);
    remove_fixture(&dir);
}

/// notes.txt, 40 lines of "line N", committed with lines 12 and 30 then edited, and opened as
/// an unstaged change in the whole-file view with every fold closed: a leading fold of 8 lines,
/// a middle one of 11 and a trailing one of 7.
fn with_notes(name: &str, theme: ThemeChoice, layout: DiffLayout) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let d = "2026-01-11T10:00:00+00:00";
    let text = |edit: bool| -> String {
        (1..=40)
            .map(|n| {
                if edit && (n == 12 || n == 30) {
                    format!(
                        "line {n}, edited
"
                    )
                } else {
                    format!(
                        "line {n}
"
                    )
                }
            })
            .collect()
    };
    std::fs::write(dir.join("notes.txt"), text(false)).unwrap();
    git(&dir, &["add", "notes.txt"], d);
    git(&dir, &["commit", "-q", "-m", "Add notes"], d);
    std::fs::write(dir.join("notes.txt"), text(true)).unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let target = DiffTarget::Unstaged("notes.txt".into());
    let opts = LoadDiffOptions { ignore_whitespace: false, whole_file: true };
    let mut diff = repo::load_diff_with(&dir, &target, opts).unwrap();
    crate::runner::decorate(target.path(), &mut diff, None);
    let w = diff.whole.as_deref().expect("the whole file loads");
    assert_eq!(w.blocks.len(), 2);
    assert_eq!(folds(w).iter().map(|f| f.len()).collect::<Vec<_>>(), [8, 11, 7]);
    st.diff = Some((target, diff.into()));
    st.settings.diff_layout = layout;
    st.settings.diff_mode = DiffMode::WholeFile;
    (st, dir)
}

/// The key of notes.txt's middle fold.
fn middle_key(st: &AppState) -> u32 {
    folds(st.diff.as_ref().unwrap().1.whole.as_deref().unwrap())[1].key
}

#[test]
fn diff_whole_folded() {
    let (st, dir) = with_notes("whole-folded", ThemeChoice::Dark, DiffLayout::Inline);
    render("diff_whole_folded", st, true);
    remove_fixture(&dir);
    let (mut st, dir) = with_notes("whole-folded-split", ThemeChoice::Light, DiffLayout::Split);
    let key = middle_key(&st);
    st.view.diff_expanded.insert(key);
    render("diff_whole_folded_split_light", st, false);
    remove_fixture(&dir);
}

#[test]
fn fold_row_click_sends_expand() {
    let (st, dir) = with_notes("whole-fold-click", ThemeChoice::Dark, DiffLayout::Inline);
    let key = middle_key(&st);
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("11 unchanged lines").click();
    h.run();
    let sent: Vec<u32> =
        h.state().1.iter().filter_map(|m| if let Msg::Diff(DiffMsg::ExpandUnchanged(k)) = m { Some(*k) } else { None }).collect();
    assert_eq!(sent, [key]);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn diff_whole_fold_back() {
    let (mut st, dir) = with_notes("whole-fold-back", ThemeChoice::Dark, DiffLayout::Inline);
    let key = middle_key(&st);
    st.view.diff_expanded.insert(key);
    render("diff_whole_fold_back", st, true);
    remove_fixture(&dir);
}

#[test]
fn fold_back_click_sends_collapse() {
    for layout in [DiffLayout::Inline, DiffLayout::Split] {
        let (mut st, dir) = with_notes(&format!("whole-fold-back-click-{layout:?}"), ThemeChoice::Dark, layout);
        let key = middle_key(&st);
        st.view.diff_expanded.insert(key);
        let mut h = harness(st, true);
        h.run();
        h.get_by_label("Fold 11 unchanged lines").click();
        h.run();
        let sent = &h.state().1;
        assert!(matches!(sent[..], [Msg::Diff(DiffMsg::CollapseUnchanged(k))] if k == key), "{layout:?}: {} messages", sent.len());
        drop(h);
        remove_fixture(&dir);
    }
}

#[test]
fn header_fold_buttons() {
    let (st, dir) = with_notes("whole-fold-header", ThemeChoice::Dark, DiffLayout::Inline);
    // Wide enough for the View button to name the layout and section.
    let mut h = harness_sized(st, true, eframe::egui::vec2(1800.0, 820.0));
    h.run();
    assert!(h.query_by_label("Expand all").is_none(), "in the View menu, closed");
    h.get_by_label("Inline \u{b7} Whole file").click();
    h.run();
    h.get_by_label("Collapse all").click();
    h.run();
    assert!(h.state().1.is_empty(), "Collapse all is off with every fold closed");
    h.get_by_label("Expand all").click();
    h.run();
    assert!(matches!(h.state().1[..], [Msg::Diff(DiffMsg::ExpandAllUnchanged)]), "{} messages", h.state().1.len());
    h.state_mut().0.settings.diff_mode = DiffMode::Hunks;
    h.run();
    assert!(h.query_by_label("Whole file").is_some(), "the menu is still open");
    assert!(h.query_by_label("Expand all").is_none() && h.query_by_label("Collapse all").is_none(), "not in the Hunks view");
    drop(h);
    remove_fixture(&dir);
}
