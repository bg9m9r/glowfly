//! Running the diff and merge tools git config names (Preferences > External tools sets none).
//! A configured command runs in sh on copies Glowfly writes, as in `compare`; a built-in tool
//! runs through `git difftool` (on those copies) or `git mergetool` (which writes and removes its
//! own, and stages the file itself). The diff tool is left running and only a failure is
//! reported: a command's within `EARLY_EXIT`, `git difftool`'s within its longer watch or,
//! since git waits for the tool, whenever it exits with an error later (`DiffToolFailedLater`).
//! The merge tool is waited for and reported as `MergeToolExited`.

use super::compare::{launch_failure, reason};
use crate::msg::Msg;
use crate::state::tool_git::{command_end, git_merge_end, git_said};
use glowfly_core::error::{Error, Result};
use glowfly_core::external_tools::compare::{diff_files, fingerprint, merge_files};
use glowfly_core::external_tools::git_config::GitToolRun;
use glowfly_core::external_tools::shell_tool::{ShellLaunch, diff_env, merge_env, run_shell_to_exit, spawn_shell};
use glowfly_core::external_tools::{Ended, early_exit_message, temp, watch_to_end};
use glowfly_core::gitcli::{self, args};
use glowfly_core::native_path::native;
use glowfly_core::repo;
use glowfly_model::tools::{GitDiffRequest, GitMergeRequest, LaunchFailure, MergeEnd, ToolKind, ToolsMsg};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

/// How long `git difftool` is watched for failing. Longer than `EARLY_EXIT`: its shell scripts
/// take a few seconds to set up on Windows before they start the tool or give up.
const GIT_DIFFTOOL_EARLY_EXIT: Duration = Duration::from_secs(10);

/// Writes the diff's versions and starts git config's diff tool on them. Only a failure is
/// reported; for `git difftool` this waits as long as the tool runs, to report a late one.
pub(super) fn open_diff(req: GitDiffRequest) -> Option<Msg> {
    open_diff_watching(req, GIT_DIFFTOOL_EARLY_EXIT)
}

/// [`open_diff`], watching `git difftool` for `window` before a failure counts as late.
fn open_diff_watching(req: GitDiffRequest, window: Duration) -> Option<Msg> {
    let failed = |failure| {
        let (program, target) = (req.tool.name.clone(), req.target.path().to_owned());
        Some(Msg::Tools(ToolsMsg::LaunchFailed { kind: ToolKind::DiffTool, program, target, failure }))
    };
    let files = match temp::root().map_err(Error::from).and_then(|root| diff_files(&root, &req.workdir, &req.target, req.filters)) {
        Ok(f) => f,
        Err(e) => return failed(LaunchFailure::Files(reason(&e))),
    };
    let started = match &req.tool.run {
        GitToolRun::Command { cmd, .. } => spawn_shell(&ShellLaunch {
            script: cmd.clone(),
            env: diff_env(&files.local, &files.remote, req.target.path()),
            cwd: req.workdir.clone(),
        })
        .map(|()| None),
        GitToolRun::BuiltIn { .. } => git_difftool(&req, &files.local, &files.remote, window),
        GitToolRun::Unknown => Err(Error::Other(format!("git config sets no command for the diff tool {}", req.tool.name))),
    };
    match started {
        // The copies stay: the tool may have handed them to a window that is still open.
        Ok(later) => later,
        Err(e) => {
            let _ = files.session.remove();
            failed(launch_failure(&e))
        }
    }
}

/// Starts `git difftool` with the built-in tool on `local` and `remote`, watching it as a
/// program Glowfly starts: a quick failure is the error, with git's most telling line. One
/// still running after the watch is waited for, and a later failure is the message returned.
fn git_difftool(req: &GitDiffRequest, local: &Path, remote: &Path, window: Duration) -> Result<Option<Msg>> {
    let tool = format!("--tool={}", req.tool.name);
    let (local, remote) = (native(local).to_string_lossy().into_owned(), native(remote).to_string_lossy().into_owned());
    let a = args(&["difftool", "--no-prompt", &tool, "--no-index", "--", &local, &remote]);
    let mut cmd = gitcli::command(&req.workdir, &a, gitcli::NO_PROMPTS);
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    let child = cmd.spawn().map_err(|e| Error::Other(format!("git couldn't start: {e}")))?;
    Ok(match watch_to_end(child, window)? {
        Ended::Early { code, stderr } if code != Some(0) => return Err(Error::Other(early_exit_message(code, &git_said(&stderr, "")))),
        Ended::Later { code, stderr } if code != Some(0) => Some(failed_later(req, code, &stderr)),
        _ => None,
    })
}

/// The report of `git difftool` for `req` exiting with `code` after its watch, having printed
/// `stderr`.
fn failed_later(req: &GitDiffRequest, code: Option<i32>, stderr: &str) -> Msg {
    let (program, target) = (req.tool.name.clone(), req.target.path().to_owned());
    Msg::Tools(ToolsMsg::DiffToolFailedLater { program, target, code, said: git_said(stderr, "") })
}

/// Runs git config's merge tool on the conflicted file until it exits and says how it ended.
pub(super) fn run_merge(req: GitMergeRequest) -> Msg {
    let end = match &req.tool.run {
        GitToolRun::Command { cmd, trust_exit_code, .. } => command_merge(&req, cmd, *trust_exit_code),
        GitToolRun::BuiltIn { .. } => git_mergetool(&req),
        GitToolRun::Unknown => {
            MergeEnd::Failed(LaunchFailure::Other(format!("git config sets no command for the merge tool {}", req.tool.name)))
        }
    };
    Msg::Tools(ToolsMsg::MergeToolExited { workdir: req.workdir, path: req.path, program: req.tool.name, end })
}

/// A merge command: run in sh on copies of the three sides, which are removed once it exits.
fn command_merge(req: &GitMergeRequest, cmd: &str, trust_exit_code: bool) -> MergeEnd {
    let files = match temp::root().map_err(Error::from).and_then(|root| merge_files(&root, &req.workdir, &req.path, req.filters)) {
        Ok(f) => f,
        Err(e) => return MergeEnd::Failed(LaunchFailure::Files(reason(&e))),
    };
    let before = fingerprint(&files.output);
    let env = merge_env(&files.base, &files.ours, &files.theirs, &req.path);
    let ran = run_shell_to_exit(&ShellLaunch { script: cmd.to_owned(), env, cwd: req.workdir.clone() });
    let changed = fingerprint(&files.output) != before;
    let _ = files.session.remove();
    match ran {
        Err(e) => MergeEnd::Failed(launch_failure(&e)),
        Ok(_) if matches!(repo::is_conflicted(&req.workdir, &req.path), Ok(false)) => MergeEnd::Resolved,
        Ok(r) => command_end(&r, changed, trust_exit_code),
    }
}

/// A built-in tool: `git mergetool` with stdin closed, so a question it asks fails rather than
/// waits. `--literal-pathspecs` reaches the git commands its script runs, so the path is never a
/// pathspec pattern; a name the script's shell would expand as a pattern is refused instead.
fn git_mergetool(req: &GitMergeRequest) -> MergeEnd {
    if !mergetool_can_open(&req.path) {
        return MergeEnd::Failed(LaunchFailure::Other(format!(
            "git mergetool can't open {}: it reads *, ?, [ or \\ in a file name as a pattern and could merge \
             another file. Set a merge tool in Preferences > External tools, or mergetool.{}.cmd in git config.",
            req.path, req.tool.name
        )));
    }
    let file = req.workdir.join(&req.path);
    let before = fingerprint(&file);
    let tool = format!("--tool={}", req.tool.name);
    let a = args(&["--literal-pathspecs", "mergetool", "--no-prompt", &tool, "--", &req.path]);
    let out = gitcli::output(&req.workdir, &a, "", &[], None);
    if out.code.is_none() && !out.timed_out {
        return MergeEnd::Failed(LaunchFailure::Other(out.stderr.trim().to_owned()));
    }
    let changed = fingerprint(&file) != before;
    git_merge_end(out.code, git_said(&out.stderr, &out.stdout), changed, repo::is_conflicted(&req.workdir, &req.path).ok())
}

/// Whether `git mergetool` can be pointed at `path`: its script splits the conflicted names
/// unquoted, so the shell expands `*`, `?` and `[...]` (and some shells `\`) against the files in
/// the top folder, and `[ab].txt` would merge and stage `a.txt`.
fn mergetool_can_open(path: &str) -> bool {
    !path.contains(['*', '?', '[', '\\'])
}

#[cfg(test)]
#[path = "git_tool_tests.rs"]
mod tests;
