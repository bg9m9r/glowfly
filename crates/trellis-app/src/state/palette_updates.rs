//! The palette's update entries: "View release notes" and "Check for updates" always, plus
//! "Download Trellis x.y.z" while a newer release is on offer and "Install Trellis x.y.z" once
//! its installer has downloaded.

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
            out.push(entry(C::Updates(UpdateMsg::Download), format!("Download Trellis {}", o.version), "update upgrade new version"));
        }
        UpdateStatus::Downloaded { offer, installer: true, .. } => {
            out.push(entry(C::Updates(UpdateMsg::Install), format!("Install Trellis {}", offer.version), "update upgrade new version"));
        }
        _ => {}
    }
}
