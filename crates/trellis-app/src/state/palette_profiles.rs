//! The palette's profile switching: the "Switch profile…" entry and the rows of its sub-list.

use super::AppState;
use super::palette_commands::{Entry, PaletteCommand, entry, with_detail};
use trellis_core::settings::{ProfileId, Settings};

const KEYWORDS: &str = "profile identity author name email user account persona work personal change switch";

/// "Switch profile…", while there are profiles to choose between and no switch is running.
pub(super) fn profile_entries(st: &AppState, out: &mut Vec<Entry>) {
    let profiles = &st.settings.profiles;
    if profiles.len() < 2 || st.profile_switch.is_some() {
        return;
    }
    let current = st.settings.active_profile().map(|p| format!("current: {}", p.name));
    out.push(with_detail(entry(PaletteCommand::ChooseProfile, "Switch profile…", KEYWORDS), current));
}

/// What a row of the profile sub-list stands for.
#[derive(Clone, Debug, PartialEq)]
pub struct ProfileRow {
    pub id: ProfileId,
    /// The dim text at the right: the author email, after "current · " on the current profile.
    pub detail: String,
}

/// The profile sub-list in settings order: the names to rank, and the row behind each.
pub fn profile_rows(settings: &Settings) -> (Vec<String>, Vec<ProfileRow>) {
    let mut names = Vec::new();
    let mut rows = Vec::new();
    for p in &settings.profiles {
        let email = p.author_email.trim();
        let current = settings.active_profile == Some(p.id);
        let detail = match (current, email.is_empty()) {
            (true, true) => "current".to_owned(),
            (true, false) => format!("current · {email}"),
            (false, true) => "no email".to_owned(),
            (false, false) => email.to_owned(),
        };
        names.push(p.name.clone());
        rows.push(ProfileRow { id: p.id, detail });
    }
    (names, rows)
}
