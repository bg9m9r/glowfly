//! Preferences > External tools: the editor, terminal, diff tool and merge tool programs
//! Trellis starts, and the argument templates it passes them (expanded by `external_tools`).

use serde::{Deserialize, Serialize};

/// The default arguments for opening one file in the editor.
pub const EDITOR_FILE_ARGS: &str = "{path}";
/// The default arguments for opening the repository folder in the editor.
pub const EDITOR_REPO_ARGS: &str = "{repo}";
/// The default arguments for comparing two versions in the diff tool.
pub const DIFF_TOOL_ARGS: &str = "{local} {remote}";
/// The default arguments for resolving a conflict in the merge tool.
pub const MERGE_TOOL_ARGS: &str = "{base} {ours} {theirs} {output}";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExternalTools {
    /// Editor program: a name on PATH or a path; "" = none set.
    pub editor: String,
    /// Arguments for opening one file ({path}, {line}, {repo}).
    pub editor_file_args: String,
    /// Arguments for opening the repository folder ({repo}).
    pub editor_repo_args: String,
    /// Terminal program; "" = detect automatically.
    pub terminal: String,
    /// Arguments for the custom terminal ({repo}).
    pub terminal_args: String,
    /// Diff tool program; "" = none set.
    pub diff_tool: String,
    /// Arguments for the diff tool ({local}, {remote}, {path}, {repo}).
    pub diff_tool_args: String,
    /// Merge tool program; "" = none set.
    pub merge_tool: String,
    /// Arguments for the merge tool ({base}, {ours}, {theirs}, {output}, {repo}).
    pub merge_tool_args: String,
}

impl Default for ExternalTools {
    fn default() -> Self {
        Self {
            editor: String::new(),
            editor_file_args: EDITOR_FILE_ARGS.into(),
            editor_repo_args: EDITOR_REPO_ARGS.into(),
            terminal: String::new(),
            terminal_args: String::new(),
            diff_tool: String::new(),
            diff_tool_args: DIFF_TOOL_ARGS.into(),
            merge_tool: String::new(),
            merge_tool_args: MERGE_TOOL_ARGS.into(),
        }
    }
}

impl ExternalTools {
    /// [`DIFF_TOOL_ARGS`], for callers outside the settings module.
    pub const DIFF_TOOL_ARGS: &str = DIFF_TOOL_ARGS;
    /// [`MERGE_TOOL_ARGS`], for callers outside the settings module.
    pub const MERGE_TOOL_ARGS: &str = MERGE_TOOL_ARGS;

    /// Puts both editor argument templates back to their defaults, leaving the programs alone.
    pub fn reset_editor_args(&mut self) {
        self.editor_file_args = EDITOR_FILE_ARGS.into();
        self.editor_repo_args = EDITOR_REPO_ARGS.into();
    }

    /// Puts the diff tool's argument template back to its default, leaving everything else.
    pub fn reset_diff_tool_args(&mut self) {
        self.diff_tool_args = DIFF_TOOL_ARGS.into();
    }

    /// Puts the merge tool's argument template back to its default, leaving everything else.
    pub fn reset_merge_tool_args(&mut self) {
        self.merge_tool_args = MERGE_TOOL_ARGS.into();
    }
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
