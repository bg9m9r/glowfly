//! Trellis's own updates: the daily automatic check (armed by `Startup`, then driven by `Tick`),
//! Check now, downloading the offered file, and opening or showing it. Automatic checks never
//! toast a failure; they log it quietly and wait `RETRY_AFTER`. Only `settings.updates` is
//! saved.

use super::release_notes::{open_notes, startup_notes};
use crate::effect::{Effect, UpdateEffect};
use crate::msg::{CheckedRelease, Msg, UpdateMsg};
use crate::state::updates::{
    RETRY_AFTER, STARTUP_DELAY, UpdateStatus, available_text, check_failed_text, download_failed_text, downloaded_text, is_installer,
    latest_text,
};
use crate::state::{AppState, ToastAction};
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::app_update;

pub(super) fn handle(st: &mut AppState, msg: UpdateMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        UpdateMsg::Startup => {
            st.updates.auto_from = Some(now + STARTUP_DELAY);
            startup_notes(st, fx);
        }
        UpdateMsg::CheckNow => {
            if !st.updates.busy() {
                check(st, true, fx);
            }
        }
        UpdateMsg::Checked { manual, result } => checked(st, manual, result, now, fx),
        UpdateMsg::Download => download(st, fx),
        UpdateMsg::Downloaded(result) => downloaded(st, result, now),
        UpdateMsg::Install => {
            if let Some(file) = st.updates.installer() {
                fx.push(Effect::Updates(UpdateEffect::Install(file.clone())));
            }
        }
        UpdateMsg::ShowFile => {
            if let UpdateStatus::Downloaded { file, .. } = &st.updates.status {
                fx.push(Effect::RevealPath(file.clone()));
            }
        }
        UpdateMsg::OpenReleasePage => {
            let page = st.updates.offer().map(|o| o.release.html_url.clone()).filter(|u| !u.is_empty());
            if let Some(url) = page.or_else(|| app_update::releases_page(app_update::REPOSITORY)) {
                fx.push(Effect::OpenUrl(url));
            }
        }
        UpdateMsg::ShowNotes => open_notes(st, None),
    }
}

/// Once per frame: opens release notes that were waiting, and starts the automatic check when
/// it is due.
pub(super) fn tick(st: &mut AppState, t: Instant, fx: &mut Vec<Effect>) {
    if st.updates.notes_pending.is_some()
        && st.modal.is_none()
        && st.onboarding.welcome.is_none()
        && st.prefs.is_none()
        && st.palette.is_none()
    {
        let since = st.updates.notes_pending.take().and_then(|p| p.since);
        open_notes(st, since);
    }
    let u = &st.updates;
    let armed = u.auto_from.is_some_and(|from| t >= from) && u.retry_at.is_none_or(|at| t >= at);
    if armed && !u.busy() && app_update::due(st.settings.updates.check, st.settings.updates.last_checked, st.view.now) {
        check(st, false, fx);
    }
}

fn check(st: &mut AppState, manual: bool, fx: &mut Vec<Effect>) {
    st.updates.status = UpdateStatus::Checking { manual };
    fx.push(Effect::Updates(UpdateEffect::Check { manual }));
}

fn download_action() -> ToastAction {
    ToastAction { label: "Download".into(), msg: Box::new(Msg::Updates(UpdateMsg::Download)), workdir: None }
}

fn checked(st: &mut AppState, manual: bool, result: Result<Option<CheckedRelease>, String>, now: Instant, fx: &mut Vec<Effect>) {
    let found = match result {
        Ok(found) => found,
        Err(e) => {
            st.log_quiet(false, check_failed_text(&e));
            if manual {
                st.updates.status = UpdateStatus::CheckFailed(e.clone());
                st.toast(false, check_failed_text(&e), now);
            } else {
                st.updates.retry_at = Some(now + RETRY_AFTER);
                st.updates.status = UpdateStatus::Idle;
            }
            return;
        }
    };
    st.settings.updates.last_checked = Some(st.view.now);
    st.updates.retry_at = None;
    fx.push(super::prefs::save_settings(st));
    let u = &st.updates;
    let offer = found.as_ref().and_then(|f| app_update::offer(&f.release, &u.current, u.platform, f.install));
    match offer {
        Some(o) => {
            let text = available_text(&o.version);
            st.log_quiet(true, text.clone());
            // A file already downloaded for this release (or a newer one) stays.
            if !matches!(&st.updates.status, UpdateStatus::Downloaded { offer, .. } if offer.version >= o.version) {
                st.updates.status = UpdateStatus::Available(Box::new(o));
                st.toast_action(true, text, download_action(), now);
            }
        }
        None => {
            st.log_quiet(true, latest_text(&st.updates.current));
            if manual {
                st.toast(true, latest_text(&st.updates.current), now);
            }
            if !matches!(st.updates.status, UpdateStatus::Downloaded { .. }) {
                st.updates.status = UpdateStatus::UpToDate;
            }
        }
    }
}

fn download(st: &mut AppState, fx: &mut Vec<Effect>) {
    let offer = match &st.updates.status {
        UpdateStatus::Available(o) | UpdateStatus::DownloadFailed { offer: o, .. } | UpdateStatus::Downloaded { offer: o, .. } => o.clone(),
        UpdateStatus::Idle
        | UpdateStatus::Checking { .. }
        | UpdateStatus::UpToDate
        | UpdateStatus::CheckFailed(_)
        | UpdateStatus::Downloading(_) => {
            return;
        }
    };
    if offer.asset.is_none() {
        // No build for this system: its page lists what there is.
        if !offer.release.html_url.is_empty() {
            fx.push(Effect::OpenUrl(offer.release.html_url.clone()));
        }
        return;
    }
    st.updates.status = UpdateStatus::Downloading(offer.clone());
    fx.push(Effect::Updates(UpdateEffect::Download(offer)));
}

fn downloaded(st: &mut AppState, result: Result<PathBuf, String>, now: Instant) {
    let UpdateStatus::Downloading(offer) = std::mem::take(&mut st.updates.status) else { return };
    match result {
        Ok(file) => {
            let installer = is_installer(&file);
            let text = downloaded_text(&offer.version);
            st.log_quiet(true, format!("{text} {}", trellis_core::native_path::shown(&file)));
            let (label, msg) = if installer { ("Install", UpdateMsg::Install) } else { ("Show file", UpdateMsg::ShowFile) };
            st.toast_action(true, text, ToastAction { label: label.into(), msg: Box::new(Msg::Updates(msg)), workdir: None }, now);
            st.updates.status = UpdateStatus::Downloaded { offer, file, installer };
        }
        Err(error) => {
            st.log_quiet(false, download_failed_text(&error));
            st.toast(false, download_failed_text(&error), now);
            st.updates.status = UpdateStatus::DownloadFailed { offer, error };
        }
    }
}
