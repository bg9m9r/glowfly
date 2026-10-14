//! Preferences > Editor snapshots: the page itself, and the diff viewer without line numbers
//! and with another tab size and font size. Shares the fixtures with `snapshot_tests` and
//! `snapshot_tests_diff`.

use super::snapshot_tests::{git, remove_fixture, render, settings, with_repo};
use super::snapshot_tests_diff::with_diff;
use crate::state::{AppState, PrefPage, PrefsState};
use std::time::Instant;
use trellis_core::repo::{self, DiffTarget, LoadOptions};
use trellis_core::settings::{DiffLayout, ThemeChoice};
use trellis_core::{highlight, intraline};

#[test]
fn preferences_editor() {
    // No git configuration is needed, so the page draws without waiting for it.
    let mut st = AppState::new(settings(ThemeChoice::Dark), Vec::new(), Instant::now());
    st.prefs = Some(PrefsState::new(PrefPage::Editor));
    assert!(st.prefs.as_ref().unwrap().git.is_none());
    render("prefs_editor", st, true);
}

#[test]
fn diff_without_line_numbers() {
    let (mut st, dir) = with_diff(DiffLayout::Inline, "no-numbers");
    st.settings.diff_line_numbers = false;
    render("diff_no_line_numbers", st, true);
    remove_fixture(&dir);
    let (mut st, dir) = with_diff(DiffLayout::Split, "no-numbers");
    st.settings.diff_line_numbers = false;
    st.settings.theme = ThemeChoice::Light;
    render("diff_no_line_numbers_split_light", st, false);
    remove_fixture(&dir);
}

#[test]
fn diff_tab_size_and_font_size() {
    // Tab-indented lines at tab size 2 and 16 pt, highlighted and word-marked as the runner does.
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "diff-tabs");
    let before = "fn main() {\n\tlet x = 1;\n\t\treturn;\n}\n";
    std::fs::write(dir.join("tabs.rs"), before).unwrap();
    let d = "2026-01-11T10:00:00+00:00";
    git(&dir, &["add", "tabs.rs"], d);
    git(&dir, &["commit", "-q", "-m", "Add tabs.rs"], d);
    std::fs::write(dir.join("tabs.rs"), before.replace("let x = 1;", "let x = 2;").replace("\t\treturn;", "\t\treturn\tx;")).unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let target = DiffTarget::Unstaged("tabs.rs".into());
    let mut diff = repo::load_diff(&dir, &target).unwrap();
    assert!(diff.hunks[0].lines.iter().any(|l| l.text == "\t\treturn\tx;"), "lines keep their tabs");
    diff.highlight = highlight::highlight_diff(target.path(), &diff, None);
    diff.intraline = intraline::intraline(&diff);
    st.diff = Some((target, diff.into()));
    st.settings.diff_layout = DiffLayout::Inline;
    st.settings.tab_size = 2;
    st.settings.diff_font_size = 16.0;
    render("diff_tab_size_font_size", st, true);
    remove_fixture(&dir);
}
