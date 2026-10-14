//! Fixture helpers shared by `trellis-core` tests: throwaway repositories and git commands.

use crate::gitcli::{args, run};
use std::path::{Path, PathBuf};

/// Runs `git <a>` in `dir`, failing the test if it fails.
pub(crate) fn git(dir: &Path, a: &[&str]) {
    let (ok, out) = run(dir, &args(a));
    assert!(ok, "git {a:?} failed: {out}");
}

/// An empty repository on `main` with a fixed identity, in a fresh temporary folder.
pub(crate) fn scratch_repo(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("trellis-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-b", "main"]);
    git(&dir, &["config", "user.name", "Test"]);
    git(&dir, &["config", "user.email", "test@example.com"]);
    git(&dir, &["config", "core.autocrlf", "false"]);
    git(&dir, &["config", "commit.gpgsign", "false"]);
    dir
}

pub(crate) fn write(dir: &Path, file: &str, text: &str) {
    std::fs::write(dir.join(file), text).unwrap();
}

/// A repository with one commit on `main`, pushed with upstream to a bare `origin` next to it.
/// Returns (working repository, bare repository).
pub(crate) fn with_origin(name: &str) -> (PathBuf, PathBuf) {
    let work = scratch_repo(name);
    write(&work, "a.txt", "one\n");
    git(&work, &["add", "."]);
    git(&work, &["commit", "-q", "-m", "first"]);
    let bare = work.with_file_name(format!("{}-origin.git", work.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&bare);
    std::fs::create_dir_all(&bare).unwrap();
    git(&bare, &["init", "-q", "--bare", "-b", "main"]);
    git(&work, &["remote", "add", "origin", &bare.display().to_string()]);
    git(&work, &["push", "-q", "-u", "origin", "main"]);
    (work, bare)
}

/// Commits `file` on `branch` in the bare repository `bare` (through a temporary clone), so
/// the remote gets ahead of local copies.
pub(crate) fn remote_commit(bare: &Path, branch: &str, file: &str, msg: &str) {
    let tmp = bare.with_file_name(format!("{}-clone", bare.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&tmp);
    let parent = tmp.parent().unwrap();
    git(parent, &["clone", "-q", "-b", branch, &bare.display().to_string(), &tmp.display().to_string()]);
    git(&tmp, &["config", "user.name", "Test"]);
    git(&tmp, &["config", "user.email", "test@example.com"]);
    git(&tmp, &["config", "commit.gpgsign", "false"]);
    write(&tmp, file, msg);
    git(&tmp, &["add", "."]);
    git(&tmp, &["commit", "-q", "-m", msg]);
    git(&tmp, &["push", "-q", "origin", branch]);
    let _ = std::fs::remove_dir_all(&tmp);
}
