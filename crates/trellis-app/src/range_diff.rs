//! Messages and the effect of the Range-diff view: two versions of a line of commits compared
//! commit by commit with `git range-diff` (state/range_diff.rs, update/range_diff.rs,
//! runner/range_diff.rs, the view in ui/range_diff/). It is opened from the branch menus (the
//! version before the last rewrite), the journal's preview (a state and now) and the
//! multi-commit menu (two commits).

use git2::Oid;
use std::path::PathBuf;
use trellis_core::range_diff::{Ends, Outcome};

#[derive(Clone, Debug, PartialEq)]
pub enum RangeDiffMsg {
    /// Compare commit `old` with commit `new`: the journal's Range-diff with now (`label` is the
    /// state's subject and `at` its time) or the multi-commit menu (neither).
    Open { old: Oid, new: Oid, label: Option<String>, at: Option<i64> },
    /// Compare a ref (a full name, `refs/heads/x` or `refs/remotes/r/x`) before its last
    /// rewrite with its tip now: the branch menus.
    OpenRewrite(String),
    /// Back to what was underneath (Close, Esc).
    Close,
    /// Try again, after a failure or once the repository is trusted.
    Retry,
    /// A pair in the list was clicked.
    Select(usize),
    /// The comparison for request `generation` finished.
    Loaded { generation: u64, result: Result<Box<Outcome>, String> },
}

impl RangeDiffMsg {
    /// Nothing is kept for an inactive tab: the intents only make sense in the active one, and a
    /// load in flight is asked for again when the tab returns.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::Open { .. } | Self::OpenRewrite(_) | Self::Close | Self::Retry | Self::Select(_) | Self::Loaded { .. } => None,
        }
    }
}

/// One comparison (runner/range_diff.rs): read only; scoped to the repository it reads.
#[derive(Debug)]
pub struct RangeDiffJob {
    pub workdir: PathBuf,
    pub ends: Ends,
    /// The branch that cuts both ranges (name and tip), when one was found.
    pub mainline: Option<(String, Oid)>,
    pub generation: u64,
}
