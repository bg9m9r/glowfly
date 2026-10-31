//! Preferences > SSH: Use existing (pick a key pair outside the .ssh folder in the OS file
//! dialog, list it and remember it in `Settings::ssh_extra_keys`) and Remove from list (forget
//! one again). Glowfly never copies, moves or deletes key files.

use crate::effect::Effect;
use crate::ssh::SshEffect;
use crate::state::AppState;
use glowfly_core::paths::same_spelling;
use glowfly_core::ssh_keys::{ConfigStatus, KeyFileProblem, SshKeyPair};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// The Use existing… button: opens the file dialog in the .ssh folder, once it has been read.
pub(super) fn use_existing(st: &mut AppState, fx: &mut Vec<Effect>) {
    let title = st.settings.lang().tr("prefs.ssh.keys.pick.title");
    let Some(page) = st.prefs.as_mut().map(|p| &mut p.ssh) else { return };
    let Some(listing) = &page.listing else { return };
    let effect = SshEffect::PickKey { title: title.into(), start: listing.dir.clone(), keygen: listing.keygen.clone() };
    page.pick_error = None;
    fx.push(Effect::Ssh(effect));
}

/// What the file dialog picked: a problem shows under the buttons, a new pair is listed and
/// remembered, a listed one is only named in a toast.
pub(super) fn picked(st: &mut AppState, result: Option<(PathBuf, Result<SshKeyPair, KeyFileProblem>)>, now: Instant, fx: &mut Vec<Effect>) {
    let Some((path, result)) = result else { return };
    let t = st.settings.lang();
    let Some(page) = st.prefs.as_mut().map(|p| &mut p.ssh) else { return };
    let pair = match result {
        Ok(pair) => pair,
        Err(problem) => {
            let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            page.pick_error = Some((file, problem));
            return;
        }
    };
    let file = pair.file_name();
    page.pick_error = None;
    if page.key(&pair.private).is_some() {
        st.toast(true, t.trf("prefs.ssh.keys.already.toast", &[("file", &file)]), now);
        return;
    }
    let Some(listing) = page.listing.as_mut() else { return };
    let private = pair.private.clone();
    listing.keys.push(pair);
    // settings.json holds paths as text: one that isn't valid Unicode is listed for now only.
    let extra = &mut st.settings.ssh_extra_keys;
    if private.to_str().is_some() && !extra.iter().any(|p| same_spelling(p, &private)) {
        extra.push(private);
        fx.extend(super::prefs::save_settings(st));
    }
    st.toast(true, t.trf("prefs.ssh.keys.added.toast", &[("file", &file)]), now);
}

/// Remove from list: forgets a key added with Use existing. Its files stay where they are; a
/// key that wasn't added that way is left alone, and one the config also names stays listed.
pub(super) fn forget(st: &mut AppState, key: &Path, fx: &mut Vec<Effect>) {
    let extra = &mut st.settings.ssh_extra_keys;
    let before = extra.len();
    extra.retain(|p| !same_spelling(p, key));
    if extra.len() == before {
        return;
    }
    // A key ~/.ssh/config names stays listed: only the remembered entry goes.
    if let Some(listing) = st.prefs.as_mut().and_then(|p| p.ssh.listing.as_mut()) {
        let named = listing.config.iter().any(|c| matches!(&c.status, ConfigStatus::Key(p) if same_spelling(p, key)));
        if !named {
            listing.keys.retain(|k| !same_spelling(&k.private, key));
        }
    }
    fx.extend(super::prefs::save_settings(st));
}
