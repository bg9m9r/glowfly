//! Glowfly's own updates: the daily automatic check (armed by `Startup`, then driven by `Tick`),
//! Check now, downloading the offered file (update/updates_download.rs), and showing it or
//! installing it (Install and restart, update/updates_restart.rs). Automatic checks never toast a failure; they log it
//! quietly and wait `RETRY_AFTER`. A Flatpak copy is only told about a release (Flatpak installs
//! it) and never downloads one. Only `settings.updates` is saved.

use super::release_notes::{open_notes, startup_notes};
use super::updates_download as dl;
use crate::effect::OsEffect;
use crate::effect::{Effect, UpdateEffect};
use crate::msg::{CheckedRelease, Msg, UpdateMsg};
use crate::state::updates::{
    RETRY_AFTER, STARTUP_DELAY, UpdateStatus, available_text, check_failed_text, downloaded_text, flatpak_available_text, install_label,
    latest_text,
};
use crate::state::{AppState, ToastAction};
use glowfly_core::app_update::{self, InstallMethod};
use glowfly_core::i18n::Lang;
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, msg: UpdateMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        UpdateMsg::Startup => {
            st.updates.auto_from = Some(now + STARTUP_DELAY);
            startup_notes(st, fx);
            fx.push(Effect::Updates(UpdateEffect::Tidy));
        }
        UpdateMsg::CheckNow => {
            if !st.updates.blocked() {
                check(st, true, fx);
            }
        }
        UpdateMsg::Checked { manual, result } => checked(st, manual, result, now, fx),
        UpdateMsg::Download => dl::download(st, fx),
        UpdateMsg::CancelDownload => dl::cancel(st, fx),
        UpdateMsg::DownloadProgress(p) => dl::progress(st, p),
        UpdateMsg::Downloaded(result) => dl::downloaded(st, result, now),
        UpdateMsg::DownloadCancelled => dl::cancelled(st),
        UpdateMsg::SavedChecked { file, problem } => dl::saved_checked(st, file, problem),
        UpdateMsg::InstallAndRestart => super::updates_restart::ask(st, false, now, fx),
        UpdateMsg::InstallAndRestartConfirmed => super::updates_restart::ask(st, true, now, fx),
        UpdateMsg::RestartReady(result) => super::updates_restart::ready(st, result, now, fx),
        UpdateMsg::ShowFile => {
            if let UpdateStatus::Downloaded { file, .. } = st.updates.settled() {
                fx.push(Effect::Os(OsEffect::RevealPath(file.clone())));
            }
        }
        UpdateMsg::OpenReleasePage => {
            let page = st.updates.offer().map(|o| o.release.html_url.clone()).filter(|u| !u.is_empty());
            if let Some(url) = page.or_else(|| app_update::releases_page(app_update::REPOSITORY)) {
                fx.push(Effect::Os(OsEffect::OpenUrl(url)));
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
    if armed && !u.blocked() && app_update::due(st.settings.updates.check, st.settings.updates.last_checked, st.view.now) {
        check(st, false, fx);
    }
}

/// Starts a check, keeping the current status for the answer to fall back on.
fn check(st: &mut AppState, manual: bool, fx: &mut Vec<Effect>) {
    let previous = Box::new(std::mem::take(&mut st.updates.status));
    st.updates.status = UpdateStatus::Checking { manual, previous };
    fx.push(Effect::Updates(UpdateEffect::Check { manual }));
}

fn download_action(t: Lang) -> ToastAction {
    ToastAction { label: t.tr("updates.download.button").into(), msg: Box::new(Msg::Updates(UpdateMsg::Download)), workdir: None }
}

/// Release notes, the action of a Flatpak copy's toast (it has nothing to download).
fn notes_action(t: Lang) -> ToastAction {
    ToastAction { label: t.tr("updates.release_notes.button").into(), msg: Box::new(Msg::Updates(UpdateMsg::ShowNotes)), workdir: None }
}

fn checked(st: &mut AppState, manual: bool, result: Result<Option<CheckedRelease>, String>, now: Instant, fx: &mut Vec<Effect>) {
    let t = st.settings.lang();
    // What the status was before the check: a downloaded file or an offer outlives the answer.
    let prior = match std::mem::take(&mut st.updates.status) {
        UpdateStatus::Checking { previous, .. } => *previous,
        other => other,
    };
    let found = match result {
        Ok(found) => {
            if let Some(f) = &found {
                st.updates.install = Some(f.install);
            }
            found
        }
        Err(e) => {
            st.log_quiet(false, check_failed_text(t, &e));
            if manual {
                st.toast(false, check_failed_text(t, &e), now);
            } else {
                st.updates.retry_at = Some(now + RETRY_AFTER);
            }
            st.updates.status = if manual && prior.offer().is_none() { UpdateStatus::CheckFailed(e) } else { prior };
            return;
        }
    };
    st.settings.updates.last_checked = Some(st.view.now);
    st.updates.retry_at = None;
    fx.push(super::prefs::save_settings(st));
    let u = &st.updates;
    let offer = found.as_ref().and_then(|f| app_update::offer(&f.release, &u.current, u.platform, f.install));
    // A file already downloaded for this release (or a newer one) stays.
    let kept = match (&offer, &prior) {
        (Some(o), UpdateStatus::Downloaded { offer: d, method, .. }) if d.version >= o.version => Some((d.version.clone(), *method)),
        _ => None,
    };
    if let Some((version, method)) = kept {
        st.log_quiet(true, available_text(t, &version));
        if manual {
            downloaded_toast(st, &version, method, now);
        }
        st.updates.status = prior;
        // The file may have gone or changed since: read it again.
        dl::check_saved(st, fx);
        return;
    }
    match offer {
        Some(o) => {
            let (text, action) = if o.through_flatpak() {
                (flatpak_available_text(t, &o.version), notes_action(t))
            } else {
                (available_text(t, &o.version), download_action(t))
            };
            st.log_quiet(true, text.clone());
            // An automatic check toasts a release once, not again each day.
            let known = prior.offer().is_some_and(|p| p.version >= o.version);
            if manual || !known {
                st.toast_action(true, text, action, now);
            }
            // A file found gone or changed for this release still says Download again.
            let stale = matches!(&prior, UpdateStatus::Stale { offer: d, .. } if d.version >= o.version);
            st.updates.status = if stale { prior } else { UpdateStatus::Available(Box::new(o)) };
        }
        None => {
            st.log_quiet(true, latest_text(t, &st.updates.current));
            if manual {
                st.toast(true, latest_text(t, &st.updates.current), now);
            }
            let keep = matches!(prior, UpdateStatus::Downloaded { .. } | UpdateStatus::Stale { .. });
            st.updates.status = if keep { prior } else { UpdateStatus::UpToDate };
        }
    }
}

/// "Downloaded Glowfly x.y.z." with Install and restart or Open installer when the file can
/// be installed, else Show file.
pub(super) fn downloaded_toast(st: &mut AppState, version: &app_update::Version, method: InstallMethod, now: Instant) {
    let t = st.settings.lang();
    let (label, msg) = match method {
        InstallMethod::OpenAndQuit => (t.tr("updates.open_installer.button"), UpdateMsg::InstallAndRestart),
        m if m.acts() => (install_label(t, m), UpdateMsg::InstallAndRestart),
        _ => (t.tr("updates.show_file.button"), UpdateMsg::ShowFile),
    };
    let action = ToastAction { label: label.into(), msg: Box::new(Msg::Updates(msg)), workdir: None };
    st.toast_action(true, downloaded_text(t, version), action, now);
}
