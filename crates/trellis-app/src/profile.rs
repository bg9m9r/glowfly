//! Messages and effects of named identity profiles: Preferences > Profile and switching from
//! there or the palette (state/profiles.rs, update/profiles.rs, runner/profiles.rs,
//! ui/prefs/profiles.rs), and their folder rules with the open repository's identity
//! (state/profile_rules.rs, state/repo_identity.rs, update/profile_rules.rs,
//! ui/prefs/profile_rules.rs).

use crate::msg::Inactive;
use crate::state::prefs::GitConfigValues;
use std::path::PathBuf;
use trellis_core::identity::Identity;
use trellis_core::profile_includes::IncludePlan;
use trellis_core::repo_identity::RepoIdentity;
use trellis_core::settings::{FolderRule, ProfileId};

// `Applied` carries the config read back, as `PrefsMsg::GitConfigLoaded` does; each `Msg` is
// that size anyway, so boxing it would only add an allocation.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum ProfileMsg {
    /// The navigation row: switch Preferences to the page, seed or sync the profiles and fill
    /// the page.
    ShowPage,
    /// Edit this profile's fields on the page.
    Select(ProfileId),
    /// The New profile form's Add button: copy the current identity, and start with the current
    /// connected accounts.
    Add { name: String, copy_current: bool, copy_accounts: bool },
    /// The inline rename field was committed.
    Rename { id: ProfileId, name: String },
    /// The Delete button: ask first.
    AskDelete(ProfileId),
    /// Accepting the Delete confirmation.
    Delete(ProfileId),
    /// Write this profile's identity to the global git config and make it current.
    Switch(ProfileId),
    // ---- from the runner
    /// The switch's writes finished: the config as read back afterwards, or git's error.
    Applied { id: ProfileId, result: Result<GitConfigValues, String> },
    /// The current profile's identity, edited while a switch ran, was written afterwards.
    Rewritten { id: ProfileId, result: Result<GitConfigValues, String> },
    // ---- folder rules, from the page
    /// Add folder…: pick a folder for this profile.
    AddFolder(ProfileId),
    /// Use for this repository: a rule for the open repository's common git dir.
    AddThisRepo(ProfileId),
    /// A rule's Remove (its `path` as saved).
    RemoveFolder { id: ProfileId, path: String },
    /// Use <profile> here: ask before removing the repository's own identity.
    AskUseHere(ProfileId),
    /// Accepting that confirmation, for the repository whose git dir is `git_dir`.
    UseHere { id: ProfileId, git_dir: PathBuf },
    // ---- folder rules, from the runner
    /// A picked folder checked on disk: its rule, or why it can't have one.
    FolderChecked { id: ProfileId, result: Result<FolderRule, String> },
    /// The include files and global entries were written (or not).
    IncludesSynced(Result<(), String>),
    /// The effective identity of the repository whose git dir is `git_dir`.
    IdentityRead { git_dir: PathBuf, result: Result<RepoIdentity, String> },
    /// Its own identity was removed, then read again.
    LocalCleared { git_dir: PathBuf, result: Result<RepoIdentity, String> },
}

/// One of a profile's identity fields on the page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IdentityField {
    AuthorName,
    AuthorEmail,
    SigningKey,
}

impl IdentityField {
    /// The git config key the field is written to.
    pub fn git_key(self) -> &'static str {
        match self {
            IdentityField::AuthorName => "user.name",
            IdentityField::AuthorEmail => "user.email",
            IdentityField::SigningKey => "user.signingkey",
        }
    }
}

impl ProfileMsg {
    /// The profiles' effects are unscoped (git config is global), so nothing arrives for an
    /// inactive tab.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::ShowPage
            | Self::Select(_)
            | Self::Add { .. }
            | Self::Rename { .. }
            | Self::AskDelete(_)
            | Self::Delete(_)
            | Self::Switch(_)
            | Self::Applied { .. }
            | Self::Rewritten { .. }
            | Self::AddFolder(_)
            | Self::AddThisRepo(_)
            | Self::RemoveFolder { .. }
            | Self::AskUseHere(_)
            | Self::UseHere { .. }
            | Self::FolderChecked { .. }
            | Self::IncludesSynced(_)
            // Keyed by git dir, never scoped: stored for whichever tab shows that repository.
            | Self::IdentityRead { .. }
            | Self::LocalCleared { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum ProfileEffect {
    /// Write `identity` to the global git config, then read the config back.
    Apply { cwd: PathBuf, has_repo: bool, id: ProfileId, identity: Identity },
    /// Write the current profile's `identity` again (edited during a switch), then read the
    /// config back.
    Rewrite { cwd: PathBuf, has_repo: bool, id: ProfileId, identity: Identity },
    /// Check a picked folder (or the open repository's common git dir) for a rule.
    CheckFolder { id: ProfileId, path: PathBuf, repository: bool },
    /// Write the include files and, with `place`, put the entries at the end of the global
    /// config.
    SyncIncludes { cwd: PathBuf, plan: IncludePlan, place: bool },
    /// Read the repository's effective identity and its origin from git.
    ReadIdentity { workdir: PathBuf, git_dir: PathBuf, rules: Vec<(ProfileId, FolderRule)> },
    /// Remove the repository's own user.name and user.email, then read the identity again.
    ClearLocalIdentity { workdir: PathBuf, git_dir: PathBuf, rules: Vec<(ProfileId, FolderRule)> },
}

impl ProfileEffect {
    pub fn name(&self) -> &'static str {
        match self {
            ProfileEffect::Apply { .. } => "apply-profile",
            ProfileEffect::Rewrite { .. } => "rewrite-profile",
            ProfileEffect::CheckFolder { .. } => "check-profile-folder",
            ProfileEffect::SyncIncludes { .. } => "sync-profile-includes",
            ProfileEffect::ReadIdentity { .. } => "read-repo-identity",
            ProfileEffect::ClearLocalIdentity { .. } => "clear-local-identity",
        }
    }
}
