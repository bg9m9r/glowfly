//! Sweeps of fixture repositories: the baseline, folders and files coming and going, edits found
//! by rotation or a racy listing, what is never walked, the git dir's files and the size limit.

use super::*;
use crate::testutil::{git, scratch_repo, write};
use std::time::Duration;

/// Well after every file the test wrote: no listing is racy.
fn later() -> SystemTime {
    SystemTime::now() + Duration::from_secs(60)
}

fn start(dir: &Path) -> (Sweep, Ignores) {
    let mut s = Sweep::new(dir, &dir.join(".git"), None);
    let mut ig = Ignores::open(dir);
    assert_eq!(s.sweep(&mut ig, later()), None, "the first sweep records and reads everything");
    (s, ig)
}

/// A repository with a.txt and `n` folders d00… each holding x.txt, all committed.
fn repo(name: &str, n: usize) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "a\n");
    for i in 0..n {
        std::fs::create_dir_all(dir.join(format!("d{i:02}"))).unwrap();
        write(&dir, &format!("d{i:02}/x.txt"), "x\n");
    }
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "files"]);
    dir
}

#[test]
fn the_baseline_then_nothing() {
    let dir = repo("sweep-baseline", 2);
    let (mut s, mut ig) = start(&dir);
    assert_eq!(s.sweep(&mut ig, later()), Some(Vec::new()));
    assert_eq!(s.sweep(&mut ig, later()), Some(Vec::new()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn files_and_folders_coming_and_going_are_reported() {
    let dir = repo("sweep-come-go", 1);
    let (mut s, mut ig) = start(&dir);
    write(&dir, "new.txt", "new\n");
    assert_eq!(s.sweep(&mut ig, later()), Some(vec![dir.join("new.txt")]));

    std::fs::create_dir_all(dir.join("n/deep")).unwrap();
    write(&dir, "n/deep/x.txt", "x\n");
    assert_eq!(s.sweep(&mut ig, later()), Some(vec![dir.join("n")]), "only the new folder: its path covers the rest");
    assert_eq!(s.sweep(&mut ig, later()), Some(Vec::new()), "walked into, not reported again");
    write(&dir, "n/deep/y.txt", "y\n");
    assert_eq!(s.sweep(&mut ig, later()), Some(vec![dir.join("n/deep/y.txt")]), "the new folder is watched too");

    std::fs::remove_file(dir.join("a.txt")).unwrap();
    assert_eq!(s.sweep(&mut ig, later()), Some(vec![dir.join("a.txt")]));
    std::fs::remove_dir_all(dir.join("n")).unwrap();
    assert_eq!(s.sweep(&mut ig, later()), Some(vec![dir.join("n")]));
    for _ in 0..SWEEP_ROUNDS {
        assert_eq!(s.sweep(&mut ig, later()), Some(Vec::new()), "its folders are forgotten");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_content_edit_is_found_by_rotation() {
    let dir = repo("sweep-rotation", 20);
    let (mut s, mut ig) = start(&dir);
    write(&dir, "d15/x.txt", "longer\n");
    let mut found = Vec::new();
    for _ in 0..SWEEP_ROUNDS {
        found.extend(s.sweep(&mut ig, later()).expect("a sweep"));
    }
    assert_eq!(found, [dir.join("d15/x.txt")]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_racy_folder_is_listed_again_next_time() {
    let dir = repo("sweep-racy", 20);
    let file = dir.join("d19/x.txt");
    let stamp = SystemTime::now() + Duration::from_secs(60);
    let set = |t| std::fs::File::options().write(true).open(&file).unwrap().set_modified(t).unwrap();
    set(stamp);
    let mut s = Sweep::new(&dir, &dir.join(".git"), None);
    let mut ig = Ignores::open(&dir);
    assert_eq!(s.sweep(&mut ig, stamp), None, "taken at the file's own time");
    // Rewritten with another length but the same time; the folder's own stamp doesn't change.
    write(&dir, "d19/x.txt", "longer\n");
    set(stamp);
    assert_eq!(s.sweep(&mut ig, stamp + Duration::from_secs(60)), Some(vec![file.clone()]), "d19 isn't in the first share");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ignored_and_nested_git_folders_are_not_walked() {
    let dir = scratch_repo("sweep-ignored");
    write(&dir, ".gitignore", "target/\nbuild/\n");
    for d in ["target", "build", "nested/.git"] {
        std::fs::create_dir_all(dir.join(d)).unwrap();
    }
    write(&dir, "target/x.o", "obj");
    write(&dir, "build/keep.txt", "keep\n");
    write(&dir, "nested/.git/HEAD", "ref: refs/heads/main\n");
    git(&dir, &["add", ".gitignore"]);
    git(&dir, &["add", "-f", "build/keep.txt"]);
    git(&dir, &["commit", "-q", "-m", "files"]);
    let (mut s, mut ig) = start(&dir);

    write(&dir, "target/y.o", "obj");
    write(&dir, "nested/.git/HEAD", "ref: refs/heads/other\n");
    write(&dir, "nested/.git/ORIG_HEAD", "x\n");
    write(&dir, ".git/description", "changed\n");
    let mut found = Vec::new();
    for _ in 0..SWEEP_ROUNDS {
        found.extend(s.sweep(&mut ig, later()).expect("a sweep"));
    }
    assert!(found.is_empty(), "{found:?}");

    write(&dir, "build/keep.txt", "keep\nmore\n");
    for _ in 0..SWEEP_ROUNDS {
        found.extend(s.sweep(&mut ig, later()).expect("a sweep"));
    }
    assert_eq!(found, [dir.join("build/keep.txt")], "a tracked file under an ignored folder");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_git_dir_s_files_are_compared() {
    let dir = repo("sweep-git", 1);
    let gd = dir.join(".git");
    let (mut s, mut ig) = start(&dir);
    write(&dir, "a.txt", "edited\n");
    git(&dir, &["add", "a.txt"]);
    let found = s.sweep(&mut ig, later()).unwrap();
    assert!(found.contains(&gd.join("index")), "{found:?}");
    git(&dir, &["branch", "x"]);
    assert_eq!(s.sweep(&mut ig, later()), Some(vec![gd.join("HEAD")]), "a ref moved");
    std::fs::create_dir_all(gd.join("info")).unwrap();
    write(&gd, "info/exclude", "*.tmp\n");
    assert_eq!(s.sweep(&mut ig, later()), Some(vec![gd.join("info/exclude")]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_tree_past_the_limit_reads_everything_every_time() {
    let dir = repo("sweep-limit", 3);
    let mut s = Sweep::with_limit(&dir, &dir.join(".git"), None, 3);
    let mut ig = Ignores::open(&dir);
    for _ in 0..3 {
        assert_eq!(s.sweep(&mut ig, later()), None);
    }
    let _ = std::fs::remove_dir_all(&dir);
}
