//! The git Glowfly runs, as `update` and the views see it: found and asked its version at
//! startup and whenever another one is chosen, then shown in Preferences and, while it is missing
//! or too old, in a banner. Also the Preferences > Git page's scan and check.

use glowfly_core::git_program::Candidate;
use glowfly_core::git_version::{self, Feature, GitInfo, Version};
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct GitState {
    /// None until the startup check answers.
    pub info: Option<GitInfo>,
    /// The banner was closed (until Glowfly restarts or another git is chosen).
    pub banner_dismissed: bool,
    /// The PATH git checked while the saved git can't run (from the background check, or from
    /// the latest scan), for the banner's offer.
    pub path_git: Option<Candidate>,
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
    /// The check came from the banner's offer, so its success is toasted.
    pub from_banner: bool,
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

    /// The git saved in Preferences > Git can't run (missing, a folder, broken).
    pub fn saved_git_unusable(&self) -> bool {
        self.info.as_ref().is_some_and(|i| i.chosen && i.version.is_none())
    }

    /// The PATH git the banner offers, with its version: only while the saved git can't run and
    /// the PATH git works and is new enough ([`git_version::new_enough`]).
    pub fn path_offer(&self) -> Option<(&Path, Version)> {
        if !self.saved_git_unusable() {
            return None;
        }
        let c = self.path_git.as_ref()?;
        match c.check {
            Ok(v) if git_version::new_enough(v) => Some((c.path.as_path(), v)),
            _ => None,
        }
    }

    /// The banner's text, unless it was closed or all is well.
    pub fn banner(&self) -> Option<String> {
        if self.banner_dismissed {
            return None;
        }
        self.info.as_ref().and_then(GitInfo::banner)
    }
}
