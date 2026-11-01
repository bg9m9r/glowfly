//! Preferences > Commit Signing: listing the keys for the configured format, generating a GPG
//! key and copying a public key.

use crate::effect::{Effect, PrefsEffect, UiEffect};
use crate::state::AppState;
use glowfly_core::gpg_prompt::extend::ExtendFailed;
use glowfly_core::signing::{SigningFormat, SigningKey};
use std::time::Instant;

fn non_empty(value: &str, default: &str) -> String {
    if value.trim().is_empty() { default.to_owned() } else { value.trim().to_owned() }
}

/// Lists the keys usable for the configured signing format.
pub(super) fn load_keys(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(p) = st.prefs.as_mut() else { return };
    let Some(git) = p.git.as_ref() else { return };
    let format = SigningFormat::from_config(&git.signing_format);
    p.signing.keys = None;
    p.signing.extend_error = None;
    fx.push(Effect::Prefs(PrefsEffect::ListSigningKeys { format, program: non_empty(git.program(), format.default_program()) }));
}

pub(super) fn keys_loaded(st: &mut AppState, result: Result<Vec<SigningKey>, String>) {
    let Some(p) = st.prefs.as_mut() else { return };
    match result {
        Ok(keys) => {
            p.signing.keys = Some(keys);
            p.signing.keys_error = None;
        }
        Err(e) => {
            p.signing.keys = Some(Vec::new());
            p.signing.keys_error = Some(e);
        }
    }
}

pub(super) fn generate_gpg_key(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(p) = st.prefs.as_mut() else { return };
    let Some(git) = p.git.as_ref() else { return };
    let program = non_empty(&git.gpg_program, "gpg");
    p.signing.generating = true;
    fx.push(Effect::Prefs(PrefsEffect::GenerateGpgKey {
        program,
        name: git.user_name.clone(),
        email: git.user_email.clone(),
        passphrase: std::mem::take(&mut p.signing.passphrase),
    }));
}

/// A generated key becomes the global signing key.
pub(super) fn gpg_key_generated(st: &mut AppState, result: Result<String, String>, now: Instant, fx: &mut Vec<Effect>) {
    if let Some(p) = st.prefs.as_mut() {
        p.signing.generating = false;
    }
    match result {
        Ok(id) => {
            if let Some(git) = st.prefs.as_mut().and_then(|p| p.git.as_mut()) {
                git.signing_key = id.clone();
            }
            fx.push(Effect::Prefs(PrefsEffect::SetGitConfig {
                cwd: st.any_dir(),
                global: true,
                key: "user.signingkey".into(),
                value: Some(id.clone()),
            }));
            st.toast(true, format!("Created GPG key {id} and set it as your signing key"), now);
            load_keys(st, fx);
        }
        Err(e) => st.log(false, format!("Could not generate a GPG key: {e}")),
    }
}

/// Extend by 2 years on the chosen OpenPGP key.
pub(super) fn extend_key(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(p) = st.prefs.as_mut() else { return };
    let Some(git) = p.git.as_ref() else { return };
    if p.signing.extending || git.signing_key.is_empty() || SigningFormat::from_config(&git.signing_format) != SigningFormat::OpenPgp {
        return;
    }
    p.signing.extending = true;
    p.signing.extend_error = None;
    let program = non_empty(&git.gpg_program, SigningFormat::OpenPgp.default_program());
    fx.push(Effect::Prefs(PrefsEffect::ExtendGpgKey { program, key: git.signing_key.clone() }));
}

/// Says how extending went; a change shows in the reloaded list. A failure stays on the page,
/// with the help link, until the next try or reload.
pub(super) fn key_extended(st: &mut AppState, result: Result<(), ExtendFailed>, now: Instant, fx: &mut Vec<Effect>) {
    let t = st.settings.lang();
    let Some(p) = st.prefs.as_mut() else { return };
    p.signing.extending = false;
    let key = p.git.as_ref().map(|g| g.signing_key.clone()).unwrap_or_default();
    let error = match result {
        Ok(()) => {
            st.toast(true, t.trf("prefs.signing.extend.done", &[("key", &key)]), now);
            return load_keys(st, fx);
        }
        // The user chose to stop: nothing to explain.
        Err(ExtendFailed::Cancelled) => return,
        Err(ExtendFailed::TimedOut) => t.tr("prefs.signing.extend.timed_out").to_owned(),
        Err(ExtendFailed::Gpg(e)) => e,
    };
    p.signing.extend_error = Some(error.clone());
    st.log(false, t.trf("prefs.signing.extend.failed", &[("key", &key), ("error", &error)]));
}

pub(super) fn copy_public_key(st: &AppState, fx: &mut Vec<Effect>) {
    let Some(git) = st.prefs.as_ref().and_then(|p| p.git.as_ref()) else { return };
    if git.signing_key.is_empty() {
        return;
    }
    let format = SigningFormat::from_config(&git.signing_format);
    let program = non_empty(git.program(), format.default_program());
    fx.push(Effect::Prefs(PrefsEffect::ExportPublicKey { format, program, key: git.signing_key.clone() }));
}

pub(super) fn public_key_exported(st: &mut AppState, result: Result<String, String>, now: Instant, fx: &mut Vec<Effect>) {
    match result {
        Ok(text) => {
            fx.push(Effect::Ui(UiEffect::CopyText(text)));
            st.toast(true, "Public key copied to the clipboard", now);
        }
        Err(e) => st.log(false, format!("Could not export the public key: {e}")),
    }
}
