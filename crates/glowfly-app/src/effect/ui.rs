//! Effects that need the window, carried out by the UI shell (`Effect::Ui`).

use super::EffectMeta;
#[cfg(test)]
use super::SandboxRule;
use crate::state::window_chrome::WindowCmd;
use std::path::{Path, PathBuf};

/// Effects that need the window, carried out by the UI shell.
#[derive(Debug, PartialEq)]
pub enum UiEffect {
    CopyText(String),
    SetTitle(String),
    /// A credential prompt opened: flash the taskbar entry if the window isn't focused.
    RequestAttention,
    /// A link arrived: un-minimize, raise and focus the window (or flash it when that fails).
    FocusWindow,
    /// Quit went ahead: close the main window.
    CloseWindow,
    /// Keep the window open after the OS asked to close it (the quit question opened instead).
    CancelClose,
    /// Read the clipboard into an Event::Paste for the focused widget on the next frame.
    RequestPaste,
    /// Move, resize, minimise or maximise the window, or show the system menu (the drawn
    /// title bar), or enter or leave full screen.
    Window(WindowCmd),
}

impl EffectMeta for UiEffect {
    fn name(&self) -> &'static str {
        "ui"
    }

    fn scope(&self) -> Option<&PathBuf> {
        None
    }

    /// Handled by the shell; it never reaches a worker.
    fn timed(&self) -> bool {
        false
    }

    fn queue(&self) -> Option<&Path> {
        None
    }

    #[cfg(test)]
    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Run
    }
}
