//! "Remember this sign-in": once the job that asked ends, HTTPS sign-ins typed with the box
//! ticked are saved with git's credential helper (the job worked) or dropped from it (the server
//! rejected them), and the results are reported.

use crate::effect::{CredentialEffect, Effect};
use crate::state::AppState;
use crate::state::askpass::CredentialPrompts;
use glowfly_core::auth_failure;
use glowfly_core::git_credential::{HttpsCredential, Remembered};
use std::path::Path;
use std::time::Instant;

/// The job ended (`ok`, its `output`): approve every typed sign-in, or reject those the server
/// turned down. Nothing is rejected when a prompt was cancelled or timed out, or when the job
/// failed for another reason. The rest are dropped, which wipes their passwords.
pub(super) fn settle(p: &mut CredentialPrompts, ok: bool, cwd: Option<&Path>, output: &str, fx: &mut Vec<Effect>) {
    let typed = std::mem::take(&mut p.typed);
    if typed.is_empty() {
        return;
    }
    let cwd = cwd.map(Path::to_path_buf).unwrap_or_else(AppState::outside_dir);
    if ok {
        let remember = |credential| Effect::Credential(CredentialEffect::Remember { cwd: cwd.clone(), credential });
        fx.extend(typed.into_iter().map(remember));
        return;
    }
    if p.ended.is_some() {
        return;
    }
    let hosts: Vec<String> = auth_failure::rejected_urls(output).iter().filter_map(|u| auth_failure::url_host(u)).collect();
    let rejected: Vec<HttpsCredential> = if hosts.is_empty() {
        // git named no URL: only a lone sign-in can be the one that was turned down.
        if typed.len() == 1 && auth_failure::credentials_rejected(output) { typed } else { Vec::new() }
    } else {
        typed.into_iter().filter(|c| hosts.contains(&c.host)).collect()
    };
    fx.extend(rejected.into_iter().map(|credential| Effect::Credential(CredentialEffect::Forget { cwd: cwd.clone(), credential })));
}

/// Saving the sign-in for `host` ended. Handing it to the user's own helper is routine, and
/// saving nothing because the user turned `credential.helper` off is what they asked for: both
/// are only logged. Setting up a store (or git's in-memory cache), or failing to save, is also
/// toasted. No text holds the password.
pub(super) fn remembered(st: &mut AppState, host: &str, result: Result<Remembered, String>, now: Instant) {
    let t = st.settings.lang();
    let (ok, text) = match result {
        Ok(Remembered::Helper) => return st.log_quiet(true, t.trf("sign_in.saved.helper", &[("host", &host)])),
        Ok(Remembered::TurnedOff) => return st.log_quiet(true, t.trf("sign_in.saved.turned_off", &[("host", &host)])),
        Ok(Remembered::SetUp(name)) => (true, t.trf("sign_in.saved.set_up", &[("helper", &name), ("host", &host)])),
        Ok(Remembered::Cached) => (true, t.trf("sign_in.saved.cached", &[("host", &host)])),
        Err(e) => (false, t.trf("sign_in.saved.failed", &[("host", &host), ("why", &e)])),
    };
    st.toast(ok, text.clone(), now);
    st.log(ok, text);
}

/// Dropping the rejected sign-in for `host` ended.
pub(super) fn forgotten(st: &mut AppState, host: &str, result: Result<(), String>) {
    match result {
        Ok(()) => st.log_quiet(true, format!("Dropped the rejected sign-in for {host} from git's credential helper.")),
        Err(e) => st.log(false, format!("Couldn't drop the rejected sign-in for {host}: {e}")),
    }
}
