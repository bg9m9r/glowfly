use super::super::fixture::{conflicted, conflicted_files, git};
use super::*;
use glowfly_core::external_tools::git_config::GitTool;
use glowfly_core::repo::DiffTarget;

/// Whether sh is available here; says so when it isn't, so the caller skips.
fn have_sh() -> bool {
    match glowfly_core::custom_commands::find_sh() {
        Ok(_) => true,
        Err(e) => {
            eprintln!("skipping: {e}");
            false
        }
    }
}

fn command(cmd: &str, trust_exit_code: bool) -> GitTool {
    let run = GitToolRun::Command { key: "mergetool.t.cmd".into(), cmd: cmd.into(), trust_exit_code, from_repo: false };
    GitTool { name: "t".into(), chosen_by: "merge.tool".into(), run }
}

fn builtin(name: &str) -> GitTool {
    GitTool { name: name.into(), chosen_by: "merge.tool".into(), run: GitToolRun::BuiltIn { path: None } }
}

fn merge(dir: &Path, tool: GitTool) -> MergeEnd {
    merge_at(dir, "a.txt", tool)
}

fn merge_at(dir: &Path, file: &str, tool: GitTool) -> MergeEnd {
    match run_merge(GitMergeRequest { tool, workdir: dir.into(), path: file.into(), filters: true }) {
        Msg::Tools(ToolsMsg::MergeToolExited { path, end, .. }) if path == file => end,
        _ => panic!("expected MergeToolExited"),
    }
}

#[test]
fn a_configured_merge_command_runs_with_gits_variables() {
    if !have_sh() {
        return;
    }
    let dir = conflicted("git-merge-command");
    assert_eq!(merge(&dir, command("cp \"$REMOTE\" \"$MERGED\"", false)), MergeEnd::Saved);
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "theirs\n");
    let e = merge(&dir, command("exit 3", false));
    assert_eq!(e, MergeEnd::Exited { code: Some(3), said: String::new(), changed: false });
    assert_eq!(merge(&dir, command("exit 0", true)), MergeEnd::Saved, "a trusted exit code counts even unchanged");
    // The ancestor and ours are copies too.
    assert_eq!(merge(&dir, command("test \"$(cat \"$BASE\")\" = one && cp \"$LOCAL\" \"$MERGED\"", false)), MergeEnd::Saved);
    assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "ours\n");
    assert_eq!(merge(&dir, command("git add -- \"$MERGED\"", false)), MergeEnd::Resolved);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_builtin_merge_tool_runs_through_git_mergetool() {
    let dir = conflicted("git-mergetool");
    let e = merge(&dir, builtin("glowfly-no-such-tool"));
    match &e {
        MergeEnd::Exited { code: Some(c), said, changed: false } => {
            assert!(*c != 0 && said.contains("glowfly-no-such-tool"), "{e:?}");
        }
        other => panic!("expected an exit with an error, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn git_mergetool_never_reads_the_file_name_as_a_pattern() {
    // `git mergetool` runs a tool git config names: here a command git itself runs, so this
    // needs no sh of Glowfly's own.
    let dir = conflicted_files("git-mergetool-pattern", &["a.txt", "[ab].txt", "b c.txt"]);
    git(&dir, &["config", "mergetool.t.cmd", "cp \"$REMOTE\" \"$MERGED\""]);
    git(&dir, &["config", "mergetool.keepBackup", "false"]);
    // Trust cp's exit code: otherwise mergetool asks whether the merge worked when the file isn't
    // newer than its backup, which a copy within the same timestamp tick isn't (Linux).
    git(&dir, &["config", "mergetool.t.trustExitCode", "true"]);
    // `[ab].txt` would merge and stage a.txt: refused, and neither file is touched.
    let e = merge_at(&dir, "[ab].txt", builtin("t"));
    assert!(matches!(&e, MergeEnd::Failed(LaunchFailure::Other(t)) if t.contains("[ab].txt")), "{e:?}");
    for f in ["a.txt", "[ab].txt"] {
        assert_eq!(repo::is_conflicted(&dir, f).ok(), Some(true), "{f}");
    }
    assert!(std::fs::read_to_string(dir.join("a.txt")).unwrap().contains("<<<<<<<"));
    // A name with a space goes through, as one literal path.
    assert_eq!(merge_at(&dir, "b c.txt", builtin("t")), MergeEnd::Resolved);
    assert_eq!(
        std::fs::read_to_string(dir.join("b c.txt")).unwrap(),
        "theirs
"
    );
    assert_eq!(repo::is_conflicted(&dir, "a.txt").ok(), Some(true));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn mergetool_refuses_only_names_its_shell_would_expand() {
    for ok in ["a.txt", "dir/b c.txt", "x(1).txt", ":c.txt", "{a,b}.txt"] {
        assert!(mergetool_can_open(ok), "{ok}");
    }
    for refused in ["[ab].txt", "a*.txt", "a?.txt", "dir/x[1]", r"a\b"] {
        assert!(!mergetool_can_open(refused), "{refused}");
    }
}

#[test]
fn git_diff_tools_report_only_failures() {
    let dir = conflicted("git-diff-tool");
    git(&dir, &["checkout", "-q", "--theirs", "--", "a.txt"]);
    git(&dir, &["add", "a.txt"]);
    std::fs::write(dir.join("a.txt"), "edited\n").unwrap();
    let open = |tool: GitTool| {
        open_diff(GitDiffRequest { tool, workdir: dir.clone(), target: DiffTarget::Unstaged("a.txt".into()), filters: false })
    };
    let failure = |m: Option<Msg>| match m {
        Some(Msg::Tools(ToolsMsg::LaunchFailed { kind: ToolKind::DiffTool, program, target, failure })) => (program, target, failure),
        Some(Msg::Tools(other)) => panic!("expected a diff tool failure, got {other:?}"),
        _ => panic!("expected a diff tool failure, got nothing"),
    };
    if have_sh() {
        assert!(open(command("test -f \"$LOCAL\" && test \"$MERGED\" = a.txt && test \"$BASE\" = a.txt", false)).is_none());
        let (program, target, f) = failure(open(command("exit 5", false)));
        assert_eq!((program.as_str(), target.as_str()), ("t", "a.txt"));
        assert!(matches!(&f, LaunchFailure::Other(t) if t.contains("code 5")), "{f:?}");
    }
    let (program, _, f) = failure(open(builtin("glowfly-no-such-tool")));
    assert_eq!(program, "glowfly-no-such-tool");
    assert!(matches!(&f, LaunchFailure::Other(t) if t.contains("difftool.glowfly-no-such-tool.cmd not set")), "{f:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_git_difftool_failure_after_the_watch_is_reported_when_it_comes() {
    let dir = conflicted("git-diff-tool-late");
    git(&dir, &["checkout", "-q", "--theirs", "--", "a.txt"]);
    git(&dir, &["add", "a.txt"]);
    std::fs::write(
        dir.join("a.txt"),
        "edited
",
    )
    .unwrap();
    let req = GitDiffRequest {
        tool: builtin("glowfly-no-such-tool"),
        workdir: dir.clone(),
        target: DiffTarget::Unstaged("a.txt".into()),
        filters: false,
    };
    // No watch at all: git's failure comes after it, and is still reported, with git's line.
    match open_diff_watching(req, Duration::ZERO) {
        Some(Msg::Tools(ToolsMsg::DiffToolFailedLater { program, target, code: Some(c), said })) => {
            assert_eq!((program.as_str(), target.as_str()), ("glowfly-no-such-tool", "a.txt"));
            assert!(c != 0 && said.contains("glowfly-no-such-tool"), "{c}: {said}");
        }
        Some(Msg::Tools(other)) => panic!("expected a late diff tool failure, got {other:?}"),
        _ => panic!("expected a late diff tool failure"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}
