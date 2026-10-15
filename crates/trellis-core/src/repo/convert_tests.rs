//! Staging and discarding files that git converts on the way in or out (a `filter` driver, as
//! Git LFS and git-crypt use, or a `working-tree-encoding`): whole-file operations run the
//! filters through git; hunk and line operations, which libgit2 would apply to unconverted
//! bytes, are refused and change nothing.

use super::*;
use crate::error::Error;
use crate::testutil::*;
use git2::Repository;
use std::path::Path;

pub(super) fn index_text(dir: &Path, path: &str) -> String {
    let repo = Repository::open(dir).unwrap();
    let entry = repo.index().unwrap().get_path(Path::new(path), 0).expect("in the index");
    String::from_utf8(repo.find_blob(entry.id).unwrap().content().to_vec()).unwrap()
}

pub(super) fn read(dir: &Path, path: &str) -> String {
    std::fs::read_to_string(dir.join(path)).unwrap()
}

/// A repository where `*.up` files are stored upper-case and checked out lower-case, with
/// `a.up` committed and a two-hunk edit of it in the working tree.
pub(super) fn filtered_repo(name: &str) -> std::path::PathBuf {
    let dir = scratch_repo(name);
    git(&dir, &["config", "filter.upper.clean", "tr a-z A-Z"]);
    git(&dir, &["config", "filter.upper.smudge", "tr A-Z a-z"]);
    write(&dir, ".gitattributes", "*.up filter=upper\n");
    let base: String = (1..=20).map(|i| format!("line {i}\n")).collect();
    write(&dir, "a.up", &base);
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    assert_eq!(index_text(&dir, "a.up"), base.to_uppercase(), "git ran the clean filter");
    write(&dir, "a.up", &base.replace("line 2\n", "line two\n").replace("line 19\n", "line nineteen\n"));
    dir
}

#[test]
fn stage_file_runs_the_clean_filter() {
    let dir = filtered_repo("cv-stage");
    stage_file(&dir, "a.up").unwrap();
    assert!(index_text(&dir, "a.up").contains("LINE TWO\n"), "staged through the clean filter");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stage_all_runs_the_clean_filter() {
    let dir = filtered_repo("cv-stage-all");
    write(&dir, "new.up", "fresh\n");
    stage_all(&dir).unwrap();
    assert!(index_text(&dir, "a.up").contains("LINE NINETEEN\n"));
    assert_eq!(index_text(&dir, "new.up"), "FRESH\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn discard_file_runs_the_smudge_filter() {
    let dir = filtered_repo("cv-discard");
    discard_file(&dir, "a.up").unwrap();
    let text = read(&dir, "a.up");
    assert!(text.starts_with("line 1\nline 2\n"), "checked out through the smudge filter: {text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stage_file_stages_a_removal_and_reads_paths_literally() {
    let dir = scratch_repo("cv-literal");
    write(&dir, "a[b].txt", "star\n");
    write(&dir, "ab.txt", "b\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    write(&dir, "a[b].txt", "star 2\n");
    write(&dir, "ab.txt", "b 2\n");
    stage_file(&dir, "a[b].txt").unwrap();
    let (unstaged, staged, _) = status_at(&dir, true).unwrap();
    assert_eq!(staged.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["a[b].txt"], "not a glob");
    assert!(unstaged.iter().any(|f| f.path == "ab.txt"));
    std::fs::remove_file(dir.join("ab.txt")).unwrap();
    stage_file(&dir, "ab.txt").unwrap();
    let (_, staged, _) = status_at(&dir, true).unwrap();
    assert!(staged.iter().any(|f| f.path == "ab.txt" && f.status == 'D'));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hunk_and_line_edits_of_converted_files_are_refused() {
    let dir = filtered_repo("cv-refuse");
    let before = read(&dir, "a.up");
    let diff = load_diff(&dir, &DiffTarget::Unstaged("a.up".into())).unwrap();
    let hunk = diff.hunks[0].clone();
    for op in [HunkOp::Stage, HunkOp::Discard] {
        match apply_hunk(&dir, "a.up", 0, &hunk, op) {
            Err(Error::Unsupported(m)) => assert!(m.contains("filter=upper"), "{m}"),
            other => panic!("{op:?}: expected a refusal, got {other:?}"),
        }
        let lines: Vec<usize> = (0..hunk.lines.len()).collect();
        assert!(matches!(apply_lines(&dir, "a.up", 0, &hunk, &lines, op), Err(Error::Unsupported(_))), "{op:?}");
    }
    assert_eq!(read(&dir, "a.up"), before);
    assert_eq!(index_text(&dir, "a.up"), (1..=20).map(|i| format!("LINE {i}\n")).collect::<String>());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn working_tree_encoding_counts_as_a_conversion() {
    let dir = scratch_repo("cv-encoding");
    write(&dir, ".gitattributes", "*.txt working-tree-encoding=UTF-16LE\n");
    let repo = Repository::open(&dir).unwrap();
    assert_eq!(attrs::content_conversion(&repo, "x.txt").as_deref(), Some("working-tree-encoding=UTF-16LE"));
    assert_eq!(attrs::content_conversion(&repo, "x.md"), None);
    let _ = std::fs::remove_dir_all(&dir);
}
