//! Messages of the tab bar (update/tabs.rs).

use std::path::PathBuf;
use trellis_core::repo::CommonDir;

/// Opening, switching, moving, closing and restoring tabs (update/tabs.rs).
pub enum TabsMsg {
    NewTab,
    SelectTab(usize),
    /// Select the tab this many places to the right (negative: left), wrapping around.
    CycleTab(isize),
    /// Close tab `i`; `usize::MAX` closes the active tab.
    CloseTab(usize),
    ReopenClosedTab,
    /// Move tab `from` toward slot `to`; with grouping on, a drop outside its group moves the
    /// whole group (`tab_groups::move_order`). The active tab stays active.
    MoveTab {
        from: usize,
        to: usize,
    },
    /// Close every tab except tab `i`.
    CloseOtherTabs(usize),
    /// Close every tab to the right of tab `i`.
    CloseTabsToRight(usize),
    /// Tabs saved by the last session; only the active one is loaded now.
    RestoreTabs {
        paths: Vec<PathBuf>,
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
            | Self::MoveTab { .. }
            | Self::CloseOtherTabs(..)
            | Self::CloseTabsToRight(..)
            | Self::RestoreTabs { .. }
            | Self::GroupsRead(..) => None,
        }
    }
}
