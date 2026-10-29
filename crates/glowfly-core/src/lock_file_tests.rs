//! Delete lock file on fixture repositories: what is deleted, and every refusal, including a
//! real git process running beside it.

use super::{Found, Refusal, check, check_with, delete_with};
use crate::git_procs::{GitProcesses, Unknown};
use crate::testutil::{alias_of, git, scratch_repo, write};
use std::path::{Path, PathBuf};

fn none() -> GitProcesses {
    GitProcesses::None
}

/// The repository's `.git` folder as git spells it in its errors (forward slashes).
fn named(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// A repository with one commit and a stale `index.lock`.
fn locked(name: &str) -> (PathBuf, PathBuf) {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "a\n");
    git(&dir, &["add", "a.txt"]);
    git(&dir, &["commit", "-m", "a"]);
    let lock = dir.join(".git").join("index.lock");
    std::fs::write(&lock, "").unwrap();
    (dir, lock)
}

#[test]
fn a_stale_lock_is_deleted_and_a_missing_one_is_gone() {
    let (dir, lock) = locked("lock-stale");
    let real = crate::paths::resolve(&lock);
    assert_eq!(check_with(&dir, &named(&lock), none), Ok(Found::Stale(real.clone())));
    assert!(lock.exists(), "a check deletes nothing");
    assert_eq!(delete_with(&dir, &named(&lock), none), Ok(Found::Stale(real)));
    assert!(!lock.exists());
    assert_eq!(delete_with(&dir, &named(&lock), none), Ok(Found::Gone), "already gone: the job may run again");
    // A ref's lock deep in the git dir counts too.
    let reflock = dir.join(".git/refs/heads/main.lock");
    std::fs::write(&reflock, "").unwrap();
    assert!(matches!(delete_with(&dir, &named(&reflock), none), Ok(Found::Stale(_))));
    assert!(!reflock.exists());
}

#[test]
fn it_is_refused_while_git_runs_or_when_that_is_unknown() {
    let (dir, lock) = locked("lock-running");
    let running = || GitProcesses::Running(vec![(42, "git.exe".into())]);
    assert_eq!(delete_with(&dir, &named(&lock), running), Err(Refusal::GitRunning(vec![(42, "git.exe".into())])));
    let unknown = || GitProcesses::Unknown(Unknown::Hidden);
    assert_eq!(delete_with(&dir, &named(&lock), unknown), Err(Refusal::Unknown(Unknown::Hidden)));
    assert!(lock.exists(), "nothing was deleted");
}

#[test]
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn a_real_git_process_refuses_it() {
    use std::process::{Command, Stdio};
    let (dir, lock) = locked("lock-real-git");
    // Waits on its input until it is closed, inside the repository.
    let mut child =
        Command::new("git").args(["hash-object", "--stdin"]).current_dir(&dir).stdin(Stdio::piped()).stdout(Stdio::null()).spawn().unwrap();
    let got = check(&dir, &named(&lock));
    drop(child.stdin.take());
    let _ = child.wait();
    assert!(matches!(got, Err(Refusal::GitRunning(_) | Refusal::Unknown(_))), "{got:?}");
    assert!(lock.exists());
}

#[test]
fn only_a_lock_inside_the_git_dir() {
    let (dir, _) = locked("lock-outside");
    let in_tree = dir.join("a.lock");
    std::fs::write(&in_tree, "").unwrap();
    assert_eq!(delete_with(&dir, &named(&in_tree), none), Err(Refusal::Outside), "the working tree isn't the git dir");
    assert!(in_tree.exists());
    let other = scratch_repo("lock-outside-other");
    let theirs = other.join(".git/index.lock");
    std::fs::write(&theirs, "").unwrap();
    assert_eq!(delete_with(&dir, &named(&theirs), none), Err(Refusal::Outside), "another repository's");
    let dotted = format!("{}/.git/../a.lock", named(&dir));
    assert_eq!(delete_with(&dir, &dotted, none), Err(Refusal::Outside));
    assert_eq!(delete_with(&dir, &named(&dir.join(".git/config")), none), Err(Refusal::NotLock));
    assert_eq!(delete_with(&dir, ".git/index.lock", none), Err(Refusal::NotLock), "a relative path");
    assert_eq!(delete_with(&dir, &named(&dir.join(".git/.lock")), none), Err(Refusal::NotLock));
    assert!(theirs.exists() && in_tree.exists());
}

#[test]
fn never_a_folder_or_a_link() {
    let (dir, _) = locked("lock-not-file");
    let folder = dir.join(".git/odd.lock");
    std::fs::create_dir(&folder).unwrap();
    assert_eq!(delete_with(&dir, &named(&folder), none), Err(Refusal::NotAFile));
    assert!(folder.is_dir());
    // A folder inside .git that links elsewhere: its lock is outside.
    let away = scratch_repo("lock-not-file-away");
    std::fs::write(away.join("x.lock"), "").unwrap();
    if let Some(link) = alias_of(&away) {
        let inside = dir.join(".git/away");
        std::fs::rename(&link, &inside).unwrap();
        assert_eq!(delete_with(&dir, &named(&inside.join("x.lock")), none), Err(Refusal::Outside));
        assert!(away.join("x.lock").exists());
    }
    // A lock that is itself a link.
    let target = away.join("target.txt");
    std::fs::write(&target, "keep").unwrap();
    let link = dir.join(".git/link.lock");
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(&target, &link).is_ok();
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_file(&target, &link).is_ok();
    if made {
        assert_eq!(delete_with(&dir, &named(&link), none), Err(Refusal::NotAFile));
        assert!(target.exists() && std::fs::symlink_metadata(&link).is_ok());
    }
}

#[test]
fn a_worktree_may_delete_its_own_and_the_common_dirs_locks() {
    let (dir, lock) = locked("lock-worktree");
    std::fs::remove_file(&lock).unwrap();
    let wt = dir.with_file_name(format!("{}-wt", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&wt);
    git(&dir, &["worktree", "add", "-b", "side", &wt.to_string_lossy()]);
    let repo = git2::Repository::open(&wt).unwrap();
    let own = repo.path().join("index.lock");
    std::fs::write(&own, "").unwrap();
    assert!(matches!(delete_with(&wt, &named(&own), none), Ok(Found::Stale(_))));
    let common = dir.join(".git/refs/heads/side.lock");
    std::fs::write(&common, "").unwrap();
    assert!(matches!(delete_with(&wt, &named(&common), none), Ok(Found::Stale(_))));
    assert!(!own.exists() && !common.exists());
}
