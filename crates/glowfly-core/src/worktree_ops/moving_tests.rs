use super::*;
use crate::testutil::{git, scratch_repo, write};
use std::path::PathBuf;

/// An absolute path on this platform.
fn abs(rest: &str) -> PathBuf {
    if cfg!(windows) { PathBuf::from(format!("C:/{rest}")) } else { PathBuf::from(format!("/{rest}")) }
}

#[test]
fn move_cmd_form() {
    let (from, to) = (abs("w/side"), abs("w/moved/side"));
    let (f, t) = (from.display().to_string(), to.display().to_string());
    assert_eq!(move_cmd(&from, &to), ["worktree", "move", &f, &t]);
}

#[test]
fn targets_on_disk() {
    let root = crate::paths::temp_dir().join(format!("glowfly-wt-move-target-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    assert_eq!(check_move_target(&root.join("missing")), Ok(()));
    assert_eq!(check_move_target(&root.join("missing/deeper")), Ok(()), "missing parents too");
    std::fs::create_dir(root.join("empty")).unwrap();
    assert_eq!(check_move_target(&root.join("empty")), Err(MoveProblem::Taken), "even an empty folder");
    std::fs::write(root.join("file.txt"), "x").unwrap();
    assert_eq!(check_move_target(&root.join("file.txt")), Err(MoveProblem::Taken));
    let msg = MoveProblem::Taken.message("X");
    assert_eq!(msg, "Something is already at X. Choose a folder that doesn't exist yet.");
    let _ = std::fs::remove_dir_all(&root);
}

/// A repository with one commit and a linked worktree on a new branch `side`, next to it.
fn with_linked(name: &str) -> (PathBuf, PathBuf) {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    let linked = dir.with_file_name(format!("{}-side", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&linked);
    git(&dir, &["worktree", "add", "-q", "-b", "side", &linked.display().to_string()]);
    (dir, linked)
}

/// The folder moves go to, next to the repository; emptied first.
fn moves_root(dir: &Path) -> PathBuf {
    let root = dir.with_file_name(format!("{}-moves", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&root);
    root
}

fn cleanup(dirs: &[&Path]) {
    for d in dirs {
        let _ = std::fs::remove_dir_all(d);
    }
}

fn listed_at(dir: &Path, path: &Path) -> Option<crate::repo::WorktreeInfo> {
    crate::repo::worktrees_at(dir).unwrap().into_iter().find(|w| crate::paths::same_folder(&w.path, path))
}

#[test]
fn moves_into_new_nested_folders() {
    let (dir, linked) = with_linked("wt-move");
    let root = moves_root(&dir);
    let to = root.join("a").join("b").join("side");
    let rep = move_worktree(&dir, &linked, &to);
    assert_eq!(rep.outcome, MoveOutcome::Moved, "{}", rep.output);
    assert!(rep.output.starts_with("$ git worktree move"), "{}", rep.output);
    assert!(!linked.exists(), "the old folder is gone");
    assert!(to.join("a.txt").exists());
    let wt = listed_at(&dir, &to).expect("listed at the new folder");
    assert_eq!(wt.branch.as_deref(), Some("side"));
    assert!(listed_at(&dir, &linked).is_none());
    cleanup(&[&root, &linked, &dir]);
}

#[test]
fn refusals_leave_it_in_place() {
    let (dir, linked) = with_linked("wt-move-refused");
    let root = moves_root(&dir);
    let taken = root.join("taken");
    std::fs::create_dir_all(&taken).unwrap();
    let rep = move_worktree(&dir, &linked, &taken);
    assert_eq!(rep.outcome, MoveOutcome::Taken);
    assert!(rep.output.starts_with("Something is already at"), "{}", rep.output);
    assert!(!rep.output.contains("$ git"), "no git ran: {}", rep.output);
    assert!(linked.join("a.txt").exists() && listed_at(&dir, &linked).is_some());

    git(&dir, &["worktree", "lock", &linked.display().to_string()]);
    let rep = move_worktree(&dir, &linked, &root.join("free"));
    assert_eq!(rep, MoveReport { outcome: MoveOutcome::Failed, output: "It is locked. Unlock it first.".into() });
    assert!(!root.join("free").exists() && linked.exists());

    let rep = move_worktree(&dir, &root.join("nope"), &root.join("free"));
    assert_eq!(rep.outcome, MoveOutcome::Failed);
    assert!(rep.output.contains("no longer a worktree"), "{}", rep.output);
    git(&dir, &["worktree", "unlock", &linked.display().to_string()]);
    cleanup(&[&root, &linked, &dir]);
}
