//! Diff viewer snapshots: layouts, line selection, syntax and word highlights, and hidden
//! whitespace. Shares the fixture repository and renderer with `snapshot_tests`.

use super::snapshot_tests::{git, render, with_repo};
use crate::state::AppState;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::highlight;
use trellis_core::intraline;
use trellis_core::repo::{self, DiffTarget, LoadOptions};
use trellis_core::settings::{DiffLayout, ThemeChoice};

/// The fixture's app.txt with an uneven edit: one line replaced, one added. `name` keeps
/// tests running in parallel apart.
fn with_diff(layout: DiffLayout, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, &format!("{name}-{layout:?}"));
    std::fs::write(dir.join("app.txt"), "app\nfixed build\nnew line\n").unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap()); // lists app.txt as changed
    let target = DiffTarget::Unstaged("app.txt".into());
    let mut diff = repo::load_diff(&dir, &target).unwrap();
    diff.intraline = intraline::intraline(&diff);
    st.diff = Some((target, diff));
    st.settings.diff_layout = layout;
    (st, dir)
}

#[test]
fn diff_inline_and_split() {
    let (st, dir) = with_diff(DiffLayout::Inline, "diff");
    render("diff_inline", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    let (st, dir) = with_diff(DiffLayout::Split, "diff");
    render("diff_split", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn diff_line_selection() {
    use crate::msg::Msg;
    use crate::state::lines::LineClick;
    for (layout, name) in [(DiffLayout::Inline, "diff_lines_inline"), (DiffLayout::Split, "diff_lines_split")] {
        let (mut st, dir) = with_diff(layout, "diff-lines");
        // Select `-fix` and `+new line`, as a click and a Ctrl-click would.
        let lines = &st.diff.as_ref().unwrap().1.hunks[0].lines;
        let at = |origin: char, text: &str| lines.iter().position(|l| l.origin == origin && l.text == text).unwrap();
        let (fix, new_line) = (at('-', "fix"), at('+', "new line"));
        for (line, how) in [(fix, LineClick::Only), (new_line, LineClick::Toggle)] {
            crate::update::update(&mut st, Msg::SelectDiffLine { hunk: 0, line, how }, Instant::now());
        }
        assert_eq!(st.view.diff_lines.count_in(&DiffTarget::Unstaged("app.txt".into()), 0), 2);
        render(name, st, true);
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
}

/// A Rust file with a doc comment, strings, numbers, keywords, a type and a function name;
/// the edit replaces one line and adds another.
fn with_rust_diff(layout: DiffLayout, theme: ThemeChoice) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, &format!("diff-syntax-{layout:?}"));
    let before = "/// Prints a greeting.
fn main() {
    let name = \"world\";
    let count = 3;
    println!(\"hello {name} x{count}\");
}
";
    let after = before.replace("let count = 3;", "let count: u32 = 5; // more greetings").replace(
        "x{count}\");
",
        "x{count}\");
    std::process::exit(0);
",
    );
    std::fs::write(dir.join("main.rs"), before).unwrap();
    let d = "2026-01-11T10:00:00+00:00";
    git(&dir, &["add", "main.rs"], d);
    git(&dir, &["commit", "-q", "-m", "Add main.rs"], d);
    std::fs::write(dir.join("main.rs"), after).unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let target = DiffTarget::Unstaged("main.rs".into());
    let mut diff = repo::load_diff(&dir, &target).unwrap();
    diff.highlight = highlight::highlight_diff(target.path(), &diff, None);
    assert!(diff.highlight.is_some(), "main.rs is highlighted as Rust");
    diff.intraline = intraline::intraline(&diff);
    st.diff = Some((target, diff));
    st.settings.diff_layout = layout;
    (st, dir)
}

#[test]
fn diff_syntax_highlighting() {
    let (st, dir) = with_rust_diff(DiffLayout::Inline, ThemeChoice::Dark);
    render("diff_syntax_inline", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    let (st, dir) = with_rust_diff(DiffLayout::Split, ThemeChoice::Light);
    render("diff_syntax_split_light", st, false);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

/// A one-word edit in a Rust file, highlighted and word-marked as the runner does.
fn with_word_diff(layout: DiffLayout, theme: ThemeChoice) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, &format!("diff-words-{layout:?}"));
    let before = "fn total(price: u32, quantity: u32) -> u32 {
    let total = price * quantity;
    total
}
";
    std::fs::write(dir.join("words.rs"), before).unwrap();
    let d = "2026-01-11T10:00:00+00:00";
    git(&dir, &["add", "words.rs"], d);
    git(&dir, &["commit", "-q", "-m", "Add words.rs"], d);
    std::fs::write(dir.join("words.rs"), before.replace("price * quantity;", "price * count;")).unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let target = DiffTarget::Unstaged("words.rs".into());
    let mut diff = repo::load_diff(&dir, &target).unwrap();
    diff.highlight = highlight::highlight_diff(target.path(), &diff, None);
    diff.intraline = intraline::intraline(&diff);
    st.diff = Some((target, diff));
    st.settings.diff_layout = layout;
    (st, dir)
}

#[test]
fn diff_word_highlights() {
    let (st, dir) = with_word_diff(DiffLayout::Inline, ThemeChoice::Dark);
    let d = &st.diff.as_ref().unwrap().1;
    let marked = d.hunks[0].lines.iter().enumerate().filter(|(l, _)| !d.intraline.line(0, *l).is_empty()).count();
    assert_eq!(marked, 2, "the -/+ pair is marked");
    render("diff_words_inline", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    let (st, dir) = with_word_diff(DiffLayout::Split, ThemeChoice::Light);
    render("diff_words_split_light", st, false);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

/// `file` committed as `before` and changed to `after`, opened with whitespace hidden as the
/// runner would load it (highlighted and word-marked).
fn with_hidden_whitespace(name: &str, file: &str, before: &str, after: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    std::fs::write(dir.join(file), before).unwrap();
    let d = "2026-01-11T10:00:00+00:00";
    git(&dir, &["add", file], d);
    git(&dir, &["commit", "-q", "-m", &format!("Add {file}")], d);
    std::fs::write(dir.join(file), after).unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let target = DiffTarget::Unstaged(file.into());
    let mut diff = repo::load_diff_with(&dir, &target, repo::LoadDiffOptions { ignore_whitespace: true }).unwrap();
    diff.highlight = highlight::highlight_diff(target.path(), &diff, None);
    diff.intraline = intraline::intraline(&diff);
    st.diff = Some((target, diff));
    st.settings.diff_ignore_whitespace = true;
    (st, dir)
}

#[test]
fn diff_ignore_whitespace() {
    // A re-indent and trailing spaces around one real change: only the real change shows, and
    // the hunk buttons are greyed out.
    let before = "fn area(w: u32, h: u32) -> u32 {\n    let a = w * h;\n    a\n}\n\n\
                  fn perimeter(w: u32, h: u32) -> u32 {\n    2 * (w + h)\n}\n";
    let after = before.replace("-> u32 {\n    let", "-> u32 {  \n        let").replace("    2 * (w + h)\n", "    2 * w + 2 * h   \n");
    let (st, dir) = with_hidden_whitespace("diff-ws", "ws.rs", before, &after);
    let d = &st.diff.as_ref().unwrap().1;
    let changed: Vec<&str> = d.hunks.iter().flat_map(|h| &h.lines).filter(|l| l.origin != ' ').map(|l| l.text.as_str()).collect();
    assert_eq!(changed, ["    2 * (w + h)", "    2 * w + 2 * h   "]);
    assert!(st.hunk_ops_blocked().is_some());
    render("diff_ignore_whitespace", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());

    // A file whose changes are all whitespace: no rows, but the toggle stays to show them again.
    let (st, dir) = with_hidden_whitespace("diff-ws-only", "notes.txt", "one\n  two\n", "one  \n\ttwo\n");
    assert!(st.diff.as_ref().unwrap().1.hunks.is_empty());
    render("diff_whitespace_only", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}
