//! The files diff and merge tools are given: copies of the right versions, the working file
//! where the tool edits or saves, and fingerprints that tell whether a file changed.

use super::*;
use crate::gitcli::{args, run};
use crate::testutil::*;

/// A fresh session root for one test.
fn root(name: &str) -> PathBuf {
    let r = crate::paths::temp_dir().join(format!("glowfly-test-toolroot-{name}-{}", std::process::id()));
    let _ = crate::persist::remove_tree(&r);
    std::fs::create_dir_all(&r).unwrap();
    r
}

fn head(dir: &Path) -> git2::Oid {
    git2::Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap()
}

#[test]
fn unstaged_diffs_copy_the_index_and_use_the_working_file() {
    let dir = scratch_repo("compare-unstaged");
    write(&dir, "notes.txt", "old\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    write(&dir, "notes.txt", "new\n");
    let r = root("unstaged");
    let f = diff_files(&r, &dir, &DiffTarget::Unstaged("notes.txt".into()), false).unwrap();
    assert_eq!(read(&f.local), "old\n");
    assert!(f.local.starts_with(f.session.dir()) && f.local.ends_with("index/notes.txt"), "{}", f.local.display());
    assert_eq!(f.remote, dir.join("notes.txt"), "the working file itself");
    // No changes: nothing is written.
    git(&dir, &["add", "."]);
    assert!(diff_files(&r, &dir, &DiffTarget::Unstaged("notes.txt".into()), false).unwrap_err().is_not_found());
    f.session.remove().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = crate::persist::remove_tree(&r);
}

#[test]
fn commit_diffs_copy_both_versions_and_an_empty_file_for_a_new_one() {
    let dir = scratch_repo("compare-commit");
    write(&dir, "src.rs", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    let first = head(&dir);
    write(&dir, "src.rs", "two\n");
    git(&dir, &["commit", "-q", "-am", "second"]);
    let second = head(&dir);
    let r = root("commit");
    let f = diff_files(&r, &dir, &DiffTarget::Commit(second, "src.rs".into()), true).unwrap();
    assert_eq!((read(&f.local), read(&f.remote)), ("one\n".to_owned(), "two\n".to_owned()));
    assert!(f.local.ends_with("src.rs") && f.remote.ends_with("src.rs"), "names keep the file's name");
    assert_ne!(f.local.parent(), f.remote.parent());
    let f = diff_files(&r, &dir, &DiffTarget::Commit(first, "src.rs".into()), false).unwrap();
    assert_eq!((read(&f.local), read(&f.remote)), (String::new(), "one\n".to_owned()), "a root commit's old side is empty");
    assert!(f.local.ends_with("empty/src.rs"));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = crate::persist::remove_tree(&r);
}

/// A merge where both sides added c.txt (`add`) or changed it from a common base.
fn conflicted(name: &str, add: bool) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "keep.txt", "keep\n");
    if !add {
        write(&dir, "c.txt", "base\n");
    }
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    write(&dir, "c.txt", "theirs\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    write(&dir, "c.txt", "ours\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "ours"]);
    assert!(!run(&dir, &args(&["merge", "other"])).0, "merge should conflict");
    dir
}

#[test]
fn merge_files_copy_the_three_stages_and_name_the_working_file() {
    let r = root("merge");
    for (add, base) in [(false, "base\n"), (true, "")] {
        let dir = conflicted(&format!("compare-merge-{add}"), add);
        let f = merge_files(&r, &dir, "c.txt", true).unwrap();
        assert_eq!((read(&f.base), read(&f.ours), read(&f.theirs)), (base.to_owned(), "ours\n".to_owned(), "theirs\n".to_owned()));
        assert!(f.base.ends_with("base/c.txt") && f.ours.ends_with("ours/c.txt") && f.theirs.ends_with("theirs/c.txt"));
        assert_eq!(f.output, dir.join("c.txt"));
        let session = f.session.dir().to_owned();
        f.session.remove().unwrap();
        assert!(!session.exists());
        assert!(merge_files(&r, &dir, "keep.txt", false).unwrap_err().is_not_found());
        let _ = std::fs::remove_dir_all(&dir);
    }
    let _ = crate::persist::remove_tree(&r);
}

#[test]
fn fingerprints_change_with_the_file() {
    let r = root("fingerprint");
    let p = r.join("f.txt");
    assert_eq!(fingerprint(&p), None, "missing");
    std::fs::write(&p, "one").unwrap();
    let a = fingerprint(&p);
    assert!(a.is_some());
    assert_eq!(fingerprint(&p), a);
    std::fs::write(&p, "two").unwrap();
    assert_ne!(fingerprint(&p), a);
    let _ = crate::persist::remove_tree(&r);
}
