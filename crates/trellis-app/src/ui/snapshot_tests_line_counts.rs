//! Line counts: a commit's file rows and header total, the working tree's Unstaged and Staged
//! Files rows, and the graph's WIP row (chips and total at the default columns; narrow, where
//! the total and then the chips are hidden before the label is cut; and in High Contrast Dark).
//! Dark and light.

use super::snapshot_tests::{git, harness, harness_palette, remove_fixture, render, with_repo};
use super::theme;
use crate::state::{AppState, Selection};
use crate::ui::snapshot_check::CheckSnapshot;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use trellis_core::repo::{self, LoadOptions};
use trellis_core::settings::ResizableColumn::Refs;
use trellis_core::settings::ThemeChoice;

const D: &str = "2026-01-12T10:00:00+00:00";

fn write(dir: &Path, file: &str, text: &str) {
    if let Some(parent) = dir.join(file).parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(dir.join(file), text).unwrap();
}

fn lines(prefix: &str, n: usize) -> String {
    (1..=n).map(|i| format!("{prefix} {i}\n")).collect()
}

/// The fixture with one more commit that edits, adds, deletes and adds a binary file, selected
/// with its detail and counts loaded.
fn with_commit(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    write(&dir, "app.txt", "app\nfixed\nrouting\nconfig\n");
    write(&dir, "docs/guide.md", &lines("Step", 24));
    write(&dir, "src/search/index.rs", &lines("fn part", 9));
    std::fs::remove_file(dir.join("login.txt")).unwrap();
    std::fs::write(dir.join("logo.png"), b"\x89PNG\r\n\x1a\n\0\0\0binary").unwrap();
    git(&dir, &["add", "-A"], D);
    git(&dir, &["commit", "-q", "-m", "Add the guide and search index"], D);
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let oid = st.snap.as_ref().unwrap().head_oid.unwrap();
    st.selection = Selection::Commit(oid);
    st.detail = Some(repo::commit_detail(&dir, oid).unwrap());
    st.line_counts.start_commit(oid);
    st.line_counts.finish_commit(oid, Arc::new(repo::commit_line_counts(&dir, oid).unwrap()));
    (st, dir)
}

/// The fixture with staged and unstaged edits, an untracked file and a deletion, with the
/// working tree's counts loaded.
fn with_wip(theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    write(&dir, "app.txt", "app\nfix\nstaged line\n");
    git(&dir, &["add", "app.txt"], D);
    write(&dir, "app.txt", "app\nfix\nstaged line\nand one more\n");
    write(&dir, "search.txt", &lines("result", 30));
    git(&dir, &["add", "search.txt"], D);
    write(&dir, "notes/todo.md", &lines("- item", 4));
    std::fs::remove_file(dir.join("login.txt")).unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    st.selection = Selection::Wip;
    let generation = st.line_counts.start_wip().unwrap();
    st.line_counts.finish_wip(generation, Some(repo::wip_line_counts(&dir).unwrap()));
    (st, dir)
}

#[test]
fn details_commit_line_counts() {
    let (st, dir) = with_commit(ThemeChoice::Dark, "lines-commit");
    let counts = st.line_counts.commit(st.detail.as_ref().unwrap().oid).unwrap();
    assert!(counts.complete && counts.get("logo.png").is_none(), "the binary file has no counts");
    render("details_commit_line_counts", st, true);
    let (st, _) = with_commit(ThemeChoice::Light, "lines-commit");
    render("details_commit_line_counts_light", st, false);
    remove_fixture(&dir);
}

#[test]
fn details_wip_line_counts() {
    let (st, dir) = with_wip(ThemeChoice::Dark, "lines-wip");
    let w = st.line_counts.wip().unwrap();
    assert!(w.unstaged.get("notes/todo.md").is_some() && w.staged.get("search.txt").is_some());
    render("details_wip_line_counts", st, true);
    let (st, _) = with_wip(ThemeChoice::Light, "lines-wip");
    render("details_wip_line_counts_light", st, false);
    remove_fixture(&dir);
}

/// At the reference size with the default columns: the row spans the empty commit and date
/// columns, so the label, chips and total all show.
#[test]
fn graph_wip_row() {
    let (st, dir) = with_wip(ThemeChoice::Dark, "lines-graph-wip");
    assert!(st.line_counts.wip().unwrap().total().is_some());
    assert_eq!(st.settings.column_widths, Default::default(), "the default columns");
    render("graph_wip_row", st, true);
    let (st, _) = with_wip(ThemeChoice::Light, "lines-graph-wip");
    render("graph_wip_row_light", st, false);
    let (st, _) = with_wip(ThemeChoice::HighContrastDark, "lines-graph-wip");
    let mut h = harness_palette(st, theme::HIGH_CONTRAST_DARK);
    h.run();
    h.check_snapshot("graph_wip_row_high_contrast");
    remove_fixture(&dir);
}

/// With the commit and date columns hidden and a wide BRANCH / TAG column, the row is narrow:
/// the total goes first, then the chips, and the label stays whole.
#[test]
fn graph_wip_row_narrow() {
    for (refs, name) in [(370.0, "graph_wip_row_narrow"), (450.0, "graph_wip_row_narrower")] {
        let (mut st, dir) = with_wip(ThemeChoice::Dark, "lines-graph-narrow");
        st.settings.columns.commit = false;
        st.settings.columns.date = false;
        st.settings.column_widths = st.settings.column_widths.with(Refs, refs);
        let mut h = harness(st, true);
        h.run();
        h.check_snapshot(name);
        remove_fixture(&dir);
    }
}
