//! Opening the release notes dialog: from the palette and the status bar, and once by itself
//! the first time a new version starts (after an upgrade, never on a new install's welcome).

use crate::effect::Effect;
use crate::state::release_notes::{NotesDialog, NotesOpen};
use crate::state::{AppState, Modal};
use glowfly_core::app_update::Version;
use glowfly_core::changelog;

/// Opens the dialog, with the newer release on offer (if any) on top; releases after `since`
/// are marked new.
pub(super) fn open_notes(st: &mut AppState, since: Option<Version>) {
    let u = &st.updates;
    let dialog = NotesDialog::new(u.bundled_notes, &u.current, since, u.offer());
    st.modal = Some(Modal::ReleaseNotes(Box::new(dialog)));
}

/// At startup: when this version's notes haven't been seen, records it, and shows them on an
/// upgrade (now, or once the dialog in the way closes).
pub(super) fn startup_notes(st: &mut AppState, fx: &mut Vec<Effect>) {
    let current = st.updates.current.to_string();
    let seen = st.settings.updates.notes_seen.clone();
    if seen.as_deref() == Some(current.as_str()) {
        return;
    }
    st.settings.updates.notes_seen = Some(current.clone());
    fx.extend(super::prefs::save_settings(st));
    // A new install shows the welcome instead; a file from before release notes counts as an
    // upgrade (it has seen the welcome).
    let upgrade = st.settings.welcome_done && st.onboarding.welcome.is_none();
    if !upgrade || !changelog::has_version(&changelog::parse(st.updates.bundled_notes), &current) {
        return;
    }
    let since = seen.as_deref().and_then(Version::parse);
    if st.modal.is_none() && st.palette.is_none() && st.prefs.is_none() {
        open_notes(st, since);
    } else {
        st.updates.notes_pending = Some(NotesOpen { since });
    }
}
