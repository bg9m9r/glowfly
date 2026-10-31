//! Removing and moving a worktree whose folder git's file system monitor daemon watches: the
//! stop runs first, so the folder is free (on Windows the daemon holds it open).

use super::*;
use glowfly_core::testkit::git;
use glowfly_core::worktree_ops::{MoveOutcome, RemoveForce};
use std::path::{Path, PathBuf};

/// A repository with one commit and a linked worktree `wt` beside it, with git's daemon watching
/// that worktree; None (and the folders cleaned up) when this git has no daemon.
fn watched(name: &str) -> Option<(PathBuf, PathBuf)> {
    let root = glowfly_core::paths::temp_dir().join(format!("glowfly-runner-wt-monitor-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let repo = root.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["-c", "user.name=T", "-c", "user.email=t@example.com", "commit", "-q", "--allow-empty", "-m", "first"]);
    if glowfly_core::repo::daemon_built(&repo) != Some(true) {
        let _ = std::fs::remove_dir_all(&root);
        return None;
    }
    let wt = root.join("wt");
    git(&repo, &["worktree", "add", "-q", "-b", "wt", &wt.display().to_string()]);
    git(&wt, &["fsmonitor--daemon", "start"]);
    Some((repo, wt))
}

/// Stops any daemon left in `dirs` and removes the test's folder.
fn cleanup(root: &Path, dirs: &[&Path]) {
    for d in dirs {
        let _ = glowfly_core::gitcli::run(d, &args(&["fsmonitor--daemon", "stop"]));
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_watched_worktree_is_stopped_then_removed() {
    let Some((repo, wt)) = watched("remove") else { return eprintln!("skipped: this git has no built-in file system monitor") };
    let r = RemoveRequest {
        repo: repo.clone(),
        path: wt.clone(),
        name: "wt".into(),
        force: RemoveForce::None,
        delete_branch: None,
        delete_remote: None,
        stop_monitor: true,
    };
    let Msg::Worktree(WorktreeMsg::Removed(done)) = remove(r) else { panic!("not a remove result") };
    assert!(done.output.starts_with("$ git fsmonitor--daemon stop"), "the stop runs first: {}", done.output);
    assert_eq!(done.outcome, RemoveOutcome::Removed, "{}", done.output);
    assert!(!wt.exists());
    cleanup(repo.parent().unwrap(), &[&repo]);
}

#[test]
fn a_watched_worktree_is_stopped_then_moved() {
    let Some((repo, wt)) = watched("move") else { return eprintln!("skipped: this git has no built-in file system monitor") };
    let to = wt.with_file_name("moved");
    let request = MoveRequest { repo: repo.clone(), from: wt.clone(), to: to.clone(), name: "wt".into(), folder: None, stop_monitor: true };
    let Msg::Worktree(WorktreeMsg::Moved { outcome, output, .. }) = move_(request) else { panic!("not a move result") };
    assert!(output.starts_with("$ git fsmonitor--daemon stop"), "the stop runs first: {output}");
    assert_eq!(outcome, MoveOutcome::Moved, "{output}");
    assert!(to.is_dir() && !wt.exists());
    cleanup(repo.parent().unwrap(), &[&repo, &to]);
}

#[test]
fn nothing_is_stopped_unless_asked() {
    assert_eq!(stop_first(false, Path::new(".")), "");
    assert_eq!(stop_first(true, &glowfly_core::paths::temp_dir().join("glowfly-no-such-worktree")), "", "no folder, no git");
    assert_eq!(joined(String::new(), "b".into()), "b");
    assert_eq!(joined("a".into(), String::new()), "a");
    assert_eq!(joined("a".into(), "b".into()), "a\nb");
}
