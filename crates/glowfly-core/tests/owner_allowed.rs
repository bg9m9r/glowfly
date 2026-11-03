//! The process-only `safe.directory` list (`safe_directory::allowed`) as libgit2 reads it, and
//! `safe_directory::open` deciding by git's verdict. A test binary of its own: `init` changes
//! libgit2's search path, which is safe only while no other thread uses libgit2, and the test
//! points HOME and the cache folder elsewhere, so nothing else may run beside it.

use glowfly_core::safe_directory::{allowed, config_value};
use std::path::{Path, PathBuf};
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> bool {
    Command::new("git").arg("-C").arg(dir).args(args).status().is_ok_and(|s| s.success())
}

/// The `safe.directory` values libgit2 reads outside any repository.
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
fn only_folders_git_opens_reach_libgit2() {
    let scratch = std::env::temp_dir().join(format!("glowfly-allowed-{}", std::process::id()));
    let home = scratch.join("home");
    std::fs::create_dir_all(&home).unwrap();
    // The user's own global config: still read, and still at the global level, through the list.
    std::fs::write(home.join(".gitconfig"), "[user]\n\tname = Ada\n[core]\n\tsshCommand = ssh -v\n").unwrap();
    // SAFETY: the only test in this binary, before libgit2 or any other thread starts.
    unsafe {
        std::env::set_var("HOME", &home);
        std::env::set_var("USERPROFILE", &home);
        std::env::set_var(if cfg!(windows) { "LOCALAPPDATA" } else { "XDG_CACHE_HOME" }, scratch.join("cache"));
    }
    let before = listed();
    allowed::init().unwrap();
    assert!(listed() == before, "nothing is allowed until a folder is");

    let dir: PathBuf = scratch.join("repo");
    std::fs::create_dir_all(&dir).unwrap();
    assert!(git(&dir, &["init", "-q"]));
    let value = config_value(&dir, cfg!(windows));
    assert!(allowed::allow(std::slice::from_ref(&value)));
    let now = listed();
    assert!(now.contains(&value), "the allowed folder is listed");
    assert!(before.iter().all(|b| now.contains(b)), "the real lists still count");

    let repo = git2::Repository::open(&dir).unwrap();
    let config = repo.config().unwrap().snapshot().unwrap();
    let name = config.get_entry("user.name").unwrap();
    assert_eq!((name.value(), name.level()), (Some("Ada"), git2::ConfigLevel::Global), "read through the list");
    assert!(glowfly_core::trust::scan(&repo).config.is_empty(), "the user's own settings aren't the repository's");

    // A name that would end its line never reaches the list.
    assert!(!allowed::allow(&["/srv/x\n[safe]\n\tdirectory = *".to_owned()]));
    assert!(!listed().iter().any(|v| v == "*" || v.contains('\n')));

    // On a Linux runner with passwordless sudo: a repository root owns.
    #[cfg(target_os = "linux")]
    root_owned(&dir);
    let _ = std::fs::remove_dir_all(&scratch);
}

/// git refuses it: `open` says so and lists nothing. git accepts it (a `safe.directory` only
/// git reads): `open` opens it.
#[cfg(target_os = "linux")]
fn root_owned(base: &Path) {
    use glowfly_core::error::Error;
    use glowfly_core::safe_directory;
    let sudo = |a: &[&str]| Command::new("sudo").arg("-n").args(a).status().is_ok_and(|s| s.success());
    let owned = base.join("owned");
    std::fs::create_dir_all(&owned).unwrap();
    assert!(git(&owned, &["init", "-q"]));
    let safe = Command::new("git").args(["config", "--get-all", "safe.directory"]).output();
    let all_safe = safe.is_ok_and(|o| String::from_utf8_lossy(&o.stdout).lines().any(|l| l.trim() == "*"));
    if all_safe || !sudo(&["chown", "-R", "root:root", &owned.display().to_string()]) {
        eprintln!("skipped: needs passwordless sudo and no safe.directory=*");
        return;
    }
    let before = listed();
    match safe_directory::open(&owned) {
        Err(Error::NotOwned(r)) => assert_eq!(r.root, glowfly_core::paths::resolve(&owned)),
        other => panic!("expected NotOwned, got {:?}", other.map(|_| ())),
    }
    assert!(listed() == before, "a refusal lists nothing");
    // SAFETY: still the only thread; git reads these, libgit2 doesn't.
    unsafe {
        std::env::set_var("GIT_CONFIG_COUNT", "1");
        std::env::set_var("GIT_CONFIG_KEY_0", "safe.directory");
        std::env::set_var("GIT_CONFIG_VALUE_0", &owned);
    }
    safe_directory::open(&owned).expect("git opens it, so Glowfly does");
    safe_directory::discover(&owned).expect("and finds it");
    unsafe {
        std::env::remove_var("GIT_CONFIG_COUNT");
    }
    let _ = sudo(&["rm", "-rf", &owned.display().to_string()]);
}
