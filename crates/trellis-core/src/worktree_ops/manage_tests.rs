use super::*;
use crate::testutil::{git, scratch_repo, write};
use std::path::PathBuf;

/// An absolute path on this platform.
fn abs(rest: &str) -> PathBuf {
    if cfg!(windows) { PathBuf::from(format!("C:/{rest}")) } else { PathBuf::from(format!("/{rest}")) }
}

#[test]
fn remove_cmd_forms() {
    let dir = abs("w/side");
    let d = dir.display().to_string();
    assert_eq!(remove_cmd(&dir, RemoveForce::None), ["worktree", "remove", &d]);
    assert_eq!(remove_cmd(&dir, RemoveForce::Changes), ["worktree", "remove", "--force", &d]);
    assert_eq!(remove_cmd(&dir, RemoveForce::Lock), ["worktree", "remove", "--force", "--force", &d]);
}

#[test]
fn lock_unlock_prune_cmds() {
    let dir = abs("w/side");
    let d = dir.display().to_string();
    assert_eq!(lock_cmd(&dir, Some("usb")), ["worktree", "lock", "--reason", "usb", &d]);
    assert_eq!(lock_cmd(&dir, Some("  ")), ["worktree", "lock", &d]);
    assert_eq!(lock_cmd(&dir, None), ["worktree", "lock", &d]);
    assert_eq!(lock_cmd(&dir, Some("on the\nusb drive")), ["worktree", "lock", "--reason", "on the usb drive", &d]);
    assert_eq!(unlock_cmd(&dir), ["worktree", "unlock", &d]);
    assert_eq!(prune_cmd(), ["worktree", "prune", "--verbose"]);
}

#[test]
fn refusals() {
    let dirty = "fatal: 'C:/w/side' contains modified or untracked files, use --force to delete it";
    assert_eq!(refusal_in(dirty), Some(Refusal::Changes(0)));
    let locked = "fatal: cannot remove a locked working tree, lock reason: usb\nuse 'remove -f -f' to override or unlock first";
    assert_eq!(refusal_in(locked), Some(Refusal::Locked("usb".into())));
    let bare_lock = "fatal: cannot remove a locked working tree;\nuse 'remove -f -f' to override or unlock first";
    assert_eq!(refusal_in(bare_lock), Some(Refusal::Locked(String::new())));
    assert_eq!(refusal_in("fatale: arbre verrouillé ; utilisez 'remove -f -f'"), Some(Refusal::Locked(String::new())));
    assert_eq!(refusal_in("fatal: 'x' is not a working tree"), None);
    assert_eq!(Refusal::Changes(2).force(), RemoveForce::Changes);
    assert_eq!(Refusal::Locked("x".into()).force(), RemoveForce::Lock);
}

#[test]
fn classify_by_listing() {
    let dirty = "fatal: 'x' contains modified or untracked files, use --force to delete it";
    assert_eq!(classify(true, true, dirty), RemoveOutcome::NeedsForce(Refusal::Changes(0)));
    assert_eq!(classify(true, true, "fatal: something else"), RemoveOutcome::Failed);
    assert_eq!(classify(false, true, "error: failed to delete 'x': Permission denied"), RemoveOutcome::FolderLeft);
    assert_eq!(classify(false, false, ""), RemoveOutcome::Removed);
}

/// A repository with one commit and a linked worktree on a new branch `side`, next to it.
fn with_linked(name: &str) -> (PathBuf, PathBuf) {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "one\n");
    write(&dir, ".gitignore", "*.log\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    let linked = dir.with_file_name(format!("{}-side", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&linked);
    git(&dir, &["worktree", "add", "-q", "-b", "side", &linked.display().to_string()]);
    (dir, linked)
}

fn cleanup(dir: &Path, linked: &Path) {
    let _ = std::fs::remove_dir_all(linked);
    let _ = std::fs::remove_dir_all(dir);
}

fn listed(dir: &Path, linked: &Path) -> bool {
    crate::repo::worktrees_at(dir).unwrap().iter().any(|w| crate::worktree_ops::same_folder(&w.path, linked))
}

#[test]
fn remove_clean() {
    let (dir, linked) = with_linked("wt-rm-clean");
    let rep = remove(&dir, &linked, RemoveForce::None);
    assert_eq!(rep.outcome, RemoveOutcome::Removed, "{}", rep.output);
    assert!(rep.output.starts_with("$ git worktree remove"), "{}", rep.output);
    assert!(!linked.exists() && !listed(&dir, &linked));
    cleanup(&dir, &linked);
}

#[test]
fn remove_dirty_needs_force() {
    let (dir, linked) = with_linked("wt-rm-dirty");
    write(&linked, "new.txt", "x\n");
    let rep = remove(&dir, &linked, RemoveForce::None);
    assert_eq!(rep.outcome, RemoveOutcome::NeedsForce(Refusal::Changes(1)));
    assert!(!rep.output.contains("$ git"), "no git ran: {}", rep.output);
    assert!(linked.join("new.txt").exists() && listed(&dir, &linked));
    let rep = remove(&dir, &linked, RemoveForce::Changes);
    assert_eq!(rep.outcome, RemoveOutcome::Removed, "{}", rep.output);
    assert!(!linked.exists());
    cleanup(&dir, &linked);
}

#[test]
fn remove_locked_needs_double_force() {
    let (dir, linked) = with_linked("wt-rm-locked");
    git(&dir, &["worktree", "lock", "--reason", "usb", &linked.display().to_string()]);
    for force in [RemoveForce::None, RemoveForce::Changes] {
        let rep = remove(&dir, &linked, force);
        assert_eq!(rep.outcome, RemoveOutcome::NeedsForce(Refusal::Locked("usb".into())), "{force:?}");
    }
    assert!(linked.exists());
    let rep = remove(&dir, &linked, RemoveForce::Lock);
    assert_eq!(rep.outcome, RemoveOutcome::Removed, "{}", rep.output);
    assert!(rep.output.contains("--force --force"));
    assert!(!listed(&dir, &linked));
    cleanup(&dir, &linked);
}

#[test]
fn remove_missing_and_unknown() {
    let (dir, linked) = with_linked("wt-rm-missing");
    std::fs::remove_dir_all(&linked).unwrap();
    let rep = remove(&dir, &linked, RemoveForce::None);
    assert_eq!(rep.outcome, RemoveOutcome::Removed, "{}", rep.output);
    assert!(!listed(&dir, &linked));
    let rep = remove(&dir, &linked, RemoveForce::None);
    assert_eq!(rep.outcome, RemoveOutcome::Failed);
    assert!(rep.output.contains("no longer a worktree"), "{}", rep.output);
    cleanup(&dir, &linked);
}

#[test]
fn dirty_counts() {
    let (dir, linked) = with_linked("wt-dirty-count");
    assert_eq!(dirty_count(&linked).unwrap(), 0);
    write(&linked, "debug.log", "ignored\n");
    assert_eq!(dirty_count(&linked).unwrap(), 0, "ignored files don't count");
    write(&linked, "a.txt", "two\n");
    write(&linked, "staged.txt", "s\n");
    git(&linked, &["add", "staged.txt"]);
    write(&linked, "untracked.txt", "u\n");
    assert_eq!(dirty_count(&linked).unwrap(), 3);
    cleanup(&dir, &linked);
}
