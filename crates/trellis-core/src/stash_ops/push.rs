//! `git stash push` command lines for the Stash dialog: every change, only the staged changes,
//! or chosen paths, each with an optional message.

use crate::gitcli::args;
use std::path::Path;

/// What goes into a new stash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StashScope {
    /// Every change, untracked files included.
    All,
    /// Only what is staged; unstaged changes and untracked files stay in the working tree.
    Staged,
    /// Only the chosen paths (tracked or untracked).
    Files,
}

/// The first git version that knows `git stash push --staged`.
pub const STAGED_MIN_GIT: (u32, u32) = (2, 35);

/// The most bytes of paths passed on the command line; longer selections go through a pathspec
/// file, which keeps them under the Windows command-line limit.
pub const PATHS_ON_COMMAND_LINE: usize = 8 * 1024;

/// The stash message for the typed `text`: whitespace runs (newlines and tabs too, since a
/// reflog line can't hold them) become single spaces and the ends are trimmed. None when
/// nothing is left, so git writes its own "WIP on <branch>: …" text.
pub fn push_message(text: &str) -> Option<String> {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!text.is_empty()).then_some(text)
}

/// A stash command line, and what it reads from its pathspec file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PushCommand {
    pub cmd: Vec<String>,
    /// NUL-separated paths for `--pathspec-from-file`, written before the command runs.
    pub pathspec_file: Option<String>,
}

/// The `git stash push` command for `scope` with `message`. `Files` stashes `paths`, read
/// literally (`--literal-pathspecs`, which git also passes on to its child processes as
/// GIT_LITERAL_PATHSPECS), so names with `*`, `?` or `[` are never globs. When the paths add up
/// to more than `PATHS_ON_COMMAND_LINE` they are read from `pathspec_file` instead. `Staged` has
/// no `-u`: git refuses `--staged` together with `--include-untracked`.
pub fn push_command(scope: StashScope, message: Option<&str>, paths: &[String], pathspec_file: &Path) -> PushCommand {
    let mut cmd = match scope {
        StashScope::All => args(&["stash", "push", "-u"]),
        StashScope::Staged => args(&["stash", "push", "--staged"]),
        StashScope::Files => args(&["--literal-pathspecs", "stash", "push", "-u"]),
    };
    if let Some(m) = message {
        cmd.extend(args(&["-m", m]));
    }
    let mut file = None;
    if scope == StashScope::Files {
        if paths.iter().map(|p| p.len() + 1).sum::<usize>() > PATHS_ON_COMMAND_LINE {
            cmd.push(format!("--pathspec-from-file={}", pathspec_file.display()));
            cmd.push("--pathspec-file-nul".into());
            file = Some(paths.join("\0"));
        } else {
            cmd.push("--".into());
            cmd.extend(paths.iter().cloned());
        }
    }
    PushCommand { cmd, pathspec_file: file }
}

/// Whether git's `output` says it doesn't know `--staged` (git older than 2.35). git runs
/// untranslated (`gitcli::output`), so the English text can be matched.
pub fn staged_unsupported(output: &str) -> bool {
    output.lines().any(|l| l.contains("unknown option") && l.contains("staged"))
}
