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
