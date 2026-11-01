//! A copy's source asked for by name is its own change, not the copy's (`rpath::delta_of`), for
//! a revert and for an external tool's sides alike, with `diff.renames=copies`.

use super::super::*;
use crate::testutil::*;
use git2::{Oid, Repository};
use std::path::{Path, PathBuf};

fn lines(n: usize) -> String {
    (1..=n).map(|i| format!("src line {i}\n")).collect()
}

/// A commit changing `src.txt` and adding `copy.txt`, a copy of the old `src.txt` plus a line;
/// returns (dir, parent, commit).
fn copied(name: &str) -> (PathBuf, Oid, Oid) {
    let dir = scratch_repo(name);
    set_config(&dir, &[("diff.renames", "copies")]);
    let commit = |dir: &Path| {
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "-q", "-m", "c"]);
        Repository::open(dir).unwrap().head().unwrap().target().unwrap()
    };
    write(&dir, "src.txt", &lines(40));
    let c1 = commit(&dir);
    write(&dir, "src.txt", &format!("{}changed\n", lines(40)));
    write(&dir, "copy.txt", &format!("{}copied\n", lines(40)));
    let c2 = commit(&dir);
    let copy = commit_detail(&dir, c2).unwrap().files.into_iter().find(|f| f.path == "copy.txt").unwrap();
    assert_eq!((copy.status, copy.renamed_from.as_deref()), ('A', Some("src.txt")), "detected as a copy");
    (dir, c1, c2)
}

fn read(dir: &Path, path: &str) -> String {
    std::fs::read_to_string(dir.join(path)).unwrap()
}

fn revert_hunk(dir: &Path, target: DiffTarget) -> Reverted {
    let expected = load_diff(dir, &target).unwrap().hunks[0].clone();
    let req = RevertRequest { target, part: RevertPart::Hunk { index: 0, expected }, dest: None, ignore_whitespace: false };
    revert(dir, &req).unwrap()
}

#[test]
fn reverting_the_source_or_the_copy_changes_only_that_file() {
    let (dir, _, c2) = copied("copy-source-revert");
    let copy = read(&dir, "copy.txt");
    assert_eq!(revert_hunk(&dir, DiffTarget::Commit(c2, "src.txt".into())), Reverted::Applied);
    assert_eq!(read(&dir, "src.txt"), lines(40), "the source loses its own change");
    assert_eq!(read(&dir, "copy.txt"), copy, "the copy is untouched");

    let (dir2, _, c2) = copied("copy-copy-revert");
    let src = read(&dir2, "src.txt");
    assert_eq!(revert_hunk(&dir2, DiffTarget::Commit(c2, "copy.txt".into())), Reverted::Applied);
    assert_eq!(read(&dir2, "copy.txt"), lines(40), "the copy loses its line against its source");
    assert_eq!(read(&dir2, "src.txt"), src, "the source is untouched");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&dir2);
}

#[test]
fn a_tool_compares_the_source_with_itself_and_the_copy_with_its_source() {
    let (dir, c1, c2) = copied("copy-source-tool");
    let repo = Repository::open(&dir).unwrap();
    let blob = |oid: Oid, path: &str| repo.find_commit(oid).unwrap().tree().unwrap().get_path(Path::new(path)).unwrap().id();
    let side = |oid: Oid, at: &str, path: &str| SideSource::Blob { id: blob(oid, at), path: path.into(), commit: Some(oid) };
    let src = diff_sides(&dir, &DiffTarget::Commit(c2, "src.txt".into())).unwrap();
    assert_eq!((src.old, src.new), (side(c1, "src.txt", "src.txt"), side(c2, "src.txt", "src.txt")));
    let copy = diff_sides(&dir, &DiffTarget::Commit(c2, "copy.txt".into())).unwrap();
    assert_eq!((copy.old, copy.new), (side(c1, "src.txt", "src.txt"), side(c2, "copy.txt", "copy.txt")), "against its source");
    let _ = std::fs::remove_dir_all(&dir);
}
