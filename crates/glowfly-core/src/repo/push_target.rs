//! Where a plain `git push` of a branch goes, as git decides it (git-push(1) DEFAULT BEHAVIOR),
//! or why git refuses; worked out at load for every branch with an upstream. The remote is
//! `branch.<b>.pushRemote`, else `remote.pushDefault`, else the upstream's. A configured push
//! remote that is neither a remote here, nor `.`, nor URL-like is passed over for the
//! upstream's remote (git would take it for a URL and fail), and the caller pushes there
//! explicitly. What goes there is decided by the remote's `remote.<r>.push` refspecs, else
//! `remote.<r>.mirror`, else `push.default`; the destination's remote-tracking ref comes from
//! the remote's fetch refspecs (`push_refspecs.rs`), and its counts from `GraphCache`.

use super::graph_cache::GraphCache;
use super::push_config::{PushMode, PushRemotes};
use super::{BranchInfo, Tracking};
use crate::gitcli::args;
use git2::{Oid, Repository};
use std::collections::HashMap;

#[path = "push_refspecs.rs"]
mod refspecs;

use refspecs::RemoteSpecs;

/// Where a plain push of a branch goes, or why git refuses it.
#[derive(Clone, Debug, PartialEq)]
pub struct PushTarget {
    /// The remote the push goes to: a remote name, "." for this repository, or a URL config names.
    pub remote: String,
    /// The setting that chose `remote` (`branch.<b>.pushRemote` / `remote.pushDefault`); None for the upstream's remote.
    pub key: Option<String>,
    /// A configured push remote that isn't a remote here (key, value), passed over for the upstream's remote.
    pub missing: Option<(String, String)>,
    pub dest: Result<PushDest, PushRefusal>,
}

/// What a plain push updates on `PushTarget::remote`.
#[derive(Clone, Debug, PartialEq)]
pub struct PushDest {
    /// The ref it updates there: refs/heads/main (refs/for/main through a push refspec).
    pub dest_ref: String,
    /// The remote-tracking ref mirroring it (refs/remotes/fork/main), when fetch refspecs map it.
    pub tracking_ref: Option<String>,
    /// `tracking_ref`'s commit when it exists (None: the push creates it, or it isn't fetched).
    pub tip: Option<Oid>,
    /// Commits the branch has that `tip` lacks; 0 without a tip.
    pub ahead: usize,
    /// Commits `tip` has that the branch lacks; 0 without a tip.
    pub behind: usize,
    pub rule: PushRule,
}

/// What decided `PushDest::dest_ref`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PushRule {
    /// `push.default` current, upstream or simple.
    Default,
    /// `push.default` matching: every same-named branch on the remote goes too.
    Matching,
    /// A `remote.<r>.push` refspec.
    Refspec,
    /// `remote.<r>.mirror`: the remote is made to match this repository.
    Mirror,
}

/// Why git refuses a plain push of the branch.
#[derive(Clone, Debug, PartialEq)]
pub enum PushRefusal {
    /// `push.default` is nothing.
    Nothing,
    /// `push.default` has a value git doesn't accept (as written).
    Invalid(String),
    /// push.default=upstream and `remote` isn't the upstream's remote.
    NotUpstreamRemote { upstream_remote: String },
    /// simple, same remote, upstream of another name (the upstream branch name).
    NameDiffers { merge: String },
    /// matching, and the remote has no branch of this name.
    NotOnRemote,
    /// remote.<r>.push doesn't include the branch.
    NotInRefspecs,
    /// No remote to push to: the upstream's remote (or origin) isn't configured.
    NoRemote,
}

impl PushTarget {
    /// "fork/main" for refs/heads/main on a named remote; "main" on "."; "<remote> <dest_ref>"
    /// for another ref; the URL for a URL target.
    pub fn label(&self, d: &PushDest) -> String {
        let branch = d.dest_ref.strip_prefix("refs/heads/");
        if self.remote == "." {
            return branch.unwrap_or(&d.dest_ref).to_owned();
        }
        if d.tracking_ref.is_none() && url_like(&self.remote) {
            return self.remote.clone();
        }
        match branch {
            Some(b) => format!("{}/{b}", self.remote),
            None => format!("{} {}", self.remote, d.dest_ref),
        }
    }

    /// `git push -- <remote> refs/heads/<branch>:<dest_ref>`: only this branch, to where a
    /// plain push sends it. None when git refuses the plain push.
    pub fn explicit_cmd(&self, branch: &str) -> Option<Vec<String>> {
        let d = self.dest.as_ref().ok()?;
        Some(args(&["push", "--", &self.remote, &format!("refs/heads/{branch}:{}", d.dest_ref)]))
    }
}

/// Whether a push remote value is a URL or path git would push to as it is: it holds `:`
/// (`git@host:me/r.git`, `https://…`), `/` or `\`.
fn url_like(v: &str) -> bool {
    v.contains("://") || v.contains([':', '/', '\\'])
}

/// The remote `choose_remote` picks, with the setting that named it and a passed-over one.
#[derive(Debug, PartialEq)]
struct Chosen {
    remote: String,
    key: Option<String>,
    missing: Option<(String, String)>,
}

/// The remote a plain push of `branch` (upstream `tracking`) goes to, among `remotes`: the
/// configured push remote when it is one of them, ".", or URL-like; otherwise the upstream's.
fn choose_remote(cfg: &PushRemotes, branch: &str, tracking: &Tracking, remotes: &[&str]) -> Chosen {
    let upstream = || tracking.remote.clone();
    match cfg.for_branch(branch) {
        Some((key, r)) if r == "." || remotes.contains(&r) || url_like(r) => Chosen { remote: r.to_owned(), key: Some(key), missing: None },
        Some((key, r)) => Chosen { remote: upstream(), key: None, missing: Some((key, r.to_owned())) },
        None => Chosen { remote: upstream(), key: None, missing: None },
    }
}

/// A `branch.<b>.merge` value as a full ref.
fn merge_ref(merge: &str) -> String {
    if merge.starts_with("refs/") { merge.to_owned() } else { format!("refs/heads/{merge}") }
}

/// The ref `push.default` (`mode`) updates on `remote` for `branch`, or why git refuses.
/// Matching gives the branch's own name; whether the remote has it is `resolve`'s check.
fn by_mode(mode: &PushMode, branch: &str, remote: &str, tracking: &Tracking) -> Result<String, PushRefusal> {
    let own = format!("refs/heads/{branch}");
    let same_remote = remote == tracking.remote;
    match mode {
        PushMode::Nothing => Err(PushRefusal::Nothing),
        PushMode::Invalid(v) => Err(PushRefusal::Invalid(v.clone())),
        PushMode::Current | PushMode::Matching => Ok(own),
        PushMode::Upstream if !same_remote => Err(PushRefusal::NotUpstreamRemote { upstream_remote: tracking.remote.clone() }),
        PushMode::Upstream => Ok(merge_ref(&tracking.merge)),
        PushMode::Simple if !same_remote => Ok(own),
        PushMode::Simple if merge_ref(&tracking.merge) == own => Ok(own),
        PushMode::Simple => Err(PushRefusal::NameDiffers { merge: tracking.merge.clone() }),
    }
}

/// Resolves pushes against one load's config, reading each remote's refspecs once.
struct Resolver<'a> {
    repo: &'a Repository,
    cfg: &'a PushRemotes,
    remotes: &'a [&'a str],
    specs: HashMap<String, RemoteSpecs>,
}

impl<'a> Resolver<'a> {
    fn new(repo: &'a Repository, cfg: &'a PushRemotes, remotes: &'a [&'a str]) -> Self {
        Resolver { repo, cfg, remotes, specs: HashMap::new() }
    }

    fn resolve(&mut self, b: &BranchInfo) -> Option<PushTarget> {
        let tracking = b.tracking.as_ref()?;
        let Chosen { remote, key, missing } = choose_remote(self.cfg, &b.name, tracking, self.remotes);
        let dest = self.dest(b, tracking, &remote);
        Some(PushTarget { remote, key, missing, dest })
    }

    fn dest(&mut self, b: &BranchInfo, tracking: &Tracking, remote: &str) -> Result<PushDest, PushRefusal> {
        let here = remote == ".";
        let named = self.remotes.contains(&remote);
        if !here && !named && !url_like(remote) {
            return Err(PushRefusal::NoRemote);
        }
        let full = format!("refs/heads/{}", b.name);
        let repo = self.repo;
        let specs = named.then(|| &*self.specs.entry(remote.to_owned()).or_insert_with(|| refspecs::read(repo, remote)));
        let (dest_ref, rule) = match specs {
            Some(s) if !s.push.is_empty() => {
                (refspecs::push_dest(&s.push, &full, b.is_head).ok_or(PushRefusal::NotInRefspecs)?, PushRule::Refspec)
            }
            Some(s) if s.mirror => (full, PushRule::Mirror),
            _ => {
                let rule = if self.cfg.mode == PushMode::Matching { PushRule::Matching } else { PushRule::Default };
                (by_mode(&self.cfg.mode, &b.name, remote, tracking)?, rule)
            }
        };
        let tracking_ref = if here { Some(dest_ref.clone()) } else { specs.and_then(|s| refspecs::tracking_ref(&s.fetch, &dest_ref)) };
        let tip = tracking_ref.as_deref().and_then(|r| repo.refname_to_id(r).ok());
        if rule == PushRule::Matching && tip.is_none() {
            return Err(PushRefusal::NotOnRemote);
        }
        Ok(PushDest { dest_ref, tracking_ref, tip, ahead: 0, behind: 0, rule })
    }
}

/// Where a plain `git push` of `b` would go were it checked out, among the configured
/// `remotes`, from `cfg` and the remotes' own settings; None when `b` has no upstream
/// configured. The counts are left at 0 (`fill` sets them).
pub fn resolve(repo: &Repository, cfg: &PushRemotes, remotes: &[&str], b: &BranchInfo) -> Option<PushTarget> {
    Resolver::new(repo, cfg, remotes).resolve(b)
}

/// Sets `BranchInfo::push` of every branch in `locals`, with the counts against the push
/// target's tracking ref: the upstream's own when it is that ref, otherwise from `cache`.
pub(super) fn fill(repo: &Repository, cache: &GraphCache, locals: &mut [BranchInfo], cfg: &PushRemotes, remotes: &[&str]) {
    let mut resolver = Resolver::new(repo, cfg, remotes);
    for b in locals.iter_mut() {
        let mut push = resolver.resolve(b);
        if let Some(d) = push.as_mut().and_then(|p| p.dest.as_mut().ok()) {
            if d.tracking_ref.is_some() && d.tracking_ref == b.upstream_ref {
                (d.ahead, d.behind) = (b.ahead, b.behind);
            } else if let (Some(ours), Some(tip)) = (b.oid, d.tip) {
                (d.ahead, d.behind) = cache.ahead_behind(repo, ours, tip);
            }
        }
        b.push = push;
    }
}

#[cfg(test)]
#[path = "push_target_rules_tests.rs"]
mod rules_tests;
#[cfg(test)]
#[path = "push_target_tests.rs"]
mod tests;
