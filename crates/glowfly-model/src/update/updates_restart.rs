//! Install and restart: after Preferences' unsaved edits (prefs_leave), a running operation, and
//! the tabs' unsaved work and terminals running a program (asked about first,
//! update/terminal_busy.rs) have been dealt with, the downloaded update is prepared on a worker
//! (runner/update_restart.rs). Once it is ready the window closes, the same way as Quit;
//! a failure changes nothing and says why. At the next start, an installer that failed or was
//! cancelled is said too (`last_install`).

use crate::effect::{Effect, RestartJob, UiEffect, UpdateEffect};
use crate::msg::{Msg, UpdateMsg};
use crate::state::terminal::busy::{BusyTerminal, running_ids};
use crate::state::updates::{UpdateStatus, last_install_text, opened_text, restart_failed_text, restarting_text};
use crate::state::{AppState, ToastAction, tab_losses};
use crate::terminal::BusyCheck;
use glowfly_core::app_update::{InstallMethod, InstallProblem, LastInstall};
use std::time::Instant;

/// `InstallAndRestart` (`confirmed`: from the confirmation, which already named the losses and
/// the busy terminals). Running shells are asked about first (the answer comes to `checked`).
pub(super) fn ask(st: &mut AppState, confirmed: bool, now: Instant, fx: &mut Vec<Effect>) {
    decide(st, confirmed, None, now, fx);
}

/// The shells' answer for Install and restart: `busy` are the terminals running a program.
pub(super) fn checked(st: &mut AppState, busy: Vec<BusyTerminal>, now: Instant, fx: &mut Vec<Effect>) {
    decide(st, false, Some(busy), now, fx);
}

fn decide(st: &mut AppState, confirmed: bool, busy: Option<Vec<BusyTerminal>>, now: Instant, fx: &mut Vec<Effect>) {
    let Some((_, method)) = st.updates.downloaded() else { return };
    if !method.acts() || st.updates.blocked() {
        return;
    }
    let t = st.settings.lang();
    if st.job.is_some() {
        st.toast(false, t.tr("updates.restart_busy"), now);
        return;
    }
    if !confirmed {
        let busy = match busy {
            Some(busy) => busy,
            None => {
                let ids = running_ids(st);
                if !ids.is_empty() {
                    super::terminal_busy::check(ids, BusyCheck::Restart, fx);
                    return;
                }
                Vec::new()
            }
        };
        if let Some(modal) = tab_losses::restart_confirm(st, method, &busy, Msg::Updates(UpdateMsg::InstallAndRestartConfirmed)) {
            st.palette = None;
            crate::state::dialog_guard::open_over(&mut st.modal, &mut st.dialog, modal);
            return;
        }
    }
    let UpdateStatus::Downloaded { offer, file, sha256, method } = st.updates.settled() else { return };
    let job = RestartJob { file: file.clone(), sha256: *sha256, method: *method, version: offer.version.clone() };
    st.updates.restarting = true;
    fx.push(Effect::Updates(UpdateEffect::Restart(Box::new(job))));
}

/// The worker's answer: the window closes, or the failure is said and nothing else changes.
pub(super) fn ready(st: &mut AppState, result: Result<(), String>, now: Instant, fx: &mut Vec<Effect>) {
    if !st.updates.restarting {
        return;
    }
    st.updates.restarting = false;
    let t = st.settings.lang();
    let Some((version, method)) = st.updates.offer().map(|o| o.version.clone()).zip(st.updates.downloaded().map(|d| d.1)) else {
        return;
    };
    match result {
        Ok(()) => {
            let text = if method == InstallMethod::OpenAndQuit { opened_text(t, &version) } else { restarting_text(t, &version) };
            st.log_quiet(true, text);
            // Nothing else starts while the window closes. Tabs and settings are saved as they
            // change.
            st.updates.restarting = true;
            fx.push(Effect::Ui(UiEffect::CloseWindow));
        }
        Err(e) => {
            let text = restart_failed_text(t, &e);
            st.log_quiet(false, text.clone());
            let action = ToastAction {
                label: t.tr("updates.show_file.button").into(),
                msg: Box::new(Msg::Updates(UpdateMsg::ShowFile)),
                workdir: None,
            };
            st.toast_action(false, text, action, now);
            // A file gone or changed since is offered for download again.
            super::updates_download::check_saved(st, fx);
        }
    }
}

/// The update log says the last installer run failed or was cancelled: a toast says so, with
/// Show log when the log says more.
pub(super) fn last_install(st: &mut AppState, last: LastInstall, now: Instant) {
    let t = st.settings.lang();
    let text = last_install_text(t, &last);
    st.log_quiet(false, text.clone());
    if matches!(last.problem, InstallProblem::Failed(_) | InstallProblem::NotRun(_)) {
        let action = ToastAction {
            label: t.tr("updates.show_log.button").into(),
            msg: Box::new(Msg::Updates(UpdateMsg::ShowUpdateLog(last.log))),
            workdir: None,
        };
        st.toast_action(false, text, action, now);
    } else {
        st.toast(false, text, now);
    }
}
