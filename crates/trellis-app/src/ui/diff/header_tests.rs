//! Layout tests of the diff header bar.

use super::*;
use eframe::egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

const TOGGLES: Toggles = Toggles { ignore_ws: Some(false), wrap: Some(false) };

/// The top of Expand all, of Wrap lines and of Close in a header bar `width` wide.
fn tops(width: f32) -> (f64, f64, f64) {
    let target = DiffTarget::Unstaged("notes.txt".into());
    let folds = Some(FoldCounts { open: 1, closed: 1 });
    let rows = Some((DiffLayout::Inline, DiffMode::WholeFile));
    let mut h = Harness::builder().with_size(vec2(width, 80.0)).build_ui(|ui| {
        header_bar(ui, &target, None, rows, [StepKind::Change; 2], TOGGLES, folds, &Keymap::default(), None, &mut Vec::new());
    });
    h.run();
    let top = |label: &str| h.get_by_label(label).bounding_box().expect("laid out").y0;
    (top("Expand all"), top(WRAP), top("Close  (Esc)"))
}

#[test]
fn fold_buttons_wrap_only_when_short_of_room() {
    let (expand, wrap, close) = tops(1400.0);
    assert!((expand - close).abs() < 4.0, "one row when wide: {expand} vs {close}");
    assert!((wrap - close).abs() < 4.0, "Wrap lines on the first row: {wrap} vs {close}");
    let (expand, wrap, close) = tops(740.0);
    assert!(expand > close + 10.0, "a second row when narrow: {expand} vs {close}");
    assert!((wrap - expand).abs() < 4.0, "Wrap lines moves down with Expand all: {wrap} vs {expand}");
}

#[test]
fn wrap_toggle_sends_msg() {
    let target = DiffTarget::Unstaged("notes.txt".into());
    let rows = Some((DiffLayout::Inline, DiffMode::Hunks));
    let mut h = Harness::builder().with_size(vec2(1400.0, 80.0)).build_ui_state(
        |ui, msgs: &mut Vec<Msg>| header_bar(ui, &target, None, rows, [StepKind::Change; 2], TOGGLES, None, &Keymap::default(), None, msgs),
        Vec::new(),
    );
    h.run();
    h.get_by_label(WRAP).click();
    h.run();
    assert!(h.state().iter().any(|m| matches!(m, Msg::Diff(DiffMsg::SetWordWrap(true)))));

    let no_rows = Toggles { ignore_ws: Some(false), wrap: None };
    let mut h = Harness::builder().with_size(vec2(1400.0, 80.0)).build_ui(|ui| {
        header_bar(ui, &target, None, None, [StepKind::End; 2], no_rows, None, &Keymap::default(), None, &mut Vec::new());
    });
    h.run();
    assert!(h.query_by_label(WRAP).is_none(), "no rows, no wrap toggle");
    assert!(h.query_by_label(IGNORE_WS).is_some());
}

/// The top of Ignore whitespace, Open, Close and the path in a commit's header bar `width`
/// wide, and the right edge of the path's kind and the left edge of the leftmost toggle.
fn commit_layout(width: f32) -> ([f64; 4], f64, f64) {
    let oid = git2::Oid::from_bytes(&[7; 20]).unwrap();
    let target = DiffTarget::Commit(oid, "notes.txt".into());
    let file = CommitFile { oid, path: "notes.txt", status: 'M', open_path: "notes.txt", in_workdir: true };
    let rows = Some((DiffLayout::Inline, DiffMode::Hunks));
    let mut h = Harness::builder().with_size(vec2(width, 80.0)).build_ui(|ui| {
        header_bar(ui, &target, Some(&file), rows, [StepKind::Change; 2], TOGGLES, None, &Keymap::default(), None, &mut Vec::new());
    });
    h.run();
    let rect = |label: &str| h.get_by_label(label).bounding_box().expect("laid out");
    let tops = [rect(IGNORE_WS).y0, rect(file_buttons::OPEN_SHORT).y0, rect(CLOSE).y0, rect("notes.txt").y0];
    (tops, rect("commit 0707070").x1, rect("Whole file").x0)
}

#[test]
fn tail_wraps_for_commit_file_buttons() {
    let ([ws, open, close, _], _, _) = commit_layout(1400.0);
    assert!((ws - close).abs() < 4.0 && (open - close).abs() < 4.0, "one row when wide: {ws}, {open} vs {close}");
    let ([ws, open, close, _], _, _) = commit_layout(900.0);
    assert!(ws > close + 10.0, "the tail moves to a second row: {ws} vs {close}");
    assert!((open - close).abs() < 4.0, "the file buttons stay on the first row: {open} vs {close}");
    // Narrower still, the file buttons move down too rather than run over the path.
    for width in [760.0, 700.0] {
        let ([ws, open, close, _], kind_right, toggles_left) = commit_layout(width);
        assert!(open > close + 10.0 && (ws - open).abs() < 4.0, "{width}: {ws}, {open} vs {close}");
        assert!(toggles_left >= kind_right, "{width}: the path is not overrun: {toggles_left} vs {kind_right}");
    }
}

#[test]
fn step_tips_follow_the_keymap() {
    let mut keys = Keymap::default();
    assert_eq!(step_tip(&keys, true, StepKind::Change), "Next change (F7)");
    keys.set(BindingId::NextChange, vec![crate::keymap::Chord::plain(crate::keymap::KeyName::F(8))]);
    assert_eq!(step_tip(&keys, true, StepKind::File), "Next change, in the next file (F8)");
    keys.set(BindingId::PrevChange, Vec::new());
    assert_eq!(step_tip(&keys, false, StepKind::End), "No changes before this file", "no key, no parentheses");
}

#[test]
fn toggles_move_down_in_a_narrow_bar() {
    let (a, b) = (git2::Oid::from_bytes(&[1; 20]).unwrap(), git2::Oid::from_bytes(&[2; 20]).unwrap());
    let target = DiffTarget::Between(a, b, "search.txt".into());
    let rows = Some((DiffLayout::Inline, DiffMode::Hunks));
    let layout = |width: f32| {
        let mut h = Harness::builder().with_size(vec2(width, 120.0)).build_ui(|ui| {
            header_bar(ui, &target, None, rows, [StepKind::Change; 2], TOGGLES, None, &Keymap::default(), None, &mut Vec::new());
        });
        h.run();
        let rect = |label: &str| h.get_by_label(label).bounding_box().expect("laid out");
        (rect(CLOSE).y0, rect("Split").y0, rect(IGNORE_WS).y0, rect("0101010..0202020").x1, rect("Hunks").x0.min(rect("Whole file").x0))
    };
    let (close, split, ws, _, _) = layout(1000.0);
    assert!((split - close).abs() < 4.0 && (ws - close).abs() < 4.0, "one row when wide");
    let (close, split, _, kind_right, toggles_left) = layout(660.0);
    assert!((split - close).abs() < 4.0, "the toggles stay up while they fit");
    assert!(toggles_left >= kind_right, "the path is not overrun: {toggles_left} vs {kind_right}");
    let (close, split, ws, _, _) = layout(370.0);
    assert!(split > close + 10.0, "the toggles move down: {split} vs {close}");
    assert!(ws > split + 10.0, "and the whitespace toggle below them: {ws} vs {split}");
}
