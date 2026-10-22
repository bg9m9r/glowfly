//! Messages of toasts and desktop notifications (update/notify.rs).

use super::Inactive;
use std::path::PathBuf;

/// Holding and closing toasts, and a click on a desktop notification.
#[derive(Clone, Debug, PartialEq)]
pub enum NotifyMsg {
    /// The pointer moved onto toast `id`, or (None) off every toast. ui/toasts.rs sends it only
    /// when that differs from `st.held_toast()`.
    HoldToast(Option<u64>),
    /// A toast's x: removes it without running its action.
    CloseToast(u64),
    /// The OS reported a click on a desktop notification about `workdir` (runner/notify.rs):
    /// brings the window forward and shows that repository.
    DesktopClicked { workdir: Option<PathBuf> },
}

impl NotifyMsg {
    /// None: none of them is a result for one tab.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::HoldToast(_) | Self::CloseToast(_) | Self::DesktopClicked { .. } => None,
        }
    }
}
