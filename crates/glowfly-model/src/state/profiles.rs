//! Preferences > Profile: which profile the page edits, the text buffers of its fields, the
//! inline rename and New profile forms, and the texts the page, its dialog and toasts show
//! (and the Integrations page's line naming the profile its accounts belong to).

use crate::state::prefs::GitConfigValues;
use glowfly_core::forge::Connection;
use glowfly_core::identity::Identity;
use glowfly_core::settings::{ProfileId, Settings};

pub const DELETE_TITLE: &str = "Delete profile";
pub const DELETE_BUTTON: &str = "Delete";
pub const STILL_SWITCHING: &str = "A profile switch is still running.";
pub const GONE_AFTER_SWITCH: &str = "Switched Git identity, but that profile no longer exists.";

pub const LIST_LABEL: &str = "Profiles";
pub const LIST_HELP: &str = "Each profile keeps an author identity, connected accounts and app preferences. Switching writes the identity to your global Git config and brings back the profile's accounts and preferences.";
pub const CURRENT_CHIP: &str = "Current profile";
pub const NO_IDENTITY: &str = "No author identity";
pub const SWITCH_BUTTON: &str = "Switch to this profile";
pub const RENAME_BUTTON: &str = "Rename";
pub const DELETE_DISABLED: &str = "Switch to another profile before deleting this one";
pub const ADD_BUTTON: &str = "Add profile";
pub const NEW_LABEL: &str = "New profile";
pub const NAME_LABEL: &str = "Profile name";
pub const NEW_NAME_HINT: &str = "Work";
pub const COPY_CURRENT: &str = "Copy the identity of the current profile";
pub const COPY_ACCOUNTS: &str = "Start with the current profile's connected accounts";
pub const CURRENT_HELP: &str =
    "Changes to the current profile are written to your global Git config (user.name, user.email, user.signingkey) when you save.";
pub const OTHER_HELP: &str = "Saved with the profile. Written to your global Git config when you switch to it.";
pub const KEY_HELP: &str = "Leave empty to keep whatever key Git has when you switch to this profile.";
pub const ACCOUNTS_LABEL: &str = "Accounts";
pub const PREFS_LABEL: &str = "Preferences";
pub const NO_ACCOUNTS: &str = "No connected accounts";
pub const ACCOUNTS_INHERIT: &str = "Uses the current profile's accounts until you switch to it";
pub const ACCOUNTS_CURRENT_HELP: &str = "Connect or disconnect accounts in Integrations; they belong to this profile.";
pub const ACCOUNTS_OTHER_HELP: &str = "Connected again when you switch to this profile. Sign-ins are shared per account.";
pub const PREFS_CURRENT_HELP: &str = "Your General, Appearance and Diff viewer settings are saved with this profile.";
pub const PREFS_SAVED_HELP: &str =
    "General, Appearance and Diff viewer settings are saved with this profile; they come back when you switch to it.";
pub const PREFS_INHERIT_HELP: &str = "None saved yet: switching to this profile keeps the current preferences.";

pub fn delete_body(name: &str, rules: usize) -> String {
    match rules {
        0 => format!(
            "Delete the profile \"{name}\"? Its saved accounts and preferences go with it. Your Git config is not changed. This can't be undone."
        ),
        n => {
            let rules = if n == 1 { "Its folder rule is".to_owned() } else { format!("Its {n} folder rules are") };
            format!(
                "Delete the profile \"{name}\"? Its saved accounts and preferences go with it. {rules} removed from your Git config. This can't be undone."
            )
        }
    }
}

/// The Integrations page's line over the connection card.
pub fn accounts_owner(name: &str) -> String {
    format!("Accounts connected here belong to the profile {name}.")
}

/// One connected account on the Profile page.
pub fn account_line(c: &Connection) -> String {
    format!("{} · {} · {}", c.account.provider.name(), c.account.display(), c.user)
}

/// The accounts a profile has: the ones in use for the current profile, else what it saved.
pub enum SetupView<'a> {
    Live(&'a [Connection]),
    Saved(&'a [Connection]),
    /// Nothing saved: switching to it keeps the current accounts.
    Inherit,
}

pub fn accounts_of(settings: &Settings, id: ProfileId) -> SetupView<'_> {
    if settings.active_profile == Some(id) {
        return SetupView::Live(&settings.connections);
    }
    match settings.profile(id).and_then(|p| p.connections.as_deref()) {
        Some(list) => SetupView::Saved(list),
        None => SetupView::Inherit,
    }
}

/// The Preferences row's text for a profile.
pub fn prefs_status(settings: &Settings, id: ProfileId) -> &'static str {
    if settings.active_profile == Some(id) {
        PREFS_CURRENT_HELP
    } else if settings.profile(id).is_some_and(|p| p.prefs.is_some()) {
        PREFS_SAVED_HELP
    } else {
        PREFS_INHERIT_HELP
    }
}

pub fn switched(name: &str) -> String {
    format!("Switched to profile {name}")
}

pub fn switch_failed(name: &str, e: &str) -> String {
    format!("Could not switch to profile {name}: {e}")
}

pub fn rewrite_failed(name: &str, e: &str) -> String {
    format!("Could not write profile {name}'s identity to your Git config: {e}")
}

/// The caption over the selected profile's fields.
pub fn editing_caption(name: &str, current: bool) -> String {
    if current { format!("Editing {name} (current)") } else { format!("Editing {name}") }
}

/// The Signing Key field's hint: git's key when the profile has none of its own.
pub fn key_hint(profile_key: Option<&str>, git_key: &str) -> String {
    if profile_key.is_none() && !git_key.trim().is_empty() { format!("uses Git's key {}", git_key.trim()) } else { "optional".into() }
}

/// The profile row's second line.
pub fn identity_line(name: &str, email: &str) -> Option<String> {
    match (name.trim(), email.trim()) {
        ("", "") => None,
        (n, "") => Some(n.to_owned()),
        ("", e) => Some(format!("<{e}>")),
        (n, e) => Some(format!("{n} <{e}>")),
    }
}

/// The global identity in `v`, its signing key only when set.
pub fn git_identity(v: &GitConfigValues) -> Identity {
    let key = v.signing_key.trim();
    Identity { name: v.user_name.clone(), email: v.user_email.clone(), signing_key: (!key.is_empty()).then(|| key.to_owned()) }
}

#[derive(Default)]
pub struct ProfilesPage {
    /// The profile whose fields the page shows.
    pub selected: Option<ProfileId>,
    /// The fields' text buffers.
    pub author_name: String,
    pub author_email: String,
    pub signing_key: String,
    pub renaming: Option<RenameForm>,
    pub adding: Option<AddForm>,
    /// Why the last folder rule wasn't added (state/profile_rules.rs).
    pub rules_error: Option<String>,
}

/// A row's inline rename field.
pub struct RenameForm {
    pub id: ProfileId,
    pub name: String,
    pub error: Option<String>,
    /// Ask the view to focus the field once.
    pub focus: bool,
}

impl RenameForm {
    pub fn new(id: ProfileId, name: &str) -> Self {
        Self { id, name: name.to_owned(), error: None, focus: true }
    }
}

/// The New profile form.
pub struct AddForm {
    pub name: String,
    pub copy_current: bool,
    /// Start with the current profile's connected accounts (off: none).
    pub copy_accounts: bool,
    pub error: Option<String>,
}

impl AddForm {
    pub fn new() -> Self {
        Self { name: String::new(), copy_current: true, copy_accounts: false, error: None }
    }
}

impl Default for AddForm {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfilesPage {
    /// Selects `select`, else the kept selection while it exists, else the current profile,
    /// and fills the fields from it.
    pub fn fill(&mut self, settings: &Settings, select: Option<ProfileId>) {
        let exists = |id: &ProfileId| settings.profile(*id).is_some();
        self.selected = select.filter(exists).or(self.selected.filter(exists)).or(settings.active_profile().map(|p| p.id));
        let p = self.selected.and_then(|id| settings.profile(id));
        self.author_name = p.map(|p| p.author_name.clone()).unwrap_or_default();
        self.author_email = p.map(|p| p.author_email.clone()).unwrap_or_default();
        self.signing_key = p.and_then(|p| p.signing_key.clone()).unwrap_or_default();
    }

    /// Whether the fields differ from the selected profile's stored identity (edits the Save /
    /// Revert bar would save).
    pub fn edited(&self, settings: &Settings) -> bool {
        let Some(profile) = self.selected.and_then(|id| settings.profile(id)) else { return false };
        let differ = |a: &str, b: &str| a.trim() != b.trim();
        differ(&self.author_name, &profile.author_name)
            || differ(&self.author_email, &profile.author_email)
            || differ(&self.signing_key, profile.signing_key.as_deref().unwrap_or(""))
    }

    /// `fill(settings, None)` after a change that left the selected profile's identity alone
    /// (a rename, another profile deleted, a rewrite): edited fields of a profile that still
    /// exists keep their text, so the change doesn't throw the edits away.
    pub fn refresh(&mut self, settings: &Settings) {
        if !self.edited(settings) {
            self.fill(settings, None);
        }
    }
}
