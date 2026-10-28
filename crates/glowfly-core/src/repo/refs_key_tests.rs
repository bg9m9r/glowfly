use super::refs_key;
use super::refs_key::{RACY_WINDOW, stamp_trustworthy};
use crate::testutil::*;
use std::time::{Duration, SystemTime};

#[test]
fn refs_key_follows_refs_config_and_stashes_but_not_the_working_tree() {
    let dir = scratch_repo("refs-key");
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    let key = || refs_key(&dir).unwrap();
    let k0 = key();
    write(&dir, "a.txt", "two\n");
    write(&dir, "new.txt", "new\n");
    git(&dir, &["add", "new.txt"]);
    assert_eq!(key(), k0, "staging and editing are status changes");

    git(&dir, &["stash", "-q"]);
    let k1 = key();
    assert_ne!(k1, k0, "a stash");
    write(&dir, "a.txt", "three\n");
    git(&dir, &["stash", "-q"]);
    let k2 = key();
    git(&dir, &["stash", "drop", "-q", "stash@{1}"]);
    assert_ne!(key(), k2, "dropping an older stash rewrites only the reflog");

    let k3 = key();
    git(&dir, &["branch", "side"]);
    assert_ne!(key(), k3, "a new branch");
    let k4 = key();
    git(&dir, &["checkout", "-q", "side"]);
    assert_ne!(key(), k4, "HEAD moved to another branch");
    let k5 = key();
    git(&dir, &["config", "branch.side.description", "x"]);
    assert_ne!(key(), k5, "config");
    let k6 = key();
    git(&dir, &["update-ref", "refs/glowfly/discarded/x", "HEAD"]);
    assert_eq!(key(), k6, "Glowfly's own records are never loaded");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn recent_modification_times_are_not_trusted() {
    let now = SystemTime::now();
    assert!(stamp_trustworthy(Some(now - RACY_WINDOW), now));
    assert!(stamp_trustworthy(Some(now - Duration::from_secs(3600)), now));
    assert!(!stamp_trustworthy(Some(now - Duration::from_millis(1999)), now), "within one coarse tick");
    assert!(!stamp_trustworthy(Some(now), now));
    assert!(!stamp_trustworthy(Some(now + Duration::from_secs(5)), now), "from the future: the clock moved");
    assert!(!stamp_trustworthy(None, now), "no time at all");
}

#[test]
fn a_same_length_config_rewrite_with_the_same_time_is_seen() {
    // `git remote set-url` to a URL of the same length within one tick of a coarse clock leaves
    // the config's size and time as they were.
    let dir = scratch_repo("refs-key-racy");
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    git(&dir, &["remote", "add", "origin", "https://example.com/aaaa.git"]);
    let config = dir.join(".git").join("config");
    let when = std::fs::metadata(&config).unwrap().modified().unwrap();
    let k0 = refs_key(&dir).unwrap();
    git(&dir, &["remote", "set-url", "origin", "https://example.com/bbbb.git"]);
    let f = std::fs::File::options().write(true).open(&config).unwrap();
    f.set_modified(when).unwrap();
    drop(f);
    assert_eq!(std::fs::metadata(&config).unwrap().modified().unwrap(), when);
    assert_ne!(refs_key(&dir).unwrap(), k0, "the remote's URL changed");

    // An old modification time is trusted for a large file only; a small one is always read.
    let old = SystemTime::now() - Duration::from_secs(3600);
    let f = std::fs::File::options().write(true).open(&config).unwrap();
    f.set_modified(old).unwrap();
    drop(f);
    let k1 = refs_key(&dir).unwrap();
    assert_eq!(refs_key(&dir).unwrap(), k1, "nothing changed");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn refs_key_follows_the_worktree_config() {
    let dir = scratch_repo("refs-key-wtconfig");
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    git(&dir, &["config", "extensions.worktreeConfig", "true"]);
    let k0 = refs_key(&dir).unwrap();
    git(&dir, &["config", "--worktree", "core.sshCommand", "ssh -v"]);
    assert_ne!(refs_key(&dir).unwrap(), k0, "the main worktree's config.worktree");

    let linked = dir.with_file_name(format!("{}-linked", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&linked);
    git(&dir, &["worktree", "add", "-q", "-b", "linked", &linked.display().to_string()]);
    let k1 = refs_key(&linked).unwrap();
    git(&linked, &["config", "--worktree", "x.y", "z"]);
    assert_ne!(refs_key(&linked).unwrap(), k1, "a linked worktree's own config.worktree");
    let _ = std::fs::remove_dir_all(&linked);
    let _ = std::fs::remove_dir_all(&dir);
}
