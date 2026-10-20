//! Window-level actions from the menu bar and a drawn title bar (update/app.rs).

use super::Inactive;
use crate::state::window_chrome::WindowCmd;

#[derive(Clone, Debug, PartialEq)]
pub enum AppMsg {
    /// File > Quit: close the window like its title-bar close button (tabs and settings are
    /// already saved when they change).
    Quit,
    /// Help > About Trellis: open the About dialog.
    ShowAbout,
    /// A drawn title bar's window action: move, resize, minimise, maximise or restore.
    Window(WindowCmd),
}

impl AppMsg {
    /// UI intents, never results of background work: nothing to keep for an inactive tab.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::Quit | Self::ShowAbout | Self::Window(_) => None,
        }
    }
}
