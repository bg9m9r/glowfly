use super::*;

fn tool(name: &str, chosen_by: &str, run: GitToolRun) -> GitTool {
    GitTool { name: name.into(), chosen_by: chosen_by.into(), run }
}

fn command(cmd: &str, from_repo: bool) -> GitToolRun {
    GitToolRun::Command { key: "difftool.meld.cmd".into(), cmd: cmd.into(), trust_exit_code: false, from_repo }
}

fn report(code: Option<i32>, said: &str) -> ExitReport {
    ExitReport { code, said: said.into() }
}

#[test]
fn command_ends_follow_trust_exit_code() {
    let exited = |code, changed| MergeEnd::Exited { code, said: "x".into(), changed };
    // Trusted exit code: 0 is a success even when unchanged.
    assert_eq!(command_end(&report(Some(0), ""), false, true), MergeEnd::Saved);
    assert_eq!(command_end(&report(Some(0), ""), true, true), MergeEnd::Saved);
    assert_eq!(command_end(&report(Some(1), "x"), true, true), exited(Some(1), true));
    assert_eq!(command_end(&report(None, "x"), false, true), exited(None, false));
    // Otherwise the file decides.
    assert_eq!(command_end(&report(Some(3), "x"), true, false), MergeEnd::Saved);
    assert_eq!(command_end(&report(Some(0), ""), true, false), MergeEnd::Saved);
    assert_eq!(command_end(&report(Some(0), ""), false, false), MergeEnd::Unchanged);
    assert_eq!(command_end(&report(Some(3), "x"), false, false), exited(Some(3), false));
}

#[test]
fn git_mergetool_ends() {
    assert_eq!(git_merge_end(Some(0), String::new(), true, Some(false)), MergeEnd::Resolved);
    assert_eq!(git_merge_end(Some(0), String::new(), true, Some(true)), MergeEnd::Saved);
    assert_eq!(git_merge_end(Some(0), String::new(), true, None), MergeEnd::Saved);
    assert_eq!(git_merge_end(Some(0), String::new(), false, Some(true)), MergeEnd::Unchanged);
    let said = "merge of a.txt failed".to_owned();
    assert_eq!(
        git_merge_end(Some(1), said.clone(), false, Some(false)),
        MergeEnd::Exited { code: Some(1), said: said.clone(), changed: false }
    );
    assert_eq!(git_merge_end(None, said.clone(), true, Some(true)), MergeEnd::Exited { code: None, said, changed: true });
}

#[test]
fn git_said_prefers_errors() {
    let stderr = "warning: in the working copy of 'a', LF will be replaced by CRLF\nerror: difftool.x.cmd not set for tool 'x'\nfatal: external diff died\n";
    assert_eq!(git_said(stderr, "out"), "error: difftool.x.cmd not set for tool 'x'");
    assert_eq!(git_said("\n  The diff tool meld is not available as 'meld'\nmore", ""), "The diff tool meld is not available as 'meld'");
    assert_eq!(git_said(" \n", "\nmerge of a.txt failed\n"), "merge of a.txt failed");
    assert_eq!(git_said("", ""), "");
    assert_eq!(git_said("error: a\tb\u{1b}c", ""), "error: a bc");
}

#[test]
fn texts_name_the_tool_and_its_keys() {
    let unknown = tool("compare-tool", "merge.tool", GitToolRun::Unknown);
    let text = unknown_text(ToolMode::Diff, &unknown);
    assert_eq!(
        text,
        "git config names the diff tool \"compare-tool\" (merge.tool), but sets no difftool.compare-tool.cmd or \
         mergetool.compare-tool.cmd, and git has no built-in tool by that name. Set one in git config, or choose a program in \
         Preferences > External tools."
    );
    let text = unknown_text(ToolMode::Merge, &unknown);
    assert!(text.contains("the merge tool \"compare-tool\"") && text.contains("sets no mergetool.compare-tool.cmd, and"), "{text}");
    let repo = tool("meld", "diff.tool", command("meld", true));
    assert_eq!(
        held_text(ToolMode::Diff, &repo),
        "meld (git config's diff tool) wasn't opened: its command difftool.meld.cmd comes from this repository's own config. \
         Trust the repository, then open it again."
    );
    let builtin = tool("meld", "diff.tool", GitToolRun::BuiltIn { path: None });
    assert!(held_text(ToolMode::Diff, &builtin).contains("wasn't opened: git difftool would run git in this repository."));
    assert_eq!(read_failed_text(ToolMode::Merge, "bad"), "Could not read git config's merge tool: bad");
    assert_eq!(inactive_text(ToolMode::Diff), "Open in external diff tool wasn't run: its repository is no longer the active tab.");
    assert!(inactive_text(ToolMode::Merge).starts_with("Resolve in external merge tool wasn't run"));
}

#[test]
fn page_hints_say_what_runs() {
    let en = Lang::En;
    let long = format!("meld\t\"$LOCAL\" \"$REMOTE\" {}", "x".repeat(400));
    let h = page_hint(en, ToolMode::Diff, &tool("meld", "diff.tool", command(&long, false)), true);
    assert_eq!(h.using, "Empty, so git config's diff tool is used: meld (from diff.tool).");
    let detail = h.detail.unwrap();
    assert!(detail.starts_with("It runs difftool.meld.cmd in sh: meld \"$LOCAL\" \"$REMOTE\" xx"), "{detail}");
    assert!(detail.ends_with('…') && detail.chars().count() == "It runs difftool.meld.cmd in sh: ".len() + 301, "{detail}");
    assert!(h.warn.is_empty(), "the user's own command runs in a held repository");

    let h = page_hint(en, ToolMode::Diff, &tool("meld", "diff.tool", command("meld", true)), true);
    assert_eq!(h.warn, ["This command comes from the repository's own config: it runs only once you trust the repository."]);
    let h = page_hint(en, ToolMode::Diff, &tool("meld", "diff.tool", command("meld", true)), false);
    assert!(h.warn.is_empty());

    let h = page_hint(en, ToolMode::Merge, &tool("vimdiff", "merge.tool", GitToolRun::BuiltIn { path: None }), true);
    assert_eq!(h.using, "Empty, so git config's merge tool is used: vimdiff (from merge.tool).");
    assert_eq!(h.detail.as_deref(), Some("A tool git knows: Trellis runs git mergetool --tool=vimdiff."));
    assert_eq!(h.warn, ["git mergetool runs git in this repository: it runs only once you trust the repository."]);
    let path = GitToolRun::BuiltIn { path: Some("/opt/kdiff3".into()) };
    let h = page_hint(en, ToolMode::Diff, &tool("kdiff3", "diff.tool", path), false);
    assert_eq!(h.detail.as_deref(), Some("A tool git knows: Trellis runs git difftool --tool=kdiff3, which starts /opt/kdiff3."));

    let unknown = tool("compare-tool", "merge.tool", GitToolRun::Unknown);
    let h = page_hint(en, ToolMode::Diff, &unknown, true);
    assert_eq!(h.detail, None);
    assert_eq!(
        h.warn,
        ["git config sets no difftool.compare-tool.cmd / mergetool.compare-tool.cmd and git has no built-in tool called \
          compare-tool, so it can't be used."]
    );
    assert_ne!(page_hint(Lang::De, ToolMode::Diff, &unknown, true), h);
}
