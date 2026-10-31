//! The built-in terminal panel: shell sessions (terminal tabs) per repository, global (not
//! parked with a tab) so a hidden tab's shells keep running. The terminals a close would end
//! while they run a program are named in `busy`.

pub mod busy;

use super::AppState;
use glowfly_core::settings::{ShellSetting, TerminalPrefs};
use glowfly_core::terminal::{TermPoint, TermScreen};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The grid a session starts with before the panel has reported its size: (rows, cols).
pub const DEFAULT_SIZE: (u16, u16) = (24, 80);

#[derive(Default)]
pub struct TerminalPanel {
    /// Whether the panel is shown; saved as `TerminalPrefs::panel_open`.
    pub open: bool,
    /// Every repository's sessions, each repository's in tab order.
    pub sessions: Vec<TermSession>,
    /// The active session per repository working directory.
    pub active: HashMap<PathBuf, u64>,
    /// The id the next session gets.
    pub next_id: u64,
    /// The grid size (rows, cols) the panel last reported.
    pub grid: Option<(u16, u16)>,
    /// Whether the terminal has the keyboard; written by the view every frame.
    pub focused: bool,
    /// Give the terminal the keyboard on the next frame; set by update, cleared by the view.
    pub focus_requested: bool,
    /// Copy or cut commands waiting for their key's release, with how many arrived; view state.
    pub clip_pending: Option<(Clip, u32)>,
    /// Wheel movement short of a whole row, carried to the next frame; view state.
    pub wheel_rem: f32,
    /// Rows short of a whole one that dragging a selection past the grid's edge has scrolled,
    /// carried to the next frame; view state.
    pub edge_rem: f32,
    /// Whether the panel's edge was being dragged last frame; view state.
    pub resizing: bool,
    /// Whether the panel takes the main area instead of sitting under it. Not saved.
    pub maximized: bool,
    /// The tab being renamed in place, with the field's text; view state.
    pub renaming: Option<TabRename>,
}

/// A terminal tab's inline rename field.
#[derive(Clone, Debug, PartialEq)]
pub struct TabRename {
    pub id: u64,
    /// The field's text, edited by the view.
    pub text: String,
    /// The name the tab showed when the field opened; submitting it unchanged keeps the name.
    pub shown: String,
    /// Give the field the keyboard; cleared by the view once it has asked for it.
    pub focus: bool,
}

/// egui's copy and cut commands. On Windows they come from Ctrl+Insert and Shift+Delete as
/// well as Ctrl+C and Ctrl+X, without the key, so the terminal waits for the key's release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clip {
    Copy,
    Cut,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SessionStatus {
    Starting,
    Running,
    /// The shell ended, with its exit code when known.
    Exited(Option<u32>),
    /// The shell could not be started.
    Failed(String),
    /// No shell runs yet: git is held in its repository (trust), so the shell, whose prompt may
    /// run git, waits; trusting it starts it (update/terminal.rs `release_held`).
    Held,
}

pub struct TermSession {
    pub id: u64,
    /// The lowest number free in its repository when it started: it tells apart tabs that
    /// would show the same name ("pwsh · demo (2)").
    pub number: u32,
    /// The repository the shell was started in; compared with `PathBuf` equality only.
    pub workdir: PathBuf,
    /// The shell's short name (pwsh, bash), once it runs.
    pub program: String,
    /// The shell it was started with; Restart starts the same one.
    pub shell: ShellSetting,
    /// The name the user gave the tab, shown instead of the automatic one.
    pub name: Option<String>,
    pub screen: TermScreen,
    pub status: SessionStatus,
    /// Text selected with the mouse; written by the view, cleared by update.
    pub selection: Option<Selection>,
}

/// A stream selection between two cell boundaries, in the screen's line numbers of `epoch`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Selection {
    /// Where the drag started.
    pub anchor: TermPoint,
    /// Where the pointer is.
    pub head: TermPoint,
    pub epoch: u64,
}

impl Selection {
    /// The selected range in reading order, or None when it is empty or its lines can no
    /// longer be followed. Rows that dropped off the front of the history are left out.
    pub fn range(&self, screen: &TermScreen) -> Option<(TermPoint, TermPoint)> {
        if self.epoch != screen.epoch() {
            return None;
        }
        let (start, end) = glowfly_core::terminal::ordered(self.anchor, self.head);
        let start = start.max(TermPoint { line: screen.first_line(), col: 0 });
        (start < end).then_some((start, end))
    }
}

impl TermSession {
    /// Whether the shell is starting or running (input and Kill still reach it).
    pub fn alive(&self) -> bool {
        matches!(self.status, SessionStatus::Starting | SessionStatus::Running)
    }
}

impl TerminalPanel {
    /// The panel as the last run left it.
    pub fn restored(p: &TerminalPrefs) -> Self {
        Self { open: p.panel_open, ..Default::default() }
    }

    /// The active session of `workdir`, alive or not.
    pub fn for_workdir(&self, workdir: &Path) -> Option<&TermSession> {
        let id = self.active_id(workdir)?;
        self.sessions.iter().find(|s| s.id == id)
    }

    /// `workdir`'s sessions in tab order.
    pub fn sessions_for<'a>(&'a self, workdir: &'a Path) -> impl Iterator<Item = &'a TermSession> + 'a {
        self.sessions.iter().filter(move |s| s.workdir == workdir)
    }

    /// The active session of `workdir`: the one recorded if it still exists, else its first.
    pub fn active_id(&self, workdir: &Path) -> Option<u64> {
        let recorded = self.active.get(workdir).filter(|id| self.sessions.iter().any(|s| s.id == **id && s.workdir == workdir));
        recorded.copied().or_else(|| self.sessions_for(workdir).next().map(|s| s.id))
    }

    /// The lowest tab number none of `workdir`'s sessions uses.
    pub fn next_number(&self, workdir: &Path) -> u32 {
        (1..).find(|n| !self.sessions_for(workdir).any(|s| s.number == *n)).unwrap_or(1)
    }

    pub fn index_of(&self, id: u64) -> Option<usize> {
        self.sessions.iter().position(|s| s.id == id)
    }
}

/// Whether the panel is drawn in the main area: open, maximised, and on a repository view (not
/// the dashboard or Preferences).
pub fn maximized_shown(st: &AppState) -> bool {
    st.terminal.open && st.terminal.maximized && st.snap.is_some() && !st.dashboard.open && st.prefs.is_none()
}

/// The active repository's active session, if it has one.
pub fn active(st: &AppState) -> Option<&TermSession> {
    st.terminal.for_workdir(&st.snap.as_ref()?.workdir)
}
