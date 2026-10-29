//! Effects of Glowfly's own updates, run by runner/updates.rs.

use super::EffectMeta;
#[cfg(test)]
use super::SandboxRule;
use super::partial::StopKey;
use glowfly_core::app_update::{InstallMethod, Offer, Version};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum UpdateEffect {
    /// Ask for the newest release; reported as `UpdateMsg::Checked`.
    Check { manual: bool },
    /// Download and verify the offer's file into the Downloads folder, streamed to disk: reports
    /// `UpdateMsg::DownloadProgress` a few times a second, then `UpdateMsg::Downloaded`, or
    /// `UpdateMsg::DownloadCancelled` once `StopGit(StopKey::UpdateDownload)` stopped it.
    Download(Box<Offer>),
    /// Check that the downloaded `file` is still there with the SHA-256 it was saved with;
    /// reported as `UpdateMsg::SavedChecked`.
    CheckSaved { file: PathBuf, sha256: [u8; 32] },
    /// Install and restart (or open the installer); reported as `UpdateMsg::RestartReady`.
    Restart(Box<RestartJob>),
    /// Remove staging folders earlier updates left in the temp folder.
    Tidy,
}

/// The downloaded update to install.
#[derive(Clone, Debug, PartialEq)]
pub struct RestartJob {
    pub file: PathBuf,
    /// The checksum it matched when it was downloaded, checked again before it is used.
    pub sha256: [u8; 32],
    pub method: InstallMethod,
    pub version: Version,
}

impl EffectMeta for UpdateEffect {
    /// A short name for the worker thread and the debug log.
    fn name(&self) -> &'static str {
        match self {
            UpdateEffect::Check { .. } => "update-check",
            UpdateEffect::Download(_) => "update-download",
            UpdateEffect::CheckSaved { .. } => "update-check-saved",
            UpdateEffect::Restart(_) => "update-restart",
            UpdateEffect::Tidy => "update-tidy",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        None
    }

    /// Whether the runner times it: a check is one request; a download is always slow, reading
    /// a large file back takes a while, a restart hands off to the system, and tidying runs
    /// once at start.
    fn timed(&self) -> bool {
        match self {
            UpdateEffect::Check { .. } => true,
            UpdateEffect::Download(_) | UpdateEffect::CheckSaved { .. } | UpdateEffect::Restart(_) | UpdateEffect::Tidy => false,
        }
    }

    /// Cancel stops a download between chunks.
    fn stop_key(&self) -> Option<StopKey> {
        match self {
            UpdateEffect::Download(_) => Some(StopKey::UpdateDownload),
            UpdateEffect::Check { .. } | UpdateEffect::CheckSaved { .. } | UpdateEffect::Restart(_) | UpdateEffect::Tidy => None,
        }
    }

    fn queue(&self) -> Option<&Path> {
        None
    }

    #[cfg(test)]
    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Drop
    }
}
