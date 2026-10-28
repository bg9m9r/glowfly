//! Comparing the versions of the hooks that follow the working tree (follow.rs) which some
//! commits would bring in with a saved decision: the check a checkout, merge, pull, rebase or
//! pick gets before git runs (incoming.rs names the commits, pull.rs pins a pull's). Objects
//! only: no checkout, hook, filter or fsmonitor runs.

use super::incoming::{Unchecked, allows};
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
