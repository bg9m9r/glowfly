//! Messages and effects of the built-in terminal panel (state/terminal.rs, update/terminal.rs,
//! runner/terminal.rs, ui/terminal/).

use std::path::PathBuf;
use trellis_core::settings::ShellSetting;
use trellis_core::terminal::TermInput;

/// Carried by an `Output` message: dropping it tells the session's output thread that the
/// message was handled (runner/terminal_out.rs). The default does nothing.
#[derive(Default)]
pub struct Ack(Option<Box<dyn FnOnce() + Send>>);

impl Ack {
    pub fn new(on_drop: impl FnOnce() + Send + 'static) -> Self {
        Self(Some(Box::new(on_drop)))
    }
}

impl Drop for Ack {
    fn drop(&mut self) {
        if let Some(f) = self.0.take() {
            f();
        }
    }
}

pub enum TerminalMsg {
    /// Show or hide the panel (toolbar button, palette, Ctrl+`).
    Toggle,
    Hide,
    /// Stop the active tab's shell.
    Kill,
    /// Start a new shell in the active tab's place after its shell ended.
    Restart,
    /// Keyboard input for the active tab's shell.
    Input(Vec<TermInput>),
    /// The grid the panel has room for, in cells.
    Resize {
        rows: u16,
        cols: u16,
    },
    /// Start another terminal tab in the active repository (showing the panel if it is hidden).
    NewTab,
    /// Make session `id` the active tab.
    SelectTab(u64),
    /// Stop session `id`'s shell if it runs and remove its tab.
    CloseTab(u64),
    /// Ask before stopping every shell of the active repository.
    AskKillAll,
    /// Stop every shell of the active repository.
    KillAll,
    /// Scroll the active tab's view this many rows back into its history (forward when negative).
    Scroll(i32),
    /// Copy the active tab's selection; `keep` leaves it selected.
    CopySelection {
        keep: bool,
    },
    SelectAll,
    /// Clear the active tab's history and screen.
    Clear,
    /// Paste the clipboard into the active tab's shell.
    Paste,
    /// A drag of the panel's edge ended at this height, in points.
    PanelHeight(f32),
    // ---- from the runner
    /// Session `id`'s shell is running; `program` is its short name (pwsh, bash).
    Started {
        id: u64,
        program: String,
    },
    Output {
        id: u64,
        bytes: Vec<u8>,
        /// Dropped once the output has been handled: the session then sends what came since.
        ack: Ack,
    },
    /// Session `id`'s shell ended; `code` is None when it is unknown.
    Exited {
        id: u64,
        code: Option<u32>,
    },
    SpawnFailed {
        id: u64,
        error: String,
    },
}

impl TerminalMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::Toggle
            | Self::Hide
            | Self::Kill
            | Self::Restart
            | Self::Input(..)
            | Self::Resize { .. }
            | Self::NewTab
            | Self::SelectTab(..)
            | Self::CloseTab(..)
            | Self::AskKillAll
            | Self::KillAll
            | Self::Scroll(..)
            | Self::CopySelection { .. }
            | Self::SelectAll
            | Self::Clear
            | Self::Paste
            | Self::PanelHeight(..)
            | Self::Started { .. }
            | Self::Output { .. }
            | Self::Exited { .. }
            | Self::SpawnFailed { .. } => None,
        }
    }
}

/// Work on the terminal sessions' shells, carried out by the runner's session threads.
/// `Write` bytes may be secrets the user typed: never log them (the effect is only ever named
/// "terminal").
pub enum TerminalEffect {
    Spawn {
        id: u64,
        cwd: PathBuf,
        rows: u16,
        cols: u16,
        /// The shell Preferences chose, resolved by the runner.
        shell: ShellSetting,
    },
    Write {
        id: u64,
        bytes: Vec<u8>,
    },
    Resize {
        id: u64,
        rows: u16,
        cols: u16,
    },
    Kill(u64),
    /// Forget session `id` (it ended or its tab closed); its thread ends once the shell has.
    Close(u64),
}

impl std::fmt::Debug for TerminalEffect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn { id, cwd, rows, cols, shell } => {
                write!(f, "Spawn {{ id: {id}, cwd: {cwd:?}, rows: {rows}, cols: {cols}, shell: {:?} }}", shell.choice)
            }
            Self::Write { id, bytes } => write!(f, "Write {{ id: {id}, bytes: <{} bytes> }}", bytes.len()),
            Self::Resize { id, rows, cols } => write!(f, "Resize {{ id: {id}, rows: {rows}, cols: {cols} }}"),
            Self::Kill(id) => write!(f, "Kill({id})"),
            Self::Close(id) => write!(f, "Close({id})"),
        }
    }
}
