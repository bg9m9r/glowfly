//! Effects of Glowfly's own updates, run by runner/updates.rs.

use glowfly_core::app_update::{InstallMethod, Offer, Version};
use std::path::PathBuf;

#[derive(Debug)]
pub enum UpdateEffect {
    /// Ask for the newest release; reported as `UpdateMsg::Checked`.
    Check { manual: bool },
    /// Download and verify the offer's file into the Downloads folder; reported as
    /// `UpdateMsg::Downloaded`.
    Download(Box<Offer>),
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

impl UpdateEffect {
    /// A short name for the worker thread and the debug log.
    pub fn name(&self) -> &'static str {
        match self {
            UpdateEffect::Check { .. } => "update-check",
            UpdateEffect::Download(_) => "update-download",
            UpdateEffect::Restart(_) => "update-restart",
            UpdateEffect::Tidy => "update-tidy",
        }
    }

    /// Whether the runner times it: a check is one request; a download is always slow, a
    /// restart hands off to the system, and tidying runs once at start.
    pub fn timed(&self) -> bool {
        match self {
            UpdateEffect::Check { .. } => true,
            UpdateEffect::Download(_) | UpdateEffect::Restart(_) | UpdateEffect::Tidy => false,
        }
    }
}
