//! Headless tests for the diff viewer's word wrap toggle: saving, keeping the top row, and
//! row-based navigation that doesn't depend on how tall the rows are drawn.

use super::diff_tests::three_hunks;
use super::tests::state;
use super::update;
use crate::effect::Effect;
use crate::msg::DiffMsg;
use crate::msg::Msg;
use crate::state::AppState;
use glowfly_core::repo::DiffTarget;
use std::time::Instant;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn with_diff() -> AppState {
    let mut st = state();
    st.session.diff.view = Some((DiffTarget::Unstaged("f.txt".into()), three_hunks().into()));
    st
}

#[test]
fn set_word_wrap_saves_and_keeps_top_row() {
    let mut st = with_diff();
    st.session.tab_view.diff_top_row = 4;
    st.view.diff_cursor = Some(4);
    let fx = run(&mut st, Msg::Diff(DiffMsg::SetWordWrap(true)));
    assert!(matches!(&fx[..], [Effect::SaveSettings(s)] if s.diff_word_wrap));
    assert!(st.settings.diff_word_wrap);
    assert_eq!(st.settings_file.saved, serde_json::to_string(&st.settings).unwrap());
    assert_eq!(st.view.diff_scroll_to_row, Some(4), "the top row stays in place");
    assert_eq!(st.view.diff_cursor, Some(4), "rows are numbered the same");

    st.view.diff_scroll_to_row = None;
    assert!(run(&mut st, Msg::Diff(DiffMsg::SetWordWrap(true))).is_empty(), "no change, nothing to do");
    assert_eq!(st.view.diff_scroll_to_row, None);
}

#[test]
fn set_word_wrap_without_a_diff_only_saves() {
    let mut st = state();
    let fx = run(&mut st, Msg::Diff(DiffMsg::SetWordWrap(true)));
    assert!(matches!(&fx[..], [Effect::SaveSettings(s)] if s.diff_word_wrap));
    assert_eq!(st.view.diff_scroll_to_row, None);
}

#[test]
fn steps_land_on_the_same_rows_wrapped() {
    let tops = |wrap: bool| {
        let mut st = with_diff();
        st.settings.diff_word_wrap = wrap;
        (0..2)
            .map(|_| {
                run(&mut st, Msg::Diff(DiffMsg::JumpToChange(1)));
                st.session.tab_view.diff_top_row
            })
            .collect::<Vec<_>>()
    };
    let wrapped = tops(true);
    assert_eq!(wrapped, tops(false));
    assert!(wrapped[1] > wrapped[0], "{wrapped:?}");
}

#[test]
fn set_word_wrap_is_dropped_for_inactive_tabs() {
    assert!(DiffMsg::SetWordWrap(true).on_inactive().is_none());
}
