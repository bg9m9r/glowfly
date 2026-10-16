//! What switches with a profile besides its identity: the app preferences (General, UI
//! Customization and Editor, except word wrap) and the connected hosting-service accounts.
//! Everything else in `Settings` (repository-specific settings, trust, favorites, tools, diff
//! header toggles, window and dialog state) stays global; a test makes every new setting pick
//! a side.
//!
//! The current profile's saved `connections` and `prefs` are always None and never read: the
//! live values are `Settings.connections` and the `Settings` fields themselves. A switch saves
//! the live values into the outgoing profile and loads the incoming one's.

use super::{BranchVisibility, ColumnWidths, Corner, GraphColumns, ProfileId, Settings, ThemeChoice};
use crate::forge::Account;
use serde::{Deserialize, Serialize};

/// Declares `ProfilePrefs` and its copies to and from `Settings` from one field list, so the
/// three can't drift apart.
macro_rules! profile_prefs {
    ($($f:ident: $t:ty),* $(,)?) => {
        /// A profile's saved app preferences: the `Settings` fields of the same names.
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct ProfilePrefs {
            $(pub $f: $t),*
        }

        impl ProfilePrefs {
            /// The preferences in use in `s`.
            pub fn from_settings(s: &Settings) -> Self {
                Self { $($f: s.$f.clone()),* }
            }

            /// Puts these preferences into `s`, leaving every other setting alone.
            pub fn apply_to(&self, s: &mut Settings) {
                $(s.$f = self.$f.clone();)*
            }
        }
    };
}

profile_prefs! {
    // General
    auto_fetch_minutes: u32,
    auto_prune: bool,
    default_branch: String,
    initial_commits: usize,
    load_more_on_scroll: bool,
    reopen_last_repo: bool,
    watch_files: bool,
    conflict_check: bool,
    delete_to_trash: bool,
    // UI Customization
    theme: ThemeChoice,
    notification_location: Corner,
    behind_alerts: bool,
    desktop_notifications: bool,
    date_locale: String,
    date_format: String,
    date_word_format: String,
    date_time_format: String,
    date_verbose_format: String,
    relative_dates: bool,
    toolbar_labels: bool,
    zoom_percent: u16,
    show_left_panel: bool,
    show_details_panel: bool,
    show_toolbar: bool,
    spellcheck: bool,
    use_initials: bool,
    ghost_refs: bool,
    highlight_ref_rows: bool,
    worktree_wips: bool,
    branch_visibility: BranchVisibility,
    columns: GraphColumns,
    column_widths: ColumnWidths,
    // Editor
    diff_font_size: f32,
    tab_size: u8,
    diff_line_numbers: bool,
    syntax_highlighting: bool,
    diff_word_highlights: bool,
}

impl Default for ProfilePrefs {
    fn default() -> Self {
        Self::from_settings(&Settings::default())
    }
}

/// The settings that stay the same whatever profile is current (only the test that makes every
/// setting pick a side reads this).
#[cfg(test)]
pub const GLOBAL_FIELDS: &[&str] = &[
    "diff_layout",
    "diff_mode",
    "diff_ignore_whitespace",
    // Also a diff header toggle, which stays put when the profile changes.
    "diff_word_wrap",
    "push_after_commit",
    "stage_renames",
    "push_new_tags",
    "connections",
    "repo_trackers",
    "graph_filters",
    "repo_trust",
    "favorite_repos",
    "external_tools",
    "profiles",
    "active_profile",
    "use_ssh_agent",
    "gpg_prompts_in_app",
    // Preferences > Keyboard is not one of the pages a profile keeps.
    "keybindings",
];

/// What a profile switch changed in the settings in use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SetupSwap {
    /// The connected accounts.
    pub connections: bool,
    /// Any app preference.
    pub prefs: bool,
}

impl Settings {
    /// Saves the connections and preferences in use into the current profile and loads `to`'s.
    /// A part `to` has nothing saved for stays as it is. Nothing happens when `to` is missing or
    /// already current. Doesn't change `active_profile`: the caller makes `to` current.
    pub fn swap_profile_setup(&mut self, to: ProfileId) -> SetupSwap {
        if self.active_profile == Some(to) {
            return SetupSwap::default();
        }
        let Some(incoming) = self.profile_mut(to) else { return SetupSwap::default() };
        let (connections, prefs) = (incoming.connections.take(), incoming.prefs.take());
        let (live_connections, live_prefs) = (self.connections.clone(), ProfilePrefs::from_settings(self));
        if let Some(outgoing) = self.active_profile.and_then(|id| self.profile_mut(id)) {
            outgoing.connections = Some(live_connections);
            outgoing.prefs = Some(live_prefs.clone());
        }
        let mut swap = SetupSwap::default();
        if let Some(list) = connections {
            swap.connections = list != self.connections;
            self.connections = list;
        }
        if let Some(p) = prefs {
            swap.prefs = p != live_prefs;
            p.apply_to(self);
        }
        swap
    }

    /// Whether the connections in use, or any profile's saved ones, include `a`: its stored
    /// sign-in is shared, so it stays while any of them does.
    pub fn account_in_use(&self, a: &Account) -> bool {
        self.connection(a).is_some()
            || self.profiles.iter().filter_map(|p| p.connections.as_ref()).any(|list| list.iter().any(|c| &c.account == a))
    }
}

#[cfg(test)]
#[path = "profiles_prefs_tests.rs"]
mod tests;
