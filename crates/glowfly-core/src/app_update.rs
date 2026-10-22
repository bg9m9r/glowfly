//! Glowfly's own updates: whether a check is due, which release is newer than this copy, which
//! of its files fits this system and how this copy was installed, and downloading that file
//! after checking it against the release's published SHA-256 checksums.
//!
//! - `version.rs`: semantic versions and their precedence.
//! - `release.rs`: the hosting service's release (tag, notes, files), with unsafe links and file
//!   names dropped.
//! - `assets.rs`: this system's platform, how this copy was installed, and which file to fetch.
//! - `sums.rs`: `SHA256SUMS` and `<file>.sha256` files.
//! - `download.rs`: fetching, verifying and saving a file (I/O).
//! - `feed.rs`: where releases are read from: the hosting service, or a local folder named by
//!   GLOWFLY_UPDATE_FEED.
//! - `check.rs`: the headless check behind `glowfly --check-update`.
//! - `method.rs`: how a downloaded file is installed (run the MSI, swap the portable program,
//!   open the installer and quit, or only show it).
//! - `archive.rs`: the program and doc files of a portable copy's zip or tar.gz.
//! - `swap.rs`: replacing a portable copy's program in place, with rollback.
//! - `staging.rs`: the private temporary folder an update is prepared in, and tidying it.
//! - `plan.rs`: the update helper's instructions.
//! - `restart.rs`: preparing Install and restart and handing it to the helper.
//! - `launch.rs`: starting the helper, and Glowfly again, detached.
//! - `finish.rs`: the helper itself (`glowfly --finish-update <plan>`).
//!
//! The release itself is read through `Feed` (by default `forge::latest_release`). A
//! downloaded installer runs, or a portable copy's program is swapped, only when the user asks
//! (Install and restart), and only after its SHA-256 matches again.

mod archive;
mod assets;
mod check;
mod download;
mod feed;
mod finish;
mod launch;
mod method;
mod plan;
mod release;
mod restart;
mod staging;
mod sums;
mod swap;
mod version;

#[cfg(test)]
mod feed_tests;
#[cfg(test)]
mod tests;

pub use assets::{Arch, CURRENT_PLATFORM, InstallKind, Platform, SumsSource, TargetOs, install_kind, install_kind_for, pick, sums_source};
pub use check::{Checked, check_and_download};
pub use download::{MAX_DOWNLOAD, Saved, download, download_saved, free_name, read_verified, split_extension};
pub use feed::{FEED_ENV, FEED_RELEASE, Feed};
pub use finish::run as finish_update;
pub use method::{InstallMethod, install_method_for};
pub use release::{Asset, Release, asset_name_ok, parse_release};
pub use restart::{RestartRequest, start as start_restart};
pub use staging::tidy as tidy_staging;
pub use sums::{expected as expected_sum, parse as parse_sums, sha256};
pub use version::{PreId, Version};

/// The command-line flag that runs the update helper on a plan.
pub const FINISH_FLAG: &str = "--finish-update";
/// This build's version.
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
/// The repository whose releases are checked (a fork built with its own `repository` field
/// checks its own releases).
pub const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
/// How long after a successful check the next automatic one is due, in seconds.
pub const CHECK_EVERY_SECS: i64 = 86_400;

/// Whether an automatic check is due at `now` (Unix seconds): never when turned off or without
/// a clock; otherwise when there was no check yet, a day has passed, or the last check lies in
/// the future (the clock went back).
pub fn due(enabled: bool, last_checked: Option<i64>, now: i64) -> bool {
    if !enabled || now <= 0 {
        return false;
    }
    match last_checked {
        None => true,
        Some(last) => last > now || now - last >= CHECK_EVERY_SECS,
    }
}

/// A release newer than this copy, with the file to fetch for this system (None when there is
/// no build for it) and where its checksum is published.
#[derive(Clone, Debug, PartialEq)]
pub struct Offer {
    pub release: Release,
    pub version: Version,
    pub asset: Option<Asset>,
    pub sums: SumsSource,
}

/// The offer `release` makes to a copy at `current`: Some only when its tag is a version newer
/// than `current`.
pub fn offer(release: &Release, current: &Version, platform: Option<Platform>, kind: InstallKind) -> Option<Offer> {
    let version = Version::parse(&release.tag)?;
    if version <= *current {
        return None;
    }
    let asset = platform.and_then(|p| pick(release, p, kind));
    let sums = asset.as_ref().map_or(SumsSource::None, |a| sums_source(release, a));
    Some(Offer { release: release.clone(), version, asset, sums })
}

/// The releases page of `repository` (`https://github.com/o/r/releases`), when it is an https
/// URL.
pub fn releases_page(repository: &str) -> Option<String> {
    let base = repository.trim_end_matches('/');
    let base = base.strip_suffix(".git").unwrap_or(base);
    crate::forge::https_host(base).map(|_| format!("{base}/releases"))
}
