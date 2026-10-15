//! Messages of the commit graph: multi-selection and cherry-pick, the ref filter and hover,
//! column widths, the commit search, and avatars (update/selection.rs, update/graph_filter.rs,
//! update/graph_columns.rs, update/search.rs, update/avatars.rs).

use git2::Oid;
use trellis_core::repo::Comparison;
use trellis_core::settings::{FilterOp, ResizableColumn};

/// Selecting several graph rows, comparing two commits and cherry-picking (update/selection.rs).
pub enum SelectionMsg {
    /// Ctrl/Cmd-click on a graph row: add or remove the commit from the selection.
    ToggleCommit(Oid),
    /// Shift-click on a graph row: select every row from the anchor to this commit.
    SelectCommitRange(Oid),
    /// Esc: back to the one commit the multi-selection started from.
    CollapseSelection,
    /// Cherry-pick these commits onto HEAD, oldest first (the order is decided in update).
    CherryPick(Vec<Oid>),
    /// The files that differ from commit `from` (older) to `to` (newer), with which of them are
    /// missing from the working tree.
    CompareLoaded { from: Oid, to: Oid, result: Result<Comparison, String> },
}

impl SelectionMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::ToggleCommit(..)
            | Self::SelectCommitRange(..)
            | Self::CollapseSelection
            | Self::CherryPick(..)
            | Self::CompareLoaded { .. } => None,
        }
    }
}

/// Hidden and soloed refs, the ref under the pointer (update/graph_filter.rs), and column
/// widths (update/graph_columns.rs).
pub enum GraphMsg {
    /// Hide, show or solo a ref in the active repository's graph (saved per repository).
    GraphFilter(FilterOp),
    /// The ref under the pointer changed (`None`: no ref): highlight its branch's rows.
    HoverRef(Option<Oid>),
    /// A header divider drag ended: store this width (clamped) and save settings.
    ResizeColumn { col: ResizableColumn, width: f32 },
    /// A header divider was double-clicked: restore that column's default width.
    ResetColumnWidth(ResizableColumn),
}

impl GraphMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::GraphFilter(..) | Self::HoverRef(..) | Self::ResizeColumn { .. } | Self::ResetColumnWidth(..) => None,
        }
    }
}

/// The graph's search bar: the query, stepping through matches, the filter and the author
/// picker (update/search.rs).
pub enum SearchMsg {
    /// The view edited the query (`ViewState::search`).
    QueryChanged,
    /// Enter in the query field: select the next match (wrapping).
    Next,
    /// The Filter toggle or Ctrl+Shift+F: show only the matching commits, or all again.
    ToggleFilter,
    /// The bar's Close button: hide it, clear the query and turn the filter off.
    Close,
    /// The author picker is open and has no author list yet (sent each frame, like WantStats;
    /// handled once).
    WantAuthors,
    /// The author picker's choice: an author's email, or `None` for "Any author".
    PickAuthor(Option<String>),
}

impl SearchMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            // All are intents for the active tab.
            Self::QueryChanged | Self::Next | Self::ToggleFilter | Self::Close | Self::WantAuthors | Self::PickAuthor(..) => None,
        }
    }
}

/// Author avatars: requests from the views and fetched images (update/avatars.rs).
pub enum AvatarMsg {
    /// Views asked for these authors' avatars (lowercased emails, each asked for once).
    WantAvatars(Vec<String>),
    /// Fetched avatars; `None` when an author has none (the view shows initials).
    AvatarsLoaded(Vec<(String, Option<trellis_core::avatars::Image>)>),
}

impl AvatarMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::WantAvatars(..) | Self::AvatarsLoaded(..) => None,
        }
    }
}
