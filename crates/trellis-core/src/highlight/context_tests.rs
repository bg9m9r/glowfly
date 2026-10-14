//! Hunks highlighted in context: from the old file's lines before them, with both sides' parse
//! states carried across hunks, within the limits, and falling back to per-hunk parsing.

use super::tests::{assert_covers, diff, kinds};
use super::*;
use crate::repo::{self, DiffTarget, Hunk, LoadDiffOptions};
use crate::testutil::{git, scratch_repo, write};
use std::cell::Cell;
use std::path::{Path, PathBuf};

/// A hunk whose first line is old line `start + 1` (and new line `start + 1`).
pub(super) fn hunk_at(start: usize, lines: &[(char, &str)]) -> Hunk {
    let (mut old, mut new) = (start as u32 + 1, start as u32 + 1);
    let lines: Vec<_> = lines
        .iter()
        .map(|&(origin, text)| {
            let o = matches!(origin, ' ' | '-').then_some(old);
            let n = matches!(origin, ' ' | '+').then_some(new);
            old += o.is_some() as u32;
            new += n.is_some() as u32;
            DiffLine { origin, old: o, new: n, text: text.into() }
        })
        .collect();
    let olds = lines.iter().filter(|l| l.old.is_some()).count();
    let header = if olds == 0 { format!("@@ -{start},0 +{} @@", start + 1) } else { format!("@@ -{},{olds} +{} @@", start + 1, start + 1) };
    Hunk { header, lines }
}

/// A repository with `file` committed as `before` and changed to `after` in the working tree.
pub(super) fn changed(name: &str, file: &str, before: &str, after: &str) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, file, before);
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    write(&dir, file, after);
    dir
}

/// The unstaged diff of `file` with its old text.
pub(super) fn unstaged(dir: &Path, file: &str) -> (FileDiff, Option<Arc<OldText>>) {
    repo::load_diff_for_view(dir, &DiffTarget::Unstaged(file.into()), LoadDiffOptions::default()).map(|v| (v.diff, v.old)).unwrap()
}

fn only(spans: &[Span], kind: TokenKind) -> bool {
    !spans.is_empty() && spans.iter().all(|s| s.kind == kind)
}

fn has(spans: &[Span], kind: TokenKind) -> bool {
    kinds(spans).contains(&kind)
}

/// `fn a() {}`, a 30-line block comment, then `fn b` with 20 `let` lines; `edit` maps each line.
fn commented(edit: impl Fn(usize, String) -> String) -> String {
    let mut lines = vec!["fn a() {}".to_owned(), "/*".to_owned()];
    lines.extend((1..=30).map(|n| format!("let x = {n}; // in comment")));
    lines.push("*/".into());
    lines.push("fn b() {".into());
    lines.extend((1..=20).map(|n| format!("    let y = {n};")));
    lines.push("}".into());
    lines.into_iter().enumerate().map(|(i, l)| edit(i + 1, l) + "\n").collect()
}

#[test]
fn hunk_inside_block_comment_is_comment() {
    let before = commented(|_, l| l);
    let after = commented(|n, l| match n {
        22 => "let x = 20; // changed".into(),
        48 => "    let y = 150;".into(),
        _ => l,
    });
    let dir = changed("hl-ctx-comment", "a.rs", &before, &after);
    let (d, old) = unstaged(&dir, "a.rs");
    assert_eq!(d.hunks.len(), 2);
    assert!(old.is_some(), "the index blob matches the hunks");
    let hl = highlight_in_context("a.rs", &d, None, old.as_ref()).expect("rust");
    assert!(!hl.is_pending());
    assert!(hl.old_text.is_some());
    for (l, dl) in d.hunks[0].lines.iter().enumerate() {
        assert!(only(hl.line(0, l), TokenKind::Comment), "hunk 0 line {l} {:?}: {:?}", dl.text, hl.line(0, l));
        assert_covers(hl.line(0, l), &dl.text);
    }
    for (l, _) in d.hunks[1].lines.iter().enumerate().filter(|(_, dl)| dl.text.contains("let ")) {
        assert!(has(hl.line(1, l), TokenKind::Keyword), "hunk 1 line {l}: {:?}", hl.line(1, l));
    }
    // Each hunk on its own (the old behaviour) colors the comment as code.
    let alone = highlight_diff("a.rs", &d, None).unwrap();
    assert!((0..d.hunks[0].lines.len()).any(|l| has(alone.line(0, l), TokenKind::Keyword)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sides_diverge_across_hunks() {
    let before: String = (1..=50).map(|n| format!("let v{n} = {n};\n")).collect();
    // The new side opens a comment after line 4 and closes it after line 39; line 38 changes.
    let mut after = String::new();
    for n in 1..=50 {
        after += &if n == 38 { "let v38 = 380;\n".to_owned() } else { format!("let v{n} = {n};\n") };
        if n == 4 {
            after += "/*\n";
        }
        if n == 39 {
            after += "*/\n";
        }
    }
    let dir = changed("hl-ctx-diverge", "a.rs", &before, &after);
    let (d, old) = unstaged(&dir, "a.rs");
    assert_eq!(d.hunks.len(), 2, "{:?}", d.hunks.iter().map(|h| &h.header).collect::<Vec<_>>());
    let hl = highlight_in_context("a.rs", &d, None, old.as_ref()).expect("rust");
    assert!(hl.old_text.is_some());
    let h1 = &d.hunks[1].lines;
    let close = h1.iter().position(|l| l.origin == '+' && l.text == "*/").expect("the added */");
    let context: Vec<_> = (0..close).filter(|&l| h1[l].origin == ' ').collect();
    assert!(!context.is_empty());
    for l in context {
        assert!(only(hl.line(1, l), TokenKind::Comment), "new side is in the comment: {:?} {:?}", h1[l].text, hl.line(1, l));
    }
    let removed = h1.iter().position(|l| l.origin == '-').expect("the removed line 38");
    assert!(has(hl.line(1, removed), TokenKind::Keyword), "old side has no comment: {:?}", hl.line(1, removed));
    let added = h1.iter().position(|l| l.origin == '+' && l.text.starts_with("let")).unwrap();
    assert!(only(hl.line(1, added), TokenKind::Comment), "{:?}", hl.line(1, added));
    let after_close = h1.iter().enumerate().skip(close + 1).find(|(_, l)| l.origin == ' ').map(|(i, _)| i).unwrap();
    assert!(has(hl.line(1, after_close), TokenKind::Keyword), "code again after */");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn python_docstring() {
    let doc = |changed: bool| -> String {
        let body: String = (1..=25)
            .map(|n| {
                if n == 13 && changed {
                    "    for every item in the list\n".to_owned()
                } else {
                    format!("    for each item {n} in the list\n")
                }
            })
            .collect();
        format!("def f():\n    \"\"\"\n{body}    \"\"\"\n    return 1\n")
    };
    let dir = changed("hl-ctx-python", "a.py", &doc(false), &doc(true));
    let (d, old) = unstaged(&dir, "a.py");
    let hl = highlight_in_context("a.py", &d, None, old.as_ref()).expect("python");
    assert!(hl.old_text.is_some());
    // The grammar scopes a docstring as a string and as documentation (a comment): either way,
    // one kind for the whole line, never code.
    for (l, dl) in d.hunks[0].lines.iter().enumerate() {
        let doc = only(hl.line(0, l), TokenKind::String) || only(hl.line(0, l), TokenKind::Comment);
        assert!(doc, "line {l} {:?}: {:?}", dl.text, hl.line(0, l));
    }
    let alone = highlight_diff("a.py", &d, None).unwrap();
    assert!((0..d.hunks[0].lines.len()).any(|l| has(alone.line(0, l), TokenKind::Keyword)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn old_text_rejects_mismatch() {
    let h = hunk_at(0, &[(' ', "a"), (' ', "b"), (' ', "c"), ('+', "d")]);
    assert!(OldText::new(b"a\nb\nc\n", std::slice::from_ref(&h)).is_some());
    assert_eq!(OldText::new(b"a\nb\nX\n", std::slice::from_ref(&h)), None, "line 3 differs");
    assert_eq!(OldText::new(b"a\nb\n", std::slice::from_ref(&h)), None, "line 3 is missing");

    // A trailing newline adds no empty line: three lines, so a hunk can start after line 3 but
    // not after line 4.
    let end = OldText::new(b"a\nb\nc\n", &[hunk_at(3, &[('+', "d")])]).expect("inserts after line 3");
    assert_eq!(format!("{end:?}"), "OldText(3 lines)");
    assert_eq!(end.gaps(&[hunk_at(3, &[('+', "d")])]).map(|g| g[0].len()), Some(3));
    assert_eq!(OldText::new(b"a\nb\nc\n", &[hunk_at(4, &[('+', "d")])]), None);
    assert_eq!(end.gaps(&[hunk_at(10, &[('+', "z")])]), None, "hunks from another diff");
    assert_eq!(end.gaps(&[hunk_at(2, &[(' ', "c")])]), None);
    // Overlapping hunks can't be placed.
    let overlap = [hunk_at(0, &[(' ', "a"), (' ', "b")]), hunk_at(1, &[(' ', "b")])];
    assert_eq!(OldText::new(b"a\nb\nc\n", &overlap), None);

    // CRLF content (no conversion) validates against the hunks loaded from it.
    let crlf =
        |edit: usize| -> String { (1..=20).map(|n| if n == edit { "changed\r\n".to_owned() } else { format!("line {n}\r\n") }).collect() };
    let dir = changed("hl-ctx-crlf", "c.rs", &crlf(0), &crlf(12));
    let (d, old) = unstaged(&dir, "c.rs");
    assert_eq!(d.hunks.len(), 1);
    assert!(old.is_some(), "CRLF lines are trimmed like the diff's");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `let xN = N;` for N in 0..n, one per line.
fn lets(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("let x{i} = {i};")).collect()
}

/// Old text of `lines` and a diff adding `let z = 1;` after the last of them (which is its one
/// context line).
fn after_lines(lines: &[String]) -> (FileDiff, Arc<OldText>) {
    let content: String = lines.iter().map(|l| format!("{l}\n")).collect();
    let n = lines.len();
    let h = hunk_at(n - 1, &[(' ', &lines[n - 1]), ('+', "let z = 1;")]);
    let old = OldText::new(content.as_bytes(), std::slice::from_ref(&h)).expect("matches");
    (diff(vec![h]), Arc::new(old))
}

#[test]
fn context_counts_toward_inline_limit() {
    let (mut d, old) = after_lines(&lets(MAX_LINES + 50));
    let hl = highlight_in_context("a.rs", &d, None, Some(&old)).expect("within the background limits");
    assert!(hl.is_pending(), "the lines before the hunk count");
    assert!(hl.old_text.is_some());
    assert!(has(hl.line(0, 1), TokenKind::Keyword), "provisional spans from the hunk alone: {:?}", hl.line(0, 1));
    d.highlight = Some(hl);

    let done = finish(&d, &|| false).expect("finished");
    assert!(!done.is_pending());
    assert!(done.old_text.is_some(), "parsed in context");
    assert!((0..2).all(|l| has(done.line(0, l), TokenKind::Keyword)));

    // Cancelled once parsing has started: nothing.
    let calls = Cell::new(0);
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() > 1
    };
    assert_eq!(finish(&d, &cancelled), None);
    assert!(calls.get() > 1, "checked while parsing the lines before the hunk");
}

#[test]
fn too_large_context_falls_back_per_hunk() {
    let lines: Vec<String> = (0..=BACKGROUND_MAX_LINES).map(|_| "let a = 1;".to_owned()).collect();
    let (d, old) = after_lines(&lines);
    let hl = highlight_in_context("a.rs", &d, None, Some(&old)).expect("per hunk");
    assert!(!hl.is_pending());
    assert_eq!(hl.old_text, None);
    assert!(has(hl.line(0, 1), TokenKind::Keyword));
    assert_eq!(Some(hl), highlight_diff("a.rs", &d, None));
}

#[test]
fn unchanged_reload_keeps_context_spans() {
    let text = lets(10);
    let (mut prev, old) = after_lines(&text);
    let mut hl = highlight_in_context("a.rs", &prev, None, Some(&old)).unwrap();
    assert!(!hl.is_pending() && hl.old_text.is_some());
    // Mark the spans so reuse is visible: a fresh parse would never produce this.
    let marker = vec![Span { start: 0, end: 1, kind: TokenKind::Attribute }];
    hl.lines[0][0] = marker.clone();
    prev.highlight = Some(hl);

    // Same hunks, equal old text in another Arc: the finished spans come back.
    let (same, again) = after_lines(&text);
    assert!(!Arc::ptr_eq(&old, &again));
    let kept = highlight_in_context("a.rs", &same, Some(&prev), Some(&again)).unwrap();
    assert!(!kept.is_pending());
    assert_eq!(kept.line(0, 0), marker.as_slice());

    // Other old text (line 1 differs, outside the hunk): parsed afresh.
    let mut edited = text.clone();
    edited[0] = "/* x */ let x0 = 0;".into();
    let (_, other) = after_lines(&edited);
    let fresh = highlight_in_context("a.rs", &same, Some(&prev), Some(&other)).unwrap();
    assert_ne!(fresh.line(0, 0), marker.as_slice());
    assert_eq!(fresh.old_text.as_deref(), Some(&*other));

    // Other hunks with the same text: no shortcut.
    let mut moved = same.clone();
    moved.hunks[0].lines[1].text = "let w = 2;".into();
    let fresh = highlight_in_context("a.rs", &moved, Some(&prev), Some(&again)).unwrap();
    assert_ne!(fresh.line(0, 0), marker.as_slice());
}

#[test]
fn shebang_from_old_text() {
    let script = |edit: bool| -> String {
        let body: String = (2..=30).map(|n| if n == 20 && edit { "echo changed\n".to_owned() } else { format!("echo {n}\n") }).collect();
        format!("#!/bin/bash\n{body}")
    };
    let dir = changed("hl-ctx-shebang", "run", &script(false), &script(true));
    let (d, old) = unstaged(&dir, "run");
    assert!(old.is_some());
    let syntax = highlight_in_context("run", &d, None, old.as_ref()).expect("detected from the old text's line 1").syntax;
    assert!(syntax.contains("Shell") || syntax.contains("bash"), "{syntax}");
    assert_eq!(highlight_diff("run", &d, None), None, "the hunks alone don't reach line 1");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ignore_whitespace_load_highlights() {
    let opts = LoadDiffOptions { ignore_whitespace: true, whole_file: false };
    let load = |dir: &Path| repo::load_diff_for_view(dir, &DiffTarget::Unstaged("a.rs".into()), opts).map(|v| (v.diff, v.old)).unwrap();
    let before = commented(|_, l| l);
    let edit = |ws_in_context: bool| {
        commented(move |n, l| match n {
            36 => l.replace("let y", "let  y"),
            24 if ws_in_context => format!("{l}   "),
            26 => "let x = 24; // changed".into(),
            _ => l,
        })
    };

    // A whitespace-only change away from the hunk: the hunk's lines match the old text.
    let dir = changed("hl-ctx-ws", "a.rs", &before, &edit(false));
    let (d, old) = load(&dir);
    assert_eq!(d.hunks.len(), 1, "the whitespace-only change is hidden");
    assert!(old.is_some());
    let hl = highlight_in_context("a.rs", &d, None, old.as_ref()).expect("rust");
    assert_eq!(hl.hunk_lens(), vec![d.hunks[0].lines.len()]);
    for l in 0..d.hunks[0].lines.len() {
        assert!(only(hl.line(0, l), TokenKind::Comment), "line {l}: {:?}", hl.line(0, l));
    }
    let _ = std::fs::remove_dir_all(&dir);

    // One inside the hunk's context: xdiff takes context text from the new side, so it no
    // longer matches the old text and each hunk is parsed on its own.
    let dir = changed("hl-ctx-ws2", "a.rs", &before, &edit(true));
    let (d, old) = load(&dir);
    assert_eq!(d.hunks.len(), 1);
    assert!(d.hunks[0].lines.iter().any(|l| l.origin == ' ' && l.text.ends_with("   ")), "context from the new side");
    assert!(old.is_none(), "so it doesn't match the old blob");
    let hl = highlight_in_context("a.rs", &d, None, old.as_ref()).expect("still highlighted");
    assert_eq!(hl.hunk_lens(), vec![d.hunks[0].lines.len()]);
    assert_eq!(hl.old_text, None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn old_text_per_target() {
    let numbered = |edit: &[usize]| -> String {
        (1..=20).map(|n| if edit.contains(&n) { format!("let c{n} = 0;\n") } else { format!("let v{n} = {n};\n") }).collect()
    };
    let dir = scratch_repo("hl-ctx-targets");
    write(&dir, "f.rs", &numbered(&[]));
    write(&dir, "r.rs", &numbered(&[]).replace("let v", "let r"));
    write(&dir, "b.bin", "a\0b\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "a"]);
    let a = head(&dir);
    write(&dir, "f.rs", &numbered(&[10]));
    git(&dir, &["mv", "r.rs", "s.rs"]);
    write(&dir, "s.rs", &numbered(&[]).replace("let v", "let r").replace("let r5 = 5;", "let r5 = 50;"));
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "b"]);
    let b = head(&dir);
    write(&dir, "f.rs", &numbered(&[3, 10]));
    git(&dir, &["add", "f.rs"]);
    write(&dir, "f.rs", &numbered(&[3, 10, 17]));
    write(&dir, "n.rs", "let n = 1;\n");
    write(&dir, "staged_new.rs", "let s = 1;\n");
    git(&dir, &["add", "staged_new.rs"]);
    write(&dir, "b.bin", "a\0c\n");

    let view = |t: DiffTarget, opts: LoadDiffOptions| repo::load_diff_for_view(&dir, &t, opts).map(|v| (v.diff, v.old)).unwrap();
    let old = |t: DiffTarget| view(t, LoadDiffOptions::default()).1;
    assert!(old(DiffTarget::Unstaged("f.rs".into())).is_some(), "the index blob");
    assert!(old(DiffTarget::Staged("f.rs".into())).is_some(), "the HEAD blob");
    assert!(old(DiffTarget::Commit(b, "f.rs".into())).is_some(), "the parent's blob");
    assert!(old(DiffTarget::Between(a, b, "f.rs".into())).is_some(), "from's blob");
    let (renamed, text) = view(DiffTarget::Commit(b, "s.rs".into()), LoadDiffOptions::default());
    assert!(!renamed.hunks.is_empty() && text.is_some(), "the old path's blob");

    assert_eq!(old(DiffTarget::Unstaged("n.rs".into())), None, "untracked");
    assert_eq!(old(DiffTarget::Staged("staged_new.rs".into())), None, "added");
    let (bin, text) = view(DiffTarget::Unstaged("b.bin".into()), LoadDiffOptions::default());
    assert!(bin.binary && text.is_none());
    let whole = LoadDiffOptions { ignore_whitespace: false, whole_file: true };
    let (d, text) = view(DiffTarget::Unstaged("f.rs".into()), whole);
    assert!(d.whole.is_some() && text.is_none(), "the whole file is its own context");

    let plain = repo::load_diff_with(&dir, &DiffTarget::Unstaged("f.rs".into()), LoadDiffOptions::default()).unwrap();
    assert_eq!(plain, view(DiffTarget::Unstaged("f.rs".into()), LoadDiffOptions::default()).0);
    let _ = std::fs::remove_dir_all(&dir);
}

fn head(dir: &Path) -> git2::Oid {
    git2::Repository::open(dir).unwrap().head().unwrap().peel_to_commit().unwrap().id()
}
