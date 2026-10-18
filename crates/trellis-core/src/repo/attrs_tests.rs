//! `linguist-language` for the viewer: from the commit's own `.gitattributes` files for commit and
//! comparison diffs, from the working tree and index for the others.

use super::super::{DiffTarget, LoadDiffOptions, load_diff_for_view};
use crate::testutil::{git, scratch_repo, write};
use git2::{Oid, Repository};
use std::path::Path;

fn head(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn language(dir: &Path, target: DiffTarget) -> Option<String> {
    load_diff_for_view(dir, &target, LoadDiffOptions::default()).expect("loads").language
}

fn commit(dir: &Path, message: &str) -> Oid {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
    head(dir)
}

#[test]
fn commit_diffs_read_the_commits_gitattributes() {
    let dir = scratch_repo("attrs-commit");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    write(&dir, ".gitattributes", "*.inc linguist-language=PHP\n");
    write(&dir, "sub/.gitattributes", "*.inc linguist-language=Python\n");
    write(&dir, "a.inc", "one\n");
    write(&dir, "sub/x.inc", "one\n");
    let c1 = commit(&dir, "one");
    write(&dir, ".gitattributes", "*.inc linguist-language=C\n");
    write(&dir, "a.inc", "two\n");
    write(&dir, "sub/x.inc", "two\n");
    let c2 = commit(&dir, "two");
    // Uncommitted: the working tree says Rust.
    write(&dir, ".gitattributes", "*.inc linguist-language=Rust\n");
    write(&dir, "a.inc", "three\n");

    let lang = |t| language(&dir, t);
    assert_eq!(lang(DiffTarget::Commit(c1, "a.inc".into())).as_deref(), Some("PHP"));
    assert_eq!(lang(DiffTarget::Commit(c2, "a.inc".into())).as_deref(), Some("C"));
    assert_eq!(lang(DiffTarget::Between(c1, c2, "a.inc".into())).as_deref(), Some("C"));
    assert_eq!(lang(DiffTarget::Unstaged("a.inc".into())).as_deref(), Some("Rust"));
    assert_eq!(lang(DiffTarget::ToWorkdir(c1, "a.inc".into())).as_deref(), Some("Rust"));
    // A folder's own file overrides the root's.
    assert_eq!(lang(DiffTarget::Commit(c1, "sub/x.inc".into())).as_deref(), Some("Python"));
    assert_eq!(lang(DiffTarget::Commit(c2, "sub/x.inc".into())).as_deref(), Some("Python"));

    // `.git/info/attributes` still wins over the commit's files.
    std::fs::create_dir_all(dir.join(".git/info")).unwrap();
    write(&dir, ".git/info/attributes", "a.inc linguist-language=Go\n");
    assert_eq!(lang(DiffTarget::Commit(c2, "a.inc".into())).as_deref(), Some("Go"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_commit_without_gitattributes_has_no_override() {
    let dir = scratch_repo("attrs-none");
    write(&dir, "a.inc", "one\n");
    let c = commit(&dir, "one");
    write(&dir, ".gitattributes", "*.inc linguist-language=Rust\n");
    assert_eq!(language(&dir, DiffTarget::Commit(c, "a.inc".into())), None);
    // The working tree's file applies to its own diffs.
    write(&dir, "a.inc", "two\n");
    assert_eq!(language(&dir, DiffTarget::Unstaged("a.inc".into())).as_deref(), Some("Rust"));
    let _ = std::fs::remove_dir_all(&dir);
}
