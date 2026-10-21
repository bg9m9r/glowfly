//! Preferences > SSH on Windows: Use Windows' OpenSSH writes the global core.sshCommand that
//! makes git run Windows' own ssh (which reads keys from the agent service), and Use Git's own
//! ssh unsets it. Both run only in the state `state::ssh_windows::windows_offer` offers them in,
//! and share `choice_busy` with the Which key writes: parallel `git config` writes would fight
//! over the config file's lock.

use crate::effect::Effect;
use crate::ssh::SshEffect;
use crate::state::AppState;
use crate::state::ssh_windows::{WindowsOffer, windows_blocked, windows_offer};
use std::time::Instant;

/// The row's button: switch (`on`) or undo, unless the page's state doesn't offer that now.
pub(super) fn use_windows_ssh(st: &mut AppState, on: bool, fx: &mut Vec<Effect>) {
    let t = st.settings.lang();
    let Some(page) = st.prefs.as_mut().map(|p| &mut p.ssh) else { return };
    let program = match (on, windows_offer(page, t)) {
        (true, WindowsOffer::Offer { windows, .. }) if !page.choice_busy => Some(windows),
        (false, WindowsOffer::On { .. }) if !page.choice_busy => None,
        _ => {
            let why = windows_blocked(page, t).unwrap_or(t.tr("prefs.ssh.windows.unavailable.tip"));
            page.windows_error = Some(why.into());
            return;
        }
    };
    page.choice_busy = true;
    page.choice_repo = false;
    page.choice_key = None;
    page.windows_error = None;
    fx.push(Effect::Ssh(SshEffect::SetWindowsSsh { program }));
}

/// The write finished: on success, say so and read the choice again.
pub(super) fn windows_ssh_set(st: &mut AppState, on: bool, result: Result<(), String>, now: Instant, fx: &mut Vec<Effect>) {
    let t = st.settings.lang();
    if let Some(p) = st.prefs.as_mut() {
        p.ssh.choice_busy = false;
    }
    match result {
        Ok(()) => {
            let text = if on { t.tr("prefs.ssh.windows.on.toast") } else { t.tr("prefs.ssh.windows.off.toast") };
            st.toast(true, text, now);
            super::ssh_use::read_choice(st, fx);
        }
        Err(e) => {
            if let Some(p) = st.prefs.as_mut() {
                p.ssh.windows_error = Some(e.clone());
            }
            st.log(false, format!("Could not change core.sshCommand: {e}"));
        }
    }
}
