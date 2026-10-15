use super::*;
use crate::testutil::*;

fn keys(s: &TrustScan) -> Vec<(&str, &str)> {
    s.config.iter().map(|c| (c.key.as_str(), c.value.as_str())).collect()
}

/// Writes an executable hook script `name` into `dir`.
fn hook(dir: &Path, name: &str) {
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

fn same_dir(a: &Path, b: &Path) -> bool {
    std::fs::canonicalize(a).unwrap() == std::fs::canonicalize(b).unwrap()
}

#[test]
fn a_new_repository_runs_nothing() {
    // git init's *.sample hooks never run, and the machine's own credential.helper (if any) is
    // the user's, not the repository's.
    let dir = scratch_repo("trust-clean");
    let s = scan_at(&dir).unwrap();
    assert!(s.is_empty(), "{s:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn settings_that_run_programs_are_listed_in_file_order() {
    let dir = scratch_repo("trust-config");
    for (k, v) in [
        ("core.sshCommand", "ssh -i key"),
        ("credential.helper", "!f() { echo x; }; f"),
        ("credential.https://example.com.helper", "store"),
        ("core.editor", "vim"),
        ("filter.x.clean", "clean-it"),
        ("diff.x.textconv", "conv"),
        ("remote.origin.uploadpack", "up"),
        ("core.fsmonitor", ".git/fsmon.sh"),
    ] {
        git(&dir, &["config", k, v]);
    }
    let s = scan_at(&dir).unwrap();
    // git config adds a key to its section if the file has one already, so [core] comes first.
    assert_eq!(
        keys(&s),
        [
            ("core.sshCommand", "ssh -i key"),
            ("core.editor", "vim"),
            ("core.fsmonitor", ".git/fsmon.sh"),
            ("credential.helper", "!f() { echo x; }; f"),
            ("credential.https://example.com.helper", "store"),
            ("filter.x.clean", "clean-it"),
            ("diff.x.textconv", "conv"),
            ("remote.origin.uploadpack", "up"),
        ]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn harmless_settings_are_not_listed() {
    let dir = scratch_repo("trust-harmless");
    git(&dir, &["config", "core.fsmonitor", "true"]);
    git(&dir, &["config", "credential.helper", ""]);
    git(&dir, &["config", "remote.origin.url", "https://example.com/r.git"]);
    git(&dir, &["config", "alias.x", "!echo"]);
    let s = scan_at(&dir).unwrap();
    assert!(s.config.is_empty(), "{s:?}");
    // ext:: runs a command as the transport.
    git(&dir, &["config", "remote.origin.pushurl", "ext::sh -c x"]);
    assert_eq!(keys(&scan_at(&dir).unwrap()), [("remote.origin.pushurl", "ext::sh -c x")]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn only_hooks_git_would_run_are_listed() {
    let dir = scratch_repo("trust-hooks");
    let hooks = dir.join(".git/hooks");
    hook(&hooks, "pre-commit");
    std::fs::write(hooks.join("notes.txt"), "x").unwrap();
    std::fs::create_dir_all(hooks.join("post-merge")).unwrap();
    #[cfg(unix)]
    std::fs::write(hooks.join("post-checkout"), "#!/bin/sh\n").unwrap();
    let s = scan_at(&dir).unwrap();
    assert_eq!(s.hooks, ["pre-commit"]);
    assert!(hooks.join("pre-push.sample").exists(), "git init wrote samples");
    assert!(same_dir(s.hooks_dir.as_deref().unwrap(), &hooks));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_local_hooks_path_is_followed() {
    let dir = scratch_repo("trust-hookspath");
    git(&dir, &["config", "core.hooksPath", ".githooks"]);
    hook(&dir.join(".githooks"), "pre-push");
    let s = scan_at(&dir).unwrap();
    assert_eq!(keys(&s), [("core.hooksPath", ".githooks")]);
    assert_eq!(s.hooks, ["pre-push"]);
    assert!(same_dir(s.hooks_dir.as_deref().unwrap(), &dir.join(".githooks")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn included_files_count_as_the_repository_s_own() {
    let dir = scratch_repo("trust-include");
    std::fs::write(dir.join(".git/extra.cfg"), "[core]\n\tsshCommand = ssh -v\n").unwrap();
    git(&dir, &["config", "include.path", "extra.cfg"]);
    assert_eq!(keys(&scan_at(&dir).unwrap()), [("core.sshCommand", "ssh -v")]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_linked_worktree_reports_its_main_repository_s_hooks() {
    let dir = scratch_repo("trust-worktree");
    write(&dir, "a.txt", "a\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    hook(&dir.join(".git/hooks"), "pre-commit");
    let wt = dir.with_file_name(format!("{}-wt", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&wt);
    git(&dir, &["worktree", "add", "-q", "-b", "side", &wt.display().to_string()]);
    let s = scan_at(&wt).unwrap();
    assert_eq!(s.hooks, ["pre-commit"]);
    let _ = std::fs::remove_dir_all(&wt);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn init_output_tells_a_new_repository_apart() {
    assert!(init_created_new("Initialized empty Git repository in /x/.git/\n"));
    assert!(!init_created_new("Reinitialized existing Git repository in /x/.git/\n"));
    assert!(!init_created_new(""));
}
