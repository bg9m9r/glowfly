//! One commit compared with the working directory (`Selection::VsWorkdir`): the commit row and
//! the WIP row selected together, and the files that differ from the commit to the working tree
//! (or, swapped, from the working tree to the commit).

use super::Selection;
use git2::Oid;
use trellis_core::repo::{Comparison, DiffTarget};

/// Why Ctrl-click on the WIP row does nothing with several commits selected.
pub const WORKDIR_NEEDS_ONE: &str = "Compare with the working directory needs exactly one selected commit.";

#[derive(Clone, PartialEq, Debug)]
pub struct WorkdirCompare {
    pub oid: Oid,
    /// The files that differ from the commit to the working tree (from the working tree to the
    /// commit when `swapped`); `None` until the first read.
    /// Kept while a newer read runs, so the list doesn't flicker.
    pub changes: Option<Result<Comparison, String>>,
    /// Counts the reads asked for: only the latest one's result is kept.
    pub generation: u64,
    /// Swap direction: the comparison runs from the working tree to the commit.
    pub swapped: bool,
}

impl WorkdirCompare {
    pub fn new(oid: Oid) -> Self {
        Self { oid, changes: None, generation: 0, swapped: false }
    }

    /// The diff of `path` in this comparison's direction.
    pub fn target(&self, path: String) -> DiffTarget {
        if self.swapped { DiffTarget::FromWorkdir(self.oid, path) } else { DiffTarget::ToWorkdir(self.oid, path) }
    }
}

/// The selection after a Ctrl-click on the WIP row: one commit gains the working directory, a
/// comparison drops it again, and otherwise the WIP row is selected alone. Several commits can't
/// be compared with it.
pub fn wip_toggled(sel: &Selection) -> Result<Selection, &'static str> {
    match sel {
        Selection::Commit(a) => Ok(Selection::VsWorkdir(WorkdirCompare::new(*a))),
        Selection::VsWorkdir(w) => Ok(Selection::Commit(w.oid)),
        Selection::Wip | Selection::None => Ok(Selection::Wip),
        Selection::Commits(_) => Err(WORKDIR_NEEDS_ONE),
    }
}
