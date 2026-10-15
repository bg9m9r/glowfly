//! Shallow clones: the commits where the downloaded history stops, read from the repository's
//! `shallow` file.

use super::Snapshot;
use git2::{Oid, Repository};

/// The shallow boundary commits of `repo`, sorted; empty for a full clone. git keeps the file
/// in the common dir, so linked worktrees share it. When libgit2 says the repository is shallow
/// but the file can't be read, HEAD (or the zero id) stands in, so the flag still reads shallow.
pub(super) fn roots(repo: &Repository) -> Vec<Oid> {
    if !repo.is_shallow() {
        return Vec::new();
    }
    let text = std::fs::read_to_string(repo.commondir().join("shallow")).unwrap_or_default();
    let mut oids: Vec<Oid> = text.lines().map(str::trim).filter(|l| !l.is_empty()).filter_map(|l| Oid::from_str(l).ok()).collect();
    oids.sort();
    oids.dedup();
    if oids.is_empty() {
        oids.push(repo.head().ok().and_then(|h| h.target()).unwrap_or_else(Oid::zero));
    }
    oids
}

impl Snapshot {
    /// Whether the repository is a shallow clone: its history stops at commits whose parents
    /// were never downloaded.
    pub fn is_shallow(&self) -> bool {
        !self.shallow_roots.is_empty()
    }
}
