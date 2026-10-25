//! The fold keys in `update`: Show opens the first closed run at or below the top (scrolling to
//! it when it is below the viewport), Fold back closes the opened run at the top; whole-file
//! folds in the whole-file view, the gaps between hunks in the hunk view.

use super::diff_gaps_tests::{land, opened};
use super::update;
use super::whole_file_tests::{three_changes, with_diff};
use crate::effect::Effect;
use crate::keymap::{BindingId, Chord, KeyName, Keymap, fold_ctrl};
use crate::msg::{DiffMsg, Msg};
use crate::state::AppState;
use glowfly_core::diff_rows::{filled, gaps};
use glowfly_core::settings::{DiffLayout, DiffMode};
use glowfly_core::whole_file::{Expanded, folds, row_of_line};
use std::time::Instant;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn open_key(st: &mut AppState) -> Vec<Effect> {
    run(st, Msg::Diff(DiffMsg::OpenFoldAtTop))
}

fn close_key(st: &mut AppState) -> Vec<Effect> {
    run(st, Msg::Diff(DiffMsg::CloseFoldAtTop))
}

/// f.txt's three changes on screen in the whole-file view, in `layout`, with `visible` rows.
fn whole(name: &str, layout: DiffLayout, visible: usize) -> (AppState, std::path::PathBuf) {
    let (mut st, dir) = with_diff(name, three_changes(true));
    st.settings.diff_mode = DiffMode::WholeFile;
    st.settings.diff_layout = layout;
    st.view.diff_visible_rows = visible;
    (st, dir)
}

#[test]
fn show_opens_the_next_closed_fold_from_the_top() {
    for layout in [DiffLayout::Inline, DiffLayout::Split] {
        let (mut st, dir) = whole(&format!("foldkey-open-{layout:?}"), layout, 40);
        let d = three_changes(true);
        let w = d.whole.as_deref().unwrap();
        let f = folds(w);
        assert_eq!(f.len(), 2);
        // The top sits on the first fold's row: it opens, and nothing scrolls.
        st.view.diff_top_row = row_of_line(w, layout, &Expanded::default(), f[0].start);
        let top = st.view.diff_top_row;
        assert!(open_key(&mut st).is_empty());
        assert!(st.view.diff_expanded.contains(f[0].key), "{layout:?}");
        assert_eq!((st.view.diff_top_row, st.view.diff_scroll_to_row), (top, None), "{layout:?}");
        // Then the next one down; then nothing is left.
        open_key(&mut st);
        assert!(st.view.diff_expanded.contains(f[1].key), "{layout:?}");
        let all = st.view.diff_expanded.clone();
        open_key(&mut st);
        assert_eq!(st.view.diff_expanded, all, "{layout:?}");

        // Fold back closes the fold holding the top line, keeping the top in place.
        close_key(&mut st);
        assert!(!st.view.diff_expanded.contains(f[0].key), "{layout:?}");
        assert_eq!(st.view.diff_top_row, top, "{layout:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn show_scrolls_to_a_fold_below_the_viewport() {
    for layout in [DiffLayout::Inline, DiffLayout::Split] {
        let (mut st, dir) = whole(&format!("foldkey-scroll-{layout:?}"), layout, 2);
        let d = three_changes(true);
        let w = d.whole.as_deref().unwrap();
        let f = folds(w)[0];
        let row = row_of_line(w, layout, &Expanded::default(), f.start);
        assert!(row >= 2, "{layout:?}: below the two visible rows");
        open_key(&mut st);
        assert!(st.view.diff_expanded.contains(f.key));
        let to = row_of_line(w, layout, &st.view.diff_expanded, f.start);
        assert_eq!((st.view.diff_top_row, st.view.diff_scroll_to_row), (to, Some(to)), "{layout:?}");
        // Nothing opened is on screen past the last fold: Fold back still finds the one at the top.
        close_key(&mut st);
        assert!(st.view.diff_expanded.is_empty(), "{layout:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn fold_keys_act_on_gaps_in_the_hunk_view() {
    let (mut st, dir) = opened("foldkey-gaps");
    st.view.diff_visible_rows = 60;
    let g = gaps(&st.diff.as_ref().unwrap().1);
    let fx = open_key(&mut st);
    assert_eq!(fx.iter().filter(|e| matches!(e, Effect::LoadDiffGaps { .. })).count(), 1, "{fx:?}");
    assert!(st.view.diff_expanded.contains(g[0].key));
    land(&mut st, fx);
    assert_eq!(filled(&st.diff.as_ref().unwrap().1, &st.view.diff_expanded).len(), 1);
    // The next press opens the second gap from the text already read.
    assert!(open_key(&mut st).is_empty());
    assert!(st.view.diff_expanded.contains(g[1].key));
    // Fold back closes the first opened gap on screen.
    close_key(&mut st);
    assert!(!st.view.diff_expanded.contains(g[0].key));
    assert!(st.view.diff_expanded.contains(g[1].key));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn e_and_shift_e_are_the_fold_keys() {
    let keys = Keymap::default();
    assert_eq!(keys.lookup(Chord::plain(KeyName::Letter('E')), fold_ctrl()), Some(BindingId::OpenFold));
    assert_eq!(keys.lookup(Chord::shift(KeyName::Letter('E')), fold_ctrl()), Some(BindingId::CloseFold));
}
