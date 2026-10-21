//! The git Glowfly runs, as `update` and the views see it: found and asked its version at
//! startup and whenever another one is chosen, then shown in Preferences and, while it is missing
//! or too old, in a banner. Also the Preferences > Git page's scan and check.

use glowfly_core::git_program::Candidate;
use glowfly_core::git_version::{Feature, GitInfo};
use std::path::PathBuf;

#[derive(Default)]
pub struct GitState {
    /// None until the startup check answers.
    pub info: Option<GitInfo>,
    /// The banner was closed (until Glowfly restarts or another git is chosen).
    pub banner_dismissed: bool,
    pub page: GitPage,
}

/// Preferences > Git: the gits found and the one being checked.
#[derive(Default)]
pub struct GitPage {
    /// Numbers each scan, so an older answer is dropped.
    pub generation: u64,
    /// None while a scan runs.
    pub candidates: Option<Vec<Candidate>>,
    pub checking: Option<Checking>,
    /// The last choice that was refused: (its shown path, why).
    pub refusal: Option<(String, String)>,
}

/// A chosen git being checked before it is used.
pub struct Checking {
    /// None: git from PATH.
    pub program: Option<PathBuf>,
}

impl GitPage {
    /// The git a bare `git` runs, when the scan found one.
    pub fn path_git(&self) -> Option<&Candidate> {
        self.candidates.as_ref()?.iter().find(|c| c.first_on_path)
    }
}

impl GitState {
    /// Whether git has `f`; true while unknown, so nothing is held up before the check answers
    /// (git's own error explains a failure then).
    pub fn has(&self, f: Feature) -> bool {
        self.info.as_ref().is_none_or(|i| i.has(f))
    }

    /// Why `f` is off ("Sparse checkout needs git 2.35 or newer (you have 2.30.1)."), or None.
    pub fn blocked(&self, f: Feature) -> Option<String> {
        (!self.has(f)).then(|| f.needs_text(self.info.as_ref().and_then(|i| i.version)))
    }

    /// The check answered and git couldn't be run at all (no version), not merely too old.
    pub fn git_unusable(&self) -> bool {
        self.info.as_ref().is_some_and(|i| i.version.is_none())
    }

    /// The banner's text, unless it was closed or all is well.
    pub fn banner(&self) -> Option<String> {
        if self.banner_dismissed {
            return None;
        }
        self.info.as_ref().and_then(GitInfo::banner)
    }
}
