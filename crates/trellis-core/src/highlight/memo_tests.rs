//! Parse checkpoints: content-defined chunks, and reloads that resume from the previous
//! highlight's checkpoints with the colours of a fresh parse.

use super::memo::{CHUNK_MASK, CHUNK_MAX, chunks, line_hash};
use super::*;
use crate::repo::Hunk;

/// Replaces old lines `at..at + del` with `ins`.
#[derive(Clone)]
struct Edit {
    at: usize,
    del: usize,
    ins: Vec<String>,
}

fn edit(at: usize, del: usize, ins: &[&str]) -> Edit {
    Edit { at, del, ins: ins.iter().map(|s| (*s).to_owned()).collect() }
}

/// `n` one-line Rust functions.
fn rust_file(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("fn f{i}() -> u32 {{ {i} }}")).collect()
}

fn apply(old: &[String], e: &Edit) -> Vec<String> {
    let mut out = old[..e.at].to_vec();
    out.extend(e.ins.iter().cloned());
    out.extend(old[e.at + e.del..].iter().cloned());
    out
}

/// The diff of `edits` (in order, far apart) against `old`, with 3 lines of context, and its
/// old text.
fn build(old: &[String], edits: &[Edit]) -> (FileDiff, Arc<OldText>) {
    let mut hunks = Vec::new();
    let mut shift = 0i64;
    for e in edits {
        let (c0, c1) = (e.at.saturating_sub(3), (e.at + e.del + 3).min(old.len()));
        let (mut o, mut n) = (c0 as u32 + 1, (c0 as i64 + shift) as u32 + 1);
        let (o0, n0) = (o, n);
        let mut lines = Vec::new();
        let mut push = |origin: char, text: &str, old_no: bool, new_no: bool| {
            lines.push(DiffLine { origin, old: old_no.then_some(o), new: new_no.then_some(n), text: text.to_owned() });
            o += old_no as u32;
            n += new_no as u32;
        };
        (c0..e.at).for_each(|i| push(' ', &old[i], true, true));
        (e.at..e.at + e.del).for_each(|i| push('-', &old[i], true, false));
        e.ins.iter().for_each(|l| push('+', l, false, true));
        (e.at + e.del..c1).for_each(|i| push(' ', &old[i], true, true));
        let header = format!("@@ -{o0},{} +{n0},{} @@", o - o0, n - n0);
        hunks.push(Hunk { header, lines });
        shift += e.ins.len() as i64 - e.del as i64;
    }
    let content = old.join("\n") + "\n";
    let old_text = OldText::new(content.as_bytes(), &hunks).expect("the hunks fit the old text");
    let diff = FileDiff { binary: false, hunks, highlight: None, intraline: Default::default(), whole: None };
    (diff, Arc::new(old_text))
}

/// `d` highlighted from scratch, finished in the background if need be.
fn fresh(d: &FileDiff, old: &Arc<OldText>) -> DiffHighlight {
    let mut d = d.clone();
    d.highlight = highlight_in_context("a.rs", &d, None, Some(old));
    if d.highlight.as_ref().is_some_and(DiffHighlight::is_pending) {
        d.highlight = finish(&d, &|| false);
    }
    d.highlight.expect("highlighted")
}

/// `d` highlighted while it loads, then finished: what a shown diff holds afterwards.
fn shown(mut d: FileDiff, old: &Arc<OldText>) -> FileDiff {
    let hl = highlight_in_context("a.rs", &d, None, Some(old)).expect("rust");
    assert!(hl.is_pending(), "over the inline limits on its own");
    d.highlight = Some(hl);
    let done = finish(&d, &|| false).expect("finished");
    assert!(!done.is_pending());
    assert!(done.checkpoints() > 0);
    d.highlight = Some(done);
    d
}

fn refs(v: &[String]) -> Vec<&str> {
    v.iter().map(String::as_str).collect()
}

#[test]
fn chunks_are_content_defined_and_bounded() {
    let a = rust_file(3_000);
    let block: Vec<String> = (0..37).map(|i| format!("let inserted{i} = {i};")).collect();
    let mut b = a[..1_000].to_vec();
    b.extend(block.iter().cloned());
    b.extend(a[1_000..].iter().cloned());
    let (ca, cb) = (chunks(&refs(&a)), chunks(&refs(&b)));
    // The chunks cover the lines in order.
    assert_eq!(ca.first().map(|r| r.start), Some(0));
    assert_eq!(ca.last().map(|r| r.end), Some(a.len()));
    assert!(ca.windows(2).all(|w| w[0].end == w[1].start));
    // After the first boundary past the insertion, every boundary is the same, shifted.
    let first = ca.iter().map(|r| r.end).find(|&e| e > 1_000).expect("a boundary after the insertion");
    let ends_a: Vec<usize> = ca.iter().map(|r| r.end).filter(|&e| e >= first).map(|e| e + block.len()).collect();
    let ends_b: Vec<usize> = cb.iter().map(|r| r.end).filter(|&e| e >= first + block.len()).collect();
    assert_eq!(ends_a, ends_b);

    // A run with no content boundary is cut at CHUNK_MAX lines.
    let plain = (0..).map(|i| format!("x{i}")).find(|l| line_hash(l) & CHUNK_MASK != 0).unwrap();
    let same = vec![plain.as_str(); 1_000];
    let cs = chunks(&same);
    assert!(cs.iter().all(|r| r.len() <= CHUNK_MAX), "{cs:?}");
    assert_eq!(cs[0].len(), CHUNK_MAX);
}

#[test]
fn staging_a_hunk_finishes_inline() {
    let old = rust_file(MAX_LINES * 3);
    let edits = [
        edit(20, 1, &["fn top() -> u32 { 0 }"]),
        edit(3_000, 2, &["fn mid() -> u32 {", "    1", "}"]),
        edit(5_900, 1, &["fn end() -> u32 { 2 }"]),
    ];
    let (d1, o1) = build(&old, &edits);
    let d1 = shown(d1, &o1);

    // Hunk 1 staged: the old file now holds it; hunks 0 and 2 stay unstaged.
    let staged = apply(&old, &edits[1]);
    let shifted = Edit { at: edits[2].at + 1, ..edits[2].clone() };
    let (d2, o2) = build(&staged, &[edits[0].clone(), shifted]);
    let hl = highlight_in_context("a.rs", &d2, Some(&d1), Some(&o2)).expect("rust");
    assert!(!hl.is_pending(), "resumed from the checkpoints inline");
    assert!(hl.checkpoints() > 0, "keeps checkpoints for the next reload");
    assert_eq!(hl, fresh(&d2, &o2));
    assert_eq!(hl.lines, fresh(&d2, &o2).lines);
}

#[test]
fn edit_far_above_reuses_checkpoints() {
    let old = rust_file(MAX_LINES * 3);
    let last = edit(5_900, 1, &["fn end() -> u32 { 2 }"]);
    let (d1, o1) = build(&old, std::slice::from_ref(&last));
    let d1 = shown(d1, &o1);

    let (d2, o2) = build(&old, &[edit(50, 1, &["fn early() -> u32 { 9 }"]), last]);
    let hl = highlight_in_context("a.rs", &d2, Some(&d1), Some(&o2)).expect("rust");
    assert!(!hl.is_pending());
    assert_eq!(hl.lines, fresh(&d2, &o2).lines);
}

#[test]
fn state_changes_are_not_reused_stale() {
    let old = rust_file(MAX_LINES * 3);
    let edits =
        [edit(10, 1, &["/* fn f10() -> u32 { 10 }"]), edit(3_000, 1, &["fn g() -> u32 { 0 }"]), edit(5_900, 1, &["fn h() -> u32 { 0 }"])];
    let (d1, o1) = build(&old, &edits);
    let d1 = shown(d1, &o1);
    let removed = |hl: &DiffHighlight, d: &FileDiff, h: usize| {
        let l = d.hunks[h].lines.iter().position(|l| l.origin == '-').unwrap();
        hl.line(h, l).to_vec()
    };
    let before = d1.highlight.as_ref().unwrap();
    assert!(removed(before, &d1, 1).iter().any(|s| s.kind != TokenKind::Comment), "the old side isn't in the comment yet");

    // Hunk 0 staged: the old side is now inside the comment too.
    let (d2, o2) = build(&apply(&old, &edits[0]), &edits[1..]);
    let hl = highlight_in_context("a.rs", &d2, Some(&d1), Some(&o2)).expect("rust");
    assert!(!hl.is_pending());
    for h in 0..2 {
        let spans = removed(&hl, &d2, h);
        assert!(!spans.is_empty() && spans.iter().all(|s| s.kind == TokenKind::Comment), "hunk {h}: {spans:?}");
    }
    assert_eq!(hl.lines, fresh(&d2, &o2).lines);
}

#[test]
fn a_pending_highlight_carries_the_checkpoints_it_was_built_from() {
    let old = rust_file(MAX_LINES * 3);
    let last = edit(5_900, 1, &["fn end() -> u32 { 2 }"]);
    let (d1, o1) = build(&old, std::slice::from_ref(&last));
    let d1 = shown(d1, &o1);

    // Too much new text to finish inline: pending, carrying d1's checkpoints.
    let block: Vec<String> = (0..MAX_LINES + 500).map(|i| format!("fn new{i}() {{}}")).collect();
    let big = Edit { at: 100, del: 0, ins: block };
    let (mut d2, o2) = build(&old, &[big, last.clone()]);
    let hl2 = highlight_in_context("a.rs", &d2, Some(&d1), Some(&o2)).expect("rust");
    assert!(hl2.is_pending());
    assert_eq!(hl2.checkpoints(), d1.highlight.as_ref().unwrap().checkpoints());
    d2.highlight = Some(hl2);

    // The next reload, a small edit, resumes from them while d2 is still pending.
    let (d3, o3) = build(&old, &[edit(60, 1, &["fn small() {}"]), last]);
    let hl3 = highlight_in_context("a.rs", &d3, Some(&d2), Some(&o3)).expect("rust");
    assert!(!hl3.is_pending());
    assert_eq!(hl3.lines, fresh(&d3, &o3).lines);

    // No previous highlight, or one in another language: pending as before.
    assert!(highlight_in_context("a.rs", &d3, None, Some(&o3)).unwrap().is_pending());
    let mut other = d1.clone();
    other.highlight.as_mut().unwrap().syntax = "JavaScript".into();
    assert!(highlight_in_context("a.rs", &d3, Some(&other), Some(&o3)).unwrap().is_pending());
}

#[test]
fn checkpoints_never_make_highlights_differ() {
    let old = rust_file(MAX_LINES * 3);
    let (d, o) = build(&old, &[edit(5_900, 1, &["fn end() {}"])]);
    let d = shown(d, &o);
    let hl = d.highlight.unwrap();
    let bare = DiffHighlight { memo: Checkpoints::default(), ..hl.clone() };
    assert_eq!(hl, bare);
    assert_eq!(bare.checkpoints(), 0);
    assert!(format!("{hl:?}").contains("Checkpoints("));
}
