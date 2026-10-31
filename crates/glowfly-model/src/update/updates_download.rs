//! Downloading Glowfly's own update: starting it (again, when the file downloaded before is
//! gone or changed), its progress, Cancel (the runner stops between chunks and removes the
//! partial file), its result, and reading a downloaded file again (`UpdateEffect::CheckSaved`,
//! when Preferences opens, a check keeps the file, or Install and restart fails), which turns a
//! missing or changed file into `UpdateStatus::Stale` with Download again.

use super::updates::downloaded_toast;
use crate::effect::OsEffect;
use crate::effect::partial::StopKey;
use crate::effect::{Effect, UpdateEffect};
use crate::state::AppState;
use crate::state::updates::{Transfer, UpdateStatus, download_failed_text, downloaded_text, stale_text};
use glowfly_core::app_update::{self, InstallKind, InstallMethod, Progress, Saved, SavedProblem};
use std::path::{Path, PathBuf};
use std::time::Instant;

pub(super) fn download(st: &mut AppState, fx: &mut Vec<Effect>) {
    if st.updates.restarting {
        return;
    }
    let offer = match &st.updates.status {
        UpdateStatus::Available(o)
        | UpdateStatus::DownloadFailed { offer: o, .. }
        | UpdateStatus::Downloaded { offer: o, .. }
        | UpdateStatus::Stale { offer: o, .. } => o.clone(),
        UpdateStatus::Idle
        | UpdateStatus::Checking { .. }
        | UpdateStatus::UpToDate
        | UpdateStatus::CheckFailed(_)
        | UpdateStatus::Downloading(_) => {
            return;
        }
    };
    // Flatpak installs it; nothing is downloaded or opened (a stale toast, say).
    if offer.through_flatpak() {
        return;
    }
    if offer.asset.is_none() {
        // No build for this system: its page lists what there is.
        if !offer.release.html_url.is_empty() {
            fx.push(Effect::Os(OsEffect::OpenUrl(offer.release.html_url.clone())));
        }
        return;
    }
    let total = offer.asset.as_ref().map(|a| a.size).filter(|&n| n > 0);
    st.updates.transfer = Transfer { progress: Progress { done: 0, total }, cancelling: false };
    st.updates.status = UpdateStatus::Downloading(offer.clone());
    fx.push(Effect::Updates(UpdateEffect::Download(offer)));
}

/// The runner's report of how far the download is; ignored once Cancel was pressed.
pub(super) fn progress(st: &mut AppState, p: Progress) {
    if matches!(st.updates.status, UpdateStatus::Downloading(_)) && !st.updates.transfer.cancelling {
        st.updates.transfer.progress = p;
    }
}

/// Cancel: asks the runner to stop. The download stays shown as cancelling until it reports
/// `DownloadCancelled` (or, when it had just finished, `Downloaded`).
pub(super) fn cancel(st: &mut AppState, fx: &mut Vec<Effect>) {
    if matches!(st.updates.status, UpdateStatus::Downloading(_)) && !st.updates.transfer.cancelling {
        st.updates.transfer.cancelling = true;
        fx.push(Effect::StopGit(StopKey::UpdateDownload));
    }
}

/// The download stopped for Cancel: the release is on offer again.
pub(super) fn cancelled(st: &mut AppState) {
    let UpdateStatus::Downloading(offer) = &st.updates.status else { return };
    let offer = offer.clone();
    let text = st.settings.lang().trf("updates.download_cancelled", &[("version", &offer.version)]);
    st.log_quiet(true, text);
    st.updates.transfer = Transfer::default();
    st.updates.status = UpdateStatus::Available(offer);
}

pub(super) fn downloaded(st: &mut AppState, result: Result<Saved, String>, now: Instant) {
    let t = st.settings.lang();
    let UpdateStatus::Downloading(offer) = &st.updates.status else { return };
    let offer = offer.clone();
    st.updates.transfer = Transfer::default();
    match result {
        Ok(Saved { path: file, sha256 }) => {
            let method = method_for(st, &file);
            let text = downloaded_text(t, &offer.version);
            st.log_quiet(true, format!("{text} {}", glowfly_core::native_path::shown(&file)));
            downloaded_toast(st, &offer.version, method, now);
            st.updates.status = UpdateStatus::Downloaded { offer, file, sha256, method };
        }
        Err(error) => {
            st.log_quiet(false, download_failed_text(t, &error));
            st.toast(false, download_failed_text(t, &error), now);
            st.updates.status = UpdateStatus::DownloadFailed { offer, error };
        }
    }
}

/// Reads the downloaded file again, when there is one and Install and restart isn't using it.
pub(super) fn check_saved(st: &AppState, fx: &mut Vec<Effect>) {
    if let UpdateStatus::Downloaded { file, sha256, .. } = st.updates.settled()
        && !st.updates.restarting
    {
        fx.push(Effect::Updates(UpdateEffect::CheckSaved { file: file.clone(), sha256: *sha256 }));
    }
}

/// The downloaded `file` read again: gone or changed, its offer says Download again (also
/// while a check runs, which keeps that).
pub(super) fn saved_checked(st: &mut AppState, file: PathBuf, problem: Option<SavedProblem>) {
    let Some(problem) = problem else { return };
    if st.updates.restarting {
        return;
    }
    let slot = match &mut st.updates.status {
        UpdateStatus::Checking { previous, .. } => previous.as_mut(),
        status => status,
    };
    let UpdateStatus::Downloaded { offer, file: saved, .. } = slot else { return };
    if *saved != file {
        return;
    }
    let offer = offer.clone();
    *slot = UpdateStatus::Stale { offer, file: file.clone(), problem };
    let text = stale_text(st.settings.lang(), &file, problem);
    st.log_quiet(false, text);
}

/// How the downloaded `file` is installed on this system, for this copy (portable when no
/// check has said); only shown when the system isn't one Glowfly knows.
fn method_for(st: &AppState, file: &Path) -> InstallMethod {
    let name = file.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
    match st.updates.platform {
        Some(p) => app_update::install_method_for(&name, st.updates.install.unwrap_or(InstallKind::Portable), p.os),
        None => InstallMethod::ShowOnly,
    }
}
