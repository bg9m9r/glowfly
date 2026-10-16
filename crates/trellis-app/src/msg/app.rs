//! Window-level actions from the menu bar (update/app.rs).

use super::Inactive;

#[derive(Clone, Debug, PartialEq)]
pub enum AppMsg {
    /// File > Quit: close the window like its title-bar close button (tabs and settings are
    /// already saved when they change).
    Quit,
    /// Help > About Trellis: open the About dialog.
    ShowAbout,
}

impl AppMsg {
    /// UI intents, never results of background work: nothing to keep for an inactive tab.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::Quit | Self::ShowAbout => None,
        }
    }
}
