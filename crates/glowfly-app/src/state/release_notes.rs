//! The release notes dialog: the notes bundled with this build (CHANGELOG.md, copied by
//! build.rs), and the notes of a newer release on offer.

use glowfly_core::app_update::{Offer, Version};
use glowfly_core::changelog::{self, ReleaseNotes};

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
