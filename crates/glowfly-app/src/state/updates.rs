//! Glowfly's own updates: the check's and download's status, the texts it shows, and when the
//! next automatic check may run (update/updates.rs). Nothing here is saved except through
//! `settings.updates`.

use super::release_notes::NotesOpen;
use glowfly_core::app_update::{self, InstallKind, InstallMethod, Offer, Platform, Progress, SavedProblem, Version};
use glowfly_core::i18n::Lang;
use glowfly_core::lfs::pointer::size_text;
use std::path::PathBuf;
use std::time::{Duration, Instant};

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
    /// The verified file, the SHA-256 it matched, and how Install and restart puts it in place.
    Downloaded {
        offer: Box<Offer>,
        file: PathBuf,
        sha256: [u8; 32],
        method: InstallMethod,
    },
    DownloadFailed {
        offer: Box<Offer>,
        error: String,
    },
    /// The file downloaded earlier is gone or no longer matches its checksum: Download again.
    Stale {
        offer: Box<Offer>,
        file: PathBuf,
        problem: SavedProblem,
    },
}

/// The running download: how far it is, and whether Cancel was pressed (it stops between
/// chunks, then reports `DownloadCancelled`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Transfer {
    pub progress: Progress,
    pub cancelling: bool,
}

/// How the status bar draws its update chip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChipKind {
    Available,
    Downloading,
    Downloaded,
    /// Install and restart is preparing the update.
    Installing,
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
    /// How this copy was installed, as the last successful check found.
    pub install: Option<InstallKind>,
    /// Install and restart is preparing (or the window is closing for it): nothing else starts.
    pub restarting: bool,
    /// The running download's progress (reset when one starts or ends).
    pub transfer: Transfer,
}

impl Default for UpdatesState {
    fn default() -> Self {
        Self {
            status: UpdateStatus::Idle,
            // The same version as core's CURRENT_VERSION, except fixed in tests.
            current: Version::parse(super::menu_bar::VERSION).unwrap_or(Version { major: 0, minor: 0, patch: 0, pre: Vec::new() }),
            platform: app_update::current_platform(),
            auto_from: None,
            retry_at: None,
            notes_pending: None,
            bundled_notes: super::release_notes::BUNDLED,
            install: None,
            restarting: false,
            transfer: Transfer::default(),
        }
    }
}

pub fn latest_text(t: Lang, current: &Version) -> String {
    t.trf("updates.latest", &[("version", current)])
}

pub fn available_text(t: Lang, v: &Version) -> String {
    t.trf("updates.available", &[("version", v)])
}

/// "Glowfly x.y.z is available. Update it with Flatpak or your software centre."
pub fn flatpak_available_text(t: Lang, v: &Version) -> String {
    t.trf("updates.flatpak.available", &[("version", v)])
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

/// "The downloaded file … is no longer there." or "… no longer matches its checksum."
pub fn stale_text(t: Lang, file: &std::path::Path, problem: SavedProblem) -> String {
    let path = glowfly_core::native_path::shown(file);
    match problem {
        SavedProblem::Missing => t.trf("updates.saved.missing", &[("path", &path)]),
        SavedProblem::Changed => t.trf("updates.saved.changed", &[("path", &path)]),
    }
}

/// "Downloading… 40% (4.0 MB of 10.0 MB)", or "Downloading… 4.0 MB" when the size isn't known.
pub fn progress_text(t: Lang, p: Progress) -> String {
    let done = size_text(p.done);
    match (p.total.filter(|&n| n > 0), p.fraction()) {
        (Some(total), Some(f)) => {
            let percent = (f * 100.0).floor() as u32;
            t.trf("updates.progress.percent", &[("percent", &percent), ("done", &done), ("total", &size_text(total))])
        }
        _ => t.trf("updates.progress.bytes", &[("done", &done)]),
    }
}

pub fn restart_failed_text(t: Lang, e: &str) -> String {
    t.trf("updates.restart_failed", &[("error", &e)])
}

/// "Restarting to install Glowfly x.y.z."
pub fn restarting_text(t: Lang, v: &Version) -> String {
    t.trf("updates.restarting", &[("version", v)])
}

/// "Opened the Glowfly x.y.z installer."
pub fn opened_text(t: Lang, v: &Version) -> String {
    t.trf("updates.opened_installer", &[("version", v)])
}

/// What the update log says went wrong with the last installer run, for its toast.
pub fn last_install_text(t: Lang, last: &glowfly_core::app_update::LastInstall) -> String {
    use glowfly_core::app_update::InstallProblem;
    let v = &last.version;
    match &last.problem {
        InstallProblem::Cancelled => t.trf("updates.last_install.cancelled", &[("version", v)]),
        InstallProblem::Busy => t.trf("updates.last_install.busy", &[("version", v)]),
        InstallProblem::Failed(Some(code)) => t.trf("updates.last_install.failed_code", &[("version", v), ("code", code)]),
        InstallProblem::Failed(None) => t.trf("updates.last_install.failed", &[("version", v)]),
        InstallProblem::NotRun(why) => t.trf("updates.last_install.not_run", &[("version", v), ("error", why)]),
    }
}

/// The button that installs a file downloaded for `method`: Install and restart, Open
/// installer and quit, or Show file.
pub fn install_label(t: Lang, method: InstallMethod) -> &'static str {
    match method {
        InstallMethod::RunMsi | InstallMethod::SwapPortable | InstallMethod::SwapAppImage => t.tr("updates.install_restart.button"),
        InstallMethod::OpenAndQuit => t.tr("updates.open_quit.button"),
        InstallMethod::ShowOnly => t.tr("updates.show_file.button"),
    }
}

impl UpdateStatus {
    /// The newer release on offer, in whatever state its download is.
    pub fn offer(&self) -> Option<&Offer> {
        match self {
            UpdateStatus::Available(o)
            | UpdateStatus::Downloading(o)
            | UpdateStatus::Downloaded { offer: o, .. }
            | UpdateStatus::DownloadFailed { offer: o, .. }
            | UpdateStatus::Stale { offer: o, .. } => Some(o),
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

    /// A check, a download or Install and restart is running: no check, download or install
    /// starts.
    pub fn blocked(&self) -> bool {
        self.busy() || self.restarting
    }

    /// The newer release on offer, in whatever state its download is.
    pub fn offer(&self) -> Option<&Offer> {
        self.settled().offer()
    }

    /// The status bar's chip: its text and kind, when there is an update to talk about.
    pub fn chip(&self, t: Lang) -> Option<(String, ChipKind)> {
        Some(match self.settled() {
            UpdateStatus::Available(o) | UpdateStatus::Stale { offer: o, .. } => {
                (t.trf("updates.chip.available", &[("version", &o.version)]), ChipKind::Available)
            }
            UpdateStatus::Downloading(o) => (t.trf("updates.chip.downloading", &[("version", &o.version)]), ChipKind::Downloading),
            UpdateStatus::Downloaded { offer, .. } if self.restarting => {
                (t.trf("updates.chip.installing", &[("version", &offer.version)]), ChipKind::Installing)
            }
            UpdateStatus::Downloaded { .. } => (t.tr("updates.chip.downloaded").to_owned(), ChipKind::Downloaded),
            UpdateStatus::DownloadFailed { .. } => (t.tr("updates.chip.failed").to_owned(), ChipKind::Failed),
            UpdateStatus::Idle | UpdateStatus::Checking { .. } | UpdateStatus::UpToDate | UpdateStatus::CheckFailed(_) => return None,
        })
    }

    /// Preferences' "Glowfly 0.2.0 · Last checked 2 hours ago" (`last_checked` already formatted).
    pub fn prefs_line(&self, t: Lang, last_checked: Option<&str>) -> String {
        match last_checked {
            Some(when) => t.trf("updates.prefs_line.checked", &[("version", &self.current), ("when", &when)]),
            None => t.trf("updates.prefs_line.never", &[("version", &self.current)]),
        }
    }

    /// The downloaded file and how it is installed, once there is one.
    pub fn downloaded(&self) -> Option<(&PathBuf, InstallMethod)> {
        match self.settled() {
            UpdateStatus::Downloaded { file, method, .. } => Some((file, *method)),
            _ => None,
        }
    }
}
