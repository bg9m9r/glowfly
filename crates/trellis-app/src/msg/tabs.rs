//! Messages of the tab bar (update/tabs.rs).

use crate::state::tabs::TabId;
use std::path::PathBuf;
use trellis_core::repo::CommonDir;

/// Opening, switching, moving, closing, renaming and restoring tabs (update/tabs.rs; closing
/// and reopening in update/tabs_close.rs, labels in update/tab_label.rs).
pub enum TabsMsg {
    NewTab,
    SelectTab(usize),
    /// Select the tab this many places to the right (negative: left), wrapping around.
    CycleTab(isize),
    /// Close tab `i`; `usize::MAX` closes the active tab. Asks first when that loses work.
    CloseTab(usize),
    ReopenClosedTab,
    /// Reopen this tab from the recently closed list, wherever it is in the list.
    ReopenClosed(PathBuf),
    /// Move tab `from` toward slot `to`; with grouping on, a drop outside its group moves the
    /// whole group (`tab_groups::move_order`). The active tab stays active.
    MoveTab {
        from: usize,
        to: usize,
    },
    /// Move the active tab one place left (-1) or right (1), following the groups like a drop.
    MoveActiveTab(isize),
    /// Close every tab except tab `i`.
    CloseOtherTabs(usize),
    /// Close every tab to the right of tab `i`.
    CloseTabsToRight(usize),
    /// The user confirmed a close that loses work: close without asking again.
    CloseConfirmed(CloseRequest),
    /// Open the Rename tab dialog for tab `i`; `usize::MAX` is the active tab.
    RenameTabDialog(usize),
    /// Give a tab a label (`None` shows the folder name again).
    SetTabLabel {
        tab: TabId,
        label: Option<String>,
    },
    /// Tabs saved by the last session; only the active one is loaded now. `labels` has one per
    /// path, or is empty for none.
    RestoreTabs {
        paths: Vec<PathBuf>,
        labels: Vec<Option<String>>,
        active: usize,
    },
    /// Result of `Effect::ReadTabGroups`: the common dir of each restored tab's folder.
    GroupsRead(Vec<(PathBuf, Option<CommonDir>)>),
}

impl TabsMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::NewTab
            | Self::SelectTab(..)
            | Self::CycleTab(..)
            | Self::CloseTab(..)
            | Self::ReopenClosedTab
            | Self::ReopenClosed(..)
            | Self::MoveTab { .. }
            | Self::MoveActiveTab(..)
            | Self::CloseOtherTabs(..)
            | Self::CloseTabsToRight(..)
            | Self::CloseConfirmed(..)
            | Self::RenameTabDialog(..)
            | Self::SetTabLabel { .. }
            | Self::RestoreTabs { .. }
            | Self::GroupsRead(..) => None,
        }
    }
}

/// Which tabs a close request closes, named by the tab it came from: that tab, every other
/// tab, or the tabs to its right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseRequest {
    One(TabId),
    Others(TabId),
    Right(TabId),
}
