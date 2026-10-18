//! Messages of the diff view, the conflict view and the file history (update/diff.rs,
//! update/highlight.rs, update/conflict.rs, update/history.rs).

use crate::effect::ConflictRead;
use crate::state::history::{FileTextData, HistoryPane};
use crate::state::lines::LineClick;
use git2::Oid;
use std::sync::Arc;
use trellis_core::conflict_restore::SavedPicks;
use trellis_core::diff_rows::GapText;
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
    /// A click on a whole-file "N unchanged lines" row, or on a hunk header's gap control in the
    /// hunk view: show the unchanged lines of the run with this key in place.
    ExpandUnchanged(u32),
    /// A click on the fold-back control on the first line of an opened run (a whole-file fold or
    /// a gap between hunks): fold the run with this key back.
    CollapseUnchanged(u32),
    /// The keyboard's Show unchanged lines: open the first closed run at or below the top row.
    OpenFoldAtTop,
    /// The keyboard's Fold back unchanged lines: close the opened run at the top.
    CloseFoldAtTop,
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
    /// A press on changed line `line` of hunk `hunk` became a drag (`how` from the modifiers
    /// held).
    StartLineDrag {
        hunk: usize,
        line: usize,
        how: LineClick,
    },
    /// During a line drag, the pointer is over line `line` of hunk `hunk` (0 for a hunk header
    /// or the gap rows above it). The selection runs from the drag's start to there, within the
    /// drag's hunk.
    DragDiffLine {
        hunk: usize,
        line: usize,
    },
    /// The button was released, so the selection stays and the drag ends.
    EndLineDrag,
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
    /// The unchanged lines between the hunks of the diff shown at `generation`, read
    /// (`Effect::LoadDiffGaps`).
    GapsLoaded {
        target: DiffTarget,
        generation: u64,
        result: Result<Arc<GapText>, String>,
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
            | Self::OpenFoldAtTop
            | Self::CloseFoldAtTop
            | Self::GapsLoaded { .. }
            | Self::ExpandAllUnchanged
            | Self::CollapseAllUnchanged
            | Self::SelectDiffLine { .. }
            | Self::StartLineDrag { .. }
            | Self::DragDiffLine { .. }
            | Self::EndLineDrag
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
    /// Drop every edit of the clean runs between the conflicts.
    ConflictDiscardCleanEdits,
    /// Open (`open`) or fold again the run of unchanged lines of clean segment `seg`.
    ConflictFold {
        seg: usize,
        open: bool,
    },
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
    /// The conflict read from the index, and the picks its working file holds (ignored when
    /// the read failed).
    ConflictLoaded {
        path: String,
        generation: u64,
        result: Result<ConflictFile, String>,
        saved: Option<SavedPicks>,
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
            | Self::ConflictDiscardCleanEdits
            | Self::ConflictFold { .. }
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
    /// The File view's Find field changed: search again and go to the first match below the
    /// top line.
    FileFindChanged,
    /// To the next (1) or previous (-1) match in the File view, wrapping round.
    FileFindStep(isize),
    /// Focus the File view's Find field (the Search key in the File pane).
    FocusFileFind,
    /// Go to the line typed in the File view's Line field.
    FileGoToLine,
    /// Select the next (1) or previous (-1) file of the selected folder entry (Next/Previous
    /// change in the File pane); nothing wraps.
    StepHistoryFile(isize),
    /// Continue the walk past the commit limit.
    MoreFileHistory,
    /// "View on graph": close the history and select the commit.
    ShowInGraph(Oid),
    FileHistoryLoaded {
        path: String,
        generation: u64,
        result: Result<FileHistory, String>,
    },
    /// The File view's text of `path` as of `oid` (with `before`, as of its parent)
    /// (`Effect::LoadFileText`).
    FileTextLoaded {
        oid: Oid,
        path: String,
        before: bool,
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
            | Self::FileFindChanged
            | Self::FileFindStep(..)
            | Self::FocusFileFind
            | Self::FileGoToLine
            | Self::StepHistoryFile(..)
            | Self::MoreFileHistory
            | Self::ShowInGraph(..)
            | Self::FileHistoryLoaded { .. }
            | Self::FileTextLoaded { .. } => None,
        }
    }
}
