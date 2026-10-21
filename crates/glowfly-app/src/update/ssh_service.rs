//! Preferences > SSH: adding a key's public half to a connected hosting-service account. Services
//! without an API for it (and sign-ins that may not write keys) get the key copied and their
//! SSH key settings page opened instead.

use crate::effect::OsEffect;
use crate::effect::{Effect, UiEffect};
use crate::ssh::{ServiceKeyError, SshEffect};
use crate::state::AppState;
use crate::state::ssh::COPIED;
use crate::state::ssh_service::{UploadForm, UploadState, fallback_note, pasted_toast};
use glowfly_core::forge::{self, Account, AddKeyOutcome};
use glowfly_core::paths::same_spelling;
use std::path::Path;
use std::time::Instant;

/// The listed key's public key line and file name.
fn listed(st: &AppState, key: &Path) -> Option<(String, String, String)> {
    let k = st.prefs.as_ref()?.ssh.key(key)?;
    Some((k.public_key.clone(), k.file_name(), k.comment.clone()))
}

fn connected(st: &AppState, account: &Account) -> bool {
    st.settings.connections.iter().any(|c| &c.account == account)
}

/// Add to service… picked an account: the form under the key, or straight to the fallback for
/// a service Trellis can't add keys to.
pub(super) fn choose(st: &mut AppState, key: &Path, account: Account, now: Instant, fx: &mut Vec<Effect>) {
    let Some((_, file, comment)) = listed(st, key) else { return };
    if !connected(st, &account) || account.provider.is_tracker_only() {
        return;
    }
    if !account.provider.adds_ssh_keys() {
        open_key_settings(st, key, &account, now, fx);
        return;
    }
    let Some(page) = st.prefs.as_mut().map(|p| &mut p.ssh) else { return };
    let private = page.key(key).map(|k| k.private.clone()).unwrap_or_else(|| key.to_path_buf());
    page.upload = Some(UploadForm::new(private, account, forge::default_title(&comment, &file)));
}

/// Copies the key and opens the account's SSH key page, when the service has one.
pub(super) fn open_key_settings(st: &mut AppState, key: &Path, account: &Account, now: Instant, fx: &mut Vec<Effect>) {
    let Some((public_key, _, _)) = listed(st, key) else { return };
    fx.push(Effect::Ui(UiEffect::CopyText(public_key)));
    match forge::ssh_keys_page(account) {
        Some(url) => {
            fx.push(Effect::Os(OsEffect::OpenUrl(url)));
            st.toast(true, pasted_toast(account.provider.name()), now);
        }
        None => st.toast(true, COPIED, now),
    }
}

/// The form's Add key button.
pub(super) fn send(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(form) = st.prefs.as_ref().and_then(|p| p.ssh.upload.as_ref()) else { return };
    if form.send_blocked(st.settings.lang()).is_some() {
        return;
    }
    let (key, account, title) = (form.key.clone(), form.account.clone(), form.title_to_send());
    let Some((public_key, _, _)) = listed(st, &key) else { return };
    if let Some(form) = st.prefs.as_mut().and_then(|p| p.ssh.upload.as_mut()) {
        form.state = UploadState::Sending;
    }
    fx.push(Effect::Ssh(SshEffect::AddToService { account, public_key, title, key }));
}

pub(super) fn added(
    st: &mut AppState,
    key: &Path,
    account: &Account,
    result: Result<AddKeyOutcome, ServiceKeyError>,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    // An answer for a form that is no longer open is dropped.
    let open = st.prefs.as_ref().and_then(|p| p.ssh.upload.as_ref()).is_some_and(|f| same_spelling(&f.key, key) && &f.account == account);
    if !open {
        return;
    }
    let service = account.provider.name();
    let file = key.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    let state = match result {
        Ok(AddKeyOutcome::Added { title }) => {
            if let Some(p) = st.prefs.as_mut() {
                p.ssh.upload = None;
            }
            st.toast(true, format!("Added {file} to {service} as \u{201c}{title}\u{201d}"), now);
            return;
        }
        Ok(AddKeyOutcome::AlreadyThere { title }) => UploadState::AlreadyThere(title),
        Err(e) if e.fallback => {
            open_key_settings(st, key, account, now, fx);
            UploadState::Failed { message: fallback_note(st.settings.lang(), service), fallback: true }
        }
        Err(e) => {
            st.log(false, format!("Could not add {file} to {service}: {}", e.message));
            UploadState::Failed { message: e.message, fallback: false }
        }
    };
    if let Some(form) = st.prefs.as_mut().and_then(|p| p.ssh.upload.as_mut()) {
        form.state = state;
    }
}
