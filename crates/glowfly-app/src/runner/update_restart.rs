//! Install and restart on the worker: an installer that can't run in place is checked again
//! and opened with the system; an MSI, a portable program or an AppImage is prepared and handed
//! to the update helper (`app_update::start_restart`). Either way the answer is
//! `UpdateMsg::RestartReady`, and on success the window closes. The places a staging folder may
//! go and this copy's own files are shared with the start's tidying (runner/updates.rs).

use crate::effect::RestartJob;
use crate::msg::{Msg, UpdateMsg};
use glowfly_core::app_update::{self, InstallMethod, RestartRequest};
use glowfly_core::native_path::shown;
use glowfly_core::paths;
use std::path::{Path, PathBuf};

pub(super) fn run(job: RestartJob) -> Msg {
    Msg::Updates(UpdateMsg::RestartReady(restart(job)))
}

fn restart(job: RestartJob) -> Result<(), String> {
    match job.method {
        InstallMethod::OpenAndQuit => {
            app_update::read_verified(&job.file, &job.sha256)?;
            open_installer(&job.file)
        }
        InstallMethod::RunMsi | InstallMethod::SwapPortable | InstallMethod::SwapAppImage => {
            let (exe, image) = own_files()?;
            let Some(platform) = app_update::CURRENT_PLATFORM else {
                return Err("This update can't be installed in place.".to_owned());
            };
            let target = if job.method == InstallMethod::SwapAppImage {
                image.ok_or_else(|| "This copy isn't running from an AppImage.".to_owned())?
            } else {
                exe.clone()
            };
            app_update::start_restart(&RestartRequest {
                file: job.file,
                sha256: job.sha256,
                method: job.method,
                version: job.version.to_string(),
                exe,
                places: staging_places(&target),
                target,
                os: platform.os,
            })
        }
        InstallMethod::ShowOnly => Err("This update can't be installed in place.".to_owned()),
    }
}

/// The running program, resolved, and the AppImage it runs from, if any.
pub(super) fn own_files() -> Result<(PathBuf, Option<PathBuf>), String> {
    let exe = std::env::current_exe().map_err(|e| format!("Couldn't find Glowfly's own program: {e}"))?;
    let exe = paths::resolve(&exe);
    let image = app_update::appimage_file(&exe).map(|i| paths::resolve(&i));
    Ok((exe, image))
}

/// Where a staging folder for replacing `target` may go, in order: the temp folder, then, for a
/// temp folder mounted noexec, beside `target` (a folder programs already run from) and the
/// cache folder.
pub(super) fn staging_places(target: &Path) -> Vec<PathBuf> {
    let mut places = vec![paths::temp_dir()];
    places.extend(target.parent().map(Path::to_path_buf));
    if let Some(cache) = glowfly_core::settings::cache_dir() {
        // Glowfly's own folder; create_runnable passes over it if it can't be made.
        let _ = std::fs::create_dir_all(&cache);
        places.push(paths::resolve(&cache));
    }
    places
}

/// Opens a downloaded (and verified) installer with the system: the MSI's installer, the disk
/// image, or the software installer for a .deb or .rpm.
fn open_installer(path: &Path) -> Result<(), String> {
    let Err(e) = open::that_detached(path) else { return Ok(()) };
    let mut text = format!("Couldn't open {}: {e}", shown(path));
    if let Some(hint) = installer_hint(path) {
        text.push('\n');
        text.push_str(&hint);
    }
    Err(text)
}

/// How to install a Linux package by hand when it couldn't be opened.
fn installer_hint(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    let tool = match ext.as_str() {
        "deb" => "apt",
        "rpm" => "dnf",
        _ => return None,
    };
    Some(format!("Install it with: sudo {tool} install {}", shown(path)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_packages_get_an_install_hint() {
        let dir = paths::temp_dir();
        let deb = dir.join("glowfly-v0.3.0-x86_64-unknown-linux-gnu.deb");
        assert_eq!(installer_hint(&deb), Some(format!("Install it with: sudo apt install {}", shown(&deb))));
        let rpm = dir.join("glowfly-v0.3.0-x86_64-unknown-linux-gnu.RPM");
        assert_eq!(installer_hint(&rpm), Some(format!("Install it with: sudo dnf install {}", shown(&rpm))));
        for other in ["glowfly.msi", "glowfly.dmg", "glowfly.AppImage", "glowfly"] {
            assert_eq!(installer_hint(&dir.join(other)), None, "{other}");
        }
    }
}
