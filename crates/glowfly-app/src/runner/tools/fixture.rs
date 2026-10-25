//! A conflicted repository for the diff and merge tool runner tests.

use glowfly_core::gitcli::{args, run};
pub(super) use glowfly_core::testkit::git;
use std::path::{Path, PathBuf};

/// A repository in the middle of a merge where a.txt conflicts ("ours" against "theirs").
pub(super) fn conflicted(name: &str) -> PathBuf {
    conflicted_files(name, &["a.txt"])
}

/// A repository in the middle of a merge where each of `files` conflicts ("ours" against
/// "theirs").
pub(super) fn conflicted_files(name: &str, files: &[&str]) -> PathBuf {
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-app-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    for (k, v) in [("user.name", "Test"), ("user.email", "t@example.com"), ("commit.gpgsign", "false"), ("core.autocrlf", "false")] {
        git(&dir, &["config", k, v]);
    }
    write_all(&dir, files, "one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    write_all(&dir, files, "theirs\n");
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    write_all(&dir, files, "ours\n");
    git(&dir, &["commit", "-q", "-am", "ours"]);
    assert!(!run(&dir, &args(&["merge", "other"])).0, "merge should conflict");
    dir
}

fn write_all(dir: &Path, files: &[&str], text: &str) {
    for f in files {
        std::fs::write(dir.join(f), text).unwrap();
    }
}
