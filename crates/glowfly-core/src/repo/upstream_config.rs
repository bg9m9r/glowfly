//! A load's one read of the branch settings: `branch.<b>.remote` and `.merge` (a branch's
//! upstream), `branch.<b>.pushRemote`, `remote.pushDefault` and `push.default`, taken from a
//! single snapshot of the repository's whole config (the last value of a key winning, included
//! files too) instead of a few lookups per branch. The upstream a branch names is worked out as
//! libgit2's `git_branch_upstream_name` does, and found among the refs the load already read.
//! Declared from `load.rs`.

use super::push_config::{PushRemotes, branch_key};
use super::push_target::refspecs::{self, Spec};
use crate::config_entry;
use crate::repo::graph_cache::GraphCache;
use crate::repo::{BranchInfo, Tracking, is_safe_ref_name, safe_names};
use git2::{Oid, Repository};
use std::collections::HashMap;

/// The branch settings of a repository.
#[derive(Default)]
pub(super) struct BranchConfig {
    keys: HashMap<String, Keys>,
    /// What the same entries say of where a push goes.
    pub push: PushRemotes,
}

/// A branch's `remote` and `merge`; None when unset, empty, bare or not UTF-8.
#[derive(Default)]
struct Keys {
    remote: Option<String>,
    merge: Option<String>,
}

/// Reads the branch settings of `repo`; none when its config can't be read.
pub(super) fn read(repo: &Repository) -> BranchConfig {
    let read = || -> Result<BranchConfig, git2::Error> {
        let cfg = repo.config()?.snapshot()?;
        let mut out = BranchConfig::default();
        let mut entries = cfg.entries(Some(r"^(branch\..+\.(remote|merge|pushremote)|remote\.pushdefault|push\.default)$"))?;
        while let Some(e) = entries.next() {
            let e = e?;
            let Some(name) = e.name() else { continue };
            if out.push.take(name, config_entry::raw(e)) {
                continue;
            }
            // A later empty, bare or unreadable value clears an earlier one.
            let value = config_entry::utf8(e).filter(|v| !v.is_empty()).map(str::to_owned);
            if let Some(b) = branch_key(name, "remote") {
                out.keys.entry(b.to_owned()).or_default().remote = value;
            } else if let Some(b) = branch_key(name, "merge") {
                out.keys.entry(b.to_owned()).or_default().merge = value;
            }
        }
        Ok(out)
    };
    read().unwrap_or_default()
}

impl BranchConfig {
    /// The upstream `branch` is configured to track (its `merge` without `refs/heads/`).
    pub(super) fn tracking(&self, branch: &str) -> Option<Tracking> {
        let k = self.keys.get(branch)?;
        let (remote, merge) = (k.remote.as_deref()?, k.merge.as_deref()?);
        Some(Tracking { remote: remote.to_owned(), merge: merge.strip_prefix("refs/heads/").unwrap_or(merge).to_owned() })
    }

    /// The ref name libgit2 gives as the upstream of `branch`: on remote `.` the `merge` value as
    /// written, otherwise it mapped through the remote's first matching fetch spec. None without
    /// both settings, for a remote that doesn't exist and when no spec maps it.
    pub(super) fn upstream_name(&self, branch: &str, specs: &mut SpecsMemo) -> Option<String> {
        let k = self.keys.get(branch)?;
        let (remote, merge) = (k.remote.as_deref()?, k.merge.as_deref()?);
        if remote == "." {
            return Some(merge.to_owned());
        }
        refspecs::upstream_of(specs.fetch_of(remote)?, merge)
    }
}

/// Each remote's fetch specs, read once.
pub(super) struct SpecsMemo<'r> {
    repo: &'r Repository,
    by_remote: HashMap<String, Option<Vec<Spec>>>,
}

impl<'r> SpecsMemo<'r> {
    pub(super) fn new(repo: &'r Repository) -> Self {
        SpecsMemo { repo, by_remote: HashMap::new() }
    }

    /// The fetch specs of `remote`; None when it isn't a remote (one without a URL isn't).
    fn fetch_of(&mut self, remote: &str) -> Option<&[Spec]> {
        let repo = self.repo;
        let found = self
            .by_remote
            .entry(remote.to_owned())
            .or_insert_with(|| repo.find_remote(remote).is_ok().then(|| refspecs::read(repo, remote).fetch));
        found.as_deref()
    }
}

/// Sets the upstream fields of `locals` from `cfg`, finding each upstream ref among `tips` (the
/// load's refs and their direct targets) and else asking libgit2. A branch whose tracking ref is
/// missing keeps `tracking` and has no upstream. The counts come from `cache`.
pub(super) fn fill_upstreams(
    repo: &Repository,
    cache: &GraphCache,
    cfg: &BranchConfig,
    tips: &HashMap<&str, Option<Oid>>,
    locals: &mut [BranchInfo],
) {
    let mut specs = SpecsMemo::new(repo);
    for b in locals {
        // An upstream whose remote or branch name git could misread is dropped as a whole.
        b.tracking = cfg.tracking(&b.name).filter(|t| safe_names::upstream_remote_ok(&t.remote) && is_safe_ref_name(&t.merge));
        let found = b.tracking.as_ref().and_then(|_| cfg.upstream_name(&b.name, &mut specs)).and_then(|name| {
            let target = match tips.get(name.as_str()) {
                Some(t) => *t,
                // Refs the pass didn't list: the lookup libgit2 itself makes (case-folded loose
                // files, refs outside heads and remotes).
                None => repo.find_reference(&name).ok()?.target(),
            };
            Some((name, target))
        });
        let Some((name, target)) = found else { continue };
        b.upstream = name.strip_prefix("refs/heads/").or_else(|| name.strip_prefix("refs/remotes/")).map(str::to_owned);
        (b.ahead, b.behind) = match (b.oid, target) {
            (Some(ours), Some(theirs)) => cache.ahead_behind(repo, ours, theirs),
            _ => (0, 0),
        };
        b.upstream_ref = Some(name);
    }
}

#[cfg(test)]
#[path = "upstream_config_tests.rs"]
mod tests;
