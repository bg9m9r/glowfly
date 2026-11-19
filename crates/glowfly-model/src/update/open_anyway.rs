//! Opening a folder whose Git data points to network places that aren't approved for it
//! (`ReadFailed::NetworkPointer`): the open fails as any failed open does (its tab closes, the
//! failure is logged), and when it was the active tab's open and nothing else is asking, the
//! user is asked whether to open it anyway. Open anyway approves those places for that folder
//! (saved in the settings, and given to every later open by `ControlEffect::NetworkPlaces`) and
//! opens it again; a place found after that asks again. Cancel leaves the failed open as it is.
//! Only the active tab's open asks: a restored tab that isn't shown loads, and so asks, when it
//! is activated. Preferences > Trusted repositories forgets a folder's places.

use crate::effect::{ControlEffect, Effect, LoadKind};
use crate::msg::{Msg, RepoMsg};
use crate::state::trust_list::folder_name;
use crate::state::{AppState, Modal};
use glowfly_core::error::{NetworkPlaces, ReadFailed};
use glowfly_core::i18n::Lang;
use glowfly_core::native_path::shown;
use glowfly_core::repo::Snapshot;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// The places a failed open can be asked about: an open (not a refresh) refused only for
/// network places that aren't approved.
pub(super) fn askable(kind: LoadKind, result: &Result<Box<Snapshot>, ReadFailed>) -> Option<NetworkPlaces> {
    match result {
        Err(ReadFailed::OpenFailed { why, .. }) if kind == LoadKind::Open => match &**why {
            ReadFailed::NetworkPointer(n) if !n.places.is_empty() => Some(n.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// The open numbered `generation` was refused for `n`'s places (`text`, the failure as
/// logged): it fails as any open does, then the active tab's open asks.
pub(super) fn refused(st: &mut AppState, generation: u64, n: NetworkPlaces, text: String, now: Instant, fx: &mut Vec<Effect>) {
    let ask = generation == st.load_gen && st.modal.is_none();
    let path = st.tabs.get(st.active_tab).and_then(|t| t.path.clone());
    let announced = st.logs.announce.is_some();
    super::repo_loaded::finished(st, generation, LoadKind::Open, Err(text), now, fx);
    let Some(path) = path.filter(|_| ask) else { return };
    // The dialog says it all: the log keeps the line, but no toast repeats it.
    if !announced {
        st.logs.announce = None;
    }
    st.modal = Some(prompt(st.settings.lang(), path, n));
}

/// The question for a folder whose Git data points to `n`'s places, opened as `path`.
pub(super) fn prompt(t: Lang, path: PathBuf, n: NetworkPlaces) -> Modal {
    let places = n.places.join(", ");
    let body = if cfg!(windows) {
        t.trf("safe_dir.network.body.windows", &[("places", &places)])
    } else {
        t.trf("safe_dir.network.body.unix", &[("places", &places)])
    };
    Modal::Confirm {
        title: t.tr("safe_dir.network.title").into(),
        body,
        button: t.tr("safe_dir.network.open").into(),
        then: Box::new(Msg::Repo(RepoMsg::OpenAnyway { path, folder: n.folder, places: n.places })),
    }
}

/// Open anyway: `places` are approved for `folder` from now on, and `path` opens again.
pub(super) fn open(st: &mut AppState, path: PathBuf, folder: &Path, places: &[String], now: Instant, fx: &mut Vec<Effect>) {
    st.settings.approve_network_places(folder, places);
    fx.extend(super::prefs::save_settings(st));
    // Before the open below starts: it follows them.
    fx.push(sync(st));
    let text = st.settings.lang().trf("safe_dir.network.allowed", &[("path", &shown(folder)), ("places", &places.join(", "))]);
    st.log(true, text);
    super::repo::handle(st, RepoMsg::Open(path), now, fx);
}

/// Preferences > Trusted repositories: forget the places approved for `folder`.
pub(super) fn forget(st: &mut AppState, folder: &Path, fx: &mut Vec<Effect>) {
    if !st.settings.forget_network_places(folder) {
        return;
    }
    fx.extend(super::prefs::save_settings(st));
    fx.push(sync(st));
    let text = st.settings.lang().trf("trust.network_forgot", &[("name", &folder_name(folder))]);
    st.log(true, text);
}

/// Gives every later open the places approved now.
pub(super) fn sync(st: &AppState) -> Effect {
    Effect::Control(ControlEffect::NetworkPlaces(st.settings.approved_network_places()))
}
