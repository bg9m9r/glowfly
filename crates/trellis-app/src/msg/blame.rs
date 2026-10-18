//! Messages of the blame view (update/blame.rs, with stepping in `blame_nav.rs` and finding in
//! `blame_find.rs`).

use super::Inactive;
use crate::state::blame::BlameData;
use git2::Oid;
use trellis_core::settings::BlameMoves;

/// The blame view (update/blame.rs).
pub enum BlameMsg {
    /// Blame `path` at HEAD (`at: None`) or as of commit `at`.
    Open {
        path: String,
        at: Option<Oid>,
    },
    Close,
    /// A run's gutter was clicked: select its commit on the graph and close the view.
    SelectCommit(Oid),
    Loaded {
        path: String,
        generation: u64,
        result: Result<Box<BlameData>, String>,
    },
    /// Back to the blame before the last "Blame before this commit".
    Back,
    /// Blame the version just before the commit of line `line` (0-based), at its path there.
    BlameParent {
        line: usize,
    },
    /// Blame again looking past this commit.
    SkipCommit(Oid),
    /// Stop skipping commits.
    ClearSkipped,
    SetIgnoreWhitespace(bool),
    SetMoves(BlameMoves),
    /// The header's Wrap lines toggle (saved in settings).
    SetWrapLines(bool),
    /// Close blame and open the file's history from the blamed commit, selecting `select` (else
    /// the blamed commit).
    OpenHistory {
        select: Option<Oid>,
    },
    /// The find field's text changed.
    FindChanged,
    /// To the next (1) or previous (-1) match, wrapping round.
    FindStep(isize),
    /// Put the keyboard in the find field.
    FocusFind,
    /// Go to the line typed in the Line field.
    GoToLine,
    /// To the first line of the next (1) or previous (-1) run.
    StepRun(isize),
}

impl BlameMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`): nothing.
    /// The view state is parked with its tab, and a load in flight is asked for again when its
    /// tab returns. A HEAD move while the tab was parked is picked up by the refresh that runs
    /// when the tab returns.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::Open { .. }
            | Self::Close
            | Self::SelectCommit(..)
            | Self::Loaded { .. }
            | Self::Back
            | Self::BlameParent { .. }
            | Self::SkipCommit(..)
            | Self::ClearSkipped
            | Self::SetIgnoreWhitespace(..)
            | Self::SetMoves(..)
            | Self::SetWrapLines(..)
            | Self::OpenHistory { .. }
            | Self::FindChanged
            | Self::FindStep(..)
            | Self::FocusFind
            | Self::GoToLine
            | Self::StepRun(..) => None,
        }
    }
}
