//! Messages and effects of named identity profiles: Preferences > Profile and switching from
//! there or the palette (state/profiles.rs, update/profiles.rs, runner/profiles.rs,
//! ui/prefs/profiles.rs).

use crate::msg::Inactive;
use crate::state::prefs::GitConfigValues;
use std::path::PathBuf;
use trellis_core::identity::Identity;
use trellis_core::settings::ProfileId;

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
    /// The New profile form's Add button.
    Add { name: String, copy_current: bool },
    /// The inline rename field was committed.
    Rename { id: ProfileId, name: String },
    /// An identity field lost focus with this text.
    SetIdentity { id: ProfileId, field: IdentityField, value: String },
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
            | Self::SetIdentity { .. }
            | Self::AskDelete(_)
            | Self::Delete(_)
            | Self::Switch(_)
            | Self::Applied { .. }
            | Self::Rewritten { .. } => None,
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
}

impl ProfileEffect {
    pub fn name(&self) -> &'static str {
        match self {
            ProfileEffect::Apply { .. } => "apply-profile",
            ProfileEffect::Rewrite { .. } => "rewrite-profile",
        }
    }
}
