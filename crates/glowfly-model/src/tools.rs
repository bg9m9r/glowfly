//! Messages and effects of the external editor, a custom terminal, and the external diff and
//! merge tools (Preferences > External tools; state/tools.rs, state/tool_compare.rs,
//! update/tools.rs, update/tool_compare.rs, runner/tools.rs, runner/tools/compare.rs,
//! ui/prefs/tools.rs), with git config's tools when Preferences sets none (state/tool_git.rs,
//! update/tool_git.rs, runner/tools/git_tool.rs).

use crate::effect::EffectMeta;
use crate::effect::SandboxRule;
use glowfly_core::external_tools::Launch;
use glowfly_core::external_tools::git_config::{GitTool, GitTools};
use glowfly_core::repo::DiffTarget;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ToolKind {
    Editor,
    Terminal,
    DiffTool,
    MergeTool,
}

/// Why a program didn't start.
#[derive(Clone, Debug, PartialEq)]
pub enum LaunchFailure {
    /// The program is not on PATH, or its path is gone.
    NotFound,
    /// The file to open is no longer in the working tree.
    Missing(PathBuf),
    /// The versions to compare or merge couldn't be written (the error).
    Files(String),
    /// Anything else, as the system put it.
    Other(String),
}

/// How a merge tool run ended.
#[derive(Clone, Debug, PartialEq)]
pub enum MergeEnd {
    /// It exited with 0 and the working file changed.
    Saved,
    /// It exited with 0 and left the working file as it was.
    Unchanged,
    /// The file is no longer conflicted (the tool staged it itself).
    Resolved,
    /// It exited with an error; `changed` says the working file changed anyway.
    Exited { code: Option<i32>, said: String, changed: bool },
    /// It didn't start, or its files couldn't be prepared.
    Failed(LaunchFailure),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ToolsMsg {
    /// From file menus: open this working-tree file (repo-relative) in the editor, at `line`
    /// (None = 1).
    OpenFile { path: String, line: Option<u32> },
    /// From the palette: open the repository folder in the editor.
    OpenRepo,
    /// From the palette: Preferences on the External Tools page.
    ShowPage,
    /// Preferences > External tools Browse… buttons.
    Browse(ToolKind),
    /// The program picker closed; None when cancelled.
    ProgramPicked { kind: ToolKind, path: Option<PathBuf> },
    /// A launch failed (runner/tools.rs). `program` as typed, `target` the file or repository
    /// name.
    LaunchFailed { kind: ToolKind, program: String, target: String, failure: LaunchFailure },
    /// `git difftool` (git config's built-in diff tool) on `target`, still running after its
    /// early-exit watch, later exited with an error (`code`, None when stopped; `said`, git's
    /// most telling line) (runner/tools/git_tool.rs).
    DiffToolFailedLater { program: String, target: String, code: Option<i32>, said: String },
    /// Open this diff's file in the external diff tool.
    OpenDiff(DiffTarget),
    /// Resolve this conflicted file (repo-relative) in the external merge tool, as a job.
    ResolveInMergeTool(String),
    /// The merge tool run as a job in `workdir` ended (runner/tools/compare.rs). `program` as
    /// typed.
    MergeToolExited { workdir: PathBuf, path: String, program: String, end: MergeEnd },
    /// git config's diff and merge tools were read in `cwd` (runner/tools.rs); `next` is the
    /// click waiting for them, None for the External tools page. Boxed: they are large.
    GitToolsRead { cwd: PathBuf, tools: Result<Box<GitTools>, String>, next: Option<GitToolNext> },
}

/// A tool entry waiting for git config's tools: no program was set in Preferences.
#[derive(Clone, Debug, PartialEq)]
pub enum GitToolNext {
    /// Open this diff in git config's diff tool.
    Diff(DiffTarget),
    /// Resolve this conflicted file (repo-relative) in git config's merge tool.
    Merge(String),
}

impl ToolsMsg {
    /// Never scoped: the effects behind these messages are unscoped and carry everything they
    /// need, so nothing is dropped when tabs change.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::OpenFile { .. }
            | Self::OpenRepo
            | Self::ShowPage
            | Self::Browse(_)
            | Self::ProgramPicked { .. }
            | Self::LaunchFailed { .. }
            | Self::DiffToolFailedLater { .. }
            | Self::OpenDiff(_)
            | Self::ResolveInMergeTool(_)
            // Checks the active tab itself (update/tool_git.rs).
            | Self::GitToolsRead { .. }
            // The job logic (ops::finish_job) decides what an inactive tab needs.
            | Self::MergeToolExited { .. } => None,
        }
    }
}

/// Opening a diff in the diff tool: the program and template from the settings, and what to
/// compare. `filters` writes copies through git's checkout filters (a trusted repository).
#[derive(Clone, Debug, PartialEq)]
pub struct DiffToolRequest {
    pub program: PathBuf,
    pub template: String,
    pub workdir: PathBuf,
    pub target: DiffTarget,
    pub filters: bool,
}

/// Resolving the conflict at `path` in the merge tool, as for `DiffToolRequest`.
#[derive(Clone, Debug, PartialEq)]
pub struct MergeToolRequest {
    pub program: PathBuf,
    pub template: String,
    pub workdir: PathBuf,
    pub path: String,
    pub filters: bool,
}

/// Opening a diff in git config's diff tool `tool`, as for `DiffToolRequest`.
#[derive(Clone, Debug, PartialEq)]
pub struct GitDiffRequest {
    pub tool: GitTool,
    pub workdir: PathBuf,
    pub target: DiffTarget,
    pub filters: bool,
}

/// Resolving the conflict at `path` in git config's merge tool `tool`, as for
/// `MergeToolRequest`.
#[derive(Clone, Debug, PartialEq)]
pub struct GitMergeRequest {
    pub tool: GitTool,
    pub workdir: PathBuf,
    pub path: String,
    pub filters: bool,
}

/// Starting the editor, terminal or a diff or merge tool, and their program pickers, run by
/// runner/tools.rs.
#[derive(Debug)]
pub enum ToolEffect {
    /// Starts `launch`; `check` is a file that must still exist first. `target` names it in
    /// errors.
    Launch { kind: ToolKind, launch: Launch, target: String, check: Option<PathBuf> },
    /// The file dialog for choosing a tool's program.
    PickProgram(ToolKind),
    /// Writes the diff's versions and starts the diff tool on them, without waiting.
    OpenDiff(DiffToolRequest),
    /// Writes the conflict's stages and runs the merge tool until it exits (the active job).
    RunMerge(MergeToolRequest),
    /// Reads git config's diff and merge tools in `cwd` (with the repository's own values when
    /// `has_repo`); runs nothing.
    ReadGitTools { cwd: PathBuf, has_repo: bool, next: Option<GitToolNext> },
    /// Writes the diff's versions and starts git config's diff tool on them, without waiting.
    OpenGitDiff(GitDiffRequest),
    /// Writes the conflict's stages (for a command) and runs git config's merge tool until it
    /// exits (the active job).
    RunGitMerge(GitMergeRequest),
}

impl EffectMeta for ToolEffect {
    /// A short name for the worker thread and the debug log.
    fn name(&self) -> &'static str {
        match self {
            ToolEffect::Launch { kind: ToolKind::Editor, .. } => "launch-editor",
            ToolEffect::Launch { kind: ToolKind::Terminal, .. } => "launch-terminal",
            ToolEffect::Launch { kind: ToolKind::DiffTool, .. } => "launch-diff-tool",
            ToolEffect::Launch { kind: ToolKind::MergeTool, .. } => "launch-merge-tool",
            ToolEffect::PickProgram(_) => "pick-tool-program",
            ToolEffect::OpenDiff(_) => "open-diff-tool",
            ToolEffect::RunMerge(_) => "run-merge-tool",
            ToolEffect::ReadGitTools { .. } => "read-git-tools",
            ToolEffect::OpenGitDiff(_) => "open-git-diff-tool",
            ToolEffect::RunGitMerge(_) => "run-git-merge-tool",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        None
    }

    /// They wait on the user or hand work to another program.
    fn timed(&self) -> bool {
        false
    }

    fn queue(&self) -> Option<&Path> {
        None
    }

    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Drop
    }
}
