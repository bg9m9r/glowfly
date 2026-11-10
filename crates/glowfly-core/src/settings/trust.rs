//! Repository trust decisions (see `trust`): what the user answered for a repository, when,
//! and which decisions were passed on from another repository (listed, revoked and forgotten
//! from Preferences > Trusted repositories).

use super::{Settings, repo_keys};
use crate::paths;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The user's answer to "Trust this repository?".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepoTrust {
    /// Git may run the programs the repository's config and hooks name, on its own too.
    Trusted,
    /// Glowfly doesn't fetch, sign in or let git read the status on its own here.
    NotTrusted,
}

/// A saved decision with what it was made on.
#[derive(Clone, Debug, PartialEq)]
pub struct SavedTrust {
    pub trust: RepoTrust,
    /// The print of the scan a Trusted decision was made on (`trust::TrustPrint`); None for
    /// Don't trust, and for decisions saved before prints were kept.
    pub print: Option<crate::trust::TrustPrint>,
}

impl SavedTrust {
    /// Trusted, made on `print`.
    pub fn trusted(print: Option<crate::trust::TrustPrint>) -> Self {
        Self { trust: RepoTrust::Trusted, print }
    }

    pub fn is_trusted(&self) -> bool {
        self.trust == RepoTrust::Trusted
    }

    /// Whether this is Trusted and its print covers `now`. A Trusted decision without a print
    /// (saved before prints were kept, or recorded where no scan was at hand) counts as made on
    /// a scan that found nothing: it covers only what runs nothing, so it asks again once.
    pub fn covers(&self, now: &crate::trust::TrustPrint) -> bool {
        self.is_trusted() && self.print.as_ref().map_or_else(|| now.is_empty(), |p| p.covers(now))
    }
}

/// One saved decision as Preferences > Trusted repositories lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct TrustDecision {
    /// The repository's folder (the decoded key).
    pub path: PathBuf,
    pub trust: RepoTrust,
    /// When it was decided (Unix seconds); None for decisions saved before times were kept.
    pub at: Option<i64>,
    /// The repository it was passed on from (a worktree's or submodule's source), if inherited.
    pub from: Option<PathBuf>,
    /// How many decisions were passed on from this one, transitively.
    pub heirs: usize,
}

impl Settings {
    /// Records when the decision for `real` was made and where it was passed on from (None for
    /// the user's own decision); a None `at` removes the time.
    pub fn stamp_trust(&mut self, real: &Path, at: Option<i64>, from: Option<&Path>) {
        repo_keys::set(&mut self.repo_trust_times, real, at);
        repo_keys::set(&mut self.repo_trust_from, real, from.map(repo_keys::key));
    }

    /// Every saved decision, sorted by path (ignoring case first, then exactly).
    pub fn trust_decisions(&self) -> Vec<TrustDecision> {
        let mut out: Vec<TrustDecision> = self
            .repo_trust
            .iter()
            .map(|(k, trust)| {
                let path = paths::decode(k);
                TrustDecision {
                    trust: *trust,
                    at: repo_keys::get(&self.repo_trust_times, &path).copied(),
                    from: repo_keys::get(&self.repo_trust_from, &path).map(|f| paths::decode(f)),
                    heirs: self.heirs_of(&path).len(),
                    path,
                }
            })
            .collect();
        out.sort_by_cached_key(|d| {
            let shown = d.path.to_string_lossy().into_owned();
            (shown.to_lowercase(), shown)
        });
        out
    }

    /// The decided repositories whose decision was passed on from `real`, directly or through
    /// another heir. Only entries that still have a decision count; cycles are visited once.
    pub fn heirs_of(&self, real: &Path) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = Vec::new();
        let mut todo = vec![real.to_path_buf()];
        while let Some(source) = todo.pop() {
            for (k, from) in &self.repo_trust_from {
                let heir = paths::decode(k);
                let new = paths::same_spelling(&paths::decode(from), &source)
                    && repo_keys::get(&self.repo_trust, &heir).is_some()
                    && !paths::same_spelling(&heir, real)
                    && !out.iter().any(|h| paths::same_spelling(h, &heir));
                if new {
                    todo.push(heir.clone());
                    out.push(heir);
                }
            }
        }
        out
    }

    /// Removes the decision for `real` and every decision passed on from it (with their prints,
    /// times and sources). Returns the folders whose decision was removed, `real` first; empty
    /// when `real` had none.
    pub fn revoke_trust(&mut self, real: &Path) -> Vec<PathBuf> {
        let Some(own) = self.repo_trust.keys().map(|k| paths::decode(k)).find(|p| paths::same_spelling(p, real)) else {
            return Vec::new();
        };
        let mut gone = vec![own];
        gone.extend(self.heirs_of(real));
        for p in &gone {
            self.set_trust(p, None);
        }
        gone
    }
}

#[cfg(test)]
#[path = "trust_tests.rs"]
mod tests;
