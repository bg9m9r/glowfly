//! Scripts that the scripts a hook runs run in turn: husky 9's hooks folder `.husky/_` runs the
//! tracked `.husky/<hook>` beside it, whose scripts are followed, so changing any of them asks
//! again; past `MAX_DEPTH` scripts down, or for a script with a NUL byte, the whole working tree
//! is followed.

use super::super::super::allows;
use super::tests::{hook_with, paths, scan_of};
use super::*;
use crate::settings::SavedTrust;
use crate::testutil::*;

/// husky 9's `.husky/_/h`, which each of its hooks sources.
const HUSKY_H: &str = r#"#!/usr/bin/env sh
[ "$HUSKY" = "2" ] && set -x
n=$(basename "$0")
s=$(dirname "$(dirname "$0")")/$n

[ ! -f "$s" ] && exit 0

if [ -f "$HOME/.huskyrc" ]; then
	echo "husky - '~/.huskyrc' is DEPRECATED, please move your code to ~/.config/husky/init.sh"
fi
i="${XDG_CONFIG_HOME:-$HOME/.config}/husky/init.sh"
[ -f "$i" ] && . "$i"

[ "${HUSKY-}" = "0" ] && exit 0

export PATH="node_modules/.bin:$PATH"
sh -e "$s" "$@"
c=$?

[ $c != 0 ] && echo "husky - $n script failed (code $c)"
[ $c = 127 ] && echo "husky - command not found in PATH=$PATH"
exit $c
"#;

/// husky 9's hook in `.husky/_`, the same for each hook name.
const HUSKY_HOOK: &str = "#!/usr/bin/env sh\n. \"$(dirname \"$0\")/h\"";

/// husky 9.0's `.husky/_/h` and hook, which spell their folders with `${0%/…}`.
const HUSKY_9_0_H: &str = r#"#!/usr/bin/env sh
[ "$HUSKY" = "2" ] && set -x
h="${0##*/}"
s="${0%/*/*}/$h"

[ ! -f "$s" ] && exit 0

for f in "${XDG_CONFIG_HOME:-$HOME/.config}/husky/init.sh" "$HOME/.huskyrc.sh"; do
	# shellcheck disable=SC1090
	[ -f "$f" ] && . "$f"
done

[ "${HUSKY-}" = "0" ] && exit 0

sh -e "$s" "$@"
c=$?

[ $c != 0 ] && echo "husky - $h script failed (code $c)"
[ $c = 127 ] && echo "husky - command not found in PATH=$PATH"
exit $c
"#;
const HUSKY_9_0_HOOK: &str = "#!/usr/bin/env sh\n. \"${0%/*}/h\"";

/// A repository set up as husky 9 sets one up, whose tracked `.husky/pre-commit` runs
/// `scripts/check.sh`, which sources `scripts/lib.sh`.
fn husky_repo(name: &str) -> PathBuf {
    husky_repo_with(name, HUSKY_HOOK, HUSKY_H)
}

fn husky_repo_with(name: &str, hook: &str, h: &str) -> PathBuf {
    let dir = scratch_repo(name);
    for d in ["scripts", ".husky"] {
        std::fs::create_dir_all(dir.join(d)).unwrap();
    }
    write(&dir, ".husky/pre-commit", "npm test\nsh scripts/check.sh\n");
    write(&dir, "scripts/check.sh", "#!/bin/sh\nset -e\n. \"$(dirname \"$0\")/lib.sh\"\ncheck_all\n");
    write(&dir, "scripts/lib.sh", "check_all() { exit 0; }\n");
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "husky"]);
    let hooks = dir.join(".husky/_");
    for name in ["pre-commit", "pre-push"] {
        hook_with(&hooks, name, hook);
    }
    write(&dir, ".husky/_/h", h);
    write(&dir, ".husky/_/.gitignore", "*\n");
    git(&dir, &["config", "core.hooksPath", ".husky/_"]);
    dir
}

#[test]
fn husky_9_s_hooks_follow_the_tracked_scripts_their_hook_files_run() {
    let dir = husky_repo("hook-chain-husky");
    let s = scan_of(&dir);
    for p in [".husky/_", ".husky/pre-commit", ".husky/pre-push", "scripts/check.sh", "scripts/lib.sh"] {
        assert!(paths(&s).contains(&p), "{p}: {:?}", s.follows);
    }
    assert!(!paths(&s).contains(&""), "{:?}", s.follows);
    // A commit that changes any script down the chain asks again.
    for (file, text) in [("scripts/lib.sh", "check_all() { echo two; }\n"), (".husky/pre-commit", "sh scripts/check.sh\n")] {
        let saved = SavedTrust::trusted(Some(scan_of(&dir).print()));
        write(&dir, file, text);
        git(&dir, &["-c", "core.hooksPath=", "commit", "-q", "-am", file]);
        assert!(!allows(Some(&saved), &scan_of(&dir)), "{file}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn husky_9_0_s_spelling_of_its_folders_is_read_too() {
    let dir = husky_repo_with("hook-chain-husky-9-0", HUSKY_9_0_HOOK, HUSKY_9_0_H);
    let s = scan_of(&dir);
    assert!(!paths(&s).contains(&""), "{:?}", s.follows);
    for p in ["scripts/check.sh", "scripts/lib.sh"] {
        assert!(paths(&s).contains(&p), "{p}: {:?}", s.follows);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn only_a_hooks_folder_called_underscore_runs_the_files_beside_it() {
    let dir = scratch_repo("hook-chain-not-underscore");
    std::fs::create_dir_all(dir.join("tools")).unwrap();
    write(&dir, "tools/pre-commit", "#!/bin/sh\n");
    hook_with(&dir.join("tools/hooks"), "pre-commit", "#!/bin/sh\nexit 0\n");
    git(&dir, &["config", "core.hooksPath", "tools/hooks"]);
    assert_eq!(paths(&scan_of(&dir)), ["tools/hooks"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn scripts_are_read_up_to_three_down_and_past_that_the_whole_working_tree_follows() {
    let dir = scratch_repo("hook-chain-depth");
    std::fs::create_dir_all(dir.join("s")).unwrap();
    hook_with(&dir.join(".git/hooks"), "pre-commit", "#!/bin/sh\nsh ./s/1.sh\n");
    write(&dir, "s/1.sh", "sh ./s/2.sh\n");
    write(&dir, "s/2.sh", "sh ./s/3.sh\n");
    write(&dir, "s/3.sh", "./s/missing.sh\n");
    assert_eq!(paths(&scan_of(&dir)), ["s/1.sh", "s/2.sh", "s/3.sh", "s/missing.sh"]);
    // A fourth script down that is there to read is past the limit.
    write(&dir, "s/missing.sh", "exit 0\n");
    assert!(paths(&scan_of(&dir)).contains(&""), "past the depth");
    // A script with a NUL byte is a compiled program: its words can't be read.
    write(&dir, "s/2.sh", "\x7fELF\0\0./s/3.sh");
    assert!(paths(&scan_of(&dir)).contains(&""), "a NUL byte");
    let _ = std::fs::remove_dir_all(&dir);
}
