//! Revert Hunk and Revert lines on a commit diff: the button on the hunk header (Revert N lines
//! with lines selected), selecting lines with a click in their gutter, enabled with whitespace
//! hidden, disabled while a reload is pending, and the message a click sends. Shares the
//! fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{harness, remove_fixture, render, with_repo};
use crate::msg::Msg;
use crate::msg::StageMsg;
use crate::state::{AppState, Selection};
use egui_kittest::kittest::Queryable;
use glowfly_core::intraline;
use glowfly_core::repo::{self, DiffTarget};
use glowfly_core::settings::{DiffLayout, ThemeChoice};
use std::path::PathBuf;

/// The fixture with the "Fix build on Windows" commit selected and its diff of app.txt open.
pub(crate) fn with_commit_diff(name: &str, theme: ThemeChoice, layout: DiffLayout, ignore_ws: bool) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let snap = st.snap.as_ref().unwrap();
    let oid = snap.commits.iter().find(|c| c.summary == "Fix build on Windows").expect("the fix commit").oid;
    st.session.selection = Selection::Commit(oid);
    st.session.detail = Some(repo::commit_detail(&dir, oid).unwrap());
    let target = DiffTarget::Commit(oid, "app.txt".into());
    let mut diff = repo::load_diff_with(&dir, &target, repo::LoadDiffOptions { ignore_whitespace: ignore_ws, whole_file: false }).unwrap();
    diff.intraline = intraline::intraline(&diff);
    st.session.diff.view = Some((target, diff.into()));
    st.settings.diff_layout = layout;
    st.settings.diff_ignore_whitespace = ignore_ws;
    (st, dir)
}

#[test]
fn diff_commit_revert() {
    let (st, dir) = with_commit_diff("revert", ThemeChoice::Dark, DiffLayout::Inline, false);
    render("diff_commit_revert", st, true);
    remove_fixture(&dir);
}

#[test]
fn diff_commit_revert_whitespace_light() {
    // With whitespace hidden, Revert Hunk stays on: it reverts only the changes shown.
    let (st, dir) = with_commit_diff("revert-ws", ThemeChoice::Light, DiffLayout::Split, true);
    assert!(st.hunk_ops_gen().is_some());
    render("diff_commit_revert_whitespace_light", st, false);
    remove_fixture(&dir);
}

fn reverts(msgs: &[Msg]) -> Vec<(usize, u64)> {
    msgs.iter()
        .filter_map(|m| if let Msg::Stage(StageMsg::RevertHunk { index, generation }) = m { Some((*index, *generation)) } else { None })
        .collect()
}

#[test]
fn revert_button_sends_msg() {
    let (st, dir) = with_commit_diff("revert-click", ThemeChoice::Dark, DiffLayout::Inline, false);
    let generation = st.hunk_ops_gen().expect("buttons enabled");
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("Revert hunk").click();
    h.run();
    assert_eq!(reverts(&h.state().1), [(0, generation)]);
    drop(h);

    // With whitespace hidden the click sends the revert too; a reload on its way disables it.
    let (st, _) = with_commit_diff("revert-click", ThemeChoice::Dark, DiffLayout::Inline, true);
    let generation = st.hunk_ops_gen().expect("buttons enabled");
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("Revert hunk").click();
    h.run();
    assert_eq!(reverts(&h.state().1), [(0, generation)], "whitespace hidden");
    drop(h);
    let (mut st, _) = with_commit_diff("revert-click", ThemeChoice::Dark, DiffLayout::Inline, false);
    st.session.diff.generation += 1;
    assert_eq!(st.hunk_ops_gen(), None);
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("Revert hunk").click();
    h.run();
    assert!(reverts(&h.state().1).is_empty(), "pending reload");
    remove_fixture(&dir);
}

/// The commit diff of app.txt replaced by a hunk with three changed lines (` app`, `-fix`,
/// `+fix the build`, `+on Windows`), with `selected` of its lines selected.
pub(crate) fn with_line_diff(name: &str, theme: ThemeChoice, layout: DiffLayout, selected: &[usize]) -> (AppState, PathBuf) {
    use crate::msg::DiffMsg;
    use crate::state::lines::LineClick;
    use glowfly_core::repo::{DiffLine, FileDiff, Hunk};
    let (mut st, dir) = with_commit_diff(name, theme, layout, false);
    let target = st.session.diff.view.as_ref().unwrap().0.clone();
    let line = |origin: char, old: Option<u32>, new: Option<u32>, text: &str| DiffLine { origin, old, new, text: text.into() };
    let lines = vec![
        line(' ', Some(1), Some(1), "app"),
        line('-', Some(2), None, "fix"),
        line('+', None, Some(2), "fix the build"),
        line('+', None, Some(3), "on Windows"),
    ];
    let mut diff = FileDiff {
        binary: false,
        hunks: vec![Hunk { header: "@@ -1,2 +1,3 @@".into(), lines }],
        highlight: None,
        intraline: Default::default(),
        whole: None,
    };
    diff.intraline = intraline::intraline(&diff);
    st.session.diff.view = Some((target.clone(), diff.into()));
    for (i, &line) in selected.iter().enumerate() {
        let how = if i == 0 { LineClick::Only } else { LineClick::Toggle };
        crate::update::update(&mut st, Msg::Diff(DiffMsg::SelectDiffLine { hunk: 0, line, how }), std::time::Instant::now());
    }
    assert_eq!(st.view.diff_lines.count_in(&target, 0), selected.len());
    (st, dir)
}

#[test]
fn diff_commit_revert_lines() {
    let (st, dir) = with_line_diff("revert-lines", ThemeChoice::Dark, DiffLayout::Inline, &[1, 2]);
    render("diff_commit_revert_lines", st, true);
    remove_fixture(&dir);
    let (st, dir) = with_line_diff("revert-lines-split", ThemeChoice::Light, DiffLayout::Split, &[1, 2]);
    render("diff_commit_revert_lines_split_light", st, false);
    remove_fixture(&dir);
}

/// Whether any label in `h` names a working-tree action.
fn stage_verbs(h: &egui_kittest::Harness<'_, (AppState, Vec<Msg>)>) -> Vec<&'static str> {
    ["Stage", "Unstage", "Discard"].into_iter().filter(|v| h.query_by_label_contains(v).is_some()).collect()
}

#[test]
fn diff_commit_readonly_dark() {
    // At rest: the signs, no check boxes or bars, and only Revert on the header.
    let (st, dir) = with_line_diff("readonly", ThemeChoice::Dark, DiffLayout::Inline, &[]);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Revert hunk").is_some());
    assert_eq!(stage_verbs(&h), Vec::<&str>::new(), "a read-only diff offers no working-tree action");
    drop(h);
    let (st, _) = with_line_diff("readonly", ThemeChoice::Dark, DiffLayout::Inline, &[]);
    render("diff_commit_readonly_dark", st, true);
    // With lines selected it reverts them, still with no stage verb.
    let (st, _) = with_line_diff("readonly", ThemeChoice::Dark, DiffLayout::Split, &[1, 2]);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Revert 2 lines").is_some());
    assert_eq!(stage_verbs(&h), Vec::<&str>::new());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn commit_diff_lines_click_and_revert() {
    use crate::msg::DiffMsg;
    use crate::state::lines::LineClick;
    use eframe::egui::{Event, Modifiers, PointerButton, pos2};
    // A click in a `+` line's gutter selects it, one on its text doesn't: line i is i + 1 rows
    // below the hunk header, and the rows start at the sidebar's edge.
    let (st, dir) = with_line_diff("revert-lines-click", ThemeChoice::Dark, DiffLayout::Inline, &[]);
    let row_h = crate::ui::diff::DiffStyle::new(&st.settings).row_h;
    let mut h = harness(st, true);
    h.run();
    let b = h.get_by_label("Revert hunk").bounding_box().expect("the button has bounds");
    let y = (b.y0 + b.y1) as f32 / 2.0 + 4.0 * row_h;
    for x in [b.x0 as f32 - 300.0, 261.0 + 100.0] {
        let events = &mut h.input_mut().events;
        events.push(Event::PointerMoved(pos2(x, y)));
        for pressed in [true, false] {
            events.push(Event::PointerButton { pos: pos2(x, y), button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        }
        h.run();
    }
    let clicked: Vec<_> = h
        .state()
        .1
        .iter()
        .filter_map(|m| if let Msg::Diff(DiffMsg::SelectDiffLine { hunk, line, how }) = m { Some((*hunk, *line, *how)) } else { None })
        .collect();
    assert_eq!(clicked, [(0, 3, LineClick::Toggle)], "only the gutter click");
    drop(h);

    // With lines selected the button reverts just those.
    let (st, _) = with_line_diff("revert-lines-click", ThemeChoice::Dark, DiffLayout::Inline, &[1, 2]);
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("Revert 2 lines").click();
    h.run();
    let msgs = &h.state().1;
    assert!(msgs.iter().any(|m| matches!(m, Msg::Stage(StageMsg::RevertLines))), "Revert lines");
    assert!(reverts(msgs).is_empty(), "not Revert Hunk");
    drop(h);
    remove_fixture(&dir);
}
