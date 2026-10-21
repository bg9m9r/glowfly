//! Named profiles (Preferences > Profile): each keeps an author identity, and the current one's
//! identity is the global git config's; other profiles also keep the connected accounts and app
//! preferences saved when they were last current (`profiles_prefs`), and its folder rules
//! (`profile_rules`). Checks for names, seeding the first profile from git, keeping the current
//! one in step with git, and adding, renaming and deleting.

use super::{ProfilePrefs, Settings};
use crate::forge::Connection;
use crate::identity::Identity;
use serde::{Deserialize, Serialize};

/// A profile's stable id in the settings file; never shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProfileId(pub u32);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Profile {
    pub id: ProfileId,
    pub name: String,
    pub author_name: String,
    pub author_email: String,
    /// None: switching leaves git's user.signingkey as it is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signing_key: Option<String>,
    /// The connected accounts saved while another profile is current (the current one's are
    /// `Settings.connections`). None: nothing saved, so switching to it keeps the current list.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connections: Option<Vec<Connection>>,
    /// The app preferences saved while another profile is current, with the same None rule.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefs: Option<ProfilePrefs>,
    /// The folders (and repositories) whose repositories commit with this profile's identity,
    /// through includeIf.gitdir entries in the global config (`profile_includes`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub folders: Vec<FolderRule>,
}

/// One folder rule of a profile: repositories whose git dir is in `path` commit as the profile.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FolderRule {
    /// `paths::encode`d, resolved on disk when added; for `repository`, the common git dir.
    pub path: String,
    /// Added with Use for this repository: `path` is the repository's common git dir.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub repository: bool,
    /// The folder's file system folds case (probed when added): written as gitdir/i.
    pub ignore_case: bool,
}

impl Profile {
    pub fn identity(&self) -> Identity {
        Identity { name: self.author_name.clone(), email: self.author_email.clone(), signing_key: self.signing_key.clone() }
    }

    pub fn set_identity(&mut self, id: &Identity) {
        self.author_name = id.name.clone();
        self.author_email = id.email.clone();
        self.signing_key = id.signing_key.clone();
    }
}

/// Why a profile change was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileProblem {
    Blank,
    Taken,
    Current,
    Missing,
}

impl ProfileProblem {
    pub fn message(self) -> &'static str {
        match self {
            ProfileProblem::Blank => "Enter a profile name.",
            ProfileProblem::Taken => "Another profile already has this name.",
            ProfileProblem::Current => "Switch to another profile before deleting this one.",
            ProfileProblem::Missing => "That profile no longer exists.",
        }
    }
}

/// The name of the profile made from the global identity.
pub const DEFAULT_PROFILE_NAME: &str = "Default";

impl Settings {
    pub fn profile(&self, id: ProfileId) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    pub fn profile_mut(&mut self, id: ProfileId) -> Option<&mut Profile> {
        self.profiles.iter_mut().find(|p| p.id == id)
    }

    /// The current profile, if `active_profile` names one.
    pub fn active_profile(&self) -> Option<&Profile> {
        self.active_profile.and_then(|id| self.profile(id))
    }

    /// The trimmed `name`, if it is usable for a profile other than `except`: not blank, and
    /// not another profile's name in any case.
    pub fn check_profile_name(&self, name: &str, except: Option<ProfileId>) -> Result<String, ProfileProblem> {
        let name = name.trim();
        if name.is_empty() {
            return Err(ProfileProblem::Blank);
        }
        let lower = name.to_lowercase();
        if self.profiles.iter().any(|p| Some(p.id) != except && p.name.trim().to_lowercase() == lower) {
            return Err(ProfileProblem::Taken);
        }
        Ok(name.to_owned())
    }

    /// Makes the first profile from `git` when there are none; otherwise makes sure one is
    /// current and gives it git's name and email (and its key, if it keeps one). Returns
    /// whether anything changed.
    pub fn seed_or_sync_profiles(&mut self, git: &Identity) -> bool {
        if self.profiles.is_empty() {
            let mut p = Profile { id: ProfileId(1), name: DEFAULT_PROFILE_NAME.into(), ..Profile::default() };
            p.set_identity(git);
            self.profiles.push(p);
            self.active_profile = Some(ProfileId(1));
            return true;
        }
        let mut changed = false;
        if self.active_profile().is_none() {
            self.active_profile = Some(self.profiles[0].id);
            changed = true;
        }
        let Some(p) = self.active_profile.and_then(|id| self.profiles.iter_mut().find(|p| p.id == id)) else { return changed };
        if p.author_name != git.name || p.author_email != git.email {
            p.author_name = git.name.clone();
            p.author_email = git.email.clone();
            changed = true;
        }
        if p.signing_key.is_some() {
            let key = git.signing_key.clone().filter(|k| !k.trim().is_empty());
            if p.signing_key != key {
                p.signing_key = key;
                changed = true;
            }
        }
        changed
    }

    /// Adds a profile (not made current) with these connected accounts and returns its id. Its
    /// preferences start unsaved, so it takes the current ones when first switched to.
    pub fn add_profile(&mut self, name: &str, identity: Identity, connections: Vec<Connection>) -> Result<ProfileId, ProfileProblem> {
        let name = self.check_profile_name(name, None)?;
        let id = ProfileId(self.profiles.iter().map(|p| p.id.0).max().unwrap_or(0) + 1);
        let mut p = Profile { id, name, connections: Some(connections), ..Profile::default() };
        p.set_identity(&identity);
        self.profiles.push(p);
        Ok(id)
    }

    pub fn rename_profile(&mut self, id: ProfileId, name: &str) -> Result<(), ProfileProblem> {
        if self.profile(id).is_none() {
            return Err(ProfileProblem::Missing);
        }
        let name = self.check_profile_name(name, Some(id))?;
        if let Some(p) = self.profile_mut(id) {
            p.name = name;
        }
        Ok(())
    }

    /// Removes a profile other than the current one (so never the last) and returns it.
    pub fn delete_profile(&mut self, id: ProfileId) -> Result<Profile, ProfileProblem> {
        if self.active_profile == Some(id) {
            return Err(ProfileProblem::Current);
        }
        let i = self.profiles.iter().position(|p| p.id == id).ok_or(ProfileProblem::Missing)?;
        if self.profiles.len() == 1 {
            return Err(ProfileProblem::Current);
        }
        Ok(self.profiles.remove(i))
    }
}

#[cfg(test)]
#[path = "profiles_tests.rs"]
mod tests;
