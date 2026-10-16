//! Messages for the window's zoom and for showing or hiding its panels (update/layout.rs).

use super::Inactive;
use crate::state::layout::Panel;

#[derive(Clone, Debug, PartialEq)]
pub enum LayoutMsg {
    /// One zoom step larger.
    ZoomIn,
    /// One zoom step smaller.
    ZoomOut,
    /// Back to 100%.
    ZoomReset,
    /// Show the panel if hidden, else hide it (keys).
    TogglePanel(Panel),
    /// Show or hide the panel (palette, status bar).
    SetPanel { panel: Panel, shown: bool },
}

impl LayoutMsg {
    /// UI intents, never results of background work: nothing to keep for an inactive tab.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::ZoomIn | Self::ZoomOut | Self::ZoomReset | Self::TogglePanel(_) | Self::SetPanel { .. } => None,
        }
    }
}
