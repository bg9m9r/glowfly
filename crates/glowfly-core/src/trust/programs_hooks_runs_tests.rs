//! Bare words a hook runs: the script after an interpreter, `.`, `source` or `exec` is
//! followed when it is a regular file in the working tree, from its top or from the hook's
//! folder there; `npx`, `npm` and `git` words follow nothing.

use super::super::tests::{hook_with, paths, scan_of};
use super::*;
use crate::testutil::*;

#[test]
fn the_word_an_interpreter_or_source_runs_is_picked() {
    let cases: &[(&str, &[&str])] = &[
        ("sh check.sh", &["check.sh"]),
        ("bash -e run.sh \"$@\"", &["run.sh"]),
        ("/usr/bin/env python3 tool.py --fix", &["tool.py"]),
        ("node lint.js && zsh fmt.zsh", &["lint.js", "fmt.zsh"]),
        ("pwsh -NoProfile -ExecutionPolicy RemoteSigned -File check.ps1", &["check.ps1"]),
        ("powershell.exe -f check.ps1", &["check.ps1"]),
        ("deno run main.ts", &["main.ts"]),
        (". lib.sh", &["lib.sh"]),
        ("source lib.sh", &["lib.sh"]),
        ("exec hook.sh", &["hook.sh"]),
        ("sh -c 'echo hi'", &[]),
        ("bash -ec 'echo hi'", &[]),
        ("node -e 'process.exit(0)'", &[]),
        ("python3 -m black .", &[]),
        ("npx lint-staged", &[]),
        ("npm test", &[]),
        ("git diff --quiet", &[]),
    ];
    for (line, want) in cases {
        assert_eq!(run_words(line), *want, "{line}");
    }
}

#[test]
fn a_bare_script_a_hook_runs_is_followed_from_the_top_and_from_its_folder() {
    let dir = scratch_repo("hook-runs-bare");
    for d in ["hooks", "sub"] {
        std::fs::create_dir_all(dir.join(d)).unwrap();
    }
    for f in ["check.sh", "tool.py", "lint-staged", "test", "diff"] {
        write(&dir, f, "exit 0\n");
    }
    // `lib.sh` is beside the hook only; what it runs is read in turn.
    write(&dir, "hooks/lib.sh", "sh more.sh\n");
    write(&dir, "more.sh", "exit 0\n");
    let text = "#!/bin/sh\nsh check.sh\n. lib.sh\npython3 tool.py\nnpx lint-staged\nnpm test\ngit diff\nsh missing.sh\nsh sub\n";
    hook_with(&dir.join("hooks"), "pre-commit", text);
    git(&dir, &["config", "core.hooksPath", "hooks"]);
    assert_eq!(paths(&scan_of(&dir)), ["check.sh", "hooks", "more.sh", "tool.py"]);
    // From `.git/hooks`, only the working tree's top counts.
    git(&dir, &["config", "--unset", "core.hooksPath"]);
    hook_with(&dir.join(".git/hooks"), "pre-commit", "#!/bin/sh\n. lib.sh\nsh check.sh\n");
    write(&dir, ".git/hooks/lib.sh", "sh tool.py\n");
    assert_eq!(paths(&scan_of(&dir)), ["check.sh"]);
    let _ = std::fs::remove_dir_all(&dir);
}
