//! The Chapters history view's state for one tab: the repository's choice between the graph and
//! the list, the grouping (`glowfly_core::chapters`), which chapters are expanded and the rows
//! the list draws. `update/chapters.rs::settle` calls `follow` and `sync` after every message, so
//! the grouping is redone only when the loaded history, HEAD or the refs changed. Selecting a
//! chapter reuses the commit and multi-commit selections (`selection_of`, `is_selected`).

pub mod moves;
#[cfg(test)]
mod tests;
pub mod words;

use super::selection::{MultiSelection, Selection};
use git2::Oid;
use glowfly_core::chapters::{Chapter, Chapters, History, group};
use glowfly_core::repo::Snapshot;
use glowfly_core::settings::{HistoryMode, Settings};
use std::collections::HashSet;
use std::path::PathBuf;

/// One row of the Chapters list below the work tree's row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    /// The heading above chapter `n`: its release tags.
    Release(u32),
    /// Chapter `n` of the list.
    Chapter(u32),
    /// A commit shown inside an expanded chapter: `row` indexes the snapshot's commits.
    Commit { chapter: u32, row: u32 },
}

/// What the grouping was made from: when it is equal, the grouping is too.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Stamp {
    len: usize,
    first: Option<Oid>,
    last: Option<Oid>,
    head: Option<Oid>,
    refs_key: u64,
    tags: usize,
}

impl Stamp {
    fn of(snap: &Snapshot) -> Stamp {
        Stamp {
            len: snap.commits.len(),
            first: snap.commits.first().map(|c| c.oid),
            last: snap.commits.last().map(|c| c.oid),
            head: snap.head_oid,
            refs_key: snap.refs_key,
            tags: snap.tags.len(),
        }
    }
}

#[derive(Default)]
pub struct ChaptersView {
    /// The repository the fields below belong to (`Snapshot::real_workdir`).
    pub repo: Option<PathBuf>,
    /// The view the repository shows (saved in the settings).
    pub mode: HistoryMode,
    /// The grouping; only while the mode is Chapters.
    pub list: Option<Chapters>,
    stamp: Option<Stamp>,
    /// The chapters shown open, by id: kept while they still exist.
    pub expanded: HashSet<Oid>,
    /// The list's rows below the work tree's row.
    pub rows: Vec<Row>,
}

impl ChaptersView {
    /// Takes the saved choice of the repository `snap` shows when it isn't the one this holds.
    pub fn follow(&mut self, snap: &Snapshot, settings: &Settings) {
        if self.repo.as_deref() == Some(snap.real_workdir.as_path()) {
            return;
        }
        *self = ChaptersView {
            repo: Some(snap.real_workdir.clone()),
            mode: settings.history_mode_for(&snap.real_workdir),
            ..Default::default()
        };
    }

    /// Brings the grouping up to date with `snap` (dropped in the Commits view).
    pub fn sync(&mut self, snap: &Snapshot) {
        if self.mode == HistoryMode::Commits {
            self.list = None;
            self.stamp = None;
            self.rows.clear();
            return;
        }
        let stamp = Stamp::of(snap);
        if self.list.is_some() && self.stamp == Some(stamp) {
            return;
        }
        let list = group(&History::of(snap));
        self.expanded.retain(|id| list.index_of(*id).is_some());
        self.list = Some(list);
        self.stamp = Some(stamp);
        self.rebuild();
    }

    /// Expands or collapses the chapter `id`; false when there is no such chapter or it has no
    /// commits to show.
    pub fn toggle(&mut self, id: Oid) -> bool {
        let Some(list) = &self.list else { return false };
        if !list.index_of(id).is_some_and(|i| !list.list[i].inside().is_empty()) {
            return false;
        }
        if !self.expanded.remove(&id) {
            self.expanded.insert(id);
        }
        self.rebuild();
        true
    }

    fn rebuild(&mut self) {
        self.rows.clear();
        let Some(list) = &self.list else { return };
        for (i, ch) in list.list.iter().enumerate() {
            let i = i as u32;
            if !ch.releases.is_empty() {
                self.rows.push(Row::Release(i));
            }
            self.rows.push(Row::Chapter(i));
            if self.expanded.contains(&ch.id) {
                self.rows.extend(ch.inside().iter().map(|&row| Row::Commit { chapter: i, row }));
            }
        }
    }
}

/// What a click on chapter `ch` selects: its one commit (a lone commit, a squash landing or a
/// merge, whose details list its files against its first parent), or the commits of a run as one
/// line whose combined changes run from the oldest one's parent to the newest.
pub fn selection_of(ch: &Chapter, snap: &Snapshot) -> Selection {
    if ch.line.len() <= 1 {
        return Selection::Commit(ch.id);
    }
    MultiSelection::build_line(line_oids(ch, snap), snap)
}

/// Whether `sel` is exactly what a click on `ch` selects.
pub fn is_selected(ch: &Chapter, snap: &Snapshot, sel: &Selection) -> bool {
    if ch.line.len() <= 1 {
        return matches!(sel, Selection::Commit(o) if *o == ch.id);
    }
    matches!(sel, Selection::Commits(m) if m.oids.len() == ch.line.len() && m.oids.first() == Some(&ch.id) && line_oids(ch, snap).eq(m.oids.iter().copied()))
}

fn line_oids<'a>(ch: &'a Chapter, snap: &'a Snapshot) -> impl Iterator<Item = Oid> + 'a {
    ch.line.iter().filter_map(|&r| snap.commits.get(r as usize)).map(|c| c.oid)
}
