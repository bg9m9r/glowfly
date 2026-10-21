//! Messages of the commit graph: multi-selection and cherry-pick, the ref filter and hover,
//! pinned branch colours, column widths, the commit search, and avatars (update/selection.rs,
//! update/graph_filter.rs, update/branch_colour.rs, update/graph_columns.rs, update/search.rs,
//! update/avatars.rs).

use crate::state::pick::PickOp;
use git2::Oid;
use glowfly_core::commit_index::CommitIndex;
use glowfly_core::repo::{Comparison, StackedCommit};
use glowfly_core::settings::{FilterOp, ResizableColumn};
use std::sync::Arc;

/// Selecting several graph rows, comparing commits (with each other or the working directory),
/// cherry-picking and reverting (update/selection.rs, update/compare_wip.rs, update/pick.rs).
pub enum SelectionMsg {
    /// Ctrl/Cmd-click on a graph row: add or remove the commit from the selection.
    ToggleCommit(Oid),
    /// Shift-click on a graph row: select every row from the anchor to this commit.
    SelectCommitRange(Oid),
    /// Esc: back to the one commit the multi-selection started from.
    CollapseSelection,
    /// Cherry-pick these commits onto HEAD, oldest first (the order is decided in update). Merge
    /// commits among them open the mainline dialog first.
    CherryPick(Vec<Oid>),
    /// Revert these commits in one job, newest first. Merge commits among them open the
    /// mainline dialog first.
    Revert(Vec<Oid>),
    /// The mainline dialog's submit: cherry-pick or revert `oids`, counting each merge's changes
    /// against parent `mainline` (1-based).
    PickWithMainline { op: PickOp, oids: Vec<Oid>, mainline: u32 },
    /// The files that differ from commit `from` to `to` (the selection's compare ends), with
    /// which of them are missing from the working tree.
    CompareLoaded { from: Oid, to: Oid, result: Result<Comparison, String> },
    /// The stacked lists of `oids` (the selection's `stack_oids` when asked): each commit's own
    /// changes, for a selection with no combined diff.
    StackedLoaded { oids: Vec<Oid>, result: Result<Vec<StackedCommit>, String> },
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
    /// Swap direction: compare the selection's ends (two commits, a line of them, or a commit
    /// and the working tree) the other way round.
    SwapCompare,
    /// Select every commit the graph shows (Cmd/Ctrl+A).
    SelectAll,
    /// The Go back to the focused commit key: select only the multi-selection's focus.
    BackToFocus,
    /// Try again after the comparison's file list (of a commit range or with the working
    /// directory) couldn't be read.
    RetryCompare,
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
            | Self::PickWithMainline { .. }
            | Self::CompareLoaded { .. }
            | Self::ToggleWip
            | Self::CompareWithWorkdir(..)
            | Self::CompareWithHead { .. }
            | Self::WorkdirCompareLoaded { .. }
            | Self::SwapCompare
            | Self::SelectAll
            | Self::BackToFocus
            | Self::RetryCompare => None,
            // Dropped like CompareLoaded: the tab asks again when it is refreshed.
            Self::StackedLoaded { .. } => None,
        }
    }
}

/// Hidden and soloed refs, the ref under the pointer (update/graph_filter.rs), pinned branch
/// colours (update/branch_colour.rs) and column widths (update/graph_columns.rs).
pub enum GraphMsg {
    /// Hide, show or solo a ref in the active repository's graph (saved per repository).
    GraphFilter(FilterOp),
    /// The ref under the pointer changed (`None`: no ref): highlight its branch's rows.
    HoverRef(Option<Oid>),
    /// A branch menu's Color submenu: pin the branch key `key` (a local branch and its remote
    /// copies) to hue `slot`, or with None (Automatic) let it pick its own (saved per repository).
    PinColour { key: String, slot: Option<u8> },
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
            Self::GraphFilter(..)
            | Self::HoverRef(..)
            | Self::PinColour { .. }
            | Self::ResizeColumn { .. }
            | Self::ResetColumnWidth(..) => None,
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
    /// Clear search (no matches): empty the query and the author filter; the bar stays open
    /// with its field focused and the filter as it was.
    Clear,
    /// The author picker is open and has no author list yet (sent each frame, like WantStats;
    /// handled once).
    WantAuthors,
    /// The author picker's choice: an author's email, or `None` for "Any author".
    PickAuthor(Option<String>),
    /// A report from Effect::SearchPaths request `request`: a chunk of the commits it looked at
    /// and which of them touch the path terms; `done` on its last chunk.
    PathsSearched { request: u64, result: Result<PathsChunk, String> },
    /// How far whole-history index build `build` has got (`IndexEffect::Build`): commits indexed
    /// and about how many there are, when git's commit-graph file says.
    IndexProgress { build: u64, indexed: usize, estimate: Option<usize> },
    /// Index build `build` finished, or failed with this error.
    IndexBuilt { build: u64, result: Result<Arc<CommitIndex>, String> },
    /// The walk positions of the index's matches for query request `request` (`IndexEffect::Query`).
    IndexQueried { request: u64, result: Result<Vec<u32>, String> },
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
            Self::Open
            | Self::QueryChanged
            | Self::Next
            | Self::ToggleFilter
            | Self::Close
            | Self::Clear
            | Self::WantAuthors
            | Self::PickAuthor(..) => None,
            // The tab asks again for what is unsearched when it is shown (tabs::activate →
            // search_paths::resume).
            Self::PathsSearched { .. } => None,
            // A parked tab keeps its finished index (update/history_index.rs `parked_built`).
            Self::IndexBuilt { build, result } => Some(crate::msg::Inactive::IndexBuilt { build, result }),
            // The status bar shows progress for the active tab only, and the tab asks for its
            // query's matches again when it is shown (tabs::activate → search_index::resume).
            Self::IndexProgress { .. } | Self::IndexQueried { .. } => None,
        }
    }
}

/// Author avatars: requests from the views and fetched images (update/avatars.rs).
pub enum AvatarMsg {
    /// Views asked for these authors' avatars (lowercased emails, each asked for once).
    WantAvatars(Vec<String>),
    /// Fetched avatars; `None` when an author has none (the view shows initials).
    AvatarsLoaded(Vec<(String, Option<glowfly_core::avatars::Image>)>),
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
