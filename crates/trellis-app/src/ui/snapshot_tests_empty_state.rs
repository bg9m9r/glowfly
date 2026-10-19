//! The diff's empty states (a binary file, changes all hidden by Ignore whitespace) in both
//! themes, and the button that shows hidden whitespace again.

use super::super::empty_state::{BINARY_TITLE, SHOW_WHITESPACE, WHITESPACE_HINT, WHITESPACE_TITLE};
use super::super::snapshot_tests::{git, harness, render};
use super::with_hidden_whitespace;
use crate::msg::{DiffMsg, Msg};
use crate::state::AppState;
use crate::state::diff_preview::DiffPreview;
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use trellis_core::repo::{self, DiffTarget, LoadOptions};
use trellis_core::settings::ThemeChoice;

/// A committed binary file changed in the working tree, its diff open with its sides loaded.
/// Named .bin, not .png, so the body is the binary empty state with both sizes (an image path
/// shows the preview's "can't preview" cards instead: `snapshot_tests_file_header`).
fn with_binary_diff(name: &str, theme: ThemeChoice) -> (AppState, PathBuf) {
    let (mut st, dir) = super::super::snapshot_tests::with_repo(theme, name);
    std::fs::write(dir.join("logo.bin"), b"\x89PNG\r\n\x1a\n\x00\x00one").unwrap();
    let d = "2026-01-11T10:00:00+00:00";
    git(&dir, &["add", "logo.bin"], d);
    git(&dir, &["commit", "-q", "-m", "Add logo.bin"], d);
    std::fs::write(dir.join("logo.bin"), b"\x89PNG\r\n\x1a\n\x00\x00and two").unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let target = DiffTarget::Unstaged("logo.bin".into());
    let diff = repo::load_diff(&dir, &target).unwrap();
    assert!(diff.binary);
    st.diff = Some((target.clone(), diff.into()));
    let sides = repo::binary_sides(&dir, &target).unwrap();
    st.diff_preview = Some(DiffPreview { target, generation: 0, sides: Some(Ok(std::sync::Arc::new(sides))) });
    (st, dir)
}

fn whitespace_only(name: &str, theme: ThemeChoice) -> (AppState, PathBuf) {
    let (mut st, dir) = with_hidden_whitespace(name, "notes.txt", "one\n  two\n", "one  \n\ttwo\n");
    st.settings.theme = theme;
    assert!(st.diff.as_ref().unwrap().1.hunks.is_empty());
    (st, dir)
}

#[test]
fn diff_binary_empty_state() {
    for (theme, dark, name) in [(ThemeChoice::Light, false, "diff_binary_light"), (ThemeChoice::Dark, true, "diff_binary_dark")] {
        let (st, dir) = with_binary_diff(&format!("diff-binary-{dark}"), theme);
        let mut h = harness(st, dark);
        h.run();
        assert!(h.query_by_label(BINARY_TITLE).is_some());
        assert!(h.query_by_label("13 bytes before, 17 bytes after").is_some(), "both sizes");
        h.check_snapshot(name);
        drop(h);
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
}

#[test]
fn diff_whitespace_only_empty_state() {
    for (theme, dark, name) in
        [(ThemeChoice::Light, false, "diff_whitespace_only_light"), (ThemeChoice::Dark, true, "diff_whitespace_only")]
    {
        let (st, dir) = whitespace_only(&format!("diff-ws-empty-{dark}"), theme);
        render(name, st, dark);
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
}

#[test]
fn show_whitespace_changes_turns_ignore_whitespace_off() {
    let (st, dir) = whitespace_only("diff-ws-show", ThemeChoice::Dark);
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label(WHITESPACE_TITLE).is_some());
    assert!(h.query_by_label(WHITESPACE_HINT).is_some());
    h.get_by_label(SHOW_WHITESPACE).click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Diff(DiffMsg::SetIgnoreWhitespace(false)))));
    drop(h);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}
