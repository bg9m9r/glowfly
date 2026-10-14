//! Hunks highlighted in context (from the old file's lines before them): inline for small files,
//! corrected in place by the background pass for larger ones, and never mixed up with per-hunk
//! spans on a reload.

use super::highlight_tests::drive_holding_highlights;
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::Msg;
use crate::runner;
use crate::state::AppState;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::highlight::{self, DiffHighlight, Span, TokenKind};
use trellis_core::repo::{self, DiffTarget, FileDiff, LoadDiffOptions};

/// `code` lines of `let vN = N;`, then a 30-line block comment of `let cN = N;` lines, then
/// `fn end() {}`. With `edit`, comment line 15 changes.
fn source(code: usize, edit: bool) -> String {
    let mut text: String = (1..=code).map(|n| format!("let v{n} = {n};\n")).collect();
    text += "/*\n";
    text +=
        &(1..=30).map(|n| if n == 15 && edit { "let c15 = 150;\n".to_owned() } else { format!("let c{n} = {n};\n") }).collect::<String>();
    text += "*/\nfn end() {}\n";
    text
}

/// A repository with a.rs committed as `source(code, false)` and edited inside its comment.
fn repo_with(name: &str, code: usize) -> PathBuf {
    let dir = scratch_repo(name);
    git(&dir, &["config", "core.autocrlf", "false"]);
    std::fs::write(dir.join("a.rs"), source(code, false)).unwrap();
    git(&dir, &["add", "a.rs"]);
    git(&dir, &["commit", "-q", "-m", "a"]);
    std::fs::write(dir.join("a.rs"), source(code, true)).unwrap();
    dir
}

fn target() -> DiffTarget {
    DiffTarget::Unstaged("a.rs".into())
}

fn shown(st: &AppState) -> &FileDiff {
    &st.diff.as_ref().expect("a diff is open").1
}

fn hl(d: &FileDiff) -> &DiffHighlight {
    d.highlight.as_ref().expect("highlighted")
}

/// The spans of every `let` line of the one hunk.
fn let_lines(d: &FileDiff) -> Vec<&[Span]> {
    assert_eq!(d.hunks.len(), 1);
    d.hunks[0].lines.iter().enumerate().filter(|(_, l)| l.text.starts_with("let ")).map(|(i, _)| hl(d).line(0, i)).collect()
}

fn all_comment(d: &FileDiff) -> bool {
    let_lines(d).iter().all(|s| !s.is_empty() && s.iter().all(|s| s.kind == TokenKind::Comment))
}

fn opened(name: &str, code: usize) -> (AppState, PathBuf, Vec<Effect>) {
    let dir = repo_with(name, code);
    let mut st = state();
    drive(&mut st, Msg::Open(dir.clone()));
    let held = drive_holding_highlights(&mut st, Msg::OpenDiff(target()));
    (st, dir, held)
}

#[test]
fn comment_hunk_colored_in_context_inline() {
    let (st, dir, held) = opened("hlc-inline", 10);
    assert!(held.is_empty(), "small: no background pass");
    let d = shown(&st);
    assert!(!hl(d).is_pending());
    assert!(all_comment(d), "{:?}", let_lines(d));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn medium_file_opens_colored_then_corrects() {
    let (mut st, dir, mut held) = opened("hlc-medium", 2_450);
    let d = shown(&st);
    assert!(hl(d).is_pending(), "the lines before the hunk are over the inline limit");
    assert!(
        let_lines(d).iter().all(|s| s.iter().any(|s| s.kind == TokenKind::Keyword)),
        "per-hunk colors at once, no plain flash: {:?}",
        let_lines(d)
    );
    assert_eq!(held.len(), 1);
    assert!(matches!(&held[0], Effect::HighlightDiff { generation, .. } if *generation == st.diff_shown_gen));

    let view = (st.view.diff_top_row, st.view.diff_cursor, st.view.diff_lines.clone());
    let msg = runner::execute(held.remove(0)).expect("a result");
    assert!(update(&mut st, msg, Instant::now()).is_empty());
    let d = shown(&st);
    assert!(!hl(d).is_pending());
    assert!(all_comment(d), "corrected in context: {:?}", let_lines(d));
    assert_eq!((st.view.diff_top_row, st.view.diff_cursor, st.view.diff_lines.clone()), view);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn adopt_requires_same_old_text() {
    let (mut st, dir, _) = opened("hlc-adopt", 2_450);
    // On screen: the same hunks, finished, but each parsed on its own.
    let (_, d) = st.diff.as_mut().unwrap();
    d.highlight = highlight::highlight_diff("a.rs", d, None);
    assert!(d.highlight.as_ref().is_some_and(|h| !h.is_pending()));
    let per_hunk = d.highlight.clone();

    // A reload arrives pending, to be parsed in context: the per-hunk spans don't stand in.
    let repo::ViewDiff { diff: mut fresh, old, .. } = repo::load_diff_for_view(&dir, &target(), LoadDiffOptions::default()).unwrap();
    assert!(old.is_some());
    runner::decorate_in_context("a.rs", None, &mut fresh, None, old.as_ref());
    assert!(hl(&fresh).is_pending());
    let msg = Msg::DiffLoaded { target: target(), reload: true, generation: st.diff_gen, result: Ok(fresh) };
    let fx = update(&mut st, msg, Instant::now());
    assert!(hl(shown(&st)).is_pending(), "not adopted");
    assert_ne!(shown(&st).highlight, per_hunk);
    assert!(fx.iter().any(|e| matches!(e, Effect::HighlightDiff { .. })), "{fx:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
