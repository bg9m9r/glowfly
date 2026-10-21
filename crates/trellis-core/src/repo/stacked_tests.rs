//! Each commit's own changes for a stacked list, read from a fixture repository.

use super::*;
use crate::testutil::{git, scratch_repo, write};

fn head(dir: &Path) -> Oid {
    Repository::open(dir).unwrap().head().unwrap().target().unwrap()
}

fn commit(dir: &Path, message: &str) -> Oid {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
    head(dir)
}

fn files(c: &StackedCommit) -> Vec<(&str, char)> {
    c.files.iter().map(|f| (f.path.as_str(), f.status)).collect()
}

#[test]
fn each_commit_lists_its_own_files_in_the_order_given() {
    let dir = scratch_repo("stacked-changes");
    write(&dir, "a.txt", "a\n");
    write(&dir, "README.md", "readme\n");
    let root = commit(&dir, "root");
    write(&dir, "a.txt", "a\nmore\n");
    let edit = commit(&dir, "edit a");
    git(&dir, &["checkout", "-q", "-b", "side"]);
    write(&dir, "s.txt", "side\n");
    commit(&dir, "side work");
    git(&dir, &["checkout", "-q", "main"]);
    write(&dir, "m.txt", "main\n");
    commit(&dir, "main work");
    git(&dir, &["merge", "-q", "--no-ff", "side", "-m", "merge side"]);
    let merge = head(&dir);
    git(&dir, &["mv", "README.md", "READ.md"]);
    let renamed = commit(&dir, "rename");
    git(&dir, &["rm", "-q", "a.txt"]);
    commit(&dir, "drop a");

    let stack = stacked_changes(&dir, &[renamed, merge, edit, root]).unwrap();
    assert_eq!(stack.iter().map(|c| c.oid).collect::<Vec<_>>(), [renamed, merge, edit, root]);
    assert_eq!(files(&stack[0]), [("READ.md", 'R')], "a rename shows under its new name");
    assert_eq!(files(&stack[1]), [("s.txt", 'A')], "a merge against its first parent");
    assert_eq!(files(&stack[2]), [("a.txt", 'M')]);
    assert_eq!(files(&stack[3]), [("README.md", 'A'), ("a.txt", 'A')], "a root commit adds everything");
    assert!(!stack[2].in_workdir("a.txt"), "a file deleted later is absent");
    assert!(stack[1].in_workdir("s.txt") && stack[0].in_workdir("READ.md"));
    assert!(!stack[3].in_workdir("README.md"), "renamed away since");

    let unknown = Oid::from_str(&"1".repeat(40)).unwrap();
    assert!(stacked_changes(&dir, &[edit, unknown]).is_err(), "a missing commit is an error");
    let _ = std::fs::remove_dir_all(&dir);
}
