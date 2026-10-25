//! Messages for the window's zoom, full screen, showing or hiding its panels and how the
//! changed-file lists show their files (update/layout.rs).

use super::Inactive;
use crate::state::file_folds::FileListId;
use crate::state::layout::Panel;
use crate::state::op_steps::OpStep;
use glowfly_core::settings::{FileListMode, SizedPanel};

#[derive(Clone, Debug, PartialEq)]
pub enum LayoutMsg {
    /// One zoom step larger.
    ZoomIn,
    /// One zoom step smaller.
    ZoomOut,
    /// Back to 100%.
    ZoomReset,
    /// Enter full screen, or leave it (key, palette, View menu); the window's state, never saved.
    ToggleFullscreen,
    /// Show the panel if hidden, else hide it (keys).
    TogglePanel(Panel),
    /// Show or hide the panel (palette, status bar).
    SetPanel { panel: Panel, shown: bool },
    /// A drag of the drawer's edge ended at `size` px (saved within its limits).
    PanelSize { panel: SizedPanel, size: f32 },
    /// Draw the commit panel in full for the tool view open now (the rail's expand control).
    ExpandDetails,
    /// Open a folded step of the WIP panel's operation steps, or fold an open one.
    ToggleOpStep(OpStep),
    /// The list header's List / Tree toggle (saved).
    SetFileListMode(FileListMode),
    /// Collapse or expand a folder row of a changed-file list's tree.
    ToggleFolder { list: FileListId, path: String },
}

impl LayoutMsg {
    /// UI intents, never results of background work: nothing to keep for an inactive tab.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::ZoomIn
            | Self::ZoomOut
            | Self::ZoomReset
            | Self::ToggleFullscreen
            | Self::TogglePanel(_)
            | Self::SetPanel { .. }
            | Self::PanelSize { .. }
            | Self::ExpandDetails
            | Self::ToggleOpStep(_)
            | Self::SetFileListMode(_)
            | Self::ToggleFolder { .. } => None,
        }
    }
}
