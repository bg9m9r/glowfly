//! Pull requests across forks: the repositories a pull request can come from or go to (the
//! remotes on the code host's account, "PR repositories"), which of them is the base (a remote
//! named exactly `upstream`, else the code host's), the remote a branch's pull request is pushed
//! to, and the create dialog's From / Into defaults and target branches. Pure.

use super::ForgeState;
use super::branches;
use git2::Oid;
use glowfly_core::forge::{self, Connection, Target};
use glowfly_core::repo::{BranchInfo, Snapshot, Tracking};

/// A configured remote whose repository is on the code host's account.
#[derive(Clone, Debug, PartialEq)]
pub struct PrRepo {
    pub remote: String,
    pub target: Target,
}

/// The create dialog's choice of repositories: `branch` is the name it was opened for, `from`
/// and `into` remote names ("" means the code host's remote).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PrRepoPick {
    pub branch: String,
    pub from: String,
    pub into: String,
}

impl PrRepoPick {
    pub fn remote_from<'a>(&'a self, fs: &'a ForgeState) -> &'a str {
        if self.from.is_empty() { &fs.remote } else { &self.from }
    }

    pub fn remote_into<'a>(&'a self, fs: &'a ForgeState) -> &'a str {
        if self.into.is_empty() { &fs.remote } else { &self.into }
    }
}

/// Which of the create dialog's repository choices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepoSide {
    From,
    Into,
}

/// The remotes whose fetch URL is a repository on `host`'s account: `host_remote` first, the
/// rest by name, each repository once (https and SSH forms, `.git` and letter case aside).
pub fn list(remote_urls: &[(String, String)], host_remote: &str, host: &Target, connections: &[Connection]) -> Vec<PrRepo> {
    let account = host.account();
    let mut found: Vec<(&str, &str, Target)> = remote_urls
        .iter()
        .filter_map(|(name, url)| {
            let t = forge::parse_remote(url, connections).filter(|t| t.account() == account)?;
            Some((name.as_str(), url.as_str(), t))
        })
        .collect();
    found.sort_by(|a, b| (a.0 != host_remote).cmp(&(b.0 != host_remote)).then_with(|| a.0.cmp(b.0)));
    let mut urls: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    for (name, url, target) in found {
        if urls.iter().any(|u| forge::same_repository(u, url, connections)) {
            continue;
        }
        urls.push(url);
        out.push(PrRepo { remote: name.to_owned(), target });
    }
    out
}

/// The repository pull requests go to by default: the remote named exactly `upstream` when it
/// is a PR repository, else the code host's.
pub fn base_remote(fs: &ForgeState) -> &str {
    if is_pr_remote(fs, "upstream") { "upstream" } else { &fs.remote }
}

pub fn repo<'a>(fs: &'a ForgeState, remote: &str) -> Option<&'a PrRepo> {
    fs.pr_repos.iter().find(|r| r.remote == remote)
}

pub fn is_pr_remote(fs: &ForgeState, remote: &str) -> bool {
    repo(fs, remote).is_some()
}

/// Whether `remote` is a fork: a PR repository that is neither the code host's remote nor the
/// base.
pub fn is_fork(fs: &ForgeState, remote: &str) -> bool {
    remote != fs.remote && remote != base_remote(fs) && is_pr_remote(fs, remote)
}

/// The remote a pull request from `b` is pushed to: the fork it tracks, else the code host's. A
/// branch started from the base repository (tracking `upstream/main`) still goes to the code
/// host's remote, never onto the branch it was started from.
pub fn push_remote(b: &BranchInfo, fs: &ForgeState) -> String {
    branches::remote_tracking(b).filter(|t| is_fork(fs, &t.remote)).map(|t| t.remote.clone()).unwrap_or_else(|| fs.remote.clone())
}

/// The branch on `remote` that `b` tracks as its own: any on the code host's remote or a fork,
/// but on a separate base repository only one of the same name (a branch tracking
/// `upstream/main` was started from it, and its pushes don't go there).
pub fn own_tracking<'a>(b: &'a BranchInfo, fs: &ForgeState, remote: &str) -> Option<&'a Tracking> {
    let base_only = remote != fs.remote && remote == base_remote(fs);
    branches::remote_tracking(b).filter(|t| t.remote == remote && (!base_only || t.merge == b.name))
}

/// `remote`'s remote-tracking branches (none when it isn't loaded).
pub fn remote_branches<'a>(snap: &'a Snapshot, remote: &str) -> &'a [(String, Oid)] {
    snap.remotes.iter().find(|(r, _)| r == remote).map(|(_, b)| b.as_slice()).unwrap_or_default()
}

fn on(snap: &Snapshot, remote: &str, name: &str) -> bool {
    remote_branches(snap, remote).iter().any(|(n, _)| n == name)
}

/// The Into repository for a dialog opened from `from`: the base, unless the preferred target
/// is only on `from` (a stack's layers stay on one repository).
pub fn default_into(snap: &Snapshot, fs: &ForgeState, from: &str, preferred: Option<&str>) -> String {
    let base = base_remote(fs);
    match preferred {
        Some(p) if from != base && on(snap, from, p) && !on(snap, base, p) => from.to_owned(),
        _ => base.to_owned(),
    }
}

/// The target branches on `into`, sorted; the code host's remote with nothing fetched lists
/// the local branches. The source is left out only when it is on the same repository.
pub fn target_branches(snap: &Snapshot, fs: &ForgeState, into: &str, from: &str, source: &str) -> Vec<String> {
    let remote = remote_branches(snap, into);
    let mut targets: Vec<String> = if remote.is_empty() && into == fs.remote {
        snap.locals.iter().map(|b| b.name.clone()).collect()
    } else {
        remote.iter().map(|(n, _)| n.clone()).collect()
    };
    if from == into {
        targets.retain(|b| b != source);
    }
    targets.sort();
    targets.dedup();
    targets
}

/// The target to preselect on `into`: the preferred one, the code host's default branch, the
/// remote's HEAD, main / master / develop, the first listed, else "main".
pub fn default_target(
    snap: &Snapshot,
    fs: &ForgeState,
    into: &str,
    targets: &[String],
    source: &str,
    from: &str,
    preferred: Option<&str>,
) -> String {
    let host_default = fs.default_branch.as_deref().filter(|d| into == fs.remote && (from != into || *d != source));
    let head = snap.remote_heads.iter().find(|(r, _)| r == into).map(|(_, b)| b.as_str());
    first_target(targets, preferred, host_default, head)
}

/// `default_target`'s order over the listed `targets`; the code host's default branch is taken
/// as it is (it may not be fetched yet).
pub fn first_target(targets: &[String], preferred: Option<&str>, host_default: Option<&str>, head: Option<&str>) -> String {
    let listed = |b: &str| targets.iter().any(|t| t == b);
    preferred
        .filter(|p| listed(p))
        .or(host_default)
        .or_else(|| head.filter(|h| listed(h)))
        .or_else(|| ["main", "master", "develop"].into_iter().find(|b| listed(b)))
        .or_else(|| targets.first().map(String::as_str))
        .unwrap_or("main")
        .to_owned()
}

/// "owner/repo (remote)", for the repository pickers.
pub fn label(r: &PrRepo) -> String {
    format!("{} ({})", r.target.display(), r.remote)
}

#[cfg(test)]
#[path = "pr_repos_tests.rs"]
mod tests;
