//! Syntax highlighting of large diffs finished in the background: plain at once, colors a
//! moment later, stale results dropped, and reloads that keep their colors.

use super::diff_tests::drive_holding_diffs;
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::runner;
use crate::state::AppState;
use crate::state::lines::LineClick;
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::highlight::{self, DiffHighlight, Span, TokenKind};
use trellis_core::repo::{DiffTarget, FileDiff, HunkOp};
use trellis_core::settings::DiffMode;

/// `fn main() {`, 3,000 `let` lines, `}`; lines whose number `changed` picks get `+ 1`.
fn big_body(changed: impl Fn(usize) -> bool) -> String {
    let lets: String =
        (1..=3000).map(|n| if changed(n) { format!("    let v{n} = {n} + 1;\n") } else { format!("    let v{n} = {n};\n") }).collect();
    format!("fn main() {{\n{lets}}}\n")
}

/// A repository whose big.rs has every 10th line changed: about 300 hunks and 2,400 diff lines,
/// over the inline highlight limit. a.txt is modified too.
pub(super) fn big_repo(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    git(&dir, &["config", "core.autocrlf", "false"]);
    std::fs::write(dir.join("big.rs"), big_body(|_| false)).unwrap();
    git(&dir, &["add", "big.rs"]);
    git(&dir, &["commit", "-q", "-m", "big"]);
    std::fs::write(dir.join("big.rs"), big_body(|n| n % 10 == 0)).unwrap();
    dir
}

/// Like `drive`, but HighlightDiff effects are held back and returned.
pub(super) fn drive_holding_highlights(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    let mut held = Vec::new();
    let mut queue = vec![msg];
    while let Some(m) = queue.pop() {
        for effect in update(st, m, Instant::now()) {
            match effect {
                Effect::HighlightDiff { .. } => held.push(effect),
                Effect::LoadRepo { .. } | Effect::LoadStatus { .. } | Effect::LoadDiff { .. } | Effect::LocalOp { .. } => {
                    queue.extend(runner::execute(effect))
                }
                _ => {}
            }
        }
    }
    held
}

/// Runs a held HighlightDiff without consuming it (to apply its result late, or twice).
fn result_of(e: &Effect) -> Msg {
    let Effect::HighlightDiff { workdir, target, generation, diff } = e else { panic!("not a HighlightDiff") };
    let e = Effect::HighlightDiff { workdir: workdir.clone(), target: target.clone(), generation: *generation, diff: diff.clone() };
    runner::execute(e).expect("finished")
}

fn up(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn big() -> DiffTarget {
    DiffTarget::Unstaged("big.rs".into())
}

pub(super) fn shown(st: &AppState) -> &FileDiff {
    &st.diff.as_ref().expect("a diff is open").1
}

fn pending(hl: Option<&DiffHighlight>) -> bool {
    hl.expect("highlighted").is_pending()
}

/// Every line's spans, hunk by hunk.
fn spans_of(d: &FileDiff) -> Vec<Vec<Vec<Span>>> {
    let hl = d.highlight.as_ref().expect("highlighted");
    d.hunks.iter().enumerate().map(|(h, hk)| (0..hk.lines.len()).map(|l| hl.line(h, l).to_vec()).collect()).collect()
}

fn shape(d: &FileDiff) -> Vec<usize> {
    d.hunks.iter().map(|h| h.lines.len()).collect()
}

/// Each `let` line has a keyword span.
fn assert_colored(d: &FileDiff) {
    let hl = d.highlight.as_ref().expect("highlighted");
    assert!(!hl.is_pending());
    assert_eq!(hl.hunk_lens(), shape(d));
    for (h, hunk) in d.hunks.iter().enumerate() {
        for (l, _) in hunk.lines.iter().enumerate().filter(|(_, line)| line.text.contains("let ")) {
            assert!(hl.line(h, l).iter().any(|s| s.kind == TokenKind::Keyword), "hunk {h} line {l}: {:?}", hl.line(h, l));
        }
    }
}

pub(super) fn opened_big(name: &str) -> (AppState, PathBuf, Vec<Effect>) {
    let dir = big_repo(name);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let held = drive_holding_highlights(&mut st, Msg::Repo(RepoMsg::OpenDiff(big())));
    (st, dir, held)
}

#[test]
fn big_diff_opens_plain_then_colors() {
    let (mut st, dir, mut held) = opened_big("hl-open");
    let d = shown(&st);
    assert_eq!(st.diff.as_ref().map(|(t, _)| t), Some(&big()));
    assert!(d.hunks.len() > 250 && shape(d).iter().sum::<usize>() > highlight::MAX_LINES, "{}", d.hunks.len());
    assert!(pending(d.highlight.as_ref()));
    assert!(spans_of(d).iter().flatten().all(Vec::is_empty), "plain at once");
    assert_eq!(held.len(), 1);
    assert!(matches!(&held[0], Effect::HighlightDiff { generation, target, .. } if *generation == st.diff_shown_gen && *target == big()));

    let view = (st.view.diff_scroll_to_row, st.view.diff_top_row, st.view.diff_cursor, st.view.diff_lines.clone());
    let msg = runner::execute(held.remove(0)).expect("a result");
    assert!(matches!(msg, Msg::Scoped { .. }));
    assert!(up(&mut st, msg).is_empty());
    assert_colored(shown(&st));
    assert_eq!((st.view.diff_scroll_to_row, st.view.diff_top_row, st.view.diff_cursor, st.view.diff_lines.clone()), view);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn small_diff_needs_no_background_pass() {
    let dir = scratch_repo("hl-small");
    std::fs::write(dir.join("main.rs"), "fn main() {\n    let x = 1;\n}\n").unwrap();
    git(&dir, &["add", "main.rs"]);
    git(&dir, &["commit", "-q", "-m", "rust"]);
    std::fs::write(dir.join("main.rs"), "fn main() {\n    let x = 2;\n}\n").unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let held = drive_holding_highlights(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("main.rs".into()))));
    assert!(held.is_empty());
    assert_colored(shown(&st));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stale_highlight_result_is_dropped() {
    let (mut st, dir, held) = opened_big("hl-stale");
    let old = &held[0];

    // (a) Another file is open by the time the result lands.
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("a.txt".into()))));
    assert!(up(&mut st, result_of(old)).is_empty());
    assert_eq!(st.diff.as_ref().map(|(t, d)| (t.path(), d.highlight.is_none())), Some(("a.txt", true)));

    // (b) big.rs again: a newer generation, so only the newer result applies.
    let newer = drive_holding_highlights(&mut st, Msg::Repo(RepoMsg::OpenDiff(big())));
    assert_eq!(newer.len(), 1);
    up(&mut st, result_of(old));
    assert!(pending(shown(&st).highlight.as_ref()), "the older pass's result is stale");
    up(&mut st, result_of(&newer[0]));
    assert_colored(shown(&st));

    // (c) Closed.
    up(&mut st, Msg::Repo(RepoMsg::CloseDiff));
    assert!(up(&mut st, result_of(&newer[0])).is_empty());
    assert!(st.diff.is_none());

    // (d) The right generation, but another diff's shape.
    drive_holding_highlights(&mut st, Msg::Repo(RepoMsg::OpenDiff(big())));
    let small = FileDiff { hunks: shown(&st).hunks[..3].to_vec(), ..shown(&st).clone() };
    let hunks = highlight::highlight_diff("big.rs", &small, None);
    assert!(hunks.as_ref().is_some_and(|h| !h.is_pending()));
    let msg = Msg::DiffHighlighted { target: big(), generation: st.diff_shown_gen, hunks, whole: None };
    assert!(up(&mut st, msg).is_empty());
    assert!(pending(shown(&st).highlight.as_ref()), "a mismatched shape is ignored");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn whole_file_over_limit_gets_colors() {
    let dir = big_repo("hl-whole");
    let mut st = state();
    st.settings.diff_mode = DiffMode::WholeFile;
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let mut held = drive_holding_highlights(&mut st, Msg::Repo(RepoMsg::OpenDiff(big())));
    let w = shown(&st).whole.as_deref().expect("the whole file is loaded");
    assert!(pending(w.diff.highlight.as_ref()));
    assert_eq!(held.len(), 1);
    let msg = runner::execute(held.remove(0)).unwrap();
    up(&mut st, msg);
    assert_colored(&shown(&st).whole.as_deref().unwrap().diff);

    // Back to hunks: no reload, and the hunks were finished by the same job.
    let fx = up(&mut st, Msg::SetDiffMode(DiffMode::Hunks));
    assert!(!fx.iter().any(|e| matches!(e, Effect::LoadDiff { .. } | Effect::HighlightDiff { .. })));
    assert!(shown(&st).whole.is_none());
    assert_colored(shown(&st));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Executes the held diff loads and applies their results, returning the effects.
pub(super) fn land(st: &mut AppState, held: Vec<Effect>) -> Vec<Effect> {
    held.into_iter().filter_map(runner::execute).flat_map(|m| up(st, m)).collect()
}

#[test]
fn reload_keeps_colors_without_plain_flash() {
    // (1) Hunk view: staging one hunk reloads the rest fully colored at once (the previous spans)
    // while one background pass parses them again in context, as the old text (the index)
    // changed. The spans come out the same, as nothing before them opens a comment or string.
    let dir = big_repo("hl-reload");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(big())));
    assert_colored(shown(&st));
    let before = spans_of(shown(&st));
    let generation = st.hunk_ops_gen().expect("hunk buttons enabled");
    let held = drive_holding_diffs(&mut st, Msg::Hunk { op: HunkOp::Stage, path: "big.rs".into(), index: 0, generation });
    let mut fx = land(&mut st, held);
    assert!(pending(shown(&st).highlight.as_ref()));
    let after = spans_of(shown(&st));
    assert_eq!(after.len(), before.len() - 1);
    for (h, spans) in after.iter().enumerate() {
        assert_eq!(*spans, before[h + 1], "hunk {h} keeps its spans: no plain flash");
    }
    let passes = |fx: &[Effect], shown_gen: u64| {
        fx.iter().filter(|e| matches!(e, Effect::HighlightDiff { generation, .. } if *generation == shown_gen)).count()
    };
    assert_eq!(passes(&fx, st.diff_shown_gen), 1, "{fx:?}");
    let i = fx.iter().position(|e| matches!(e, Effect::HighlightDiff { .. })).unwrap();
    let msg = runner::execute(fx.remove(i)).unwrap();
    up(&mut st, msg);
    assert_colored(shown(&st));
    for (h, spans) in spans_of(shown(&st)).iter().enumerate() {
        assert_eq!(*spans, before[h + 1], "hunk {h} parsed in context");
    }

    // (2) Whole-file view: an edit far away leaves line 10 colored as before until the pass ends.
    st.settings.diff_mode = DiffMode::WholeFile;
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(big())));
    let whole = |st: &AppState| shown(st).whole.as_deref().expect("whole file").diff.clone();
    assert_colored(&whole(&st));
    let line10 = whole(&st).highlight.unwrap().line(0, 10).to_vec();
    assert!(!line10.is_empty());
    std::fs::write(dir.join("big.rs"), big_body(|n| n % 10 == 0 || n == 1495)).unwrap();
    let held = drive_holding_diffs(&mut st, Msg::Repo(RepoMsg::Refresh));
    let mut fx = land(&mut st, held);
    let w = whole(&st);
    assert!(pending(w.highlight.as_ref()));
    assert_eq!(w.highlight.as_ref().unwrap().line(0, 10), line10.as_slice(), "carried, no plain flash");
    let i = fx.iter().position(|e| matches!(e, Effect::HighlightDiff { generation, .. } if *generation == st.diff_shown_gen));
    let msg = runner::execute(fx.remove(i.expect("a background pass"))).unwrap();
    up(&mut st, msg);
    assert_colored(&whole(&st));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn line_selection_survives_same_reload_while_pending() {
    let (mut st, dir, _) = opened_big("hl-select");
    let line = shown(&st).hunks[0].lines.iter().position(|l| l.origin == '+').unwrap();
    up(&mut st, Msg::SelectDiffLine { hunk: 0, line, how: LineClick::Only });
    assert!(!st.view.diff_lines.is_empty());
    let selected = st.view.diff_lines.clone();
    let held = drive_holding_diffs(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(held.iter().any(|e| matches!(e, Effect::LoadDiff { reload: true, .. })));
    let fx = land(&mut st, held);
    assert!(pending(shown(&st).highlight.as_ref()));
    assert_eq!(st.view.diff_lines, selected);
    let job = fx.into_iter().find(|e| matches!(e, Effect::HighlightDiff { .. })).expect("a new background pass");

    // The pass finishes while another reload is on its way, which arrives pending: the text is
    // the same, so the selection stays.
    let reload = drive_holding_diffs(&mut st, Msg::Repo(RepoMsg::Refresh));
    up(&mut st, result_of(&job));
    assert!(!pending(shown(&st).highlight.as_ref()));
    land(&mut st, reload);
    assert_eq!(st.view.diff_lines, selected);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reload_adopts_finished_spans_on_screen() {
    let (mut st, dir, held) = opened_big("hl-adopt");
    let reload = drive_holding_diffs(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(reload.iter().any(|e| matches!(e, Effect::LoadDiff { previous: Some(p), .. } if pending(p.highlight.as_ref()))));
    // The background pass lands before the reload does.
    up(&mut st, result_of(&held[0]));
    assert_colored(shown(&st));
    let fx = land(&mut st, reload);
    assert!(!fx.iter().any(|e| matches!(e, Effect::HighlightDiff { .. })), "no second pass");
    assert_colored(shown(&st));
    let _ = std::fs::remove_dir_all(&dir);
}
