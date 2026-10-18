//! Trellis's own updates: the check's and download's status, the texts it shows, and when the
//! next automatic check may run (update/updates.rs). Nothing here is saved except through
//! `settings.updates`.

use super::release_notes::NotesOpen;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use trellis_core::app_update::{self, Offer, Platform, Version};
use trellis_core::i18n::Lang;

/// The first automatic check waits this long after startup.
pub const STARTUP_DELAY: Duration = Duration::from_secs(15);
/// After a failed automatic check, the next one waits this long.
pub const RETRY_AFTER: Duration = Duration::from_secs(3600);

/// Where the check or download is.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum UpdateStatus {
    #[default]
    Idle,
    /// `previous` is the status before the check, which the answer may keep (a downloaded
    /// file, say); the chip, palette and Install still use it meanwhile.
    Checking {
        manual: bool,
        previous: Box<UpdateStatus>,
    },
    UpToDate,
    CheckFailed(String),
    Available(Box<Offer>),
    Downloading(Box<Offer>),
    /// The verified file; `installer` for an msi, dmg or deb (else an archive).
    Downloaded {
        offer: Box<Offer>,
        file: PathBuf,
        installer: bool,
    },
    DownloadFailed {
        offer: Box<Offer>,
        error: String,
    },
}

/// How the status bar draws its update chip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChipKind {
    Available,
    Downloading,
    Downloaded,
    Failed,
}

pub struct UpdatesState {
    pub status: UpdateStatus,
    /// This copy's version (tests set another).
    pub current: Version,
    /// This system, for picking the file to download (tests set another).
    pub platform: Option<Platform>,
    /// Automatic checks may start from here (set by `UpdateMsg::Startup`).
    pub auto_from: Option<Instant>,
    /// After a failed automatic check: not before this.
    pub retry_at: Option<Instant>,
    /// Release notes to open once no dialog is in the way.
    pub notes_pending: Option<NotesOpen>,
    /// The bundled CHANGELOG.md (tests put a fixture here).
    pub bundled_notes: &'static str,
}

impl Default for UpdatesState {
    fn default() -> Self {
        Self {
            status: UpdateStatus::Idle,
            current: Version::parse(app_update::CURRENT_VERSION).unwrap_or(Version { major: 0, minor: 0, patch: 0, pre: Vec::new() }),
            platform: app_update::CURRENT_PLATFORM,
            auto_from: None,
            retry_at: None,
            notes_pending: None,
            bundled_notes: super::release_notes::BUNDLED,
        }
    }
}

/// The installer kinds a downloaded file is opened as (anything else is shown in the folder).
pub fn is_installer(file: &std::path::Path) -> bool {
    file.extension().and_then(|x| x.to_str()).is_some_and(|x| ["msi", "dmg", "deb"].iter().any(|k| x.eq_ignore_ascii_case(k)))
}

pub fn latest_text(t: Lang, current: &Version) -> String {
    t.trf("updates.latest", &[("version", current)])
}

pub fn available_text(t: Lang, v: &Version) -> String {
    t.trf("updates.available", &[("version", v)])
}

pub fn downloaded_text(t: Lang, v: &Version) -> String {
    t.trf("updates.downloaded", &[("version", v)])
}

pub fn check_failed_text(t: Lang, e: &str) -> String {
    t.trf("updates.check_failed", &[("error", &e)])
}

pub fn download_failed_text(t: Lang, e: &str) -> String {
    t.trf("updates.download_failed", &[("error", &e)])
}

impl UpdateStatus {
    /// The newer release on offer, in whatever state its download is.
    pub fn offer(&self) -> Option<&Offer> {
        match self {
            UpdateStatus::Available(o)
            | UpdateStatus::Downloading(o)
            | UpdateStatus::Downloaded { offer: o, .. }
            | UpdateStatus::DownloadFailed { offer: o, .. } => Some(o),
            UpdateStatus::Idle | UpdateStatus::Checking { .. } | UpdateStatus::UpToDate | UpdateStatus::CheckFailed(_) => None,
        }
    }
}

impl UpdatesState {
    /// The status, or while a check runs the one before it: what the chip, palette and
    /// Install act on.
    pub fn settled(&self) -> &UpdateStatus {
        match &self.status {
            UpdateStatus::Checking { previous, .. } => previous,
            s => s,
        }
    }

    /// A check or download is running: no other starts.
    pub fn busy(&self) -> bool {
        matches!(self.status, UpdateStatus::Checking { .. } | UpdateStatus::Downloading(_))
    }

    /// The newer release on offer, in whatever state its download is.
    pub fn offer(&self) -> Option<&Offer> {
        self.settled().offer()
    }

    /// The status bar's chip: its text and kind, when there is an update to talk about.
    pub fn chip(&self, t: Lang) -> Option<(String, ChipKind)> {
        Some(match self.settled() {
            UpdateStatus::Available(o) => (t.trf("updates.chip.available", &[("version", &o.version)]), ChipKind::Available),
            UpdateStatus::Downloading(o) => (t.trf("updates.chip.downloading", &[("version", &o.version)]), ChipKind::Downloading),
            UpdateStatus::Downloaded { .. } => (t.tr("updates.chip.downloaded").to_owned(), ChipKind::Downloaded),
            UpdateStatus::DownloadFailed { .. } => (t.tr("updates.chip.failed").to_owned(), ChipKind::Failed),
            UpdateStatus::Idle | UpdateStatus::Checking { .. } | UpdateStatus::UpToDate | UpdateStatus::CheckFailed(_) => return None,
        })
    }

    /// Preferences' "Trellis 0.2.0 · Last checked 2 hours ago" (`last_checked` already formatted).
    pub fn prefs_line(&self, t: Lang, last_checked: Option<&str>) -> String {
        match last_checked {
            Some(when) => t.trf("updates.prefs_line.checked", &[("version", &self.current), ("when", &when)]),
            None => t.trf("updates.prefs_line.never", &[("version", &self.current)]),
        }
    }

    /// The downloaded installer, when there is one to open.
    pub fn installer(&self) -> Option<&PathBuf> {
        match self.settled() {
            UpdateStatus::Downloaded { file, installer: true, .. } => Some(file),
            _ => None,
        }
    }
}
