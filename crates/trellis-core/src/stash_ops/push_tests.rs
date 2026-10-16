//! The Stash dialog's `git stash push` command lines, and what they do to real repositories: a
//! staged-only stash leaves the rest in the working tree, and a pathspec stash takes only the
//! chosen files, names that look like globs included.

use super::*;
use crate::gitcli::{read, run};
use crate::testutil::{git, scratch_repo, write};
use std::path::{Path, PathBuf};

fn file() -> PathBuf {
    PathBuf::from("/repo/.git/TRELLIS_STASH_PATHS")
}

fn cmd(scope: StashScope, message: Option<&str>, paths: &[&str]) -> PushCommand {
    let paths: Vec<String> = paths.iter().map(|p| p.to_string()).collect();
    push_command(scope, message, &paths, &file())
}

/// A repository with a.txt and b.txt committed.
fn committed(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    write(&dir, "a.txt", "a\n");
    write(&dir, "b.txt", "b\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    dir
}

fn stdout(dir: &Path, a: &[&str]) -> String {
    read(dir, &args(a)).stdout
}

fn text(dir: &Path, f: &str) -> String {
    std::fs::read_to_string(dir.join(f)).unwrap().replace("\r\n", "\n")
}

fn has_staged_option(dir: &Path) -> bool {
    let v = crate::gitcli::version(dir).unwrap_or((0, 0, 0));
    (v.0, v.1) >= STAGED_MIN_GIT
}

#[test]
fn push_message_cases() {
    assert_eq!(push_message(""), None);
    assert_eq!(push_message(" \n\t "), None);
    assert_eq!(push_message("  wip \n note\t").as_deref(), Some("wip note"));
}

#[test]
fn push_command_argv() {
    let none = |c: &PushCommand| assert_eq!(c.pathspec_file, None);
    let c = cmd(StashScope::All, None, &[]);
    assert_eq!(c.cmd, args(&["stash", "push", "-u"]));
    none(&c);
    let c = cmd(StashScope::All, Some("m"), &[]);
    assert_eq!(c.cmd, args(&["stash", "push", "-u", "-m", "m"]));
    none(&c);
    let c = cmd(StashScope::Staged, Some("m"), &[]);
    assert_eq!(c.cmd, args(&["stash", "push", "--staged", "-m", "m"]));
    none(&c);
    let c = cmd(StashScope::Staged, None, &[]);
    assert_eq!(c.cmd, args(&["stash", "push", "--staged"]));
    none(&c);
    let c = cmd(StashScope::Files, Some("m"), &["a.txt", "dir/b c.txt"]);
    assert_eq!(c.cmd, args(&["--literal-pathspecs", "stash", "push", "-u", "-m", "m", "--", "a.txt", "dir/b c.txt"]));
    none(&c);
}

#[test]
fn long_path_lists_go_through_a_pathspec_file() {
    let paths: Vec<String> = (0..200).map(|i| format!("{i:03}-{}", "x".repeat(56))).collect();
    assert!(paths.iter().all(|p| p.len() == 60));
    let c = push_command(StashScope::Files, Some("m"), &paths, &file());
    let flag = format!("--pathspec-from-file={}", file().display());
    assert_eq!(c.cmd, args(&["--literal-pathspecs", "stash", "push", "-u", "-m", "m", &flag, "--pathspec-file-nul"]));
    assert!(!c.cmd.iter().any(|a| a == "--"));
    assert_eq!(c.pathspec_file, Some(paths.join("\0")));
}

#[test]
fn staged_unsupported_cases() {
    assert!(staged_unsupported("error: unknown option `staged'\nusage: git stash ..."));
    assert!(!staged_unsupported("No local changes to save"));
    assert!(!staged_unsupported("error: pathspec 'x' did not match"));
    assert!(!staged_unsupported(""));
}

#[test]
fn staged_stash_leaves_the_rest_in_the_working_tree() {
    let dir = committed("stash-push-staged");
    if !has_staged_option(&dir) {
        eprintln!("skipped: git is older than {}.{}", STAGED_MIN_GIT.0, STAGED_MIN_GIT.1);
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    write(&dir, "a.txt", "a\nstaged\n");
    git(&dir, &["add", "a.txt"]);
    write(&dir, "b.txt", "b\nunstaged\n");
    write(&dir, "c.txt", "c\n");
    let c = push_command(StashScope::Staged, Some("only staged"), &[], &dir.join(".git/unused"));
    let (ok, out) = run(&dir, &c.cmd);
    assert!(ok, "{out}");
    let top = stdout(&dir, &["stash", "list", "--format=%gs"]);
    assert!(top.lines().next().unwrap().ends_with(": only staged"), "{top}");
    assert_eq!(stdout(&dir, &["stash", "show", "--name-only", "stash@{0}"]).trim(), "a.txt");
    assert_eq!(text(&dir, "a.txt"), "a\n");
    assert_eq!(stdout(&dir, &["diff", "--cached", "--name-only"]).trim(), "", "nothing is staged");
    assert_eq!(text(&dir, "b.txt"), "b\nunstaged\n");
    assert!(dir.join("c.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pathspec_stash_takes_only_the_chosen_files() {
    let dir = committed("stash-push-files");
    write(&dir, "a.txt", "a\nedit\n");
    write(&dir, "b.txt", "b\nedit\n");
    write(&dir, "c.txt", "c\n");
    write(&dir, "weird [x].txt", "weird\n");
    write(&dir, "weird x.txt", "decoy\n");
    let paths = vec!["a.txt".to_owned(), "weird [x].txt".to_owned()];
    let c = push_command(StashScope::Files, Some("two files"), &paths, &dir.join(".git/unused"));
    assert_eq!(c.pathspec_file, None);
    let (ok, out) = run(&dir, &c.cmd);
    assert!(ok, "{out}");
    assert_eq!(text(&dir, "a.txt"), "a\n");
    assert!(!dir.join("weird [x].txt").exists());
    assert_eq!(text(&dir, "b.txt"), "b\nedit\n");
    assert!(dir.join("c.txt").exists());
    assert!(dir.join("weird x.txt").exists(), "the literal pathspec leaves the decoy");
    git(&dir, &["stash", "pop"]);
    assert_eq!(text(&dir, "a.txt"), "a\nedit\n");
    assert_eq!(text(&dir, "weird [x].txt"), "weird\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pathspec_file_stash() {
    let dir = committed("stash-push-pathspec-file");
    let paths: Vec<String> = (0..150).map(|i| format!("{i:03}-{}.txt", "long-name".repeat(7))).collect();
    for p in &paths {
        write(&dir, p, "x\n");
    }
    write(&dir, "extra.txt", "extra\n");
    let pathspec = dir.join(".git/TRELLIS_STASH_PATHS");
    let c = push_command(StashScope::Files, None, &paths, &pathspec);
    let contents = c.pathspec_file.expect("past the command-line limit");
    std::fs::write(&pathspec, contents).unwrap();
    let (ok, out) = run(&dir, &c.cmd);
    assert!(ok, "{out}");
    assert!(paths.iter().all(|p| !dir.join(p).exists()), "every listed file is stashed");
    assert!(dir.join("extra.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}
