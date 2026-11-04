//! Window-level actions from the menu bar and a drawn title bar, and the system's accessibility
//! settings the shell read (update/app.rs).

use super::Inactive;
use crate::state::system_a11y::SystemA11y;
use crate::state::window_chrome::WindowCmd;

#[derive(Clone, Debug, PartialEq)]
pub enum AppMsg {
    /// File > Quit, the Quit key or a drawn title bar's Close: close the window (tabs and
    /// settings are already saved when they change), asking first while git work runs.
    Quit,
    /// The OS asked to close the window (Alt+F4, the system frame's close button, the taskbar,
    /// the window manager): the shell saw `close_requested` this frame.
    CloseRequested,
    /// The quit question's Quit anyway.
    QuitConfirmed,
    /// Help > About Glowfly: open the About dialog.
    ShowAbout,
    /// About's Third-party notices…: open the notices dialog in About's place.
    ShowNotices,
    /// A drawn title bar's window action: move, resize, minimise, maximise or restore, or
    /// show the window's system menu.
    Window(WindowCmd),
    /// The system's reduce-motion, high-contrast and text-size settings, read by the shell at
    /// start, when the system reports a settings change and when the window regains the focus.
    SystemA11y(SystemA11y),
}

impl AppMsg {
    /// UI intents, never results of background work: nothing to keep for an inactive tab.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::Quit | Self::CloseRequested | Self::QuitConfirmed | Self::ShowAbout | Self::ShowNotices | Self::Window(_) => None,
            // The window's, whichever tab is active.
            Self::SystemA11y(_) => None,
        }
    }
}
