//! Which files a hook runs are read as shell: one a shell runs whatever its name, and a command
//! with no `#!` line (sh runs it itself); a tracked script under `node_modules` too. Which
//! words name a home folder (`$HOME`, the XDG ones, an unquoted `~`) and which don't
//! (`$PROJECT_HOME`, gradlew's `$APP_HOME`, a quoted `~`). And `.git` as folding file systems
//! name it.

use super::super::super::allows;
use super::files::in_tree;
use super::tests::{hook_with, paths, scan_of};
use super::*;
use crate::settings::SavedTrust;
use crate::testutil::*;

/// A repository whose committed `scripts/x` (no `#!` line, no `.sh`) runs `./scripts/y`, and
/// whose pre-commit hook is `hook`.
fn runs_x(name: &str, hook: &str) -> PathBuf {
    let dir = scratch_repo(name);
    std::fs::create_dir_all(dir.join("scripts")).unwrap();
    write(&dir, "scripts/x", "set -e\n./scripts/y\n");
    write(&dir, "scripts/y", "echo one\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "scripts"]);
    hook_with(&dir.join(".git/hooks"), "pre-commit", hook);
    dir
}

#[test]
fn a_script_a_shell_runs_or_a_command_without_a_hash_bang_line_is_read() {
    for (name, hook) in [("hook-shell-sh", "#!/bin/sh\nsh scripts/x\n"), ("hook-shell-direct", "#!/bin/sh\n./scripts/x\n")] {
        let dir = runs_x(name, hook);
        let s = scan_of(&dir);
        assert_eq!(paths(&s), ["scripts/x", "scripts/y"], "{name}");
        // A commit that changes what `x` runs asks again.
        let saved = SavedTrust::trusted(Some(s.print()));
        write(&dir, "scripts/y", "echo two\n");
        git(&dir, &["commit", "-q", "--no-verify", "-am", "y"]);
        assert!(!allows(Some(&saved), &scan_of(&dir)), "{name}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn a_command_whose_hash_bang_line_runs_no_shell_is_not_read() {
    let dir = runs_x("hook-shell-node", "#!/bin/sh\n./scripts/x\n");
    write(&dir, "scripts/x", "#!/usr/bin/env node\n./scripts/y\n");
    assert_eq!(paths(&scan_of(&dir)), ["scripts/x"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_tracked_script_under_node_modules_is_read_and_an_installed_one_is_not() {
    let dir = scratch_repo("hook-shell-modules");
    std::fs::create_dir_all(dir.join("node_modules/tools")).unwrap();
    std::fs::create_dir_all(dir.join("scripts")).unwrap();
    write(&dir, "node_modules/tools/check.sh", "sh ./scripts/y.sh\n");
    write(&dir, "scripts/y.sh", "exit 0\n");
    hook_with(&dir.join(".git/hooks"), "pre-commit", "#!/bin/sh\nsh node_modules/tools/check.sh\n");
    assert_eq!(paths(&scan_of(&dir)), ["node_modules/tools/check.sh"], "installed");
    git(&dir, &["add", "-f", "node_modules/tools/check.sh"]);
    assert_eq!(paths(&scan_of(&dir)), ["node_modules/tools/check.sh", "scripts/y.sh"], "tracked");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn only_home_and_the_xdg_folders_are_a_home_folder() {
    let dir = scratch_repo("hook-shell-home-vars");
    std::fs::create_dir_all(dir.join("scripts")).unwrap();
    write(&dir, "scripts/check.sh", "exit 0\n");
    let hook = "#!/bin/sh\nPROJECT_HOME=$(git rev-parse --show-toplevel)\nsh \"$PROJECT_HOME/scripts/check.sh\"\n";
    hook_with(&dir.join(".git/hooks"), "pre-commit", hook);
    assert_eq!(paths(&scan_of(&dir)), ["scripts/check.sh"]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The lines of Gradle's `gradlew` that find its wrapper jar.
const GRADLEW: &str = r#"#!/bin/sh
app_path=$0
APP_HOME=${app_path%"${app_path##*/}"}
APP_HOME=$( cd -P "${APP_HOME:-./}.." > /dev/null && printf '%s\n' "$PWD" ) || exit
CLASSPATH=$APP_HOME/gradle/wrapper/gradle-wrapper.jar
exec "$JAVACMD" "$@"
"#;

#[test]
fn gradlew_s_wrapper_jar_is_followed() {
    let dir = scratch_repo("hook-shell-gradlew");
    std::fs::create_dir_all(dir.join("gradle/wrapper")).unwrap();
    write(&dir, "gradlew", GRADLEW);
    std::fs::write(dir.join("gradle/wrapper/gradle-wrapper.jar"), b"PK\x03\x04\0\0").unwrap();
    hook_with(&dir.join(".git/hooks"), "pre-commit", "#!/bin/sh\n./gradlew check\n");
    assert_eq!(paths(&scan_of(&dir)), ["gradle/wrapper/gradle-wrapper.jar", "gradlew"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_quoted_tilde_is_a_folder_here_and_a_bare_one_a_home_folder() {
    let dir = scratch_repo("hook-shell-tilde");
    std::fs::create_dir_all(dir.join("~/tools")).unwrap();
    write(&dir, "~/tools/x.sh", "exit 0\n");
    write(&dir, "~/tools/y.sh", "exit 0\n");
    hook_with(&dir.join(".git/hooks"), "pre-commit", "#!/bin/sh\nsh \"~/tools/x.sh\"\nsh ~/tools/y.sh\n");
    let s = scan_of(&dir);
    assert_eq!(paths(&s), ["~/tools/x.sh"], "{:?}", s.unread);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn git_as_a_folding_file_system_names_it_is_never_in_the_tree() {
    let top = Path::new("/w");
    for name in [".git", ".GIT", ".git.", ".git ", ".git. ."] {
        assert!(!in_tree(top, &top.join(name).join("hooks/x")), "{name:?}");
    }
    for name in [".github", ".gitx", "git"] {
        assert!(in_tree(top, &top.join(name).join("x")), "{name:?}");
    }
}

#[test]
fn a_quoted_tilde_before_a_glob_or_in_a_bare_word_is_a_name_here() {
    let dir = scratch_repo("hook-shell-tilde-glob");
    std::fs::create_dir_all(dir.join("~/tools")).unwrap();
    write(&dir, "~/tools/a.sh", "exit 0\n");
    write(&dir, "~x.sh", "exit 0\n");
    hook_with(&dir.join(".git/hooks"), "pre-commit", "#!/bin/sh\nfor f in \"~/tools/\"*.sh; do sh \"$f\"; done\nsh \"~x.sh\"\n");
    let s = scan_of(&dir);
    assert_eq!(paths(&s), ["~/tools", "~x.sh"], "{:?}", s.unread);
    let _ = std::fs::remove_dir_all(&dir);
}
