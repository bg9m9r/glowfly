//! Headless tests for `diff_nav::current_hunk`, the hunk the Revert key acts on: in the hunk
//! view, in the whole-file view (a block maps to its normal hunk), with the stepping cursor on or
//! off screen, and without changes.

use super::diff_tests::three_hunks;
use super::tests::{git, scratch_repo, state};
use crate::state::AppState;
use crate::state::diff_nav::{change_rows_of, current_hunk};
use glowfly_core::repo::{self, DiffTarget, FileDiff, LoadDiffOptions};
use glowfly_core::settings::DiffMode;

fn rows(st: &AppState) -> Vec<usize> {
    change_rows_of(&st.diff.as_ref().unwrap().1, &st.settings, &st.view.diff_expanded)
}

#[test]
fn hunk_view() {
    let mut st = state();
    st.diff = Some((DiffTarget::Commit(git2::Oid::zero(), "f.txt".into()), three_hunks().into()));
    st.view.diff_visible_rows = 10;
    let r = rows(&st);
    assert_eq!(current_hunk(&st), Some(0), "the first hunk at the top");
    st.view.diff_top_row = r[1];
    assert_eq!(current_hunk(&st), Some(1));
    st.view.diff_top_row = r[2] - 1;
    assert_eq!(current_hunk(&st), Some(1), "the hunk at or above the top row");
    st.view.diff_top_row = r[2] + 1;
    assert_eq!(current_hunk(&st), Some(2));

    // The change last stepped to while on screen, else the top row.
    st.view.diff_top_row = r[1];
    st.view.diff_cursor = Some(r[2]);
    assert_eq!(current_hunk(&st), Some(2));
    st.view.diff_top_row = r[2] + 10;
    st.view.diff_cursor = Some(r[0]);
    assert_eq!(current_hunk(&st), Some(2), "a cursor scrolled off screen is ignored");
}

#[test]
fn whole_file_view_maps_blocks_to_hunks() {
    let dir = scratch_repo("current-hunk-whole");
    let numbered = |edits: &[(usize, &str)]| -> String {
        (1..=30).map(|i| edits.iter().find(|(n, _)| *n == i).map_or(format!("{i}\n"), |(_, t)| format!("{t}\n"))).collect()
    };
    std::fs::write(dir.join("b.txt"), numbered(&[])).unwrap();
    git(&dir, &["add", "b.txt"]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    std::fs::write(dir.join("b.txt"), numbered(&[(2, "two"), (25, "twenty-five")])).unwrap();
    git(&dir, &["add", "b.txt"]);
    git(&dir, &["commit", "-q", "-m", "two edits"]);
    let head = git2::Repository::open(&dir).unwrap().head().unwrap().target().unwrap();
    let target = DiffTarget::Commit(head, "b.txt".into());
    let diff = repo::load_diff_with(&dir, &target, LoadDiffOptions { whole_file: true, ..Default::default() }).unwrap();
    assert!(diff.whole.is_some());
    let mut st = state();
    st.settings.diff_mode = DiffMode::WholeFile;
    st.diff = Some((target, diff.into()));
    st.view.diff_visible_rows = 5;
    let r = rows(&st);
    assert_eq!(r.len(), 2);
    assert_eq!(current_hunk(&st), Some(0));
    st.view.diff_top_row = r[1];
    assert_eq!(current_hunk(&st), Some(1));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_changes() {
    let mut st = state();
    assert_eq!(current_hunk(&st), None, "no diff");
    let empty = FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None };
    st.diff = Some((DiffTarget::Commit(git2::Oid::zero(), "f.txt".into()), empty.into()));
    assert_eq!(current_hunk(&st), None);
}
