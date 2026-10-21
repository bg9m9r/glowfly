//! What a multi-selection of commits spans: a pair, one unbroken line of first-parent history
//! (whose changes combine into one diff, from the empty tree when it reaches the first commit),
//! or neither.

use git2::Oid;
use std::collections::HashSet;
use trellis_core::repo::{self, Snapshot};

/// The shape of a multi-selection.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Span {
    /// Exactly two commits, adjacent or not: the older (lower row) and the newer.
    Pair { older: Oid, newer: Oid },
    /// Three or more commits, each the first parent of the one above it: `base` is the oldest
    /// one's first parent (the empty tree when it is the repository's first commit), `newest`
    /// the top commit. Their changes together are base..newest.
    Range { base: Oid, newest: Oid },
    /// Three or more commits that aren't one first-parent line. `gap` is the newest selected
    /// commit, not the oldest and not a root, whose first parent isn't selected; `None` when
    /// every first parent is selected but the commits branch apart.
    Gaps { gap: Option<Oid> },
    /// One first-parent line whose oldest commit is a shallow clone's boundary: what came
    /// before it wasn't downloaded.
    ShallowEnd,
}

impl Span {
    /// The ends of the comparison it shows, (from, to), before any swap.
    pub fn ends(self) -> Option<(Oid, Oid)> {
        match self {
            Span::Pair { older, newer } => Some((older, newer)),
            Span::Range { base, newest } => Some((base, newest)),
            Span::Gaps { .. } | Span::ShallowEnd => None,
        }
    }
}

/// The span of `oids` (graph rows, newest first, at least two).
pub fn span_of(oids: &[Oid], snap: &Snapshot) -> Span {
    let parent = |o: &Oid| snap.row_of.get(o).and_then(|&r| snap.commits[r].parents.first().copied());
    match oids {
        [] | [_] => Span::Gaps { gap: None },
        [newer, older] => Span::Pair { older: *older, newer: *newer },
        _ => {
            if oids.windows(2).any(|w| parent(&w[0]) != Some(w[1])) {
                let members: HashSet<&Oid> = oids.iter().collect();
                let gap = oids[..oids.len() - 1].iter().find(|o| parent(o).is_some_and(|p| !members.contains(&p))).copied();
                return Span::Gaps { gap };
            }
            let last = oids[oids.len() - 1];
            if snap.shallow_roots.binary_search(&last).is_ok() {
                return Span::ShallowEnd;
            }
            match parent(&last) {
                Some(base) => Span::Range { base, newest: oids[0] },
                None => Span::Range { base: repo::empty_tree(), newest: oids[0] },
            }
        }
    }
}
