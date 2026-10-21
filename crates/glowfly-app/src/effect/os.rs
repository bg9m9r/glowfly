//! Effects that hand something to the operating system: a browser, a terminal, the file
//! manager, the default program for a file and the system's file pickers (`Effect::Os`, run by
//! runner/os.rs). Unscoped; none of them touches the repository.

use crate::msg::{FilePurpose, FolderPurpose};
use std::path::PathBuf;

#[derive(Debug)]
pub enum OsEffect {
    OpenUrl(String),
    OpenTerminal(PathBuf),
    OpenFolder(PathBuf),
    /// Opens the working-tree file `path` (repo-relative) of `workdir` in the default program.
    /// Unless `confirmed`, a file the system would run instead of show is reported back
    /// (`FilesMsg::OpenNeedsConfirm`) rather than opened.
    OpenPath {
        workdir: PathBuf,
        path: String,
        confirmed: bool,
    },
    /// Shows an absolute path selected in the file manager.
    RevealPath(PathBuf),
    PickFolder {
        title: String,
        purpose: FolderPurpose,
    },
    PickFile {
        title: String,
        purpose: FilePurpose,
    },
}

impl OsEffect {
    /// A short name for the worker thread and the debug log.
    pub fn name(&self) -> &'static str {
        match self {
            OsEffect::OpenUrl(_) => "open-url",
            OsEffect::OpenTerminal(_) => "open-terminal",
            OsEffect::OpenFolder(_) => "open-folder",
            OsEffect::OpenPath { .. } => "open-path",
            OsEffect::RevealPath(_) => "reveal-path",
            OsEffect::PickFolder { .. } => "pick-folder",
            OsEffect::PickFile { .. } => "pick-file",
        }
    }
}
