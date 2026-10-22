//! Glowfly's own updates: asking for the newest release, downloading and verifying its file,
//! Install and restart (runner/update_restart.rs), and removing the staging folders earlier
//! updates left behind.

use super::update_restart;
use crate::effect::UpdateEffect;
use crate::msg::{CheckedRelease, Msg, UpdateMsg};
use glowfly_core::{app_update, paths, settings};
use std::time::SystemTime;

pub(super) fn run(e: UpdateEffect) -> Option<Msg> {
    match e {
        UpdateEffect::Check { manual } => Some(Msg::Updates(UpdateMsg::Checked { manual, result: check() })),
        UpdateEffect::Download(offer) => {
            let result = app_update::Feed::from_env().and_then(|feed| match settings::downloads_dir() {
                Some(dir) => app_update::download_saved(&feed, &offer, &dir),
                None => Err("No folder to download into".to_owned()),
            });
            Some(Msg::Updates(UpdateMsg::Downloaded(result)))
        }
        UpdateEffect::Restart(job) => Some(update_restart::run(*job)),
        UpdateEffect::Tidy => {
            let n = app_update::tidy_staging(&paths::temp_dir(), SystemTime::now());
            tracing::debug!("removed {n} staging folders earlier updates left");
            None
        }
    }
}

/// The newest release (from the hosting service, or the folder GLOWFLY_UPDATE_FEED names), and
/// how this copy was installed.
fn check() -> Result<Option<CheckedRelease>, String> {
    let feed = app_update::Feed::from_env()?;
    if matches!(feed, app_update::Feed::Folder(_)) {
        tracing::info!("checking for updates in the {}", feed.describe());
    }
    let release = feed.latest()?;
    let exe = std::env::current_exe().map_err(|e| format!("Couldn't find Glowfly's own program: {e}"))?;
    let install = app_update::install_kind(&exe);
    Ok(release.map(|release| CheckedRelease { release, install }))
}
