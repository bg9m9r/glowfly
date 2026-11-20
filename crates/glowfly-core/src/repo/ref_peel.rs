//! Peels a load's ref targets to commits without reading commits: the object database's header
//! gives an object's type, and a tag object names its target's id and type. Each target is
//! peeled once per load. Declared from `load.rs`.

use git2::{ObjectType, Oid, Repository};
use std::collections::HashMap;

/// How many tags of tags are followed before a chain is given up on.
const MAX_HOPS: usize = 32;

/// What a ref's target is, as far as the graph and the tag lists care.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Peeled {
    /// The target is a commit.
    Commit(Oid),
    /// An annotated tag (or a chain of them) ending at this commit.
    Tag(Oid),
    /// A tree, a blob, a tag of a non-commit, or an object that is missing or unreadable.
    Other,
}

impl Peeled {
    /// The commit the target peels to.
    pub(super) fn commit(self) -> Option<Oid> {
        match self {
            Peeled::Commit(c) | Peeled::Tag(c) => Some(c),
            Peeled::Other => None,
        }
    }
}

/// Peels targets against one repository, remembering each answer.
pub(super) struct Peeler<'r> {
    repo: &'r Repository,
    odb: Option<git2::Odb<'r>>,
    memo: HashMap<Oid, Peeled>,
}

impl<'r> Peeler<'r> {
    pub(super) fn new(repo: &'r Repository) -> Self {
        Peeler { repo, odb: repo.odb().ok(), memo: HashMap::new() }
    }

    /// What `target` is (see `Peeled`).
    pub(super) fn peel(&mut self, target: Oid) -> Peeled {
        if let Some(p) = self.memo.get(&target) {
            return *p;
        }
        let p = self.read(target);
        self.memo.insert(target, p);
        p
    }

    fn read(&self, target: Oid) -> Peeled {
        let Some(odb) = &self.odb else { return self.read_whole(target) };
        match odb.read_header(target) {
            Ok((_, ObjectType::Commit)) => Peeled::Commit(target),
            Ok((_, ObjectType::Tag)) => self.through_tags(odb, target),
            _ => Peeled::Other,
        }
    }

    /// The commit an annotated tag (or a chain of them) ends at; a missing commit gives `Other`.
    fn through_tags(&self, odb: &git2::Odb<'_>, mut tag: Oid) -> Peeled {
        for _ in 0..MAX_HOPS {
            let Ok(t) = self.repo.find_tag(tag) else { return Peeled::Other };
            match t.target_type() {
                Some(ObjectType::Commit) if odb.exists(t.target_id()) => return Peeled::Tag(t.target_id()),
                Some(ObjectType::Tag) => tag = t.target_id(),
                _ => return Peeled::Other,
            }
        }
        Peeled::Other
    }

    /// Without an object database to ask: the object itself.
    fn read_whole(&self, target: Oid) -> Peeled {
        let Ok(o) = self.repo.find_object(target, None) else { return Peeled::Other };
        match (o.kind(), o.peel_to_commit()) {
            (Some(ObjectType::Commit), Ok(_)) => Peeled::Commit(target),
            (_, Ok(c)) => Peeled::Tag(c.id()),
            _ => Peeled::Other,
        }
    }
}

#[cfg(test)]
#[path = "ref_peel_tests.rs"]
mod tests;
