//! The function context in hunk headers: colored inline for small diffs, by the background pass
//! for large ones, carried across reloads, and in the whole-file view's action rows.

use super::diff_tests::drive_holding_diffs;
use super::highlight_tests::{big_repo, drive_holding_highlights, land, opened_big, shown};
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::runner;
use std::time::Instant;
use trellis_core::highlight::{self, DiffHighlight, Span, TokenKind};
use trellis_core::repo::{DiffTarget, FileDiff, HunkOp};
use trellis_core::settings::DiffMode;

fn has(spans: &[Span], kind: TokenKind) -> bool {
    spans.iter().any(|s| s.kind == kind)
}

fn hl(d: &FileDiff) -> &DiffHighlight {
    d.highlight.as_ref().expect("highlighted")
}

fn headers(d: &FileDiff) -> Vec<Vec<Span>> {
    (0..d.hunks.len()).map(|h| hl(d).header(h).to_vec()).collect()
}

fn big() -> DiffTarget {
    DiffTarget::Unstaged("big.rs".into())
}

/// Runs the held background pass for the diff on screen and applies its result.
fn finish_held(st: &mut crate::state::AppState, fx: &mut Vec<Effect>) {
    let i = fx.iter().position(|e| matches!(e, Effect::HighlightDiff { generation, .. } if *generation == st.diff_shown_gen));
    let msg = runner::execute(fx.remove(i.expect("a background pass"))).expect("a result");
    update(st, msg, Instant::now());
}

#[test]
fn small_diff_colors_header_context_inline() {
    let dir = scratch_repo("hl-header-small");
    let body = |last: u32| {
        let lets: String = (1..=6).map(|n| format!("    let a{n} = {};\n", if n == 6 { last } else { n })).collect();
        format!("fn main() {{\n{lets}}}\n")
    };
    std::fs::write(dir.join("main.rs"), body(6)).unwrap();
    git(&dir, &["add", "main.rs"]);
    git(&dir, &["commit", "-q", "-m", "rust"]);
    std::fs::write(dir.join("main.rs"), body(60)).unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let held = drive_holding_highlights(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("main.rs".into()))));
    assert!(held.is_empty(), "small: no background pass");
    let d = shown(&st);
    let header = &d.hunks[0].header;
    assert!(header.ends_with("fn main() {"), "{header}");
    let at = highlight::context_start(header).expect("a function context");
    assert_eq!(&header[at..], "fn main() {");
    let spans = hl(d).header(0);
    assert!(has(spans, TokenKind::Keyword) && has(spans, TokenKind::Function), "{spans:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn big_diff_headers_plain_then_colored() {
    let (mut st, dir, mut held) = opened_big("hl-header-big");
    let d = shown(&st);
    assert!(d.hunks.iter().all(|h| h.header.ends_with("fn main() {")), "{:?}", d.hunks[0].header);
    assert!(hl(d).is_pending());
    assert!(headers(d).iter().all(Vec::is_empty), "plain while pending");
    finish_held(&mut st, &mut held);
    let d = shown(&st);
    assert!(!hl(d).is_pending());
    for (h, spans) in headers(d).iter().enumerate() {
        assert!(has(spans, TokenKind::Keyword), "hunk {h}: {spans:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reload_carries_header_spans() {
    let dir = big_repo("hl-header-reload");
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(big())));
    let before = headers(shown(&st));
    assert!(before.iter().all(|s| has(s, TokenKind::Keyword)));
    let generation = st.hunk_ops_gen().expect("hunk buttons enabled");
    let held = drive_holding_diffs(&mut st, Msg::Hunk { op: HunkOp::Stage, path: "big.rs".into(), index: 0, generation });
    let mut fx = land(&mut st, held);
    let d = shown(&st);
    assert!(hl(d).is_pending());
    let after = headers(d);
    assert_eq!(after.len(), before.len() - 1);
    for (h, spans) in after.iter().enumerate() {
        assert_eq!(*spans, before[h + 1], "hunk {h}: no plain flash");
    }
    finish_held(&mut st, &mut fx);
    let d = shown(&st);
    assert!(!hl(d).is_pending());
    for (h, spans) in headers(d).iter().enumerate() {
        assert_eq!(*spans, before[h + 1], "hunk {h} after the background pass");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn whole_file_view_colors_block_headers() {
    let dir = big_repo("hl-header-whole");
    let mut st = state();
    st.settings.diff_mode = DiffMode::WholeFile;
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    let mut held = drive_holding_highlights(&mut st, Msg::Repo(RepoMsg::OpenDiff(big())));
    assert!(shown(&st).whole.is_some(), "the whole file is loaded");
    finish_held(&mut st, &mut held);
    // The Block rows show the normal hunks' headers, colored from the normal diff's highlight.
    let d = shown(&st);
    assert!(!hl(d).is_pending());
    for (h, spans) in headers(d).iter().enumerate() {
        assert!(has(spans, TokenKind::Keyword), "hunk {h}: {spans:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn plain_text_header_stays_plain() {
    let dir = scratch_repo("hl-header-txt");
    let body = |last: &str| {
        let items: String = (1..=8).map(|n| if n == 8 { format!("  {last}\n") } else { format!("  item {n}\n") }).collect();
        format!("Shopping list\n{items}")
    };
    std::fs::write(dir.join("list.txt"), body("item 8")).unwrap();
    git(&dir, &["add", "list.txt"]);
    git(&dir, &["commit", "-q", "-m", "list"]);
    std::fs::write(dir.join("list.txt"), body("item eight")).unwrap();
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Unstaged("list.txt".into()))));
    let d = shown(&st);
    assert!(highlight::context_start(&d.hunks[0].header).is_some(), "{}", d.hunks[0].header);
    assert!(d.highlight.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
