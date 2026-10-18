//! Messages of the commit graph: multi-selection and cherry-pick, the ref filter and hover,
//! column widths, the commit search, and avatars (update/selection.rs, update/graph_filter.rs,
//! update/graph_columns.rs, update/search.rs, update/avatars.rs).

use crate::state::pick::PickOp;
use git2::Oid;
use trellis_core::repo::Comparison;
use trellis_core::settings::{FilterOp, ResizableColumn};

/// Selecting several graph rows, comparing commits (with each other or the working directory),
/// cherry-picking and reverting (update/selection.rs, update/compare_wip.rs, update/pick.rs).
pub enum SelectionMsg {
    /// Ctrl/Cmd-click on a graph row: add or remove the commit from the selection.
    ToggleCommit(Oid),
    /// Shift-click on a graph row: select every row from the anchor to this commit.
    SelectCommitRange(Oid),
    /// Esc: back to the one commit the multi-selection started from.
    CollapseSelection,
    /// Cherry-pick these commits onto HEAD, oldest first (the order is decided in update). One
    /// merge commit opens the mainline dialog first.
    CherryPick(Vec<Oid>),
    /// Revert these commits in one job, newest first. One merge commit opens the mainline
    /// dialog first.
    Revert(Vec<Oid>),
    /// The mainline dialog's submit: cherry-pick or revert merge `oid` against parent
    /// `mainline` (1-based).
    PickMerge { op: PickOp, oid: Oid, mainline: u32 },
    /// The files that differ from commit `from` to `to` (the selection's compare ends), with
    /// which of them are missing from the working tree.
    CompareLoaded { from: Oid, to: Oid, result: Result<Comparison, String> },
    /// Ctrl/Cmd-click on the WIP row: compare the one selected commit with the working
    /// directory, or stop comparing.
    ToggleWip,
    /// The commit menu's Compare with working directory.
    CompareWithWorkdir(Oid),
    /// The remote branch menu's Compare with the current branch: select HEAD's commit and `oid`
    /// (named `name` in messages) as a pair.
    CompareWithHead { oid: Oid, name: String },
    /// The files that differ from commit `oid` to the working tree, read for request
    /// `generation`.
    WorkdirCompareLoaded { oid: Oid, generation: u64, result: Result<Comparison, String> },
    /// Swap direction: compare the selection's ends the other way round.
    SwapCompare,
    /// Select every commit the graph shows (Cmd/Ctrl+A).
    SelectAll,
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
            | Self::Revert(..)
            | Self::PickMerge { .. }
            | Self::CompareLoaded { .. }
            | Self::ToggleWip
            | Self::CompareWithWorkdir(..)
            | Self::CompareWithHead { .. }
            | Self::WorkdirCompareLoaded { .. }
            | Self::SwapCompare
            | Self::SelectAll => None,
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
    /// Edit > Find commits and the palette's Search commits: close an open diff, show the search
    /// bar and focus its field.
    Open,
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
    /// A report from Effect::SearchPaths request `request`: a chunk of the commits it looked at
    /// and which of them touch the path terms; `done` on its last chunk.
    PathsSearched { request: u64, result: Result<PathsChunk, String> },
}

/// Part of the answer to a path search (`SearchMsg::PathsSearched`).
pub struct PathsChunk {
    /// The commits looked at in this chunk, newest first.
    pub searched: Vec<Oid>,
    /// Those of them that changed a path matching a term.
    pub touching: Vec<Oid>,
    /// This is the request's last chunk.
    pub done: bool,
}

impl SearchMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            // All are intents for the active tab.
            Self::Open | Self::QueryChanged | Self::Next | Self::ToggleFilter | Self::Close | Self::WantAuthors | Self::PickAuthor(..) => {
                None
            }
            // The tab asks again for what is unsearched when it is shown (tabs::activate →
            // search_paths::resume).
            Self::PathsSearched { .. } => None,
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
