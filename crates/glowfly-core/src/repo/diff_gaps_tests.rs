//! Reading the unchanged lines outside a diff's hunks.

use super::super::{DiffTarget, FileDiff, LoadDiffOptions, load_diff_for_view};
use super::load_gap_text;
use crate::highlight::TokenKind;
use crate::testutil::{git, scratch_repo, write};
use std::path::Path;

/// `fn fN() {}` lines 1..=60, lines in `changed` given a body.
fn body(changed: &[usize]) -> String {
    (1..=60).map(|i| if changed.contains(&i) { format!("fn f{i}() {{ {i} }}\n") } else { format!("fn f{i}() {{}}\n") }).collect()
}

fn committed(name: &str, before: &str, after: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "a.rs", before);
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    write(&dir, "a.rs", after);
    dir
}

fn shown(dir: &Path, target: &DiffTarget) -> FileDiff {
    load_diff_for_view(dir, target, LoadDiffOptions::default()).unwrap().diff
}

#[test]
fn the_lines_between_three_hunks() {
    let dir = committed("gap-text", &body(&[]), &body(&[5, 25, 50]));
    let target = DiffTarget::Unstaged("a.rs".into());
    let d = shown(&dir, &target);
    assert_eq!(d.hunks.len(), 3);
    assert_eq!(d.old_lines, Some(60));
    let text = load_gap_text(&dir, &target, &d, Some("Rust")).expect("read");
    let all = text.all();
    assert_eq!(all.len(), 4);
    // Hunk 0 shows lines 2-8: line 1 is above it; the last hunk ends at 53, so 54-60 are below.
    assert_eq!((all[0].span.hunk, all[0].span.old_start, all[0].span.len, all[0].span.key), (0, 1, 1, 1));
    assert_eq!(all[0].lines, ["fn f1() {}"]);
    // Hunk 0 shows lines 2-8, hunk 1 lines 22-28: the run between is lines 9-21.
    assert_eq!((all[1].span.hunk, all[1].span.old_start, all[1].span.len), (1, 9, 13));
    let want: Vec<String> = (9..=21).map(|i| format!("fn f{i}() {{}}")).collect();
    assert_eq!(all[1].lines, want);
    assert_eq!((all[2].span.old_start, all[2].span.len), (29, 18));
    assert_eq!(all[2].lines.first().map(String::as_str), Some("fn f29() {}"));
    assert_eq!((all[3].span.hunk, all[3].span.old_start, all[3].span.len, all[3].span.key), (3, 54, 7, 54));
    let below: Vec<String> = (54..=60).map(|i| format!("fn f{i}() {{}}")).collect();
    assert_eq!(all[3].lines, below);
    assert!(all.iter().all(|g| g.spans.iter().all(|s| s.iter().any(|s| s.kind == TokenKind::Keyword))), "{all:?}");
    // Without a syntax: plain.
    let plain = load_gap_text(&dir, &target, &d, None).unwrap();
    assert!(plain.all().iter().all(|g| g.spans.iter().all(Vec::is_empty)));
    assert_eq!(plain.all()[1].lines, want);

    // The working file changed since the hunks were read.
    write(&dir, "a.rs", &body(&[5, 25, 50, 55]));
    let e = load_gap_text(&dir, &target, &d, Some("Rust")).unwrap_err();
    assert_eq!(e, crate::error::ReadFailed::FileChanged);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn single_hunks_have_their_edges_and_added_or_deleted_files_none() {
    let dir = committed("gap-none", &body(&[]), &body(&[30]));
    let target = DiffTarget::Unstaged("a.rs".into());
    let text = load_gap_text(&dir, &target, &shown(&dir, &target), None).unwrap();
    let all = text.all();
    assert_eq!(all.len(), 2);
    assert_eq!((all[0].span.hunk, all[0].span.old_start, all[0].span.len), (0, 1, 26));
    assert_eq!(all[0].lines.last().map(String::as_str), Some("fn f26() {}"));
    assert_eq!((all[1].span.hunk, all[1].span.old_start, all[1].span.len), (1, 34, 27));
    assert_eq!(all[1].lines.last().map(String::as_str), Some("fn f60() {}"));
    write(&dir, "new.rs", &body(&[]));
    git(&dir, &["add", "new.rs"]);
    let staged = DiffTarget::Staged("new.rs".into());
    assert!(load_gap_text(&dir, &staged, &shown(&dir, &staged), None).unwrap().all().is_empty());
    git(&dir, &["commit", "-q", "-m", "add"]);
    std::fs::remove_file(dir.join("new.rs")).unwrap();
    let unstaged = DiffTarget::Unstaged("new.rs".into());
    let gone = shown(&dir, &unstaged);
    assert_eq!(gone.old_lines, Some(60));
    assert!(load_gap_text(&dir, &unstaged, &gone, None).unwrap().all().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_start_and_end() {
    let dir = committed("gap-ends", &body(&[]), &body(&[1, 30, 60]));
    let target = DiffTarget::Unstaged("a.rs".into());
    let d = shown(&dir, &target);
    let text = load_gap_text(&dir, &target, &d, None).unwrap();
    let hunks: Vec<usize> = text.all().iter().map(|g| g.span.hunk).collect();
    assert_eq!(hunks, [1, 2], "no run above a change on line 1 or below one on the last line");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_trailing_newline() {
    let lines = |n: usize| {
        (1..=n)
            .map(|i| {
                format!(
                    "line {i}
"
                )
            })
            .collect::<String>()
    };
    let before = lines(20).trim_end().to_owned();
    let dir = committed(
        "gap-eol",
        &before,
        &before.replacen(
            "line 5
", "five
", 1,
        ),
    );
    let target = DiffTarget::Unstaged("a.rs".into());
    let d = shown(&dir, &target);
    assert_eq!(d.old_lines, Some(20));
    let text = load_gap_text(&dir, &target, &d, None).unwrap();
    let below = text.all().last().unwrap();
    assert_eq!((below.span.hunk, below.span.old_start, below.span.len), (1, 9, 12));
    assert_eq!(below.lines.last().map(String::as_str), Some("line 20"));
    // A change on that last line: no run below.
    write(&dir, "a.rs", &before.replacen("line 20", "twenty", 1));
    let d = shown(&dir, &target);
    let text = load_gap_text(&dir, &target, &d, None).unwrap();
    assert!(text.all().iter().all(|g| g.span.hunk < d.hunks.len()), "{:?}", text.all());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn old_lines_come_with_the_view_load() {
    let dir = committed("gap-old-lines", &body(&[]), &body(&[5, 25]));
    let target = DiffTarget::Unstaged("a.rs".into());
    assert_eq!(shown(&dir, &target).old_lines, Some(60));
    let whole = LoadDiffOptions { whole_file: true, ..LoadDiffOptions::default() };
    assert_eq!(load_diff_for_view(&dir, &target, whole).unwrap().diff.old_lines, Some(60));
    assert_eq!(crate::repo::load_diff(&dir, &target).unwrap().old_lines, None);
    // A diff whose old line count no longer matches the file's.
    let mut d = shown(&dir, &target);
    d.old_lines = Some(59);
    assert_eq!(load_gap_text(&dir, &target, &d, None).unwrap_err(), crate::error::ReadFailed::FileChanged);
    let _ = std::fs::remove_dir_all(&dir);
}
