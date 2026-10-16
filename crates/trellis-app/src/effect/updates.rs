//! Effects of Trellis's own updates, run by runner/updates.rs.

use std::path::PathBuf;
use trellis_core::app_update::Offer;

#[derive(Debug)]
pub enum UpdateEffect {
    /// Ask for the newest release; reported as `UpdateMsg::Checked`.
    Check { manual: bool },
    /// Download and verify the offer's file into the Downloads folder; reported as
    /// `UpdateMsg::Downloaded`.
    Download(Box<Offer>),
    /// Hand a downloaded installer to the system (msiexec, the disk image, the software
    /// installer).
    Install(PathBuf),
}

impl UpdateEffect {
    /// A short name for the worker thread and the debug log.
    pub fn name(&self) -> &'static str {
        match self {
            UpdateEffect::Check { .. } => "update-check",
            UpdateEffect::Download(_) => "update-download",
            UpdateEffect::Install(_) => "update-install",
        }
    }

    /// Whether the runner times it: a check is one request; a download is always slow, and an
    /// install hands off to the system.
    pub fn timed(&self) -> bool {
        match self {
            UpdateEffect::Check { .. } => true,
            UpdateEffect::Download(_) | UpdateEffect::Install(_) => false,
        }
    }
}
