//! Diff viewer snapshots: layouts, line selection, syntax and word highlights, hunk-header
//! context, and hidden whitespace (a diff with nothing to draw is in `snapshot_tests_empty_state`). Shares the fixture repository and renderer with
//! `snapshot_tests`.

use super::snapshot_tests::{git, render, with_repo};

// Declared here to keep them beside the diff snapshots they extend.
#[path = "snapshot_tests_empty_state.rs"]
mod snapshot_tests_empty_state;
#[path = "snapshot_tests_gaps.rs"]
mod snapshot_tests_gaps;
#[path = "snapshot_tests_lines_drag.rs"]
mod snapshot_tests_lines_drag;
#[path = "snapshot_tests_lines_gutter.rs"]
mod snapshot_tests_lines_gutter;
#[path = "snapshot_tests_reveal.rs"]
mod snapshot_tests_reveal;
use crate::msg::DiffMsg;
use crate::state::AppState;
use glowfly_core::highlight;
use glowfly_core::intraline;
use glowfly_core::repo::{self, DiffTarget, LoadOptions};
use glowfly_core::settings::{DiffLayout, ThemeChoice};
use std::path::PathBuf;
use std::time::Instant;

/// The fixture's app.txt with an uneven edit: one line replaced, one added. `name` keeps
/// tests running in parallel apart.
pub(super) fn with_diff(layout: DiffLayout, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, &format!("{name}-{layout:?}"));
    std::fs::write(dir.join("app.txt"), "app\nfixed build\nnew line\n").unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap()); // lists app.txt as changed
    let target = DiffTarget::Unstaged("app.txt".into());
    let mut diff = repo::load_diff(&dir, &target).unwrap();
    diff.intraline = intraline::intraline(&diff);
    st.diff = Some((target, diff.into()));
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
            crate::update::update(&mut st, Msg::Diff(DiffMsg::SelectDiffLine { hunk: 0, line, how }), Instant::now());
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
    st.diff = Some((target, diff.into()));
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

/// A change deep enough in a Rust function that the hunk header names it:
/// `@@ -4,6 +4,6 @@ fn greet(name: &str) -> String {`.
fn with_hunk_context(layout: DiffLayout, theme: ThemeChoice) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, &format!("diff-hunk-context-{layout:?}"));
    let before = "/// Greets someone.
fn greet(name: &str) -> String {
    let greeting = \"Hello\";
    let punctuation = '!';
    let width = 12;
    let shout = false;
    let count = 1;
    format!(\"{greeting}, {name}{punctuation} {width} {shout} {count}\")
}
";
    std::fs::write(dir.join("greet.rs"), before).unwrap();
    let d = "2026-01-11T10:00:00+00:00";
    git(&dir, &["add", "greet.rs"], d);
    git(&dir, &["commit", "-q", "-m", "Add greet.rs"], d);
    std::fs::write(dir.join("greet.rs"), before.replace("let count = 1;", "let count = 2;")).unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let target = DiffTarget::Unstaged("greet.rs".into());
    let mut diff = repo::load_diff(&dir, &target).unwrap();
    diff.highlight = highlight::highlight_diff(target.path(), &diff, None);
    diff.intraline = intraline::intraline(&diff);
    assert!(diff.hunks[0].header.contains("fn greet"), "{}", diff.hunks[0].header);
    assert!(diff.highlight.as_ref().is_some_and(|h| !h.header(0).is_empty()), "the context is highlighted");
    st.diff = Some((target, diff.into()));
    st.settings.diff_layout = layout;
    (st, dir)
}

#[test]
fn diff_hunk_context() {
    let (st, dir) = with_hunk_context(DiffLayout::Inline, ThemeChoice::Dark);
    render("diff_hunk_context_inline", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    let (st, dir) = with_hunk_context(DiffLayout::Split, ThemeChoice::Light);
    render("diff_hunk_context_split_light", st, false);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

/// A one-word edit in a Rust file, highlighted and word-marked as the runner does.
fn with_word_diff(layout: DiffLayout, theme: ThemeChoice) -> (AppState, PathBuf) {
    with_word_diff_in(layout, theme, &format!("diff-words-{layout:?}"))
}

/// `with_word_diff` in the fixture folder `name` (tests running in parallel need their own).
pub(super) fn with_word_diff_in(layout: DiffLayout, theme: ThemeChoice, name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(theme, name);
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
    st.diff = Some((target, diff.into()));
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
    let mut diff = repo::load_diff_with(&dir, &target, repo::LoadDiffOptions { ignore_whitespace: true, whole_file: false }).unwrap();
    diff.highlight = highlight::highlight_diff(target.path(), &diff, None);
    diff.intraline = intraline::intraline(&diff);
    st.diff = Some((target, diff.into()));
    st.settings.diff_ignore_whitespace = true;
    (st, dir)
}

#[test]
fn diff_ignore_whitespace() {
    // A re-indent and trailing spaces around one real change: only the real change shows, and
    // the hunk buttons stay on (they apply only the shown change).
    let before = "fn area(w: u32, h: u32) -> u32 {\n    let a = w * h;\n    a\n}\n\n\
                  fn perimeter(w: u32, h: u32) -> u32 {\n    2 * (w + h)\n}\n";
    let after = before.replace("-> u32 {\n    let", "-> u32 {  \n        let").replace("    2 * (w + h)\n", "    2 * w + 2 * h   \n");
    let (st, dir) = with_hidden_whitespace("diff-ws", "ws.rs", before, &after);
    let d = &st.diff.as_ref().unwrap().1;
    let changed: Vec<&str> = d.hunks.iter().flat_map(|h| &h.lines).filter(|l| l.origin != ' ').map(|l| l.text.as_str()).collect();
    assert_eq!(changed, ["    2 * (w + h)", "    2 * w + 2 * h   "]);
    assert!(st.hunk_ops_gen().is_some());
    render("diff_ignore_whitespace", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn diff_nav_first_file() {
    // app.txt is the first changed file (README.md is staged after it), opened at its only
    // change: Previous is dimmed, Next goes on to README.md.
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "diff-nav-first");
    git(&dir, &["add", "README.md"], "2026-01-11T10:00:00+00:00");
    std::fs::write(dir.join("app.txt"), "app\nfixed build\nnew line\n").unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let snap = st.snap.as_ref().unwrap();
    assert_eq!((snap.unstaged.len(), snap.staged.len()), (1, 1));
    let target = DiffTarget::Unstaged("app.txt".into());
    let mut diff = repo::load_diff(&dir, &target).unwrap();
    diff.intraline = intraline::intraline(&diff);
    st.diff = Some((target, diff.into()));
    use crate::state::diff_nav::{StepKind, step};
    assert_eq!([step(&st, -1).kind(), step(&st, 1).kind()], [StepKind::End, StepKind::File]);
    render("diff_nav_first_file", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

/// A comment inserted above two edited settings, word-marked and line-paired as the runner does.
fn with_similar_pairs(layout: DiffLayout) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, &format!("diff-similar-{layout:?}"));
    let before = "[server]\nhost = \"localhost\"\nport = 8080\ntimeout = 30\n";
    let after = "[server]\n# listen on all interfaces\nhost = \"0.0.0.0\"\nport = 9090\ntimeout = 30\n";
    std::fs::write(dir.join("config.txt"), before).unwrap();
    let d = "2026-01-11T10:00:00+00:00";
    git(&dir, &["add", "config.txt"], d);
    git(&dir, &["commit", "-q", "-m", "Add config.txt"], d);
    std::fs::write(dir.join("config.txt"), after).unwrap();
    st.snap = Some(repo::load(&dir, &LoadOptions::all(500)).unwrap());
    let target = DiffTarget::Unstaged("config.txt".into());
    let mut diff = repo::load_diff(&dir, &target).unwrap();
    diff.intraline = intraline::intraline(&diff);
    st.diff = Some((target, diff.into()));
    st.settings.diff_layout = layout;
    (st, dir)
}

#[test]
fn diff_similar_pairs() {
    use glowfly_core::diff_rows::{SplitRow, split_rows};
    let (st, dir) = with_similar_pairs(DiffLayout::Split);
    let d = &st.diff.as_ref().unwrap().1;
    let origins: String = d.hunks[0].lines.iter().map(|l| l.origin).collect();
    assert_eq!(origins, " --+++ ");
    let pair = |old, new| SplitRow::Pair { hunk: 0, old, new };
    let want = [
        SplitRow::Header(0),
        pair(Some(0), Some(0)),
        pair(None, Some(3)),
        pair(Some(1), Some(4)),
        pair(Some(2), Some(5)),
        pair(Some(6), Some(6)),
    ];
    assert_eq!(split_rows(d), want);
    let marked: Vec<usize> = (0..7).filter(|&l| !d.intraline.line(0, l).is_empty()).collect();
    assert_eq!(marked, [1, 2, 4, 5]);
    render("diff_similar_pairs_split", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    let (st, dir) = with_similar_pairs(DiffLayout::Inline);
    render("diff_similar_pairs_inline", st, true);
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn diff_hunk_buttons() {
    use super::snapshot_check::CheckSnapshot;
    use super::snapshot_tests::{harness, remove_fixture};
    use egui_kittest::kittest::Queryable;
    // Working tree: Stage Hunk at the right edge, Discard Hunk set apart to its left.
    let (st, dir) = with_diff(DiffLayout::Inline, "diff-hunk-buttons");
    let row_h = crate::ui::diff::DiffStyle::new(&st.settings).row_h;
    let mut h = harness(st, true);
    h.run();
    let stage = h.get_by_label("Stage hunk").bounding_box().expect("the button has bounds");
    let discard = h.get_by_label("Discard hunk").bounding_box().expect("the button has bounds");
    assert!(stage.x0 > discard.x1, "Stage sits at the edge, right of Discard");
    assert!(stage.x0 - discard.x1 >= 16.0, "a wider gap sets Discard apart");
    for b in [stage, discard] {
        assert!(((b.y1 - b.y0) as f32 - (row_h - 2.0)).abs() < 0.5, "the buttons fill the row");
    }
    h.check_snapshot("diff_hunk_buttons");
    drop(h);
    remove_fixture(&dir);

    // The smallest diff font: the buttons still fit the shorter row.
    let (mut st, dir) = with_diff(DiffLayout::Inline, "diff-hunk-buttons-small");
    st.settings.diff_font_size = *glowfly_core::settings::DIFF_FONT_SIZES.start();
    let row_h = crate::ui::diff::DiffStyle::new(&st.settings).row_h;
    let mut h = harness(st, true);
    h.run();
    let stage = h.get_by_label("Stage hunk").bounding_box().expect("the button has bounds");
    assert!(((stage.y1 - stage.y0) as f32 - (row_h - 2.0)).abs() < 0.5);
    h.check_snapshot("diff_hunk_buttons_small_font");
    drop(h);
    remove_fixture(&dir);

    // A reload on its way disables both, saying why.
    let (mut st, dir) = with_diff(DiffLayout::Inline, "diff-hunk-buttons-reload");
    st.diff_gen += 1;
    assert_eq!(st.hunk_ops_gen(), None);
    let mut h = harness(st, true);
    h.run();
    assert!(h.get_by_label("Stage hunk").is_disabled());
    assert!(h.get_by_label("Discard hunk").is_disabled());
    h.get_by_label("Stage hunk").hover();
    h.run();
    assert!(h.query_by_label_contains("Updating the diff").is_some(), "the hover says why");
    drop(h);
    remove_fixture(&dir);
}
