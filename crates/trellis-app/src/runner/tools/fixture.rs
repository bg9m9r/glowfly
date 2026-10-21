//! A conflicted repository for the diff and merge tool runner tests.

use std::path::{Path, PathBuf};
use trellis_core::gitcli::{args, run};

pub(super) fn git(dir: &Path, a: &[&str]) {
    let (ok, out) = run(dir, &args(a));
    assert!(ok, "git {a:?} failed: {out}");
}

/// A repository in the middle of a merge where a.txt conflicts ("ours" against "theirs").
pub(super) fn conflicted(name: &str) -> PathBuf {
    let dir = trellis_core::paths::temp_dir().join(format!("trellis-app-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    for (k, v) in [("user.name", "Test"), ("user.email", "t@example.com"), ("commit.gpgsign", "false"), ("core.autocrlf", "false")] {
        git(&dir, &["config", k, v]);
    }
    std::fs::write(dir.join("a.txt"), "one\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    git(&dir, &["checkout", "-q", "-b", "other"]);
    std::fs::write(dir.join("a.txt"), "theirs\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "theirs"]);
    git(&dir, &["checkout", "-q", "main"]);
    std::fs::write(dir.join("a.txt"), "ours\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "ours"]);
    assert!(!run(&dir, &args(&["merge", "other"])).0, "merge should conflict");
    dir
}
