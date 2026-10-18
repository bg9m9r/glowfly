//! Messages of the diff view, the conflict view and the file history (update/diff.rs,
//! update/highlight.rs, update/conflict.rs, update/history.rs).

use crate::effect::ConflictRead;
use crate::state::history::{FileTextData, HistoryPane};
use crate::state::lines::LineClick;
use git2::Oid;
use std::sync::Arc;
use trellis_core::highlight::DiffHighlight;
use trellis_core::repo::{ConflictFile, ConflictSummary, DiffTarget, FileHistory, Side};
use trellis_core::settings::{DiffLayout, DiffMode};

/// The open diff: layout and mode, stepping between changes, folds, line selection, and the
/// background highlighting (update/diff.rs, update/highlight.rs).
// `DiffHighlighted` is large, as it was when it was a `Msg` variant (each `Msg` is that size
// anyway); boxing it would only add an allocation per result.
#[allow(clippy::large_enum_variant)]
pub enum DiffMsg {
    /// Switch the diff between one column and side by side (saved in settings).
    SetDiffLayout(DiffLayout),
    /// Switch the diff between hunks only and the whole file (saved in settings).
    SetDiffMode(DiffMode),
    /// Diff header toggle: hide whitespace-only changes and reload the open diff.
    SetIgnoreWhitespace(bool),
    /// Diff header toggle: wrap long lines to the viewer's width (saved in settings).
    SetWordWrap(bool),
    /// Scroll the diff to the next (1) or previous (-1) change; at either end, open the next or
    /// previous changed file of the list the diff came from (on its first or last change).
    JumpToChange(isize),
    /// A click on a whole-file "N unchanged lines" row: show the lines of the fold with this key
    /// in place.
    ExpandUnchanged(u32),
    /// A click on the fold-back control on the first line of an opened whole-file run: fold
    /// the run with this key back into one row.
    CollapseUnchanged(u32),
    /// The diff header's Expand all: open every fold of the whole file on screen.
    ExpandAllUnchanged,
    /// The diff header's Collapse all: close every opened fold.
    CollapseAllUnchanged,
    /// A click on line `line` of hunk `hunk` in the open diff (line-level staging).
    SelectDiffLine {
        hunk: usize,
        line: usize,
        how: LineClick,
    },
    ClearLineSelection,
    /// A HighlightDiff finished: exact spans for the hunks and/or the whole file of the diff shown
    /// at `generation`, each only where it was pending. Applied only while that diff is still on
    /// screen.
    DiffHighlighted {
        target: DiffTarget,
        generation: u64,
        hunks: Option<DiffHighlight>,
        whole: Option<DiffHighlight>,
    },
}

impl DiffMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::SetDiffLayout(..)
            | Self::SetDiffMode(..)
            | Self::SetIgnoreWhitespace(..)
            | Self::SetWordWrap(..)
            | Self::JumpToChange(..)
            | Self::ExpandUnchanged(..)
            | Self::CollapseUnchanged(..)
            | Self::ExpandAllUnchanged
            | Self::CollapseAllUnchanged
            | Self::SelectDiffLine { .. }
            | Self::ClearLineSelection
            | Self::DiffHighlighted { .. } => None,
        }
    }
}

/// The conflict view: choosing sides chunk by chunk, saving, and resolving with one side
/// (update/conflict.rs).
pub enum ConflictMsg {
    OpenConflict(String),
    CloseConflict,
    /// Tick or untick a side of chunk `chunk` in the open conflict.
    ConflictToggle {
        chunk: usize,
        side: Side,
    },
    /// Tick or untick line `line` of `side` in chunk `chunk`: a ticked line goes last.
    ConflictToggleLine {
        chunk: usize,
        side: Side,
        line: usize,
    },
    ConflictTakeAll(Side),
    ConflictReset,
    /// Show (true) or hide the output as editable text.
    ConflictEditOutput(bool),
    /// The view changed the output editor's text.
    ConflictOutputEdited,
    /// Drop the text typed for chunk `k`, bringing its picks back.
    ConflictDiscardEdit(usize),
    /// Make the next (1) or previous (-1) chunk current.
    ConflictStep(isize),
    /// Make chunk `i` current (a click on it) without scrolling.
    ConflictSelect(usize),
    /// Write the output to the file and stage it.
    SaveConflict,
    /// Write the output to the file with markers for the chunks not resolved yet, leaving it
    /// conflicted.
    SaveConflictKeepMarkers,
    /// Binary or deleted files: resolve with one side's whole version.
    ResolveWithSide {
        path: String,
        side: Side,
    },
    ConflictLoaded {
        path: String,
        generation: u64,
        result: Result<ConflictFile, String>,
    },
    /// Mark conflicted files resolved: their working files are checked for conflict markers
    /// first, and any left are confirmed before staging.
    MarkResolved(Vec<String>),
    /// Mark resolved after the markers warning: stage without checking again.
    MarkResolvedConfirmed(Vec<String>),
    /// Bring back the conflict of resolved files (the view has confirmed).
    MarkConflicted(Vec<String>),
    /// The working files of `paths` read for markers (and the resolved files, for a summary).
    ConflictFilesRead {
        purpose: ConflictRead,
        paths: Vec<String>,
        result: Result<ConflictSummary, String>,
    },
}

impl ConflictMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::OpenConflict(..)
            | Self::CloseConflict
            | Self::ConflictToggle { .. }
            | Self::ConflictToggleLine { .. }
            | Self::ConflictEditOutput(..)
            | Self::ConflictOutputEdited
            | Self::ConflictDiscardEdit(..)
            | Self::SaveConflictKeepMarkers
            | Self::ConflictTakeAll(..)
            | Self::ConflictReset
            | Self::ConflictStep(..)
            | Self::ConflictSelect(..)
            | Self::SaveConflict
            | Self::ResolveWithSide { .. }
            | Self::ConflictLoaded { .. }
            | Self::MarkResolved(..)
            | Self::MarkResolvedConfirmed(..)
            | Self::MarkConflicted(..)
            | Self::ConflictFilesRead { result: Ok(_), .. } => None,
            Self::ConflictFilesRead { result: Err(e), .. } => Some(crate::msg::Inactive::Log(e)),
        }
    }
}

/// The history of one file or folder (update/history.rs, update/history_view.rs).
pub enum HistoryMsg {
    /// The commits that changed `path`, from HEAD (`at: None`) or as seen from commit `at`.
    OpenFileHistory {
        path: String,
        at: Option<Oid>,
    },
    /// The commits that changed anything under the folder `path` (no trailing '/').
    OpenFolderHistory {
        path: String,
        at: Option<Oid>,
    },
    CloseFileHistory,
    /// Select entry `i` of the open file history and show its diff.
    SelectHistoryEntry(usize),
    /// Select file `i` of the selected folder entry and show its diff (or text).
    SelectHistoryFile(usize),
    /// The header's Diff / File toggle.
    SetHistoryPane(HistoryPane),
    /// Continue the walk past the commit limit.
    MoreFileHistory,
    /// "View on graph": close the history and select the commit.
    ShowInGraph(Oid),
    FileHistoryLoaded {
        path: String,
        generation: u64,
        result: Result<FileHistory, String>,
    },
    /// The File view's text of `path` as of `oid` (`Effect::LoadFileText`).
    FileTextLoaded {
        oid: Oid,
        path: String,
        result: Result<Arc<FileTextData>, String>,
    },
}

impl HistoryMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`); `None`
    /// drops it.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::OpenFileHistory { .. }
            | Self::OpenFolderHistory { .. }
            | Self::CloseFileHistory
            | Self::SelectHistoryEntry(..)
            | Self::SelectHistoryFile(..)
            | Self::SetHistoryPane(..)
            | Self::MoreFileHistory
            | Self::ShowInGraph(..)
            | Self::FileHistoryLoaded { .. }
            | Self::FileTextLoaded { .. } => None,
        }
    }
}
