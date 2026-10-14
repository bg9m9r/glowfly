//! The built-in terminal panel: one shell session per repository, global (not parked with a
//! tab) so a hidden tab's shell keeps running.

use super::AppState;
use std::path::{Path, PathBuf};
use trellis_core::terminal::TermScreen;

/// The grid a session starts with before the panel has reported its size: (rows, cols).
pub const DEFAULT_SIZE: (u16, u16) = (24, 80);

#[derive(Default)]
pub struct TerminalPanel {
    /// Whether the panel is shown (not saved across restarts).
    pub open: bool,
    pub sessions: Vec<TermSession>,
    /// The id the next session gets.
    pub next_id: u64,
    /// The grid size (rows, cols) the panel last reported.
    pub grid: Option<(u16, u16)>,
    /// Whether the terminal has the keyboard; written by the view every frame.
    pub focused: bool,
    /// Give the terminal the keyboard on the next frame; set by update, cleared by the view.
    pub focus_requested: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SessionStatus {
    Starting,
    Running,
    /// The shell ended, with its exit code when known.
    Exited(Option<u32>),
    /// The shell could not be started.
    Failed(String),
}

pub struct TermSession {
    pub id: u64,
    /// The repository the shell was started in; compared with `PathBuf` equality only.
    pub workdir: PathBuf,
    /// The shell's short name (pwsh, bash), once it runs.
    pub program: String,
    pub screen: TermScreen,
    pub status: SessionStatus,
}

impl TermSession {
    /// Whether the shell is starting or running (input and Kill still reach it).
    pub fn alive(&self) -> bool {
        matches!(self.status, SessionStatus::Starting | SessionStatus::Running)
    }
}

impl TerminalPanel {
    /// The session started in `workdir`, alive or not.
    pub fn for_workdir(&self, workdir: &Path) -> Option<&TermSession> {
        self.sessions.iter().find(|s| s.workdir == workdir)
    }

    pub fn index_of(&self, id: u64) -> Option<usize> {
        self.sessions.iter().position(|s| s.id == id)
    }
}

/// The active tab's session, if it has one.
pub fn active(st: &AppState) -> Option<&TermSession> {
    st.terminal.for_workdir(&st.snap.as_ref()?.workdir)
}
