use super::super::fixture::{conflicted, git};
use super::*;
use trellis_core::external_tools::git_config::GitTool;
use trellis_core::repo::DiffTarget;

/// Whether sh is available here; says so when it isn't, so the caller skips.
fn have_sh() -> bool {
    match trellis_core::custom_commands::find_sh() {
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
    match run_merge(GitMergeRequest { tool, workdir: dir.into(), path: "a.txt".into(), filters: true }) {
        Msg::Tools(ToolsMsg::MergeToolExited { path, end, .. }) if path == "a.txt" => end,
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
    let e = merge(&dir, builtin("trellis-no-such-tool"));
    match &e {
        MergeEnd::Exited { code: Some(c), said, changed: false } => {
            assert!(*c != 0 && said.contains("trellis-no-such-tool"), "{e:?}");
        }
        other => panic!("expected an exit with an error, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
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
    let (program, _, f) = failure(open(builtin("trellis-no-such-tool")));
    assert_eq!(program, "trellis-no-such-tool");
    assert!(matches!(&f, LaunchFailure::Other(t) if t.contains("difftool.trellis-no-such-tool.cmd not set")), "{f:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
