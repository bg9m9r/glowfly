//! Rules and texts for git config's diff and merge tools, used when Preferences > External
//! tools sets no program: what the log and toasts say, how a merge tool run ended (a command
//! Trellis ran in sh, or `git mergetool`), git's most telling line, and what the External tools
//! page shows about the tool git config names.

use crate::tools::MergeEnd;
use trellis_core::external_tools::ExitReport;
use trellis_core::external_tools::git_config::{GitTool, GitToolRun, ToolMode};
use trellis_core::i18n::Lang;
use trellis_core::text::printable;

/// How much of a command the External tools page shows.
const SHOWN_CMD_CHARS: usize = 300;

/// The tool's kind in English: "diff tool" or "merge tool".
fn kind(mode: ToolMode) -> String {
    format!("{} tool", mode.word())
}

/// A tool git config names but that can't run: no command and no built-in tool by its name.
pub fn unknown_text(mode: ToolMode, tool: &GitTool) -> String {
    format!(
        "git config names the {} \"{}\" ({}), but sets no {}, and git has no built-in tool by that name. Set one in git config, \
         or choose a program in Preferences > External tools.",
        kind(mode),
        tool.name,
        tool.chosen_by,
        tool.cmd_keys(mode).join(" or "),
    )
}

/// Why a tool that needs trust didn't start in a held repository.
pub fn held_text(mode: ToolMode, tool: &GitTool) -> String {
    let why = match &tool.run {
        GitToolRun::Command { key, .. } => format!("its command {key} comes from this repository's own config"),
        _ => format!("git {} would run git in this repository", mode.command()),
    };
    format!("{} (git config's {}) wasn't opened: {why}. Trust the repository, then open it again.", tool.name, kind(mode))
}

/// git config couldn't be read for a click.
pub fn read_failed_text(mode: ToolMode, e: &str) -> String {
    format!("Could not read git config's {}: {e}", kind(mode))
}

/// A click whose tab is no longer the active one when git config's tools are known.
pub fn inactive_text(mode: ToolMode) -> String {
    let entry = match mode {
        ToolMode::Diff => "Open in external diff tool",
        ToolMode::Merge => "Resolve in external merge tool",
    };
    format!("{entry} wasn't run: its repository is no longer the active tab.")
}

/// How a merge command Trellis ran in sh ended (the file still conflicted), as git judges it:
/// by the exit code with `trust_exit_code`, else by whether the working file `changed`.
pub fn command_end(r: &ExitReport, changed: bool, trust_exit_code: bool) -> MergeEnd {
    let exited = || MergeEnd::Exited { code: r.code, said: r.said.clone(), changed };
    match (trust_exit_code, r.success(), changed) {
        (true, true, _) => MergeEnd::Saved,
        (true, false, _) => exited(),
        (false, _, true) => MergeEnd::Saved,
        (false, true, false) => MergeEnd::Unchanged,
        (false, false, false) => exited(),
    }
}

/// How `git mergetool` ended: its exit `code`, its most telling line `said`, whether the
/// working file `changed`, and whether the file is still `conflicted` (None when unknown).
pub fn git_merge_end(code: Option<i32>, said: String, changed: bool, conflicted: Option<bool>) -> MergeEnd {
    match code {
        Some(0) if conflicted == Some(false) => MergeEnd::Resolved,
        Some(0) if changed => MergeEnd::Saved,
        Some(0) => MergeEnd::Unchanged,
        code => MergeEnd::Exited { code, said, changed },
    }
}

/// git's most telling line: the first stderr line starting `error:` or `fatal:` (warnings such
/// as line-ending notes may come first), else the first non-empty stderr line, else the first
/// non-empty stdout line; made printable.
pub fn git_said(stderr: &str, stdout: &str) -> String {
    fn lines(s: &str) -> Vec<&str> {
        s.lines().map(str::trim).filter(|l| !l.is_empty()).collect()
    }
    let (err, out) = (lines(stderr), lines(stdout));
    let line = err
        .iter()
        .find(|l| l.starts_with("error:") || l.starts_with("fatal:"))
        .or_else(|| err.first())
        .or_else(|| out.first())
        .copied()
        .unwrap_or_default();
    printable(line)
}

/// What the External tools page shows under an empty program field: which tool git config
/// names (`using`), what it runs (`detail`) and why it can't run or runs only once trusted
/// (`warn`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitToolHint {
    pub using: String,
    pub detail: Option<String>,
    pub warn: Vec<String>,
}

/// `s` cut to `max` characters, with an ellipsis when cut.
fn cut(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s.to_owned(),
    }
}

/// The page's hint for `mode`'s git config tool `tool`; `held` says git is held in the active
/// repository.
pub fn page_hint(t: Lang, mode: ToolMode, tool: &GitTool, held: bool) -> GitToolHint {
    let args: &[(&str, &dyn std::fmt::Display)] = &[("name", &tool.name), ("key", &tool.chosen_by)];
    let using = match mode {
        ToolMode::Diff => t.trf("prefs.tools.diff.git_tool", args),
        ToolMode::Merge => t.trf("prefs.tools.merge.git_tool", args),
    };
    let command = mode.command();
    let mut warn = Vec::new();
    let detail = match &tool.run {
        GitToolRun::Command { key, cmd, .. } => {
            let cmd = cut(&printable(cmd), SHOWN_CMD_CHARS);
            Some(t.trf("prefs.tools.git_tool.command", &[("key", key), ("cmd", &cmd)]))
        }
        GitToolRun::BuiltIn { path: Some(path) } => {
            Some(t.trf("prefs.tools.git_tool.builtin_path", &[("command", &command), ("name", &tool.name), ("path", &printable(path))]))
        }
        GitToolRun::BuiltIn { path: None } => Some(t.trf("prefs.tools.git_tool.builtin", &[("command", &command), ("name", &tool.name)])),
        GitToolRun::Unknown => {
            let keys = tool.cmd_keys(mode).join(" / ");
            warn.push(t.trf("prefs.tools.git_tool.unknown", &[("keys", &keys), ("name", &tool.name)]));
            None
        }
    };
    if held && tool.needs_trust() {
        warn.push(match tool.run {
            GitToolRun::Command { .. } => t.tr("prefs.tools.git_tool.repo_command").to_owned(),
            _ => t.trf("prefs.tools.git_tool.repo_git", &[("command", &command)]),
        });
    }
    GitToolHint { using, detail, warn }
}

#[cfg(test)]
#[path = "tool_git_tests.rs"]
mod tests;
