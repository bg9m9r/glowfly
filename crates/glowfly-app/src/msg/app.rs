//! Window-level actions from the menu bar and a drawn title bar (update/app.rs).

use super::Inactive;
use crate::state::window_chrome::WindowCmd;

#[derive(Clone, Debug, PartialEq)]
pub enum AppMsg {
    /// File > Quit: close the window like its title-bar close button (tabs and settings are
    /// already saved when they change).
    Quit,
    /// Help > About Glowfly: open the About dialog.
    ShowAbout,
    /// About's Third-party notices…: open the notices dialog in About's place.
    ShowNotices,
    /// A drawn title bar's window action: move, resize, minimise, maximise or restore, or
    /// show the window's system menu.
    Window(WindowCmd),
}

impl AppMsg {
    /// UI intents, never results of background work: nothing to keep for an inactive tab.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::Quit | Self::ShowAbout | Self::ShowNotices | Self::Window(_) => None,
        }
    }
}
