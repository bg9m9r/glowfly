//! Glowfly's own updates: asking for the newest release, downloading and verifying its file,
//! and handing a downloaded installer to the system. Glowfly never replaces its own program.

use crate::effect::UpdateEffect;
use crate::msg::{CheckedRelease, Msg, UpdateMsg};
use glowfly_core::{app_update, settings};
use std::path::Path;

pub(super) fn run(e: UpdateEffect) -> Option<Msg> {
    match e {
        UpdateEffect::Check { manual } => Some(Msg::Updates(UpdateMsg::Checked { manual, result: check() })),
        UpdateEffect::Download(offer) => {
            let result = app_update::Feed::from_env().and_then(|feed| match settings::downloads_dir() {
                Some(dir) => app_update::download(&feed, &offer, &dir),
                None => Err("No folder to download into".to_owned()),
            });
            Some(Msg::Updates(UpdateMsg::Downloaded(result)))
        }
        UpdateEffect::Install(path) => install(&path),
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

/// Opens a downloaded (and verified) installer with the system: msiexec for an MSI, the disk
/// image for a DMG, the software installer for a DEB.
fn install(path: &Path) -> Option<Msg> {
    let shown = glowfly_core::native_path::shown(path);
    let e = open::that_detached(path).err()?;
    let mut text = format!("Couldn't open {shown}: {e}");
    if path.extension().is_some_and(|x| x.eq_ignore_ascii_case("deb")) {
        text.push_str(&format!("\nInstall it with: sudo apt install {shown}"));
    }
    Some(Msg::Logged { ok: false, text })
}
