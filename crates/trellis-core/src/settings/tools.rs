//! Preferences > External Tools: the editor and terminal programs Trellis starts, and the
//! argument templates it passes them (expanded by `external_tools`).

use serde::{Deserialize, Serialize};

/// The default arguments for opening one file in the editor.
pub const EDITOR_FILE_ARGS: &str = "{path}";
/// The default arguments for opening the repository folder in the editor.
pub const EDITOR_REPO_ARGS: &str = "{repo}";

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
}

impl Default for ExternalTools {
    fn default() -> Self {
        Self {
            editor: String::new(),
            editor_file_args: EDITOR_FILE_ARGS.into(),
            editor_repo_args: EDITOR_REPO_ARGS.into(),
            terminal: String::new(),
            terminal_args: String::new(),
        }
    }
}

impl ExternalTools {
    /// Puts both editor argument templates back to their defaults, leaving the programs alone.
    pub fn reset_editor_args(&mut self) {
        self.editor_file_args = EDITOR_FILE_ARGS.into();
        self.editor_repo_args = EDITOR_REPO_ARGS.into();
    }
}
