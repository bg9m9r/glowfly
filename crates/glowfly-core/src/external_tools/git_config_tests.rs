use super::*;
use crate::testutil::{scratch_repo, set_config};

fn e(k: &str, v: Option<&str>) -> Entry {
    (k.to_owned(), v.map(str::to_owned))
}

fn set(k: &str, v: &str) -> Entry {
    e(k, Some(v))
}

fn yes(_: &str) -> bool {
    true
}

fn no(_: &str) -> bool {
    false
}

fn run_of(all: &[Entry], mode: ToolMode, builtin: &dyn Fn(&str) -> bool) -> Option<(String, String, GitToolRun)> {
    resolve(all, &[], mode, builtin).map(|t| (t.name, t.chosen_by, t.run))
}

fn command(key: &str, cmd: &str) -> GitToolRun {
    GitToolRun::Command { key: key.into(), cmd: cmd.into(), trust_exit_code: false, from_repo: false }
}

#[test]
fn parse_z_reads_values_newlines_and_valueless_keys() {
    let out = "difftool.My.Tool.cmd\na \"$LOCAL\"\nb\0difftool.My.Tool.trustexitcode\0diff.tool\nMy.Tool\0";
    assert_eq!(
        parse_z(out),
        [set("difftool.My.Tool.cmd", "a \"$LOCAL\"\nb"), e("difftool.My.Tool.trustexitcode", None), set("diff.tool", "My.Tool")]
    );
    assert!(parse_z("").is_empty());
}

#[test]
fn diff_falls_back_to_merge_tool_like_git() {
    let only_merge = [set("merge.tool", "m"), set("mergetool.m.cmd", "m-cmd")];
    let (name, by, run) = run_of(&only_merge, ToolMode::Diff, &no).unwrap();
    assert_eq!((name.as_str(), by.as_str(), run), ("m", "merge.tool", command("mergetool.m.cmd", "m-cmd")));
    // An empty or valueless last diff.tool is unset.
    for last in [Some(""), None] {
        let all = [set("diff.tool", "d"), e("diff.tool", last), set("merge.tool", "m")];
        assert_eq!(run_of(&all, ToolMode::Diff, &yes).map(|t| t.1).as_deref(), Some("merge.tool"), "{last:?}");
    }
    // The last value counts.
    let all = [set("diff.tool", "a"), set("diff.tool", "b")];
    assert_eq!(run_of(&all, ToolMode::Diff, &yes).map(|t| t.0).as_deref(), Some("b"));
    // Merge mode never reads diff.tool.
    assert_eq!(run_of(&[set("diff.tool", "d")], ToolMode::Merge, &yes), None);
    assert_eq!(run_of(&[], ToolMode::Diff, &yes), None);
    // difftool.x.cmd, else mergetool.x.cmd, but merge mode reads only mergetool.
    let all = [set("diff.tool", "x"), set("merge.tool", "x"), set("mergetool.x.cmd", "mcmd")];
    assert_eq!(run_of(&all, ToolMode::Diff, &no).unwrap().2, command("mergetool.x.cmd", "mcmd"));
    let all = [set("merge.tool", "x"), set("difftool.x.cmd", "dcmd")];
    assert_eq!(run_of(&all, ToolMode::Merge, &no).unwrap().2, GitToolRun::Unknown);
    // An empty difftool.x.cmd is set: no fallback to mergetool.x.cmd.
    let all = [set("diff.tool", "x"), set("difftool.x.cmd", ""), set("mergetool.x.cmd", "mcmd")];
    assert_eq!(run_of(&all, ToolMode::Diff, &yes).unwrap().2, GitToolRun::BuiltIn { path: None });
    assert_eq!(run_of(&all, ToolMode::Diff, &no).unwrap().2, GitToolRun::Unknown);
    // A command overrides a built-in tool.
    let all = [set("diff.tool", "meld"), set("difftool.meld.cmd", "my-meld")];
    assert_eq!(run_of(&all, ToolMode::Diff, &yes).unwrap().2, command("difftool.meld.cmd", "my-meld"));
    // A built-in tool's path, with the same fallback.
    let all = [set("diff.tool", "meld"), set("mergetool.meld.path", "/opt/meld")];
    assert_eq!(run_of(&all, ToolMode::Diff, &yes).unwrap().2, GitToolRun::BuiltIn { path: Some("/opt/meld".into()) });
    assert_eq!(run_of(&[set("diff.tool", "meld")], ToolMode::Diff, &no).unwrap().2, GitToolRun::Unknown);
    let t = resolve(&all, &[], ToolMode::Diff, &yes).unwrap();
    assert_eq!(t.cmd_keys(ToolMode::Diff), ["difftool.meld.cmd", "mergetool.meld.cmd"]);
    assert_eq!(t.cmd_keys(ToolMode::Merge), ["mergetool.meld.cmd"]);
    assert!(t.needs_trust(), "a built-in tool runs git in the repository");
}

#[test]
fn trust_exit_code_reads_git_booleans() {
    let trust = |v: Option<Option<&str>>, mode: ToolMode| {
        let mut all = vec![set("merge.tool", "m"), set("mergetool.m.cmd", "c")];
        if let Some(v) = v {
            all.push(e("mergetool.m.trustexitcode", v));
        }
        match run_of(&all, mode, &no).unwrap().2 {
            GitToolRun::Command { trust_exit_code, .. } => trust_exit_code,
            other => panic!("{other:?}"),
        }
    };
    for v in [None, Some("yes"), Some("On"), Some("1"), Some("true")] {
        assert!(trust(Some(v), ToolMode::Merge), "{v:?}");
        assert!(!trust(Some(v), ToolMode::Diff), "diff: {v:?}");
    }
    for v in ["0", "off", "", "maybe", "false"] {
        assert!(!trust(Some(Some(v)), ToolMode::Merge), "{v:?}");
    }
    assert!(!trust(None, ToolMode::Merge), "unset");
}

#[test]
fn repository_values_are_marked() {
    let from_repo = |all: &[Entry], own: &[Entry]| match resolve(all, own, ToolMode::Diff, &no).unwrap().run {
        GitToolRun::Command { from_repo, .. } => from_repo,
        other => panic!("{other:?}"),
    };
    let own = [set("difftool.x.cmd", "repo-cmd")];
    let all = [set("diff.tool", "x"), set("difftool.x.cmd", "user-cmd"), set("difftool.x.cmd", "repo-cmd")];
    assert!(from_repo(&all, &own));
    // Overridden later (git -c): not the repository's.
    let later = [all.as_slice(), &[set("difftool.x.cmd", "cli-cmd")]].concat();
    assert!(!from_repo(&later, &own));
    assert!(!from_repo(&all[..2], &[]));
    // A repository diff.tool choosing the user's command is harmless.
    let t = resolve(&all[..2], &[set("diff.tool", "x")], ToolMode::Diff, &no).unwrap();
    assert!(!t.needs_trust());
}

#[test]
fn builtin_names_follow_gits_tool_folder() {
    let dir = crate::paths::temp_dir().join(format!("trellis-mergetools-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("a")).unwrap();
    for f in ["meld", "vimdiff", "a/meld"] {
        std::fs::write(dir.join(f), "").unwrap();
    }
    assert!(builtin_in(&dir, "meld"));
    assert!(builtin_in(&dir, "vimdiff3"));
    for name in ["vimdiff33", "nosuch", "", "../meld", "a/meld", "a\\meld", ".", "a"] {
        assert!(!builtin_in(&dir, name), "{name}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_honours_includes_and_scopes() {
    let dir = scratch_repo("git-config-tools");
    let env = crate::testutil::empty_global_config(&dir);
    let global = PathBuf::from(&env[0].1);
    std::fs::write(&global, "[diff]\n\ttool = g\n[difftool \"g\"]\n\tcmd = g-cmd\n").unwrap();
    let inc = dir.with_file_name(format!("{}-include", dir.file_name().unwrap().to_string_lossy()));
    std::fs::write(&inc, "[merge]\n\ttool = m\n[mergetool \"m\"]\n\tcmd = m-cmd \\\"$MERGED\\\"\n\ttrustExitCode = true\n").unwrap();
    set_config(&dir, &[("include.path", &inc.display().to_string().replace('\\', "/"))]);
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();

    let tools = read(&dir, true, &env).unwrap();
    let diff = tools.diff.expect("a diff tool");
    assert_eq!((diff.name.as_str(), diff.chosen_by.as_str()), ("g", "diff.tool"));
    assert_eq!(diff.run, command("difftool.g.cmd", "g-cmd"));
    let merge = tools.merge.expect("a merge tool");
    assert_eq!(merge.name, "m");
    let want =
        GitToolRun::Command { key: "mergetool.m.cmd".into(), cmd: "m-cmd \"$MERGED\"".into(), trust_exit_code: true, from_repo: true };
    assert_eq!(merge.run, want);

    set_config(&dir, &[("difftool.g.cmd", "local-cmd")]);
    let diff = read(&dir, true, &env).unwrap().diff.unwrap();
    assert!(matches!(diff.run, GitToolRun::Command { from_repo: true, ref cmd, .. } if cmd == "local-cmd"), "{diff:?}");

    // Outside any repository, only the user's own values.
    let outside = crate::paths::temp_dir();
    let tools = read(&outside, false, &env).unwrap();
    assert_eq!(tools.diff.map(|t| t.run), Some(command("difftool.g.cmd", "g-cmd")));
    assert_eq!(tools.merge, None);

    // A name with no command and no file in git's tool folder can't be used.
    let exec = gitcli::read(&outside, &gitcli::args(&["--exec-path"])).stdout.trim().to_owned();
    if PathBuf::from(&exec).join("mergetools").is_dir() {
        // libgit2 won't set a key an included file sets; git does.
        crate::testutil::git(&dir, &["config", "merge.tool", "trellis-no-such-tool"]);
        let merge = read(&dir, true, &env).unwrap().merge.unwrap();
        assert_eq!(merge.run, GitToolRun::Unknown);
    } else {
        eprintln!("git's mergetools folder wasn't found under {exec:?}; skipping the unknown-tool check");
    }
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&global);
    let _ = std::fs::remove_file(&inc);
}
