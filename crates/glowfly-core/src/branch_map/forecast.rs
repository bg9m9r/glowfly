//! Trial merges in memory. libgit2's merge of two commits gives an index that is read for its
//! conflicts and dropped; the objects it writes (automerged files, virtual merge bases) go to a
//! mempack backend added to this handle's object database, and are freed after each merge. No
//! ref, index or working-tree file is touched.

use super::Forecast;
use crate::error::Result;
use crate::op_preview::conflict_paths;
use git2::{Index, Mempack, MergeOptions, Odb, Oid, Repository};

pub(super) struct Merger<'r> {
    repo: &'r Repository,
    mem: Mempack<'r>,
    opts: MergeOptions,
}

impl<'r> Merger<'r> {
    /// `odb` is `repo`'s own object database, kept by the caller for as long as the merger
    /// lives. `repo` must be a handle opened for this read: the backend added here would
    /// otherwise catch what other users of a shared handle write later.
    pub(super) fn new(repo: &'r Repository, odb: &'r Odb<'r>) -> Result<Self> {
        let mem = odb.add_new_mempack_backend(1000)?;
        // Not `skip_reuc`: git2 0.20 sets the fail-on-conflict flag for it, and a failed merge
        // reports no paths. The resolve-undo data it would skip is never read here.
        Ok(Self { repo, mem, opts: MergeOptions::new() })
    }

    /// What merging `theirs` into `ours` would do.
    pub(super) fn forecast(&self, ours: Oid, theirs: Oid) -> Result<Forecast> {
        let (ours, theirs) = (self.repo.find_commit(ours)?, self.repo.find_commit(theirs)?);
        let result = self.repo.merge_commits(&ours, &theirs, Some(&self.opts)).map_err(Into::into).and_then(|index| read(&index));
        // The index is dropped by now: free what the merge wrote.
        self.mem.reset()?;
        result
    }
}

/// Whether the merged index has conflicts, and on which paths.
fn read(index: &Index) -> Result<Forecast> {
    if index.has_conflicts() { Ok(Forecast::Conflicts(conflict_paths(index)?)) } else { Ok(Forecast::Clean) }
}
