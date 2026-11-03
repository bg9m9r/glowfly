//! The process-only `safe.directory` list (`safe_directory::allowed`) as libgit2 reads it. A test
//! binary of its own: `init` changes libgit2's search path, which is safe only while no other
//! thread uses libgit2, so nothing else may run beside it.

use glowfly_core::safe_directory::{allowed, config_value};
use std::path::{Path, PathBuf};
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> bool {
    Command::new("git").arg("-C").arg(dir).args(args).status().is_ok_and(|s| s.success())
}

fn listed() -> Vec<String> {
    let config = git2::Config::open_default().unwrap().snapshot().unwrap();
    let mut out = Vec::new();
    let mut entries = config.entries(Some("safe.directory")).unwrap();
    while let Some(e) = entries.next() {
        out.extend(e.unwrap().value().map(str::to_owned));
    }
    out
}

#[test]
fn allowed_folders_reach_libgit2_and_nothing_else_changes() {
    let cache = std::env::temp_dir().join(format!("glowfly-allowed-cache-{}", std::process::id()));
    // `init` keeps its list under the cache folder: point that at a scratch folder first.
    // SAFETY: the only test in this binary, before any other thread exists.
    unsafe {
        std::env::set_var(if cfg!(windows) { "LOCALAPPDATA" } else { "XDG_CACHE_HOME" }, &cache);
    }
    let before = listed();
    allowed::init().unwrap();
    assert!(listed() == before, "nothing is allowed until a folder is");
    let dir: PathBuf = std::env::temp_dir().join(format!("glowfly-allowed-repo-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    assert!(git(&dir, &["init", "-q"]));
    let value = config_value(&dir, cfg!(windows));
    assert!(allowed::allow(std::slice::from_ref(&value)));
    let now = listed();
    assert!(now.contains(&value), "the allowed folder is listed");
    assert!(before.iter().all(|b| now.contains(b)), "the real lists still count");

    // On a Linux runner with passwordless sudo: a folder root owns opens once it is allowed.
    #[cfg(target_os = "linux")]
    {
        let sudo = |a: &[&str]| Command::new("sudo").arg("-n").args(a).status().is_ok_and(|s| s.success());
        let owned = dir.join("owned");
        std::fs::create_dir_all(&owned).unwrap();
        assert!(git(&owned, &["init", "-q"]));
        if !before.iter().any(|v| v == "*") && sudo(&["chown", "-R", "root:root", &owned.display().to_string()]) {
            assert_eq!(git2::Repository::open(&owned).err().map(|e| e.code()), Some(git2::ErrorCode::Owner));
            assert!(allowed::allow(&[config_value(&owned, false)]));
            git2::Repository::open(&owned).expect("allowed");
            let _ = sudo(&["rm", "-rf", &owned.display().to_string()]);
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&cache);
}
