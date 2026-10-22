//! Install and restart on the worker: an installer that can't run in place is checked again
//! and opened with the system; an MSI or a portable program is prepared and handed to the
//! update helper (`app_update::start_restart`). Either way the answer is
//! `UpdateMsg::RestartReady`, and on success the window closes.

use crate::effect::RestartJob;
use crate::msg::{Msg, UpdateMsg};
use glowfly_core::app_update::{self, InstallMethod, RestartRequest};
use glowfly_core::native_path::shown;
use glowfly_core::paths;
use std::path::Path;

pub(super) fn run(job: RestartJob) -> Msg {
    Msg::Updates(UpdateMsg::RestartReady(restart(job)))
}

fn restart(job: RestartJob) -> Result<(), String> {
    match job.method {
        InstallMethod::OpenAndQuit => {
            app_update::read_verified(&job.file, &job.sha256)?;
            open_installer(&job.file)
        }
        InstallMethod::RunMsi | InstallMethod::SwapPortable => {
            let exe = std::env::current_exe().map_err(|e| format!("Couldn't find Glowfly's own program: {e}"))?;
            let Some(platform) = app_update::CURRENT_PLATFORM else {
                return Err("This update can't be installed in place.".to_owned());
            };
            app_update::start_restart(&RestartRequest {
                file: job.file,
                sha256: job.sha256,
                method: job.method,
                version: job.version.to_string(),
                exe: paths::resolve(&exe),
                temp: paths::temp_dir(),
                os: platform.os,
            })
        }
        InstallMethod::ShowOnly => Err("This update can't be installed in place.".to_owned()),
    }
}

/// Opens a downloaded (and verified) installer with the system: the MSI's installer, the disk
/// image, or the software installer for a .deb.
fn open_installer(path: &Path) -> Result<(), String> {
    let shown = shown(path);
    let Err(e) = open::that_detached(path) else { return Ok(()) };
    let mut text = format!("Couldn't open {shown}: {e}");
    if path.extension().is_some_and(|x| x.eq_ignore_ascii_case("deb")) {
        text.push_str(&format!("\nInstall it with: sudo apt install {shown}"));
    }
    Err(text)
}
