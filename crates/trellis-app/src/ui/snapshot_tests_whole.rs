//! Whole-file diff snapshots: every line with changes in place and an action row above each
//! change block, in both layouts, for a commit and with whitespace hidden. Shares the fixture
//! repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{git, remove_fixture, render, with_repo};
use crate::state::AppState;
use std::path::PathBuf;
use trellis_core::repo::{self, DiffTarget, LoadDiffOptions, LoadOptions};
use trellis_core::settings::{DiffLayout, DiffMode, ThemeChoice};

const BEFORE: &str = "/// Shapes and their areas.
struct Rect {
    w: u32,
    h: u32,
}

impl Rect {
    fn area(&self) -> u32 {
        self.w * self.h
    }

    fn perimeter(&self) -> u32 {
        2 * (self.w + self.h)
    }
}
";

/// Two edits far enough apart to be two hunks: the doc comment and the perimeter formula.
fn after() -> String {
    BEFORE.replace("Shapes and their", "Rectangles and their").replace("2 * (self.w + self.h)", "2 * self.w + 2 * self.h")
}

/// shapes.rs committed as `BEFORE` and changed to `after`, then opened in the whole-file view as
/// the runner loads it (highlighted and word-marked). With `commit` the change is committed
/// and the commit's diff is shown; with `ignore_ws` it also re-spaces the area formula and
/// loads with whitespace hidden.
fn with_whole(name: &str, theme: ThemeChoice, layout: DiffLayout, commit: bool, ignore_ws: bool) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    let d = "2026-01-11T10:00:00+00:00";
    std::fs::write(dir.join("shapes.rs"), BEFORE).unwrap();
    git(&dir, &["add", "shapes.rs"], d);
    git(&dir, &["commit", "-q", "-m", "Add shapes.rs"], d);
    let changed = if ignore_ws { after().replace("self.w * self.h", "self.w  *  self.h") } else { after() };
    std::fs::write(dir.join("shapes.rs"), changed).unwrap();
    if commit {
        git(&dir, &["commit", "-q", "-am", "Rename shapes"], d);
    }
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let target = match st.snap.as_ref().and_then(|s| s.head_oid) {
        Some(head) if commit => DiffTarget::Commit(head, "shapes.rs".into()),
        _ => DiffTarget::Unstaged("shapes.rs".into()),
    };
    let opts = LoadDiffOptions { ignore_whitespace: ignore_ws, whole_file: true };
    let mut diff = repo::load_diff_with(&dir, &target, opts).unwrap();
    crate::runner::decorate(target.path(), &mut diff, None);
    let w = diff.whole.as_deref().expect("the whole file loads");
    assert_eq!(w.blocks.len(), 2);
    assert!(w.diff.highlight.is_some(), "shapes.rs is highlighted as Rust");
    st.diff = Some((target, diff));
    st.settings.diff_layout = layout;
    st.settings.diff_mode = DiffMode::WholeFile;
    st.settings.diff_ignore_whitespace = ignore_ws;
    (st, dir)
}

#[test]
fn diff_whole_inline_and_split() {
    let (st, dir) = with_whole("whole-inline", ThemeChoice::Dark, DiffLayout::Inline, false, false);
    render("diff_whole_inline", st, true);
    remove_fixture(&dir);
    let (st, dir) = with_whole("whole-split", ThemeChoice::Light, DiffLayout::Split, false, false);
    render("diff_whole_split_light", st, false);
    remove_fixture(&dir);
}

#[test]
fn diff_whole_commit() {
    let (st, dir) = with_whole("whole-commit", ThemeChoice::Dark, DiffLayout::Inline, true, false);
    render("diff_whole_commit", st, true);
    remove_fixture(&dir);
}

#[test]
fn diff_whole_ignore_whitespace() {
    let (st, dir) = with_whole("whole-ws", ThemeChoice::Dark, DiffLayout::Inline, false, true);
    let lines = &st.diff.as_ref().unwrap().1.whole.as_ref().unwrap().diff.hunks[0].lines;
    assert!(lines.iter().any(|l| l.origin == ' ' && l.text.contains("self.w  *  self.h")), "the re-spaced line is context");
    assert!(st.hunk_ops_blocked().is_some());
    render("diff_whole_ignore_whitespace", st, true);
    remove_fixture(&dir);
}
