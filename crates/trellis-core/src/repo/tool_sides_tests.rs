//! The versions external tools are given: each kind of diff target's sides and labels, the
//! stages of a conflict, and blob copies with and without git's checkout filters.

use super::*;
use crate::gitcli::{args, run};
use crate::testutil::*;
use git2::Repository;
use std::path::PathBuf;

/// The index blob of `path`.
fn staged_id(dir: &Path, path: &str) -> Oid {
    Repository::open(dir).unwrap().index().unwrap().get_path(Path::new(path), 0).unwrap().id
}

fn head(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

/// A repository with a.txt ("one") committed.
fn one_commit(name: &str) -> (PathBuf, Oid) {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    let c = head(&dir);
    (dir, c)
}

fn blob(id: Oid, path: &str) -> SideSource {
    SideSource::Blob { id, path: path.into() }
}

#[test]
fn unstaged_changes_compare_the_index_with_the_working_file() {
    let (dir, _) = one_commit("tool-sides-unstaged");
    let committed = staged_id(&dir, "a.txt");
    write(&dir, "a.txt", "two\n");
    let s = diff_sides(&dir, &DiffTarget::Unstaged("a.txt".into())).unwrap();
    assert_eq!((s.old, s.new), (blob(committed, "a.txt"), SideSource::Working("a.txt".into())));
    assert_eq!((s.old_label.as_str(), s.new_label.as_str()), ("index", "working tree"));

    write(&dir, "new.txt", "new\n");
    let s = diff_sides(&dir, &DiffTarget::Unstaged("new.txt".into())).unwrap();
    assert_eq!((s.old, s.new), (SideSource::Empty, SideSource::Working("new.txt".into())), "untracked");
    assert_eq!(s.old_label, "empty");

    std::fs::remove_file(dir.join("a.txt")).unwrap();
    let s = diff_sides(&dir, &DiffTarget::Unstaged("a.txt".into())).unwrap();
    assert_eq!((s.old, s.new), (blob(committed, "a.txt"), SideSource::Empty), "deleted");
    assert_eq!(s.new_label, "empty");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn staged_changes_compare_head_with_the_index() {
    let (dir, _) = one_commit("tool-sides-staged");
    write(&dir, "b.txt", "bee\n");
    git(&dir, &["add", "b.txt"]);
    let s = diff_sides(&dir, &DiffTarget::Staged("b.txt".into())).unwrap();
    assert_eq!((s.old, s.new), (SideSource::Empty, blob(staged_id(&dir, "b.txt"), "b.txt")));
    assert_eq!((s.old_label.as_str(), s.new_label.as_str()), ("empty", "index"));
    let before = staged_id(&dir, "a.txt");
    write(&dir, "a.txt", "changed\n");
    git(&dir, &["add", "a.txt"]);
    let s = diff_sides(&dir, &DiffTarget::Staged("a.txt".into())).unwrap();
    assert_eq!((s.old, s.new), (blob(before, "a.txt"), blob(staged_id(&dir, "a.txt"), "a.txt")));
    assert_eq!((s.old_label.as_str(), s.new_label.as_str()), ("HEAD", "index"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn commits_compare_with_their_first_parent_and_follow_renames() {
    let (dir, first) = one_commit("tool-sides-commit");
    let s = diff_sides(&dir, &DiffTarget::Commit(first, "a.txt".into())).unwrap();
    assert_eq!((s.old, s.new), (SideSource::Empty, blob(staged_id(&dir, "a.txt"), "a.txt")), "a root commit");
    assert_eq!((s.old_label, s.new_label), ("empty".to_owned(), crate::rebase::short(first)));

    let text: String = (1..=40).map(|i| format!("line {i}\n")).collect();
    write(&dir, "f.txt", &text);
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "f"]);
    let second = head(&dir);
    let old_id = staged_id(&dir, "f.txt");
    git(&dir, &["mv", "f.txt", "g.txt"]);
    write(&dir, "g.txt", &format!("{text}line 41\n"));
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "rename"]);
    let third = head(&dir);
    let s = diff_sides(&dir, &DiffTarget::Commit(third, "g.txt".into())).unwrap();
    assert_eq!((s.old, s.new), (blob(old_id, "f.txt"), blob(staged_id(&dir, "g.txt"), "g.txt")), "the old side at the old path");
    assert_eq!((s.old_label, s.new_label), (crate::rebase::short(second), crate::rebase::short(third)));

    let s = diff_sides(&dir, &DiffTarget::Between(first, third, "g.txt".into())).unwrap();
    assert_eq!((s.old, s.new), (SideSource::Empty, blob(staged_id(&dir, "g.txt"), "g.txt")));
    assert_eq!((s.old_label, s.new_label), ("empty".to_owned(), crate::rebase::short(third)));
    let s = diff_sides(&dir, &DiffTarget::Between(third, first, "g.txt".into())).unwrap();
    assert_eq!(s.new, SideSource::Empty, "the newer commit doesn't have it");
    assert_eq!(s.old_label, crate::rebase::short(third));

    let gone = diff_sides(&dir, &DiffTarget::Unstaged("a.txt".into())).unwrap_err();
    assert!(gone.is_not_found(), "no changes: {gone}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_commit_compared_with_the_working_tree_opens_the_working_file() {
    let (dir, first) = one_commit("tool-sides-workdir");
    let committed = staged_id(&dir, "a.txt");
    write(
        &dir, "a.txt", "two
",
    );
    let s = diff_sides(&dir, &DiffTarget::ToWorkdir(first, "a.txt".into())).unwrap();
    assert_eq!((s.old, s.new), (blob(committed, "a.txt"), SideSource::Working("a.txt".into())));
    assert_eq!((s.old_label, s.new_label), (crate::rebase::short(first), "working tree".to_owned()));

    std::fs::remove_file(dir.join("a.txt")).unwrap();
    let s = diff_sides(&dir, &DiffTarget::ToWorkdir(first, "a.txt".into())).unwrap();
    assert_eq!((s.new, s.new_label.as_str()), (SideSource::Empty, "empty"), "deleted from the working tree");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_submodule_entry_cant_be_compared() {
    let (parent, _sub) = with_submodule("tool-sides-submodule");
    let e = diff_sides(&parent, &DiffTarget::Commit(head(&parent), "libs/lib".into())).unwrap_err();
    assert!(matches!(&e, Error::Unsupported(m) if m == "libs/lib is a submodule; it can't be opened in a diff tool"), "{e}");
    let _ = std::fs::remove_dir_all(&parent);
}

/// `f` committed as `base`; branch `other` makes it `theirs` (None deletes it), main makes it
/// `ours`; then `git merge other`, which must conflict. With `base` None, both sides add it.
fn conflict(name: &str, f: &str, base: Option<&str>, ours: &str, theirs: Option<&str>) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "keep.txt", "keep\n");
    if let Some(b) = base {
        write(&dir, f, b);
    }
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    match theirs {
        Some(t) => write(&dir, f, t),
        None => std::fs::remove_file(dir.join(f)).unwrap(),
    }
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    write(&dir, f, ours);
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "ours"]);
    let (ok, _) = run(&dir, &args(&["merge", "other"]));
    assert!(!ok, "merge should conflict");
    dir
}

fn blob_of(dir: &Path, text: &str) -> Oid {
    Repository::open(dir).unwrap().blob(text.as_bytes()).unwrap()
}

#[test]
fn merge_sides_are_the_three_stages() {
    let dir = conflict("tool-sides-merge", "f.txt", Some("base\n"), "ours\n", Some("theirs\n"));
    let m = merge_sides(&dir, "f.txt").unwrap();
    assert_eq!(m, MergeSides { base: Some(blob_of(&dir, "base\n")), ours: blob_of(&dir, "ours\n"), theirs: blob_of(&dir, "theirs\n") });
    assert!(is_conflicted(&dir, "f.txt").unwrap());
    assert!(merge_sides(&dir, "keep.txt").unwrap_err().is_not_found());
    assert!(!is_conflicted(&dir, "keep.txt").unwrap());
    git(&dir, &["add", "f.txt"]);
    assert!(!is_conflicted(&dir, "f.txt").unwrap(), "resolved once staged");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn both_added_has_no_base_and_a_deletion_is_unsupported() {
    let dir = conflict("tool-sides-addadd", "n.txt", None, "ours\n", Some("theirs\n"));
    let m = merge_sides(&dir, "n.txt").unwrap();
    assert_eq!(m.base, None);
    assert_eq!((m.ours, m.theirs), (blob_of(&dir, "ours\n"), blob_of(&dir, "theirs\n")));
    let _ = std::fs::remove_dir_all(&dir);

    let dir = conflict("tool-sides-moddel", "d.txt", Some("base\n"), "ours\n", None);
    let e = merge_sides(&dir, "d.txt").unwrap_err();
    assert!(matches!(&e, Error::Unsupported(m) if m.contains("one side deleted d.txt")), "{e}");
    assert!(is_conflicted(&dir, "d.txt").unwrap());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn blob_copies_use_checkout_filters_only_when_asked() {
    let dir = scratch_repo("tool-sides-filters");
    write(&dir, ".gitattributes", "*.txt text eol=crlf\n");
    write(&dir, "a.txt", "one\ntwo\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    let id = staged_id(&dir, "a.txt");
    assert_eq!(blob_bytes(&dir, id, "a.txt", false).unwrap(), b"one\ntwo\n", "the stored bytes");
    assert_eq!(blob_bytes(&dir, id, "a.txt", true).unwrap(), b"one\r\ntwo\r\n", "as a checkout writes it");
    let missing = Oid::from_bytes(&[9; 20]).unwrap();
    assert!(blob_bytes(&dir, missing, "a.txt", true).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
