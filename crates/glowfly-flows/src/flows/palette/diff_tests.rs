//! The palette's entries for the open diff's unchanged runs: offered by state (closed runs, open
//! runs, none while whitespace is hidden in the hunk view or without a diff), in both views, and
//! running them opens or folds every run with one read.

use crate::flows::diff_gap_edges_tests::edges;
use crate::flows::update;
use crate::flows::{drive, state};
use glowfly_core::settings::DiffMode;
use glowfly_model::effect::{DiffEffect, Effect};
use glowfly_model::msg::{DiffMsg, Msg, PaletteMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::palette::{PaletteCommand as C, entries};
use std::time::Instant;

const SHOW_ALL: &str = "Show all unchanged lines in the diff";
const FOLD_ALL: &str = "Fold all unchanged lines in the diff";

fn has(st: &AppState, label: &str) -> bool {
    entries(st).iter().any(|e| e.label == label)
}

#[test]
fn the_entries_follow_the_runs() {
    let (mut st, dir) = edges("palette-diff");
    assert!(has(&st, SHOW_ALL) && !has(&st, FOLD_ALL), "closed runs only");
    st.session.tab_view.diff_expanded.insert(1);
    assert!(has(&st, SHOW_ALL) && has(&st, FOLD_ALL), "some of each");
    for key in [16, 32] {
        st.session.tab_view.diff_expanded.insert(key);
    }
    assert!(!has(&st, SHOW_ALL) && has(&st, FOLD_ALL), "every run open");

    // Not listed with Ignore whitespace on in the hunk view.
    st.settings.diff_ignore_whitespace = true;
    assert!(!has(&st, SHOW_ALL) && !has(&st, FOLD_ALL));
    st.settings.diff_ignore_whitespace = false;

    // The whole-file view's folds count too.
    st.session.tab_view.diff_expanded.clear();
    drive(&mut st, Msg::Diff(DiffMsg::SetDiffMode(DiffMode::WholeFile)));
    assert!(has(&st, SHOW_ALL) && !has(&st, FOLD_ALL));
    drive(&mut st, Msg::Diff(DiffMsg::ExpandAllUnchanged));
    assert!(!has(&st, SHOW_ALL) && has(&st, FOLD_ALL));
    st.settings.diff_ignore_whitespace = true;
    assert!(has(&st, FOLD_ALL), "the whole file's folds are not hidden by it");

    // No diff open.
    st.session.diff.view = None;
    assert!(!has(&st, SHOW_ALL) && !has(&st, FOLD_ALL));
    assert!(!has(&state(), SHOW_ALL));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn running_them_opens_and_folds_every_run() {
    let (mut st, dir) = edges("palette-diff-run");
    update(&mut st, Msg::Palette(PaletteMsg::TogglePalette), Instant::now());
    let fx = update(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::UnchangedLines(true))), Instant::now());
    let reads = fx.iter().filter(|e| matches!(e, Effect::Diff(DiffEffect::Gaps { .. }))).count();
    assert_eq!(reads, 1, "{fx:?}");
    assert_eq!(st.session.tab_view.diff_expanded.keys().collect::<Vec<_>>(), [1, 16, 32]);
    assert!(st.palette.is_none());
    crate::flows::finish(&mut st, fx);
    assert!(has(&st, FOLD_ALL) && !has(&st, SHOW_ALL));
    drive(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(C::UnchangedLines(false))));
    assert!(st.session.tab_view.diff_expanded.keys().next().is_none());
    assert_eq!(C::UnchangedLines(true).recall_key(), Some("diff-expand-all"));
    assert_eq!(C::UnchangedLines(false).recall_key(), Some("diff-collapse-all"));
    let _ = std::fs::remove_dir_all(&dir);
}
