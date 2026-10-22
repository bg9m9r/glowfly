//! The palette's update entries: "View release notes" and "Check for updates" always, plus
//! "Download Glowfly x.y.z" while a newer release is on offer and, once it has downloaded,
//! "Install Glowfly x.y.z and restart" or "Open the Glowfly x.y.z installer and quit" when it
//! can be installed.

use super::AppState;
use super::palette_commands::{Entry, PaletteCommand as C, entry, with_detail};
use super::updates::UpdateStatus;
use crate::msg::UpdateMsg;

pub(super) fn entries(st: &AppState, out: &mut Vec<Entry>) {
    out.push(entry(C::Updates(UpdateMsg::ShowNotes), "View release notes", "changelog whats new patch notes history version"));
    let check = entry(C::Updates(UpdateMsg::CheckNow), "Check for updates", "update upgrade new version latest release download");
    out.push(with_detail(check, st.updates.busy().then(|| "checking…".to_owned())));
    match st.updates.settled() {
        UpdateStatus::Available(o) | UpdateStatus::DownloadFailed { offer: o, .. } => {
            out.push(entry(C::Updates(UpdateMsg::Download), format!("Download Glowfly {}", o.version), "update upgrade new version"));
        }
        UpdateStatus::Downloaded { offer, method, .. } if method.acts() => {
            let label = if method.restarts() {
                format!("Install Glowfly {} and restart", offer.version)
            } else {
                format!("Open the Glowfly {} installer and quit", offer.version)
            };
            out.push(entry(C::Updates(UpdateMsg::InstallAndRestart), label, "update upgrade new version"));
        }
        _ => {}
    }
}
