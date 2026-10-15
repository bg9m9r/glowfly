//! Messages and effects of the external editor and a custom terminal (Preferences > External
//! Tools; state/tools.rs, update/tools.rs, runner/tools.rs, ui/prefs/tools.rs).

use std::path::PathBuf;
use trellis_core::external_tools::Launch;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ToolKind {
    Editor,
    Terminal,
}

/// Why a program didn't start.
#[derive(Clone, Debug, PartialEq)]
pub enum LaunchFailure {
    /// The program is not on PATH, or its path is gone.
    NotFound,
    /// The file to open is no longer in the working tree.
    Missing(PathBuf),
    /// Anything else, as the system put it.
    Other(String),
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
    /// Preferences > External Tools Browse… buttons.
    Browse(ToolKind),
    /// The program picker closed; None when cancelled.
    ProgramPicked { kind: ToolKind, path: Option<PathBuf> },
    /// A launch failed (runner/tools.rs). `program` as typed, `target` the file or repository
    /// name.
    LaunchFailed { kind: ToolKind, program: String, target: String, failure: LaunchFailure },
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
            | Self::LaunchFailed { .. } => None,
        }
    }
}

/// Starting the editor or terminal, and their program pickers, run by runner/tools.rs.
#[derive(Debug)]
pub enum ToolEffect {
    /// Starts `launch`; `check` is a file that must still exist first. `target` names it in
    /// errors.
    Launch { kind: ToolKind, launch: Launch, target: String, check: Option<PathBuf> },
    /// The file dialog for choosing the editor or terminal program.
    PickProgram(ToolKind),
}

impl ToolEffect {
    /// A short name for the worker thread and the debug log.
    pub fn name(&self) -> &'static str {
        match self {
            ToolEffect::Launch { kind: ToolKind::Editor, .. } => "launch-editor",
            ToolEffect::Launch { kind: ToolKind::Terminal, .. } => "launch-terminal",
            ToolEffect::PickProgram(_) => "pick-tool-program",
        }
    }
}
