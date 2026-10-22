//! The headless update check behind `glowfly --check-update`: the same feed, offer and
//! verified download as the app's Check now and Download, in one call.

use super::{Feed, InstallKind, Offer, Platform, Version, download, offer};
use std::path::{Path, PathBuf};

/// What a headless check found.
#[derive(Clone, Debug, PartialEq)]
pub enum Checked {
    /// No newer release; `latest` is the feed's tag, if it has a release.
    UpToDate { latest: Option<String> },
    /// A newer release's file, verified and saved at `path`.
    Downloaded { offer: Box<Offer>, path: PathBuf },
    /// A newer release that Flatpak installs; nothing was downloaded.
    UpdateWithFlatpak { offer: Box<Offer> },
}

/// Reads `feed`, and when it has a release newer than `current`, downloads this system's file
/// for a `kind` copy into `dir` once it matches its published checksum. A Flatpak copy is only
/// told about it.
pub fn check_and_download(
    feed: &Feed,
    current: &Version,
    platform: Option<Platform>,
    kind: InstallKind,
    dir: &Path,
) -> Result<Checked, String> {
    let Some(release) = feed.latest()? else { return Ok(Checked::UpToDate { latest: None }) };
    match offer(&release, current, platform, kind) {
        None => Ok(Checked::UpToDate { latest: Some(release.tag) }),
        Some(o) if !kind.downloads() => Ok(Checked::UpdateWithFlatpak { offer: Box::new(o) }),
        Some(o) => {
            let path = download(feed, &o, dir)?;
            Ok(Checked::Downloaded { offer: Box::new(o), path })
        }
    }
}
