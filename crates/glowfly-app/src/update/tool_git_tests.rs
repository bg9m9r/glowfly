//! Headless tests for git config's diff and merge tools, used when Preferences sets no program:
//! the read before Preferences opens, the requests planned for a command or a built-in tool
//! (with filters only where they may run), the trust rules (the user's own command runs in a
//! held repository; the repository's own or a built-in one waits for trust), an unknown tool,
//! stale reads, and the External tools page's own read.

use super::conflict_tests::conflicted_repo;
use super::tests::{drive, git, scratch_repo, state};
use super::update;
use crate::effect::Effect;
use crate::msg::{Msg, PrefsMsg, RepoMsg, TrustMsg};
use crate::state::{AppState, Modal, PrefPage};
use crate::tools::{GitDiffRequest, GitMergeRequest, GitToolNext, ToolEffect, ToolsMsg};
use glowfly_core::external_tools::git_config::{GitTool, GitToolRun, GitTools};
use glowfly_core::repo::DiffTarget;
use std::path::Path;
use std::time::Instant;

fn run(st: &mut AppState, m: ToolsMsg) -> Vec<Effect> {
    update(st, Msg::Tools(m), Instant::now())
}

fn opened(dir: &Path) -> AppState {
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.to_path_buf())));
    assert!(st.snap.is_some());
    st
}

/// `dir` opened while git is held there (a repository-configured ssh command), the first
/// prompt closed.
fn held(dir: &Path) -> AppState {
    git(dir, &["config", "core.sshCommand", "ssh -v"]);
    let mut st = opened(dir);
    drive(&mut st, Msg::CloseModal);
    assert!(st.held() && st.modal.is_none());
    st
}

fn tool(name: &str, run: GitToolRun) -> GitTool {
    GitTool { name: name.into(), chosen_by: "diff.tool".into(), run }
}

fn command(from_repo: bool) -> GitTool {
    let run = GitToolRun::Command {
        key: "difftool.meld.cmd".into(),
        cmd: "meld \"$LOCAL\" \"$REMOTE\"".into(),
        trust_exit_code: false,
        from_repo,
    };
    tool("meld", run)
}

fn builtin() -> GitTool {
    tool("vimdiff", GitToolRun::BuiltIn { path: None })
}

fn diff_tools(t: GitTool) -> GitTools {
    GitTools { diff: Some(t), merge: None }
}

fn merge_tools(t: GitTool) -> GitTools {
    GitTools { diff: None, merge: Some(t) }
}

fn read(cwd: &Path, tools: GitTools, next: GitToolNext) -> ToolsMsg {
    ToolsMsg::GitToolsRead { cwd: cwd.to_owned(), tools: Ok(Box::new(tools)), next: Some(next) }
}

fn unstaged() -> DiffTarget {
    DiffTarget::Unstaged("a.txt".into())
}

/// The only effect, which must open git config's diff tool.
fn git_diff(fx: Vec<Effect>) -> GitDiffRequest {
    match <[Effect; 1]>::try_from(fx) {
        Ok([Effect::Tool(ToolEffect::OpenGitDiff(r))]) => r,
        other => panic!("expected one OpenGitDiff, got {other:?}"),
    }
}

/// The only effect, which must run git config's merge tool.
fn git_merge(fx: Vec<Effect>) -> GitMergeRequest {
    match <[Effect; 1]>::try_from(fx) {
        Ok([Effect::Tool(ToolEffect::RunGitMerge(r))]) => r,
        other => panic!("expected one RunGitMerge, got {other:?}"),
    }
}

fn last_log(st: &AppState) -> Option<String> {
    st.log.last().map(|l| l.text.clone())
}

#[test]
fn no_program_reads_git_config_before_preferences() {
    let dir = scratch_repo("tg-read-first");
    let mut st = opened(&dir);
    let w = st.workdir().unwrap();
    let fx = run(&mut st, ToolsMsg::OpenDiff(unstaged()));
    match &fx[..] {
        [Effect::Tool(ToolEffect::ReadGitTools { cwd, has_repo: true, next: Some(GitToolNext::Diff(t)) })] => {
            assert!(*cwd == w && *t == unstaged());
        }
        other => panic!("expected one ReadGitTools, got {other:?}"),
    }
    assert!(st.prefs.is_none(), "Preferences waits for git config");
    let (toasts, log) = (st.toasts.len(), last_log(&st));
    run(&mut st, read(&w, GitTools::default(), GitToolNext::Diff(unstaged())));
    assert_eq!(st.prefs.as_ref().map(|p| p.page), Some(PrefPage::Tools));
    assert_eq!(st.prefs.as_ref().and_then(|p| p.git_tools.clone()), Some(GitTools::default()));
    assert_eq!(st.toasts.len(), toasts, "no failure toast");
    assert_eq!(last_log(&st), log, "no failure in the log");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_user_command_opens_the_diff_with_filters_when_trusted() {
    let dir = scratch_repo("tg-user-diff");
    let mut st = opened(&dir);
    let w = st.workdir().unwrap();
    let r = git_diff(run(&mut st, read(&w, diff_tools(command(false)), GitToolNext::Diff(unstaged()))));
    assert_eq!(r, GitDiffRequest { tool: command(false), workdir: w, target: unstaged(), filters: true });
    let _ = std::fs::remove_dir_all(&dir);

    // The user's own command runs in a held repository, on copies made without its filters.
    let dir = scratch_repo("tg-user-diff-held");
    let mut st = held(&dir);
    let w = st.workdir().unwrap();
    let r = git_diff(run(&mut st, read(&w, diff_tools(command(false)), GitToolNext::Diff(unstaged()))));
    assert!(!r.filters);
    assert!(st.modal.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_repository_command_or_builtin_waits_for_trust_in_a_held_repo() {
    let dir = scratch_repo("tg-held-diff");
    let mut st = held(&dir);
    let w = st.workdir().unwrap();
    for (t, why) in [(command(true), "difftool.meld.cmd comes from this repository's own config"), (builtin(), "git difftool would run")] {
        let fx = run(&mut st, read(&w, diff_tools(t.clone()), GitToolNext::Diff(unstaged())));
        assert!(!fx.iter().any(|e| matches!(e, Effect::Tool(_))), "{fx:?}");
        assert!(matches!(st.modal, Some(Modal::Trust(_))), "the trust prompt opens");
        let log = st.log.iter().rev().find(|l| l.text.contains("wasn't opened")).map(|l| l.text.clone()).unwrap_or_default();
        assert!(log.starts_with(&format!("{} (git config's diff tool)", t.name)) && log.contains(why), "{log}");
        drive(&mut st, Msg::CloseModal);
    }
    let _ = std::fs::remove_dir_all(&dir);

    let dir = scratch_repo("tg-builtin-diff");
    let mut st = opened(&dir);
    let w = st.workdir().unwrap();
    let r = git_diff(run(&mut st, read(&w, diff_tools(builtin()), GitToolNext::Diff(unstaged()))));
    assert!(r.filters && r.tool == builtin());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unknown_tool_fails_with_open_preferences() {
    let dir = scratch_repo("tg-unknown");
    let mut st = opened(&dir);
    let w = st.workdir().unwrap();
    let unknown = GitTool { name: "compare-tool".into(), chosen_by: "merge.tool".into(), run: GitToolRun::Unknown };
    assert!(run(&mut st, read(&w, diff_tools(unknown), GitToolNext::Diff(unstaged()))).is_empty());
    let t = st.toasts.last().expect("a toast");
    assert!(!t.ok && t.text.contains("no built-in tool") && t.text.contains("\"compare-tool\""), "{}", t.text);
    assert_eq!(t.action.as_ref().map(|a| a.label.as_str()), Some("Open preferences"));
    assert!(st.prefs.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_user_merge_command_runs_as_the_tool_job() {
    let dir = conflicted_repo("tg-user-merge");
    let mut st = opened(&dir);
    let w = st.workdir().unwrap();
    let fx = run(&mut st, ToolsMsg::ResolveInMergeTool("a.txt".into()));
    assert!(
        matches!(&fx[..], [Effect::Tool(ToolEffect::ReadGitTools { next: Some(GitToolNext::Merge(p)), .. })] if p == "a.txt"),
        "{fx:?}"
    );
    let next = || GitToolNext::Merge("a.txt".into());
    let r = git_merge(run(&mut st, read(&w, merge_tools(command(false)), next())));
    assert_eq!((r.path.as_str(), r.workdir.as_path(), r.filters), ("a.txt", w.as_path(), true));
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Resolve a.txt in the merge tool"));
    // One job at a time.
    assert!(run(&mut st, read(&w, merge_tools(command(false)), next())).is_empty());
    assert_eq!(last_log(&st).as_deref(), Some("Another operation is still running."));
    let _ = std::fs::remove_dir_all(&dir);

    // Even in a held repository: the command is the user's own.
    let dir = conflicted_repo("tg-user-merge-held");
    let mut st = held(&dir);
    let w = st.workdir().unwrap();
    let r = git_merge(run(&mut st, read(&w, merge_tools(command(false)), next())));
    assert!(!r.filters && st.modal.is_none() && st.job.is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_builtin_merge_tool_is_held_for_the_trust_prompt() {
    let dir = conflicted_repo("tg-builtin-merge-held");
    let mut st = held(&dir);
    let w = st.workdir().unwrap();
    let fx = run(&mut st, read(&w, merge_tools(builtin()), GitToolNext::Merge("a.txt".into())));
    assert!(!fx.iter().any(|e| matches!(e, Effect::Tool(_))), "{fx:?}");
    assert_eq!(st.trust.held.as_ref().map(|h| h.job.label.as_str()), Some("Resolve a.txt in the merge tool"));
    let real = st.snap.as_ref().unwrap().real_workdir.clone();
    let fx = update(&mut st, Msg::Trust(TrustMsg::Decide { real, trusted: true }), Instant::now());
    let r = fx.iter().find_map(|e| match e {
        Effect::Tool(ToolEffect::RunGitMerge(r)) => Some(r.clone()),
        _ => None,
    });
    let r = r.expect("the held merge runs once trusted");
    assert!(r.filters && r.tool == builtin() && r.path == "a.txt");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reads_for_another_tab_or_after_a_program_is_set() {
    let dir = conflicted_repo("tg-stale");
    let mut st = opened(&dir);
    let w = st.workdir().unwrap();
    let elsewhere = glowfly_core::paths::temp_dir().join("glowfly-tg-elsewhere");
    assert!(run(&mut st, read(&elsewhere, diff_tools(command(false)), GitToolNext::Diff(unstaged()))).is_empty());
    assert_eq!(last_log(&st).as_deref(), Some("Open in external diff tool wasn't run: its repository is no longer the active tab."));

    // A program set in Preferences meanwhile wins.
    st.settings.external_tools.diff_tool = "kdiff3".into();
    let fx = run(&mut st, read(&w, diff_tools(command(false)), GitToolNext::Diff(unstaged())));
    assert!(matches!(&fx[..], [Effect::Tool(ToolEffect::OpenDiff(r))] if r.program.as_path() == Path::new("kdiff3")), "{fx:?}");

    assert!(run(&mut st, read(&w, merge_tools(command(false)), GitToolNext::Merge("notes.txt".into()))).is_empty());
    assert_eq!(last_log(&st).as_deref(), Some("notes.txt is not conflicted"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_external_tools_page_reads_git_config() {
    let is_page_read = |e: &Effect, cwd: &Path, repo: bool| matches!(e, Effect::Tool(ToolEffect::ReadGitTools { cwd: c, has_repo, next: None }) if c == cwd && *has_repo == repo);
    let mut st = state();
    let any = st.any_dir();
    let fx = update(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Tools)), Instant::now());
    assert!(fx.iter().any(|e| is_page_read(e, &any, false)), "{fx:?}");
    let fx = run(&mut st, ToolsMsg::ShowPage);
    assert!(fx.iter().any(|e| is_page_read(e, &any, false)), "{fx:?}");

    let dir = scratch_repo("tg-page");
    let mut st = opened(&dir);
    let w = st.workdir().unwrap();
    update(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::General)), Instant::now());
    let fx = update(&mut st, Msg::Prefs(PrefsMsg::Navigate(PrefPage::Tools)), Instant::now());
    assert!(fx.iter().any(|e| is_page_read(e, &w, true)), "{fx:?}");
    assert_eq!(st.prefs.as_ref().map(|p| p.page), Some(PrefPage::Tools));

    let page = |tools| ToolsMsg::GitToolsRead { cwd: w.clone(), tools, next: None };
    let elsewhere = ToolsMsg::GitToolsRead { cwd: glowfly_core::paths::temp_dir(), tools: Ok(Box::new(diff_tools(builtin()))), next: None };
    assert!(run(&mut st, elsewhere).is_empty());
    assert_eq!(st.prefs.as_ref().unwrap().git_tools, None, "another folder's read is ignored");
    assert!(run(&mut st, page(Ok(Box::new(diff_tools(command(false)))))).is_empty());
    assert_eq!(st.prefs.as_ref().unwrap().git_tools, Some(diff_tools(command(false))));
    let toasts = st.toasts.len();
    assert!(run(&mut st, page(Err("bad config".into()))).is_empty());
    assert_eq!(st.prefs.as_ref().unwrap().git_tools, Some(diff_tools(command(false))), "an error keeps what was read");
    assert_eq!(last_log(&st).as_deref(), Some("Could not read git config's diff and merge tools: bad config"));
    assert_eq!(st.toasts.len(), toasts, "a page read error is only a quiet line");
    assert!(page(Ok(Box::default())).on_inactive().is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
