//! A branch picked in a menu, held so git is given its full ref (`refs/heads/x`,
//! `refs/remotes/origin/x`) while people see its short name. Git's short-name lookup tries tags
//! (and `refs/<name>`) before branches, so the short name alone can name the wrong commit.
//! `MergeNames` says which branches that happens to, so a merge can still use the short name (and
//! get git's own message) for every other branch.

use std::collections::HashSet;

/// A branch a merge, rebase, preview or conflict check names. Git is given its full ref, so no tag
/// or other ref with the same short name is picked; people see its short name.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BranchRef {
    /// refs/heads/<name>
    Local(String),
    /// refs/remotes/<remote>/<branch>, held as "<remote>/<branch>" (the way it's shown), so a
    /// remote name with '/' in it needs no splitting.
    Remote(String),
}

impl BranchRef {
    /// The full ref git is given: "refs/heads/x" or "refs/remotes/origin/x".
    pub fn full(&self) -> String {
        match self {
            BranchRef::Local(n) => format!("refs/heads/{n}"),
            BranchRef::Remote(n) => format!("refs/remotes/{n}"),
        }
    }

    /// The name people see: "x" or "origin/x".
    pub fn shown(&self) -> &str {
        match self {
            BranchRef::Local(n) | BranchRef::Remote(n) => n,
        }
    }

    /// The branch a full ref names: refs/heads/ gives a local branch, refs/remotes/ a
    /// remote-tracking one; anything else is None.
    pub fn from_full(full: &str) -> Option<Self> {
        if let Some(n) = full.strip_prefix("refs/heads/") {
            Some(BranchRef::Local(n.to_owned()))
        } else {
            full.strip_prefix("refs/remotes/").map(|n| BranchRef::Remote(n.to_owned()))
        }
    }
}

/// What a merge from the menus needs to write the message `git merge <short name>` writes. Read at
/// load (`Snapshot::merge_names`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MergeNames {
    /// What git's merge title ends with when merging into HEAD (" into <this>"): the checked-out
    /// branch, "HEAD" when detached, None when merge.suppressDest leaves it out (main and master by
    /// default).
    pub into: Option<String>,
    /// The full refs of the branches whose short name git takes for something else first: a
    /// same-named tag, `refs/<name>`, an all-caps file in the git dir such as ORIG_HEAD, a local
    /// branch spelled like a remote one, or a name spelled like a full object id.
    pub shadowed: HashSet<String>,
}

impl MergeNames {
    /// Whether git's short-name lookup would take `b`'s short name for another ref or object.
    pub fn is_shadowed(&self, b: &BranchRef) -> bool {
        self.shadowed.contains(&b.full())
    }
}

/// A ref name as people say it: without refs/heads/ or refs/remotes/.
pub fn short_ref(name: &str) -> &str {
    name.strip_prefix("refs/heads/").or_else(|| name.strip_prefix("refs/remotes/")).unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_and_shown_names() {
        assert_eq!(BranchRef::Local("a/b".into()).full(), "refs/heads/a/b");
        assert_eq!(BranchRef::Remote("origin/x".into()).full(), "refs/remotes/origin/x");
        assert_eq!(BranchRef::Local("a/b".into()).shown(), "a/b");
        assert_eq!(BranchRef::Remote("origin/x".into()).shown(), "origin/x");
    }

    #[test]
    fn from_full_round_trips_branches_only() {
        for b in [BranchRef::Local("a/b".into()), BranchRef::Remote("origin/x".into())] {
            assert_eq!(BranchRef::from_full(&b.full()), Some(b));
        }
        assert_eq!(BranchRef::from_full("refs/tags/x"), None);
        assert_eq!(BranchRef::from_full("x"), None);
    }

    #[test]
    fn short_ref_strips_branch_prefixes_only() {
        assert_eq!(short_ref("refs/heads/x"), "x");
        assert_eq!(short_ref("refs/remotes/origin/x"), "origin/x");
        assert_eq!(short_ref("refs/tags/x"), "refs/tags/x");
        assert_eq!(short_ref("x"), "x");
    }
}
