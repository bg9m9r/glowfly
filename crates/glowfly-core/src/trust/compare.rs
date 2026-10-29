//! Comparing the versions of the hooks that follow the working tree (follow.rs) which some
//! commits would bring in with a saved decision: the check a checkout, merge, pull, rebase or
//! pick gets before git runs (incoming.rs names the commits, merged.rs makes the merges, pull.rs
//! pins a pull's). Objects only: no checkout, hook, filter or fsmonitor runs.

use super::incoming::{Target, Unchecked, allows};
use super::{Follow, TrustScan, follow};
use crate::settings::SavedTrust;
use git2::{Oid, Repository};
use std::collections::HashSet;

/// Why commits can't be brought in as things stand.
#[derive(Clone, Debug, PartialEq)]
pub enum Refused {
    /// One of them has hooks the decision doesn't cover: what it has, for the prompt.
    Held(Box<TrustScan>),
    /// They couldn't be compared (see `Unchecked`).
    Unchecked(Unchecked),
}

/// Whether `saved` covers what each of the trees or commits `oids` has where `follows` points
/// (a version seen once is compared once). `Err` names the first that isn't covered, or why
/// one couldn't be read.
pub fn check(repo: &Repository, follows: &[Follow], saved: Option<&SavedTrust>, oids: &[Oid]) -> Result<(), Refused> {
    if follows.is_empty() {
        return Ok(());
    }
    let mut seen: HashSet<Vec<Follow>> = HashSet::new();
    for oid in oids {
        let versions =
            follow::versions(repo, follows, *oid).map_err(|e| Refused::Unchecked(Unchecked::Unreadable(e.message().to_owned())))?;
        if !seen.insert(versions.clone()) {
            continue;
        }
        if !allows(saved, &TrustScan { follows: versions, ..TrustScan::default() }) {
            let found =
                follow::incoming(repo, follows, *oid).map_err(|e| Refused::Unchecked(Unchecked::Unreadable(e.message().to_owned())))?;
            return Err(Refused::Held(Box::new(found)));
        }
    }
    Ok(())
}

/// `r`, with the one reason `accept` (what the person chose to run anyway) passed: only the
/// item that couldn't be compared for that reason is skipped, never the rest.
pub fn skip_accepted<T: Default>(r: Result<T, Unchecked>, accept: Option<&Unchecked>) -> Result<T, Refused> {
    match r {
        Err(u) if accept == Some(&u) => Ok(T::default()),
        r => r.map_err(Refused::Unchecked),
    }
}

/// Unmodeled when local changes to a place hooks run from would be stashed and put back on
/// what the command leaves (`--autostash`): the merge of the two isn't made here.
pub fn autostash(repo: &Repository, follows: &[Follow]) -> Result<(), Unchecked> {
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(false).exclude_submodules(true);
    let statuses = repo.statuses(Some(&mut opts)).map_err(|e| Unchecked::Unreadable(e.message().to_owned()))?;
    let local = statuses.iter().filter(|s| s.status() != git2::Status::CURRENT).filter_map(|s| s.path().map(str::to_owned));
    match local.into_iter().find(|p| follow::touches(p, follows)) {
        Some(p) => Err(Unchecked::Unmodeled(format!("--autostash of local changes in {p}"))),
        None => Ok(()),
    }
}

/// `check` for what each of `targets` brings in (incoming.rs), and for the merges each makes,
/// what they leave (merged.rs). `accept`: the one reason an item couldn't be compared that the
/// person chose to run anyway; every other item is still compared.
pub fn check_targets(
    repo: &Repository,
    follows: &[Follow],
    saved: Option<&SavedTrust>,
    targets: &[Target],
    accept: Option<&Unchecked>,
) -> Result<(), Refused> {
    if follows.is_empty() {
        return Ok(());
    }
    let mut oids = Vec::new();
    for t in targets {
        oids.extend(skip_accepted(super::incoming::commits(repo, follows, t), accept)?);
    }
    check(repo, follows, saved, &oids)?;
    let config =
        repo.config().and_then(|mut c| c.snapshot()).map_err(|e| Refused::Unchecked(Unchecked::Unreadable(e.message().to_owned())))?;
    for t in targets {
        let stashes = match t {
            Target::Merged { how, .. } => how.autostash || config.get_bool("merge.autoStash").unwrap_or(false),
            Target::Replayed { how, .. } => how.autostash || config.get_bool("rebase.autoStash").unwrap_or(false),
            _ => false,
        };
        if stashes {
            skip_accepted(autostash(repo, follows), accept)?;
        }

        let Some(chain) = skip_accepted(super::merged::steps(repo, t), accept)? else { continue };
        match super::merged::check(repo, follows, saved, &chain) {
            Err(Refused::Unchecked(u)) if accept == Some(&u) => {}
            r => r?,
        }
    }
    Ok(())
}
