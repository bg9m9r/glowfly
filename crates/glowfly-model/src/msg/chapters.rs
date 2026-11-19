//! Messages of the Chapters history view: its switch, and expanding and selecting chapters
//! (update/chapters.rs).

use git2::Oid;
use glowfly_core::settings::HistoryMode;

/// The Chapters view's intents; all are for the active tab.
pub enum ChaptersMsg {
    /// The view bar's switch: show the graph or the chapters, saved for this repository.
    SetMode(HistoryMode),
    /// The chevron, a double-click or Left and Right: expand or collapse the chapter with this id.
    Toggle(Oid),
    /// A click or key on a chapter row: select what it stands for.
    Select(Oid),
}

impl ChaptersMsg {
    /// What this message still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::SetMode(..) | Self::Toggle(..) | Self::Select(..) => None,
        }
    }
}
