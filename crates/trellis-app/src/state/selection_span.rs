//! What a multi-selection of commits spans: a pair, one unbroken line of first-parent history
//! (whose changes combine into one diff), or neither.

use git2::Oid;
use trellis_core::repo::Snapshot;

/// Why three or more selected commits have no combined diff: they aren't one line of history.
pub const SPAN_GAPS: &str = "These commits aren't one unbroken line of history, so they have no combined diff. Select consecutive \
                             commits, each the parent of the one above it, to see their changes together.";

/// Why a line of commits from the root has no combined diff.
pub const SPAN_FROM_ROOT: &str = "This range starts at the repository's first commit, so there is no parent to compare from.";

/// The shape of a multi-selection.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Span {
    /// Exactly two commits, adjacent or not: the older (lower row) and the newer.
    Pair { older: Oid, newer: Oid },
    /// Three or more commits, each the first parent of the one above it: `base` is the oldest
    /// one's first parent, `newest` the top commit. Their changes together are base..newest.
    Range { base: Oid, newest: Oid },
    /// Three or more commits that aren't one first-parent line.
    Gaps,
    /// One first-parent line whose oldest commit has no parent.
    FromRoot,
}

impl Span {
    /// The ends of the comparison it shows, (from, to), before any swap.
    pub fn ends(self) -> Option<(Oid, Oid)> {
        match self {
            Span::Pair { older, newer } => Some((older, newer)),
            Span::Range { base, newest } => Some((base, newest)),
            Span::Gaps | Span::FromRoot => None,
        }
    }
}

/// The span of `oids` (graph rows, newest first, at least two).
pub fn span_of(oids: &[Oid], snap: &Snapshot) -> Span {
    let parent = |o: &Oid| snap.row_of.get(o).and_then(|&r| snap.commits[r].parents.first().copied());
    match oids {
        [] | [_] => Span::Gaps,
        [newer, older] => Span::Pair { older: *older, newer: *newer },
        _ => {
            if oids.windows(2).any(|w| parent(&w[0]) != Some(w[1])) {
                return Span::Gaps;
            }
            match oids.last().and_then(parent) {
                Some(base) => Span::Range { base, newest: oids[0] },
                None => Span::FromRoot,
            }
        }
    }
}
