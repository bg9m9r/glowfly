//! The release notes dialog: the notes bundled with this build (CHANGELOG.md, copied by
//! build.rs), and the notes of a newer release on offer; and the status bar's What's new chip
//! that opens them after an upgrade (`AppState::updates_news_chip`).

use super::AppState;
use glowfly_core::app_update::{Offer, Version};
use glowfly_core::changelog::{self, ReleaseNotes};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::{NewsChip, WhatsNew};

/// CHANGELOG.md as it was when this build was made; empty before the first release.
pub const BUNDLED: &str = include_str!(concat!(env!("OUT_DIR"), "/CHANGELOG.md"));

/// The dialog's content.
pub struct NotesDialog {
    /// The bundled releases, newest first.
    pub releases: Vec<ReleaseNotes>,
    /// Releases newer than this get a "New" chip (the version seen before this one).
    pub since: Option<Version>,
    /// This copy's version.
    pub current: String,
    /// A newer release on offer, shown on top.
    pub offer: Option<Box<Offer>>,
    /// That release's notes, from its release page.
    pub offered_notes: Option<ReleaseNotes>,
}

impl NotesDialog {
    pub fn new(bundled: &str, current: &Version, since: Option<Version>, offer: Option<&Offer>) -> Self {
        let offered_notes = offer.map(|o| changelog::parse_body(&o.release.tag, &o.release.body));
        Self {
            releases: changelog::parse(bundled),
            since,
            current: current.to_string(),
            offer: offer.map(|o| Box::new(o.clone())),
            offered_notes,
        }
    }

    /// Whether `r` came after the version seen before (and so is new to this user).
    pub fn is_new(&self, r: &ReleaseNotes) -> bool {
        match (&self.since, Version::parse(&r.version)) {
            (Some(since), Some(v)) => v > *since,
            _ => false,
        }
    }
}

/// Release notes waiting to open once no other dialog is in the way.
#[derive(Clone, Debug, PartialEq)]
pub struct NotesOpen {
    pub since: Option<Version>,
}

/// How long the What's new chip waits to be clicked: three days from the first frame it showed.
pub const NEWS_CHIP_FOR: i64 = 3 * 24 * 60 * 60;

impl AppState {
    /// The What's new chip the status bar shows now: chosen in Preferences, for the version
    /// running, and not yet run out.
    pub fn updates_news_chip(&self) -> Option<&NewsChip> {
        let u = &self.settings.updates;
        let chip = u.news_chip.as_ref().filter(|_| u.whats_new == WhatsNew::Chip)?;
        let current = Version::parse(&chip.version).is_some_and(|v| v == self.updates.current);
        (current && chip.until.is_none_or(|until| self.view.now < until)).then_some(chip)
    }
}

/// "What's new in 0.4.0", the chip's text.
pub fn news_chip_text(t: Lang, version: &str) -> String {
    t.trf("updates.whats_new.chip", &[("version", &version)])
}
