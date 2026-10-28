//! What a trust decision was made on: a fingerprint of the scan the user saw. When a later scan
//! finds something the fingerprint lacks (a new setting or hook, a changed value), the decision
//! no longer covers the repository and Glowfly asks again. Each item is kept only as a short
//! hash, so the saved settings hold no command lines.

use super::{ConfigCommand, TrustScan, hook_base};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

/// The hashed items of a scan: one per setting (key and value), one per hook (its name in any
/// letter case, with its script's digest), one for the repository's commands file (its
/// digest) and one per place in the working tree hooks run from (with the version tracked
/// there), and the same for each checked-out submodule under its path.
///
/// Prints saved before hook contents and the commands file were part of it hold hooks by name
/// alone: those items never match now, so a repository trusted with hooks or a commands file
/// asks once more, listing them, rather than being trusted on what nobody saw.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TrustPrint(pub BTreeSet<String>);

impl TrustPrint {
    /// Whether everything in `now` was already in this print (items that are gone don't count).
    pub fn covers(&self, now: &TrustPrint) -> bool {
        now.0.is_subset(&self.0)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Whether this print covers the repository's commands file whose contents' digest is
    /// `digest` (`trust::content_digest`): it was trusted with exactly those contents.
    pub fn covers_commands(&self, digest: &str) -> bool {
        self.0.contains(&hash(&commands_item("", digest)))
    }

    /// The print of a scan that found only a commands file with this digest.
    pub fn of_commands(digest: &str) -> TrustPrint {
        TrustPrint([hash(&commands_item("", digest))].into())
    }

    /// This print with `other`'s items too: everything either was made on.
    pub fn with(mut self, other: &TrustPrint) -> TrustPrint {
        self.0.extend(other.0.iter().cloned());
        self
    }
}

/// The first 8 bytes of the SHA-256 of `item`, as 16 lowercase hex characters.
pub(crate) fn hash(item: &str) -> String {
    Sha256::digest(item.as_bytes())[..8].iter().map(|b| format!("{b:02x}")).collect()
}

fn config_item(prefix: &str, c: &ConfigCommand) -> String {
    format!("{prefix}c\0{}\0{}", c.key, c.value)
}

fn hook_item(prefix: &str, scan: &TrustScan, name: &str) -> String {
    // A hook without a digest was never read: it can't match what a read gave.
    let digest = scan.hook_digests.get(name).map_or("unread", String::as_str);
    format!("{prefix}h\0{}\0{digest}", hook_base(name))
}

/// A place hooks run from inside the working tree, with the version tracked there.
pub(crate) fn follow_item(prefix: &str, f: &super::Follow) -> String {
    match &f.tracked {
        Some((path, oid)) => format!("{prefix}f\0{}\0{path}\0{oid}", f.path),
        None => format!("{prefix}f\0{}\0-", f.path),
    }
}

fn commands_item(prefix: &str, digest: &str) -> String {
    format!("{prefix}x\0{digest}")
}

fn submodule_prefix(path: &str) -> String {
    format!("s\0{path}\0")
}

impl TrustScan {
    /// The fingerprint of this scan.
    pub fn print(&self) -> TrustPrint {
        let mut out = BTreeSet::new();
        own_items(self, "", &mut out);
        for sub in &self.submodules {
            own_items(&sub.scan, &submodule_prefix(&sub.path), &mut out);
        }
        TrustPrint(out)
    }

    /// What this scan found that `p` lacks: the new or changed settings, hooks and commands
    /// file (with `hooks_dir` when any hooks remain), and the submodules with such findings.
    pub fn gained_since(&self, p: &TrustPrint) -> TrustScan {
        let mut out = gained_own(self, "", p);
        out.submodules = self
            .submodules
            .iter()
            .filter_map(|sub| {
                let scan = gained_own(&sub.scan, &submodule_prefix(&sub.path), p);
                (!scan.is_empty()).then(|| super::SubmoduleScan { path: sub.path.clone(), scan })
            })
            .collect();
        out
    }
}

/// The hashed items of `scan`'s own settings and hooks (not its submodules), under `prefix`.
fn own_items(scan: &TrustScan, prefix: &str, out: &mut BTreeSet<String>) {
    out.extend(scan.config.iter().map(|c| hash(&config_item(prefix, c))));
    out.extend(scan.hooks.iter().map(|h| hash(&hook_item(prefix, scan, h))));
    out.extend(scan.commands.iter().map(|d| hash(&commands_item(prefix, d))));
    out.extend(scan.follows.iter().map(|f| hash(&follow_item(prefix, f))));
}

/// `scan`'s own settings and hooks whose items under `prefix` aren't in `p`.
fn gained_own(scan: &TrustScan, prefix: &str, p: &TrustPrint) -> TrustScan {
    let config: Vec<ConfigCommand> = scan.config.iter().filter(|c| !p.0.contains(&hash(&config_item(prefix, c)))).cloned().collect();
    let hooks: Vec<String> = scan.hooks.iter().filter(|h| !p.0.contains(&hash(&hook_item(prefix, scan, h)))).cloned().collect();
    let hook_digests = hooks.iter().filter_map(|h| Some((h.clone(), scan.hook_digests.get(h)?.clone()))).collect();
    let hooks_dir = if hooks.is_empty() { None } else { scan.hooks_dir.clone() };
    let commands = scan.commands.clone().filter(|d| !p.0.contains(&hash(&commands_item(prefix, d))));
    let follows = scan.follows.iter().filter(|f| !p.0.contains(&hash(&follow_item(prefix, f)))).cloned().collect();
    TrustScan { config, hooks, hook_digests, hooks_dir, submodules: Vec::new(), commands, follows }
}

/// Whether git may run in a repository whose saved decision is `saved` and whose scan is
/// `scan`: no decision allows only a repository where nothing runs; Trusted allows while its
/// print (if it has one) covers the scan; Don't trust never does.
pub fn allows(saved: Option<&crate::settings::SavedTrust>, scan: &TrustScan) -> bool {
    use crate::settings::RepoTrust;
    match saved {
        None => scan.is_empty(),
        Some(s) if s.trust == RepoTrust::Trusted => s.print.as_ref().is_none_or(|p| p.covers(&scan.print())),
        Some(_) => false,
    }
}

#[cfg(test)]
#[path = "print_tests.rs"]
mod tests;
