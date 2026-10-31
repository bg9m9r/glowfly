//! The open palette's command list, kept between frames (`CommandCache`). The view asks for the
//! list every frame while the palette is open; building one checkout entry per branch and tag
//! and ranking them all again each time cost milliseconds in a repository with many tags. The
//! cache keeps the checkout entries until the refs they are built from change, and the ranking
//! until the query, the entries, the recent commands or the suggestions change. The entries that
//! aren't one per ref are rebuilt every frame and compared with the kept ones, so nothing they
//! read can go stale. A query that extends the last one only ranks the last matches, as the
//! sub-lists do (`Pick::refresh`). `palette::listed` builds the same list from scratch.

use super::{Group, Listed, MAX_COMMAND_ROWS, Shown};
use crate::state::AppState;
use crate::state::palette_commands::{Entry, PaletteCommand, RefEntries, fixed_entries, ref_entries};
use crate::state::palette_recent::RecentCommands;
use crate::state::palette_suggested::{grouped, suggestion_rank};
use git2::Oid;
use glowfly_core::fuzzy;
use glowfly_core::repo::{BranchInfo, Snapshot, WorktreeInfo};
use std::cmp::Reverse;

/// The command list's entries and ranking from the last frame (see the module doc).
#[derive(Default)]
pub struct CommandCache {
    /// The entries that aren't one per ref, and where the local checkouts go among them.
    fixed: (Vec<Entry>, usize),
    /// The checkout entries, with the refs they were built from.
    refs: Option<Refs>,
    ranking: Option<Ranking>,
    /// How many times the checkout entries and the ranking were built (what the tests check).
    ref_builds: usize,
    rankings: usize,
}

/// The checkout entries and the parts of the snapshot `ref_entries` reads.
struct Refs {
    from: Option<RefInputs>,
    entries: RefEntries,
}

#[derive(PartialEq)]
struct RefInputs {
    locals: Vec<BranchInfo>,
    worktrees: Vec<WorktreeInfo>,
    remotes: Vec<(String, Vec<(String, Oid)>)>,
    tags: Vec<(String, Oid)>,
    head_branch: Option<String>,
    head_oid: Option<Oid>,
}

impl RefInputs {
    fn of(s: &Snapshot) -> Self {
        RefInputs {
            locals: s.locals.clone(),
            worktrees: s.worktrees.clone(),
            remotes: s.remotes.clone(),
            tags: s.tags.clone(),
            head_branch: s.head_branch.clone(),
            head_oid: s.head_oid,
        }
    }

    fn same(&self, s: &Snapshot) -> bool {
        self.head_oid == s.head_oid
            && self.head_branch == s.head_branch
            && self.locals == s.locals
            && self.worktrees == s.worktrees
            && self.remotes == s.remotes
            && self.tags == s.tags
    }
}

/// The list for one query over unchanged entries.
struct Ranking {
    query: String,
    recent: RecentCommands,
    /// `suggestion_rank` of each fixed entry (checkouts are never suggested).
    suggested: Vec<Option<usize>>,
    /// Every matching entry index (to narrow the next, longer query).
    matches: Vec<usize>,
    /// The first `MAX_COMMAND_ROWS` rows: entry index and the label's matched char indexes.
    rows: Vec<(usize, Vec<usize>)>,
    total: usize,
    groups: Vec<(Group, usize)>,
}

impl CommandCache {
    /// The command list for `query`, as `palette::listed` would build it now.
    pub(super) fn listed(&mut self, st: &AppState, query: &str) -> Listed {
        let fixed = fixed_entries(st);
        if self.fixed != fixed {
            self.fixed = fixed;
            self.ranking = None;
        }
        let snap = st.snap.as_ref();
        let refs_current = self.refs.as_ref().is_some_and(|r| match (&r.from, snap) {
            (Some(from), Some(s)) => from.same(s),
            (None, None) => true,
            _ => false,
        });
        if !refs_current {
            self.refs = Some(Refs { from: snap.map(RefInputs::of), entries: ref_entries(snap) });
            self.ranking = None;
            self.ref_builds += 1;
        }
        let suggested: Vec<Option<usize>> = self.fixed.0.iter().map(|e| suggestion_rank(st, &e.command)).collect();
        let current = self.ranking.as_ref().is_some_and(|r| r.query == query && r.recent == st.palette_recent && r.suggested == suggested);
        if !current {
            let last = self.ranking.take();
            self.ranking = Some(self.rank(st, query, suggested, last));
            self.rankings += 1;
        }
        let Some(r) = &self.ranking else { return Listed { rows: Vec::new(), total: 0, groups: Vec::new() } };
        let rows = r.rows.iter().filter_map(|(i, m)| Some(Shown { entry: self.entry(*i)?.clone(), matched: m.clone() })).collect();
        Listed { rows, total: r.total, groups: r.groups.clone() }
    }

    /// Entry `i` of the whole list: the fixed entries up to the split, the local checkouts, the
    /// rest of the fixed entries, then the remote and tag checkouts (`palette_commands::entries`).
    fn entry(&self, i: usize) -> Option<&Entry> {
        let (fixed, split) = (&self.fixed.0, self.fixed.1);
        let refs = &self.refs.as_ref()?.entries;
        let locals = refs.locals.len();
        if i < split {
            fixed.get(i)
        } else if i < split + locals {
            refs.locals.get(i - split)
        } else if i < fixed.len() + locals {
            fixed.get(i - locals)
        } else {
            refs.others.get(i - fixed.len() - locals)
        }
    }

    fn len(&self) -> usize {
        self.fixed.0.len() + self.refs.as_ref().map_or(0, |r| r.entries.locals.len() + r.entries.others.len())
    }

    /// Ranks the entries for `query`, only among `last`'s matches when the query extends its
    /// query (`last` is over the same entries: a change to them drops it).
    fn rank(&self, st: &AppState, query: &str, suggested: Vec<Option<usize>>, last: Option<Ranking>) -> Ranking {
        let squash = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
        let candidates: Vec<usize> = match last {
            Some(r) if !squash(&r.query).is_empty() && squash(query).starts_with(&squash(&r.query)) => {
                let mut m = r.matches;
                // Back to entry order, so ties keep ranking the earlier entry first.
                m.sort_unstable();
                m
            }
            _ => (0..self.len()).collect(),
        };
        let item = |i: usize| self.entry(i).map_or(("", ""), |e| (e.label.as_str(), e.keywords));
        let items: Vec<(&str, &str)> = candidates.iter().map(|&i| item(i)).collect();
        let mut ranked: Vec<(usize, fuzzy::Match)> = fuzzy::rank(query, &items).into_iter().map(|(pos, m)| (candidates[pos], m)).collect();
        let matches = ranked.iter().map(|(i, _)| *i).collect();
        let command = |i: usize| self.entry(i).map(|e| &e.command);
        by_recency(st, command, &mut ranked);
        let total = ranked.len();
        let mut groups = Vec::new();
        if query.trim().is_empty() {
            (ranked, groups) = grouped(st, command, ranked);
        }
        groups.retain(|&(_, first)| first < MAX_COMMAND_ROWS);
        let rows = ranked.into_iter().take(MAX_COMMAND_ROWS).map(|(i, m)| (i, m.positions)).collect();
        Ranking { query: query.to_owned(), recent: st.palette_recent.clone(), suggested, matches, rows, total, groups }
    }
}

/// Puts recently run commands first among equal scores (all of them, for an empty query). The
/// sort is stable, so the ranking's own tie-breaks still order the rest.
pub(super) fn by_recency<'a>(st: &AppState, command: impl Fn(usize) -> Option<&'a PaletteCommand>, ranked: &mut [(usize, fuzzy::Match)]) {
    if st.palette_recent.is_empty() {
        return;
    }
    let recency = |i: usize| command(i).and_then(PaletteCommand::recall_key).and_then(|k| st.palette_recent.position(k));
    ranked.sort_by_cached_key(|(i, m)| (Reverse(m.score), recency(*i).unwrap_or(usize::MAX)));
}

#[cfg(test)]
#[path = "palette_cache_tests.rs"]
mod tests;
