//! The hunk view's gaps between hunks: the control on each later hunk's header while a gap is
//! closed, and the unchanged lines filled in once opened (with their syntax colors), inline and
//! split. Shares the fixture repository and renderer with `snapshot_tests`.

use super::super::snapshot_tests::{git, render, with_repo};
use crate::state::AppState;
use glowfly_core::diff_rows::gaps;
use glowfly_core::repo::{self, DiffTarget, LoadOptions};
use glowfly_core::settings::{DiffLayout, ThemeChoice};
use glowfly_core::{highlight, intraline};
use std::path::PathBuf;
use std::sync::Arc;

/// `fn fN() {}` for lines 1..=24, with lines 2, 12 and 22 given a body.
fn body(changed: bool) -> String {
    (1..=24)
        .map(|i| if changed && [2, 12, 22].contains(&i) { format!("fn f{i}() {{ {i} }}\n") } else { format!("fn f{i}() {{}}\n") })
        .collect()
}

/// gaps.rs with three changed lines (three hunks, two gaps of 3 lines), highlighted as Rust;
/// with `open`, both gaps opened and their text read.
fn with_gaps(layout: DiffLayout, theme: ThemeChoice, open: bool) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, &format!("diff-gaps-{layout:?}-{open}"));
    std::fs::write(dir.join("gaps.rs"), body(false)).unwrap();
    let d = "2026-01-11T10:00:00+00:00";
    git(&dir, &["add", "gaps.rs"], d);
    git(&dir, &["commit", "-q", "-m", "Add gaps.rs"], d);
    std::fs::write(dir.join("gaps.rs"), body(true)).unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let target = DiffTarget::Unstaged("gaps.rs".into());
    let mut diff = repo::load_diff(&dir, &target).unwrap();
    diff.highlight = highlight::highlight_diff(target.path(), &diff, None);
    diff.intraline = intraline::intraline(&diff);
    let spans = gaps(&diff);
    assert_eq!(spans.iter().map(|g| g.len).collect::<Vec<_>>(), [3, 3]);
    if open {
        let syntax = diff.highlight.as_ref().map(|h| h.syntax.as_str());
        let text = repo::load_gap_text(&dir, &target, &diff.hunks, syntax).unwrap();
        for g in &spans {
            st.session.tab_view.diff_expanded.insert(g.key);
        }
        st.session.tab_view.diff_expanded.set_gap_text(Arc::new(text));
    }
    st.session.diff.view = Some((target, diff.into()));
    st.settings.diff_layout = layout;
    (st, dir)
}

#[test]
fn diff_gap_control_inline() {
    let (st, dir) = with_gaps(DiffLayout::Inline, ThemeChoice::Dark, false);
    render("diff_gap_control_inline", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn diff_gap_filled_inline() {
    let (st, dir) = with_gaps(DiffLayout::Inline, ThemeChoice::Dark, true);
    render("diff_gap_filled_inline", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn diff_gap_filled_split_light() {
    let (st, dir) = with_gaps(DiffLayout::Split, ThemeChoice::Light, true);
    render("diff_gap_filled_split_light", st, false);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}
