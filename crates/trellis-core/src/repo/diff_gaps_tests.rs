//! Reading the unchanged lines between a diff's hunks.

use super::super::{DiffTarget, LoadDiffOptions, load_diff_for_view};
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

fn hunks(dir: &Path, target: &DiffTarget) -> Vec<crate::repo::Hunk> {
    load_diff_for_view(dir, target, LoadDiffOptions::default()).unwrap().diff.hunks
}

#[test]
fn the_lines_between_three_hunks() {
    let dir = committed("gap-text", &body(&[]), &body(&[5, 25, 50]));
    let target = DiffTarget::Unstaged("a.rs".into());
    let h = hunks(&dir, &target);
    assert_eq!(h.len(), 3);
    let text = load_gap_text(&dir, &target, &h, Some("Rust")).expect("read");
    let all = text.all();
    assert_eq!(all.len(), 2);
    // Hunk 0 shows lines 2-8, hunk 1 lines 22-28: the run between is lines 9-21.
    assert_eq!((all[0].span.hunk, all[0].span.old_start, all[0].span.len), (1, 9, 13));
    let want: Vec<String> = (9..=21).map(|i| format!("fn f{i}() {{}}")).collect();
    assert_eq!(all[0].lines, want);
    assert_eq!((all[1].span.old_start, all[1].span.len), (29, 18));
    assert_eq!(all[1].lines.first().map(String::as_str), Some("fn f29() {}"));
    assert!(all[0].spans.iter().all(|s| s.iter().any(|s| s.kind == TokenKind::Keyword)), "{:?}", all[0].spans);
    // Without a syntax: plain.
    let plain = load_gap_text(&dir, &target, &h, None).unwrap();
    assert!(plain.all().iter().all(|g| g.spans.iter().all(Vec::is_empty)));
    assert_eq!(plain.all()[0].lines, want);

    // The working file changed since the hunks were read.
    write(&dir, "a.rs", &body(&[5, 25, 50, 55]));
    let e = load_gap_text(&dir, &target, &h, Some("Rust")).unwrap_err();
    assert!(e.to_string().contains("the file changed"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn added_files_and_single_hunks_have_no_gaps() {
    let dir = committed("gap-none", &body(&[]), &body(&[5]));
    let target = DiffTarget::Unstaged("a.rs".into());
    assert!(load_gap_text(&dir, &target, &hunks(&dir, &target), None).unwrap().all().is_empty());
    write(&dir, "new.rs", &body(&[]));
    git(&dir, &["add", "new.rs"]);
    let staged = DiffTarget::Staged("new.rs".into());
    assert!(load_gap_text(&dir, &staged, &hunks(&dir, &staged), None).unwrap().all().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
