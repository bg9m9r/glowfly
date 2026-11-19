//! Files a hook runs that aren't shell scripts (JavaScript, TypeScript, Python, programs and
//! installed packages) are followed by their bytes, never read as shell words: their comments,
//! globs and regular expressions follow nothing more, and never the whole working tree. A link
//! is followed by the file it leads to, as a plain file would be.

use super::tests::{hook_with, paths, scan_of};
use super::*;
use crate::testutil::*;

/// A repository with `files` written (folders made) and the pre-commit hook `hook`.
fn repo_with(name: &str, files: &[(&str, &[u8])], hook: &str) -> PathBuf {
    let dir = scratch_repo(name);
    for (path, bytes) in files {
        let p = dir.join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    }
    hook_with(&dir.join(".git/hooks"), "pre-commit", hook);
    dir
}

const JS: &[u8] = b"/**\n * Checks the message.\n */\nconst re = /^(feat|fix)(\\(.+\\))?: .{1,50}/;\n// see ./docs/*.md\n";

#[test]
fn simple_git_hooks_node_script_is_followed_without_being_read() {
    let hook = "#!/bin/sh\n\nif [ \"$SKIP_SIMPLE_GIT_HOOKS\" = \"1\" ]; then\n    echo \"[INFO] skipping hook.\"\n    exit 0\nfi\n\nif [ -f \"$SIMPLE_GIT_HOOKS_RC\" ]; then\n    . \"$SIMPLE_GIT_HOOKS_RC\"\nfi\n\nnode scripts/verify-commit.js\n";
    let dir = repo_with("hook-noise-js", &[("scripts/verify-commit.js", JS)], hook);
    assert_eq!(paths(&scan_of(&dir)), ["scripts/verify-commit.js"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_python_script_or_an_eslint_argument_is_followed_without_being_read() {
    let py: &[u8] =
        b"\"\"\"Checks /* every */ file.\"\"\"\nimport glob, re\nfiles = glob.glob(\"**/*.py\")\nshort = re.compile(r\".{1,50}/\")\n";
    let program: &[u8] = b"MZ\x90\0\x03\0./other.sh";
    let files: &[(&str, &[u8])] = &[("tools/check.py", py), ("src/app.ts", JS), ("bin/tool", program)];
    let hook = "#!/bin/sh\npython3 tools/check.py\nnpx eslint src/app.ts\n./bin/tool --all\n";
    let dir = repo_with("hook-noise-py", files, hook);
    // A program's NUL bytes are its own: it is followed by its bytes.
    assert_eq!(paths(&scan_of(&dir)), ["bin/tool", "src/app.ts", "tools/check.py"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_hook_s_words_from_a_home_folder_name_nothing() {
    let hook =
        "#!/bin/sh\n. \"$HOME/.hookrc\"\n[ -f \"${HOME}/x.sh\" ] && sh ~/y.sh\ni=\"${XDG_CONFIG_HOME:-$HOME/.config}/tool/init.sh\"\n";
    let dir = repo_with("hook-noise-home", &[], hook);
    let s = scan_of(&dir);
    assert!(s.follows.is_empty(), "{:?}", s.follows);
    assert!(s.unread.is_empty(), "{:?}", s.unread);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn lefthook_s_installed_program_is_followed_by_its_bytes() {
    let program: &[u8] = b"\x7fELF\x02\x01\x01\0\0\0\0./bin/sh -c x";
    let files: &[(&str, &[u8])] = &[
        ("node_modules/@evilmartians/lefthook-installer/bin/lefthook", program),
        ("node_modules/lefthook/bin/index.js", b"#!/bin/sh\nsh ./elsewhere.sh\n"),
    ];
    let dir = repo_with("hook-noise-lefthook", files, super::tests::LEFTHOOK);
    let s = scan_of(&dir);
    assert!(!paths(&s).contains(&""), "{:?}", s.follows);
    for p in ["node_modules/@evilmartians/lefthook-installer/bin/lefthook", "node_modules/lefthook/bin/index.js"] {
        assert!(paths(&s).contains(&p), "{p}: {:?}", s.follows);
    }
    assert!(!paths(&s).contains(&"elsewhere.sh"), "never read: {:?}", s.follows);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_package_s_bin_entry_is_followed_the_same_as_a_link_or_a_file() {
    let files: &[(&str, &[u8])] = &[("node_modules/lint/cli.js", b"#!/usr/bin/env node\n")];
    let dir = repo_with("hook-noise-bin", files, "#!/bin/sh\n./node_modules/.bin/lint --fix\n");
    let bin = dir.join("node_modules/.bin/lint");
    std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("../lint/cli.js", &bin).unwrap();
    #[cfg(not(unix))]
    std::fs::write(&bin, "#!/usr/bin/env node\n").unwrap();
    assert_eq!(paths(&scan_of(&dir)), ["node_modules/.bin/lint"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn a_link_out_of_the_working_tree_is_named_unread_and_a_link_to_a_script_follows_all() {
    let outside = scratch_repo("hook-noise-link-outside");
    std::fs::write(outside.join("tool.js"), "x\n").unwrap();
    let dir = repo_with("hook-noise-link", &[("scripts/real.sh", b"exit 0\n")], "#!/bin/sh\nnode ./tools/out.js\n");
    std::fs::create_dir_all(dir.join("tools")).unwrap();
    std::os::unix::fs::symlink(outside.join("tool.js"), dir.join("tools/out.js")).unwrap();
    let s = scan_of(&dir);
    assert!(s.follows.is_empty(), "{:?}", s.follows);
    assert!(s.unread.iter().any(|u| u.ends_with("out.js")), "{:?}", s.unread);
    // A link to a shell script it would read: what that runs may change through the link.
    std::os::unix::fs::symlink("real.sh", dir.join("scripts/check.sh")).unwrap();
    hook_with(&dir.join(".git/hooks"), "pre-commit", "#!/bin/sh\nsh ./scripts/check.sh\n");
    assert!(paths(&scan_of(&dir)).contains(&""));
    for d in [dir, outside] {
        let _ = std::fs::remove_dir_all(&d);
    }
}
