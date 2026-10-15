use super::status::libgit2_status;
use super::status_cli::{parse, status};
use super::*;
use crate::testutil::*;
use git2::Repository;

#[test]
fn git_and_libgit2_read_the_same_status() {
    let dir = scratch_repo("status-cli");
    for f in ["a.txt", "b.txt", "c.txt", "d.txt", "e.txt"] {
        write(&dir, f, &format!("{f}\n"));
    }
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    write(&dir, "a.txt", "changed\n");
    write(&dir, "b.txt", "staged\n");
    git(&dir, &["add", "b.txt"]);
    write(&dir, "b.txt", "and changed again\n");
    std::fs::remove_file(dir.join("c.txt")).unwrap();
    git(&dir, &["rm", "-q", "d.txt"]);
    git(&dir, &["mv", "e.txt", "renamed.txt"]);
    write(&dir, "new.txt", "new\n");
    git(&dir, &["add", "new.txt"]);
    std::fs::create_dir_all(dir.join("deep/er")).unwrap();
    write(&dir, "deep/er/untracked file.txt", "u\n");

    let repo = Repository::open(&dir).unwrap();
    let ours = libgit2_status(&repo).unwrap();
    let git_read = status(&dir).expect("git ran");
    assert_eq!(git_read, ours);
    assert!(git_read.1.iter().any(|f| f.path == "renamed.txt" && f.status == 'R'));
    assert!(git_read.0.iter().any(|f| f.path == "deep/er/untracked file.txt" && f.status == '?'));

    // A repository set up for git's fast status reads it through git; the lists don't change.
    git(&dir, &["config", "core.untrackedCache", "true"]);
    assert_eq!(status_at(&dir).map(|(u, s, _)| (u, s)).unwrap(), ours);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn conflicts_and_odd_records_parse() {
    let text = "u UU N... 100644 100644 100644 100644 a b c both.txt\0? x y.txt\0! ignored\0# branch.oid abc\0";
    let (unstaged, staged) = parse(text);
    let paths: Vec<_> = unstaged.iter().map(|f| (f.path.as_str(), f.status)).collect();
    assert_eq!(paths, [("both.txt", '!'), ("x y.txt", '?')]);
    assert!(staged.is_empty());
}
