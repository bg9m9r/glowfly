//! What a multi-selection of commits spans: a pair, one unbroken line of first-parent history
//! (whose changes combine into one diff, from the empty tree when it reaches the first commit),
//! or neither. A chapter of two commits is a line too (`line_span`).

use git2::Oid;
use glowfly_core::repo::{self, Snapshot};
use std::collections::HashSet;

/// The shape of a multi-selection.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Span {
    /// Exactly two commits, adjacent or not: the older (lower row) and the newer.
    Pair { older: Oid, newer: Oid },
    /// Three or more commits (two, for a chapter), each the first parent of the one above it: `base` is the oldest
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
            line_end(snap, oids[oids.len() - 1], oids[0])
        }
    }
}

/// The span of one first-parent line from `oldest` up to `newest`: its combined changes are
/// `oldest`'s parent (the empty tree for a root) to `newest`, or just the stacked list when
/// `oldest` is a shallow clone's boundary.
fn line_end(snap: &Snapshot, oldest: Oid, newest: Oid) -> Span {
    if snap.shallow_roots.binary_search(&oldest).is_ok() {
        return Span::ShallowEnd;
    }
    match snap.row_of.get(&oldest).and_then(|&r| snap.commits[r].parents.first().copied()) {
        Some(base) => Span::Range { base, newest },
        None => Span::Range { base: repo::empty_tree(), newest },
    }
}

/// `span_of`, except that exactly two commits where the older is the newer's first parent are a
/// line (combined from the older one's parent), not a pair to compare: what a chapter of two
/// commits selects.
pub fn line_span(oids: &[Oid], snap: &Snapshot) -> Span {
    if let [newer, older] = oids
        && snap.row_of.get(newer).and_then(|&r| snap.commits[r].parents.first()) == Some(older)
    {
        return line_end(snap, *older, *newer);
    }
    span_of(oids, snap)
}
