//! What a trust decision was made on: a fingerprint of the scan the user saw. When a later scan
//! finds something the fingerprint lacks (a new setting or hook, a changed value), the decision
//! no longer covers the repository and Glowfly asks again. Each item is kept only as a short
//! hash, so the saved settings hold no command lines.

use super::{ConfigCommand, TrustScan, hook_base};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

/// The hashed items of a scan: one per setting (key and value) and one per hook (by name, in
/// any letter case), and the same for each checked-out submodule under its path. Hook script
/// contents are not part of it.
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
}

/// The first 8 bytes of the SHA-256 of `item`, as 16 lowercase hex characters.
pub(crate) fn hash(item: &str) -> String {
    Sha256::digest(item.as_bytes())[..8].iter().map(|b| format!("{b:02x}")).collect()
}

fn config_item(prefix: &str, c: &ConfigCommand) -> String {
    format!("{prefix}c\0{}\0{}", c.key, c.value)
}

fn hook_item(prefix: &str, name: &str) -> String {
    format!("{prefix}h\0{}", hook_base(name))
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

    /// What this scan found that `p` lacks: the new or changed settings and new hooks (with
    /// `hooks_dir` when any remain), and the submodules with such findings.
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
    out.extend(scan.hooks.iter().map(|h| hash(&hook_item(prefix, h))));
}

/// `scan`'s own settings and hooks whose items under `prefix` aren't in `p`.
fn gained_own(scan: &TrustScan, prefix: &str, p: &TrustPrint) -> TrustScan {
    let config: Vec<ConfigCommand> = scan.config.iter().filter(|c| !p.0.contains(&hash(&config_item(prefix, c)))).cloned().collect();
    let hooks: Vec<String> = scan.hooks.iter().filter(|h| !p.0.contains(&hash(&hook_item(prefix, h)))).cloned().collect();
    let hooks_dir = if hooks.is_empty() { None } else { scan.hooks_dir.clone() };
    TrustScan { config, hooks, hooks_dir, submodules: Vec::new() }
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
