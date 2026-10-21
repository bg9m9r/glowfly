//! Finding the repository a link names on this machine, and what the link points at inside it.
//!
//! A folder is a copy of the linked repository when it has the link's root commit (a commit with
//! no parents), or when one of its remotes has the link's URL ([`super::same_url`]). Both are
//! read with libgit2 only: nothing from the repository's config runs, and no hook.

use super::{Link, Target, same_url};
use crate::error::Result;
use git2::{BranchType, ErrorCode, Oid, Repository, RepositoryOpenFlags};
use std::path::{Path, PathBuf};

/// The root commit of the repository at `workdir`: the end of HEAD's first-parent chain. None
/// in a shallow clone (its chain ends where the download stopped, not at the real root) and
/// when HEAD is unborn.
pub fn root_commit(workdir: &Path) -> Result<Option<String>> {
    let repo = open(workdir)?;
    if repo.is_shallow() || repo.head().ok().and_then(|h| h.target()).is_none() {
        return Ok(None);
    }
    let mut walk = repo.revwalk()?;
    walk.push_head()?;
    walk.simplify_first_parent()?;
    let mut last = None;
    for oid in walk {
        last = Some(oid?);
    }
    Ok(last.map(|o| o.to_string()))
}

/// What a link points at, in one repository.
#[derive(Clone, Debug, PartialEq)]
pub enum TargetState {
    /// Found: the commit to select, and the ref it was found through (`origin/feature-x`, a tag
    /// name, a short sha).
    Present {
        oid: Oid,
        label: String,
    },
    Missing,
}

/// A folder that is a copy of the linked repository.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    /// The folder as it was asked about.
    pub workdir: PathBuf,
    /// The repository's common git dir: worktrees of one repository share it.
    pub common_dir: PathBuf,
    /// Whether this is the main worktree rather than a linked one.
    pub main_worktree: bool,
    /// It has the link's root commit.
    pub by_root: bool,
    /// One of its remotes has the link's URL.
    pub by_url: bool,
    /// The remote whose URL matched.
    pub remote: Option<String>,
    pub target: TargetState,
}

/// How well a match fits: 3 for root and URL, 2 for the root only, 1 for the URL only.
pub fn rank(f: &Found) -> u8 {
    match (f.by_root, f.by_url) {
        (true, true) => 3,
        (true, false) => 2,
        (false, true) => 1,
        (false, false) => 0,
    }
}

fn open(path: &Path) -> std::result::Result<Repository, git2::Error> {
    Repository::open_ext(path, RepositoryOpenFlags::NO_SEARCH, std::iter::empty::<&std::ffi::OsStr>())
}

/// Whether the folder at `path` is a copy of the repository `link` names, and what the link
/// points at there. Ok(None) for a folder that isn't a repository (or is bare) and for one that
/// shares neither the root commit nor a remote URL with the link.
pub fn check(path: &Path, link: &Link) -> Result<Option<Found>> {
    let repo = match open(path) {
        Ok(r) => r,
        Err(e) if e.code() == ErrorCode::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    if repo.workdir().is_none() {
        return Ok(None);
    }
    let root = link.root.as_deref().and_then(|r| Oid::from_str(r).ok());
    let by_root = root.and_then(|oid| repo.find_commit(oid).ok()).is_some_and(|c| c.parent_count() == 0);
    let remote = link.url.as_deref().and_then(|url| remote_with_url(&repo, url));
    if !by_root && remote.is_none() {
        return Ok(None);
    }
    let target = resolve(&repo, &link.target, remote.as_deref());
    Ok(Some(Found {
        workdir: path.to_path_buf(),
        common_dir: crate::paths::resolve(repo.commondir()),
        main_worktree: !repo.is_worktree(),
        by_root,
        by_url: remote.is_some(),
        remote,
        target,
    }))
}

/// The first remote (in config order) whose URL is the link's.
fn remote_with_url(repo: &Repository, url: &str) -> Option<String> {
    let names = repo.remotes().ok()?;
    names
        .iter()
        .flatten()
        .find(|name| repo.find_remote(name).ok().is_some_and(|r| r.url().is_some_and(|u| same_url(u, url))))
        .map(str::to_owned)
}

fn resolve(repo: &Repository, target: &Target, remote: Option<&str>) -> TargetState {
    let found = match target {
        Target::Repository => repo.head().ok().and_then(|h| h.target()).map(|oid| (oid, "HEAD".to_owned())),
        Target::Branch(name) => branch(repo, name, remote),
        Target::Tag(name) => {
            repo.find_reference(&format!("refs/tags/{name}")).ok().and_then(|r| r.peel_to_commit().ok()).map(|c| (c.id(), name.clone()))
        }
        Target::Commit(sha) => repo.find_commit_by_prefix(sha).ok().map(|c| (c.id(), sha[..sha.len().min(7)].to_owned())),
    };
    match found {
        Some((oid, label)) => TargetState::Present { oid, label },
        None => TargetState::Missing,
    }
}

/// The branch `name` from the link: a local branch tracking `<remote>/<name>`, then a local
/// branch called `name`, then `<remote>/<name>`, then `name` on any remote.
fn branch(repo: &Repository, name: &str, remote: Option<&str>) -> Option<(Oid, String)> {
    let tip = |refname: &str| repo.find_reference(refname).ok().and_then(|r| r.peel_to_commit().ok()).map(|c| c.id());
    if let Some(remote) = remote {
        let merge = format!("refs/heads/{name}");
        for b in repo.branches(Some(BranchType::Local)).ok()?.flatten().map(|(b, _)| b) {
            let Some(full) = b.get().name().map(str::to_owned) else { continue };
            let tracks = repo.branch_upstream_remote(&full).ok().is_some_and(|r| r.as_str() == Some(remote))
                && repo.branch_upstream_merge(&full).ok().is_some_and(|m| m.as_str() == Some(merge.as_str()));
            if tracks && let Some(oid) = tip(&full) {
                return Some((oid, full.trim_start_matches("refs/heads/").to_owned()));
            }
        }
    }
    if let Some(oid) = tip(&format!("refs/heads/{name}")) {
        return Some((oid, name.to_owned()));
    }
    if let Some(remote) = remote
        && let Some(oid) = tip(&format!("refs/remotes/{remote}/{name}"))
    {
        return Some((oid, format!("{remote}/{name}")));
    }
    for r in repo.remotes().ok()?.iter().flatten() {
        if let Some(oid) = tip(&format!("refs/remotes/{r}/{name}")) {
            return Some((oid, format!("{r}/{name}")));
        }
    }
    None
}
