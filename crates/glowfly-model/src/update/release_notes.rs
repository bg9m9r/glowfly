//! Opening the release notes dialog: from the palette, Help and the status bar, and after an
//! upgrade (never on a new install's welcome) as Preferences > General > Updates chooses
//! (`WhatsNew`): by itself the first time the new version starts, through the status bar's
//! What's new chip, or not at all. The chip waits until the notes are opened or
//! `NEWS_CHIP_FOR` has passed since the first frame it was shown in.

use crate::effect::Effect;
use crate::state::release_notes::{NEWS_CHIP_FOR, NotesDialog, NotesOpen};
use crate::state::{AppState, Modal};
use glowfly_core::app_update::Version;
use glowfly_core::changelog;
use glowfly_core::settings::{NewsChip, WhatsNew};

/// Opens the dialog, with the newer release on offer (if any) on top; releases after `since`
/// are marked new.
pub(super) fn open_notes(st: &mut AppState, since: Option<Version>) {
    let u = &st.updates;
    let dialog = NotesDialog::new(u.bundled_notes, &u.current, since, u.offer());
    st.modal = Some(Modal::ReleaseNotes(Box::new(dialog)));
}

/// Release notes asked for (the What's new chip, Help, the palette or an update's chip or
/// toast): while the What's new chip waits, they mark what it tells about as new, and it goes.
pub(super) fn show_notes(st: &mut AppState, fx: &mut Vec<Effect>) {
    let since = match st.updates_news_chip() {
        Some(chip) => chip.since.as_deref().and_then(Version::parse),
        None => None,
    };
    open_notes(st, since);
    if st.settings.updates.news_chip.take().is_some() {
        fx.extend(super::prefs::save_settings(st));
    }
}

/// At startup: when this version's notes haven't been seen, records it, and on an upgrade
/// shows them (now, or once the dialog in the way closes) or puts up the What's new chip.
pub(super) fn startup_notes(st: &mut AppState, fx: &mut Vec<Effect>) {
    let current = st.updates.current.to_string();
    let seen = st.settings.updates.notes_seen.clone();
    if seen.as_deref() == Some(current.as_str()) {
        return;
    }
    st.settings.updates.notes_seen = Some(current.clone());
    // A chip for an older version has nothing left to tell.
    st.settings.updates.news_chip = None;
    // A new install shows the welcome instead; a file from before release notes counts as an
    // upgrade (it has seen the welcome).
    let upgrade = st.settings.welcome_done && st.onboarding.welcome.is_none();
    let has_notes = changelog::has_version(&changelog::parse(st.updates.bundled_notes), &current);
    let whats_new = if upgrade && has_notes { st.settings.updates.whats_new } else { WhatsNew::Off };
    if whats_new == WhatsNew::Chip {
        st.settings.updates.news_chip = Some(NewsChip { version: current, since: seen.clone(), until: None });
    }
    fx.extend(super::prefs::save_settings(st));
    if whats_new != WhatsNew::Dialog {
        return;
    }
    let since = seen.as_deref().and_then(Version::parse);
    if st.modal.is_none() && st.palette.is_none() && st.prefs.is_none() {
        open_notes(st, since);
    } else {
        st.updates.notes_pending = Some(NotesOpen { since });
    }
}

/// Once per frame: starts the What's new chip's time from the first frame that knows the
/// time, and takes it away once that time is up.
pub(super) fn news_chip_tick(st: &mut AppState, fx: &mut Vec<Effect>) {
    let now = st.view.now;
    let Some(chip) = st.settings.updates.news_chip.as_mut() else { return };
    match chip.until {
        None if now > 0 => chip.until = Some(now + NEWS_CHIP_FOR),
        Some(until) if now >= until => st.settings.updates.news_chip = None,
        _ => return,
    }
    fx.extend(super::prefs::save_settings(st));
}
