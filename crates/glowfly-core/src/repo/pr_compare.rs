//! What a pull request brings to its target, compared from the target's tip to a source tip
//! picked here: the head commit the service listed when it is already in the object database
//! (even when the remote-tracking source branch is missing or points elsewhere), else the
//! remote-tracking source branch. Gives the files (`branch_changes`) and the commits in
//! `target..source` (git's two-dot range: reachable from the source tip, not from the target's).

use crate::commit_text;
use crate::error::Result;
use git2::{ObjectType, Oid, Repository, Sort};
use std::path::Path;

use super::{BranchChanges, branch_changes};

/// The most commits a pull request's Commits list holds (the newest ones).
pub const PR_COMMITS_MAX: usize = 500;

/// Where the source tip came from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SourceFrom {
    /// The remote-tracking source branch.
    Branch,
    /// The head commit the service listed, already in the object database.
    Head,
}

/// The commit a pull request is compared to, and where it came from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SourceTip {
    pub oid: Oid,
    pub from: SourceFrom,
}

/// One commit of a pull request.
#[derive(Clone, PartialEq, Debug)]
pub struct RangeCommit {
    pub oid: Oid,
    pub summary: String,
    pub author: String,
    pub email: String,
    /// The author's time, in seconds since the epoch.
    pub time: i64,
    /// It has more than one parent.
    pub merge: bool,
}

/// The commits of a pull request: at most `PR_COMMITS_MAX` of the newest, oldest first
/// (children after their parents), and how many more there are.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct RangeCommits {
    pub commits: Vec<RangeCommit>,
    pub more: usize,
}

/// What a pull request brings, compared from a source tip.
#[derive(Clone, PartialEq, Debug)]
pub enum PrCompare {
    /// Neither the listed head nor the remote-tracking branch is here.
    NoSource,
    /// The source tip shares no history with the target here.
    Unrelated(SourceTip),
    Loaded {
        source: SourceTip,
        changes: BranchChanges,
        commits: RangeCommits,
    },
}

/// Whether `s` can be a commit id or the prefix of one a service lists: 7 to 64 ASCII hex
/// characters (a SHA-1 or SHA-256 id, or a prefix of one).
pub fn is_commit_prefix(s: &str) -> bool {
    (7..=64).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The commit `prefix` names, when it names exactly one commit here. Not found, ambiguous and
/// another kind of object all count as not here.
pub(super) fn find_commit(repo: &Repository, prefix: &str) -> Option<Oid> {
    if !is_commit_prefix(prefix) {
        return None;
    }
    let prefix = prefix.to_ascii_lowercase();
    repo.find_object_by_prefix(&prefix, Some(ObjectType::Commit)).ok().map(|o| o.id())
}

/// The source tip: the branch when its tip is the listed head, else the head when it is here,
/// else the branch.
pub(super) fn source_tip(repo: &Repository, branch: Option<Oid>, head: Option<&str>) -> Option<SourceTip> {
    let head = head.filter(|h| is_commit_prefix(h)).map(str::to_ascii_lowercase);
    if let (Some(b), Some(h)) = (branch, &head)
        && b.to_string().starts_with(h.as_str())
    {
        return Some(SourceTip { oid: b, from: SourceFrom::Branch });
    }
    if let Some(oid) = head.as_deref().and_then(|h| find_commit(repo, h)) {
        return Some(SourceTip { oid, from: SourceFrom::Head });
    }
    branch.map(|oid| SourceTip { oid, from: SourceFrom::Branch })
}

/// The commits in `target..tip`: newest first while walking, the newest `max` kept, then
/// reversed so the oldest comes first.
pub(super) fn range_commits(repo: &Repository, target: Oid, tip: Oid, max: usize) -> Result<RangeCommits> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
    walk.push(tip)?;
    walk.hide(target)?;
    let mut commits = Vec::new();
    let mut more = 0;
    for oid in walk {
        let oid = oid?;
        if commits.len() == max {
            more += 1;
            continue;
        }
        let c = repo.find_commit(oid)?;
        let person = commit_text::author(&c);
        commits.push(RangeCommit {
            oid,
            summary: super::walk::commit_texts(&c).0,
            author: person.name,
            email: person.email,
            time: c.author().when().seconds(),
            merge: c.parent_count() > 1,
        });
    }
    commits.reverse();
    Ok(RangeCommits { commits, more })
}

/// Compares a pull request from `target` (the target's tip) to its source tip, picked from
/// `branch` (the remote-tracking source branch's tip) and `head` (the head the service listed).
/// A target missing from the repository is an error.
pub fn pr_compare(workdir: &Path, target: Oid, branch: Option<Oid>, head: Option<&str>) -> Result<PrCompare> {
    let repo = Repository::open(workdir)?;
    repo.find_commit(target)?;
    let Some(source) = source_tip(&repo, branch, head) else { return Ok(PrCompare::NoSource) };
    match branch_changes(workdir, target, source.oid)? {
        None => Ok(PrCompare::Unrelated(source)),
        Some(changes) => {
            let commits = range_commits(&repo, target, source.oid, PR_COMMITS_MAX)?;
            Ok(PrCompare::Loaded { source, changes, commits })
        }
    }
}
