//! Change navigation and the layout switch over split rows that similarity pairing lays out.

use super::tests::state;
use super::update;
use crate::effect::Effect;
use crate::msg::{DiffMsg, Msg};
use crate::state::AppState;
use std::time::Instant;
use trellis_core::diff_rows::header_rows;
use trellis_core::intraline;
use trellis_core::repo::{DiffLine, DiffTarget, FileDiff, Hunk};
use trellis_core::settings::DiffLayout;

/// Two hunks; the first replaces `A, X` with `Y, A'`, so similarity pairing gives it one more
/// split row than pairing by position. Decorated with its word marks and line pairs.
fn paired() -> FileDiff {
    let line = |origin: char, text: &str| DiffLine { origin, old: Some(1), new: Some(1), text: text.into() };
    let hunks = vec![
        Hunk {
            header: "@@ -1,3 +1,3 @@".into(),
            lines: vec![
                line(' ', "a"),
                line('-', "let total = compute(1);"),
                line('-', "zzz"),
                line('+', "qqq"),
                line('+', "let total = compute(2);"),
            ],
        },
        Hunk { header: "@@ -9 +9 @@".into(), lines: vec![line('-', "d"), line('+', "e")] },
    ];
    let mut d = FileDiff { binary: false, hunks, highlight: None, intraline: Default::default(), whole: None };
    d.intraline = intraline::intraline(&d);
    d
}

fn with(d: &FileDiff, layout: DiffLayout) -> AppState {
    let mut st = state();
    st.settings.diff_layout = layout;
    st.diff = Some((DiffTarget::Unstaged("f.txt".into()), d.clone().into()));
    st
}

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

#[test]
fn jump_to_change_uses_similarity_rows() {
    let d = paired();
    assert!(!d.intraline.pairs().is_empty());
    let split = header_rows(&d, DiffLayout::Split);
    let mut positional = d.clone();
    positional.intraline = Default::default();
    assert_eq!(split[1], header_rows(&positional, DiffLayout::Split)[1] + 1, "one more row in the first hunk");

    let mut st = with(&d, DiffLayout::Split);
    assert!(run(&mut st, Msg::Diff(DiffMsg::JumpToChange(1))).is_empty());
    assert_eq!(st.view.diff_scroll_to_row, Some(split[1]));

    // Switching from inline at hunk 1 lands on its split header.
    let mut st = with(&d, DiffLayout::Inline);
    st.view.diff_top_row = header_rows(&d, DiffLayout::Inline)[1];
    run(&mut st, Msg::Diff(DiffMsg::SetDiffLayout(DiffLayout::Split)));
    assert_eq!(st.view.diff_scroll_to_row, Some(split[1]));
    // And back, from inside hunk 1's split rows.
    st.view.diff_top_row = split[1] + 1;
    run(&mut st, Msg::Diff(DiffMsg::SetDiffLayout(DiffLayout::Inline)));
    assert_eq!(st.view.diff_scroll_to_row, Some(header_rows(&d, DiffLayout::Inline)[1]));
}
