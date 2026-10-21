use super::*;
use crate::testutil::*;

fn keys(s: &TrustScan) -> Vec<(&str, &str)> {
    s.config.iter().map(|c| (c.key.as_str(), c.value.as_str())).collect()
}

/// Writes an executable hook script `name` into `dir`.
pub(super) fn hook(dir: &Path, name: &str) {
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

#[test]
fn rewriting_urls_to_a_command_transport_is_listed() {
    let dir = scratch_repo("trust-insteadof");
    git(&dir, &["config", "remote.origin.url", "https://example.invalid/r.git"]);
    git(&dir, &["config", "url.https://mirror.invalid/.insteadOf", "https://example.invalid/"]);
    assert!(scan_at(&dir).unwrap().is_empty(), "a plain rewrite runs nothing");
    git(&dir, &["config", "url.ext::sh -c x.insteadOf", "https://example.invalid/"]);
    git(&dir, &["config", "url.EXT::sh -c y.pushInsteadOf", "https://example.invalid/"]);
    git(&dir, &["config", "protocol.ext.allow", "always"]);
    git(&dir, &["config", "protocol.file.allow", "always"]);
    git(&dir, &["config", "branch.main.remote", "ext::sh -c z"]);
    let s = scan_at(&dir).unwrap();
    assert_eq!(
        keys(&s),
        [
            ("url.ext::sh -c x.insteadOf", "https://example.invalid/"),
            ("url.EXT::sh -c y.pushInsteadOf", "https://example.invalid/"),
            ("protocol.ext.allow", "always"),
            ("branch.main.remote", "ext::sh -c z"),
        ]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn includes_libgit2_cannot_evaluate_are_listed() {
    let dir = scratch_repo("trust-includeif");
    std::fs::write(dir.join(".git/evil.cfg"), "[core]\n\tfsmonitor = .git/fsmon.sh\n").unwrap();
    git(&dir, &["config", "remote.origin.url", "https://example.invalid/r.git"]);
    git(&dir, &["config", "includeIf.onbranch:main.path", "missing.cfg"]);
    git(&dir, &["config", "includeIf.gitdir:/nowhere/.path", "missing.cfg"]);
    assert!(scan_at(&dir).unwrap().is_empty(), "conditions libgit2 evaluates are followed, not listed");
    git(&dir, &["config", "includeIf.hasconfig:remote.*.url:https://**.path", "evil.cfg"]);
    assert_eq!(keys(&scan_at(&dir).unwrap()), [("includeIf.hasconfig:remote.*.url:https://**.path", "evil.cfg")]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(windows)]
#[test]
fn windows_hooks_match_any_case_and_an_exe_suffix() {
    let dir = scratch_repo("trust-hooks-exe");
    let hooks = dir.join(".git/hooks");
    hook(&hooks, "reference-transaction.exe");
    hook(&hooks, "Pre-Commit");
    hook(&hooks, "pre-push.sample.exe");
    assert_eq!(scan_at(&dir).unwrap().hooks, ["Pre-Commit", "reference-transaction.exe"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn checked_out_submodules_are_scanned_too() {
    let (top, sub) = (scratch_repo("trust-sub-top"), scratch_repo("trust-sub-lib"));
    write(&sub, "lib.txt", "lib\n");
    git(&sub, &["add", "."]);
    git(&sub, &["commit", "-q", "-m", "lib"]);
    let url = sub.display().to_string();
    git(&top, &["-c", "protocol.file.allow=always", "submodule", "add", "-q", &url, "deps/lib"]);
    git(&top.join("deps/lib"), &["-c", "protocol.file.allow=always", "submodule", "add", "-q", &url, "inner"]);
    assert!(scan_at(&top).unwrap().is_empty(), "{:?}", scan_at(&top).unwrap());

    git(&top.join("deps/lib"), &["config", "core.fsmonitor", ".git/fsmon.sh"]);
    hook(&top.join(".git/modules/deps/lib/modules/inner/hooks"), "post-merge");
    let s = scan_at(&top).unwrap();
    assert!(s.config.is_empty() && s.hooks.is_empty(), "{s:?}");
    let paths: Vec<&str> = s.submodules.iter().map(|m| m.path.as_str()).collect();
    assert_eq!(paths, ["deps/lib", "deps/lib/inner"]);
    assert_eq!(keys(&s.submodules[0].scan), [("core.fsmonitor", ".git/fsmon.sh")]);
    assert_eq!(s.submodules[1].scan.hooks, ["post-merge"]);
    assert!(!s.is_empty());
    let _ = std::fs::remove_dir_all(&top);
    let _ = std::fs::remove_dir_all(&sub);
}

#[test]
fn hook_names_match_any_case_on_every_os() {
    // A Linux FAT, exFAT, CIFS or casefold mount runs Post-Checkout as post-checkout, so the
    // check can't depend on the OS.
    for name in
        ["pre-commit", "Pre-Commit", "POST-CHECKOUT", "pre-comm\u{131}t", "po\u{17f}t-merge", "pre-com\u{200C}mit", "\u{FEFF}update"]
    {
        assert!(is_hook_name(name), "{name:?}");
    }
    for name in ["pre-commit.sample", "notes.txt", "pre-commits", "precommit"] {
        assert!(!is_hook_name(name), "{name:?}");
    }
    assert_eq!(is_hook_name("pre-commit.exe"), cfg!(windows), "Git for Windows also runs .exe hooks");
}

#[test]
fn a_hook_in_another_case_is_listed_on_every_os() {
    let dir = scratch_repo("trust-hooks-case");
    let hooks = dir.join(".git/hooks");
    hook(&hooks, "Post-Checkout");
    assert_eq!(scan_at(&dir).unwrap().hooks, ["Post-Checkout"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_diff_or_merge_tools_path_is_listed() {
    // git difftool / mergetool start a built-in tool from its .path.
    let dir = scratch_repo("trust-tool-path");
    git(&dir, &["config", "difftool.meld.path", "/opt/meld"]);
    git(&dir, &["config", "mergetool.kdiff3.path", "kdiff3-wrapper"]);
    git(&dir, &["config", "mergetool.kdiff3.trustExitCode", "true"]);
    let s = scan_at(&dir).unwrap();
    assert_eq!(keys(&s), [("difftool.meld.path", "/opt/meld"), ("mergetool.kdiff3.path", "kdiff3-wrapper")]);
    // Only the repository's own config is scanned: the user's tools are theirs.
    assert_eq!(command_key("difftool.meld.path", "/opt/meld").as_deref(), Some("difftool.meld.path"));
    assert_eq!(command_key("mergetool.kdiff3.trustexitcode", "true"), None);
    let _ = std::fs::remove_dir_all(&dir);
}
