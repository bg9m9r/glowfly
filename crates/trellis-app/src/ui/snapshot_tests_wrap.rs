//! Wrapped diff snapshots: long lines continuing on the next line in the inline and split
//! layouts, in the whole-file view with folds, and in the file history's embedded diff.
//! Shares the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{git, remove_fixture, render, with_repo};
use crate::msg::DiffMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::lines::LineClick;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::repo::{self, DiffTarget, FileDiff, LoadDiffOptions, LoadOptions};
use trellis_core::settings::{DiffLayout, DiffMode, ThemeChoice};
use trellis_core::whole_file::folds;
use trellis_core::{highlight, intraline};

const OLD_GREETING: &str = "\tlet message = format!(\"Hello {name}, welcome back! You have {count} new notifications waiting in your inbox, and the weekly summary of the projects you follow is ready to read whenever you have a moment to spare today.\", name = name, count = count);";
const NEW_GREETING: &str = "\tlet message = format!(\"Hello {name}, good to see you again! You have {count} unread notifications waiting in your inbox, and the weekly summary of every project you follow is ready to read whenever you have a quiet moment to spare later today or tomorrow morning.\", name = name, count = count);";
const COMMENT: &str = "// The helpers below are numbered so the fold rows have something to hide; each one returns its own number, which keeps the file long enough for the whole-file view to fold the runs of unchanged lines between the edits.";

/// wrap.rs, 26 lines: a greeting whose long, tab-indented line is reworded at line 3, a long
/// comment at line 7 that starts an unchanged run, and numbered helpers, one changed at line 16.
fn source(edited: bool) -> String {
    let greeting = if edited { NEW_GREETING } else { OLD_GREETING };
    let head = [
        "/// Builds the messages shown on the welcome page.",
        "fn greeting(name: &str, count: u32) -> String {",
        greeting,
        "\tmessage",
        "}",
        "",
        COMMENT,
    ];
    let helper =
        |n: u32| if edited && n == 16 { format!("fn helper_{n}() -> u32 {{ {n}0 }}") } else { format!("fn helper_{n}() -> u32 {{ {n} }}") };
    let lines: Vec<String> = head.iter().map(|s| s.to_string()).chain((8..=26).map(helper)).collect();
    lines.join("\n") + "\n"
}

/// A fixture with wrap.rs committed and then edited (the edit committed too with
/// `commit_edit`), and wrapping on.
fn with_wrap_repo(name: &str, theme: ThemeChoice, commit_edit: bool) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
    git(&dir, &["config", "core.autocrlf", "false"], "2026-01-11T10:00:00+00:00");
    let d = "2026-01-11T10:00:00+00:00";
    std::fs::write(dir.join("wrap.rs"), source(false)).unwrap();
    git(&dir, &["add", "wrap.rs"], d);
    git(&dir, &["commit", "-q", "-m", "Add wrap.rs"], d);
    std::fs::write(dir.join("wrap.rs"), source(true)).unwrap();
    if commit_edit {
        git(&dir, &["commit", "-q", "-m", "Reword the greeting", "wrap.rs"], "2026-01-12T10:00:00+00:00");
    }
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    st.settings.diff_word_wrap = true;
    (st, dir)
}

/// The diff of `target`, highlighted and word-marked as the runner does.
fn decorated(dir: &Path, target: &DiffTarget) -> FileDiff {
    let mut diff = repo::load_diff(dir, target).unwrap();
    diff.highlight = highlight::highlight_diff(target.path(), &diff, None);
    diff.intraline = intraline::intraline(&diff);
    assert!(diff.highlight.is_some(), "wrap.rs is highlighted as Rust");
    diff
}

/// The unstaged wrap.rs edit in the Hunks view, in `layout`.
fn with_hunks(name: &str, layout: DiffLayout) -> (AppState, PathBuf) {
    let (mut st, dir) = with_wrap_repo(name, ThemeChoice::Dark, false);
    let target = DiffTarget::Unstaged("wrap.rs".into());
    let diff = decorated(&dir, &target);
    st.diff = Some((target, diff.into()));
    st.settings.diff_layout = layout;
    (st, dir)
}

#[test]
fn diff_wrap_inline() {
    let (mut st, dir) = with_hunks("wrap-inline", DiffLayout::Inline);
    let lines = &st.diff.as_ref().unwrap().1.hunks[0].lines;
    let line = lines.iter().position(|l| l.origin == '+' && l.text == NEW_GREETING).expect("the reworded line");
    let how = LineClick::Only;
    crate::update::update(&mut st, Msg::Diff(DiffMsg::SelectDiffLine { hunk: 0, line, how }), Instant::now());
    assert_eq!(st.view.diff_lines.count_in(&DiffTarget::Unstaged("wrap.rs".into()), 0), 1);
    render("diff_wrap_inline", st, true);
    remove_fixture(&dir);
}

#[test]
fn diff_wrap_split() {
    let (st, dir) = with_hunks("wrap-split", DiffLayout::Split);
    render("diff_wrap_split", st, true);
    remove_fixture(&dir);
}

#[test]
fn diff_wrap_whole_file_light() {
    let (mut st, dir) = with_wrap_repo("wrap-whole", ThemeChoice::Light, false);
    let target = DiffTarget::Unstaged("wrap.rs".into());
    let opts = LoadDiffOptions { ignore_whitespace: false, whole_file: true };
    let mut diff = repo::load_diff_with(&dir, &target, opts).unwrap();
    crate::runner::decorate(target.path(), &mut diff, None);
    let w = diff.whole.as_deref().expect("the whole file loads");
    let all = folds(w);
    assert_eq!(all.iter().map(|f| f.len()).collect::<Vec<_>>(), [6, 7]);
    // The run starting at the long comment is open; the trailing one stays folded.
    st.view.diff_expanded.insert(all[0].key);
    st.diff = Some((target, diff.into()));
    st.settings.diff_layout = DiffLayout::Split;
    st.settings.diff_mode = DiffMode::WholeFile;
    render("diff_wrap_whole_file_light", st, false);
    remove_fixture(&dir);
}

#[test]
fn file_history_wrapped() {
    use crate::state::history::FileHistoryView;
    use trellis_core::repo::HistoryStart;
    let (mut st, dir) = with_wrap_repo("wrap-history", ThemeChoice::Dark, true);
    let h = repo::file_history(&dir, "wrap.rs", &HistoryStart::Head, 500).unwrap();
    let mut view = FileHistoryView::loading("wrap.rs".into(), None);
    view.append(h);
    assert_eq!(view.entries.len(), 2);
    view.selected = Some(0);
    let target = DiffTarget::Commit(view.entries[0].oid, view.entries[0].path.clone());
    let diff = decorated(&dir, &target);
    st.diff = Some((target, diff.into()));
    st.history = Some(view);
    render("file_history_wrapped", st, true);
    remove_fixture(&dir);
}
