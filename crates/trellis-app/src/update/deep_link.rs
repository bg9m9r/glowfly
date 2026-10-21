//! `trellis://` links: copying one, reading one that arrives, the search's answer, the
//! deep-link dialogs and the scheme registration. Opening the matched repository and selecting
//! the target is `deep_link_open.rs`.

use super::deep_link_open::{after_fetch, go};
use super::jobs::{JobSpec, Started, start_job};
use crate::deep_link::{DeepLinkEffect, DeepLinkMsg, FindOrigin, LinkTarget};
use crate::effect::OsEffect;
use crate::effect::{Effect, UiEffect};
use crate::msg::{FolderPurpose, Msg};
use crate::state::deep_link::{self as dl, Awaiting, Best, LinkDialog, Pending, SchemeView};
use crate::state::{AppState, Modal, ToastAction};
use std::path::PathBuf;
use std::time::Instant;
use trellis_core::gitcli::args;
use trellis_core::link::{self, find::Found};

pub(super) fn handle(st: &mut AppState, msg: DeepLinkMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        DeepLinkMsg::Copy(t) => copy(st, &t, now, fx),
        DeepLinkMsg::Built { what, result: Ok(uri) } => {
            fx.push(Effect::Ui(UiEffect::CopyText(uri)));
            st.toast(true, format!("Copied link to {what}"), now);
        }
        DeepLinkMsg::Built { result: Err(e), .. } => st.toast(false, format!("Can't make a link: {e}."), now),
        DeepLinkMsg::Open(text) => open(st, text, now, fx),
        DeepLinkMsg::Found { seq, origin, result } => found(st, seq, origin, result, now, fx),
        DeepLinkMsg::Locate => fx.push(Effect::Os(OsEffect::PickFolder {
            title: "Locate the linked repository".into(),
            purpose: FolderPurpose::LocateLinkedRepo,
        })),
        DeepLinkMsg::Clone => clone(st, fx),
        DeepLinkMsg::Choose(i) => {
            if let Some(Modal::DeepLink(LinkDialog::Choose { paths, picked, .. })) = st.modal.as_mut()
                && i < paths.len()
            {
                *picked = i;
            }
        }
        DeepLinkMsg::OpenChosen => {
            let picked = match &st.modal {
                Some(Modal::DeepLink(LinkDialog::Choose { picked, .. })) => *picked,
                _ => return,
            };
            if let Some(f) = st.deep_link.pending.as_ref().and_then(|p| p.chosen.get(picked)).cloned() {
                go(st, f, now, fx);
            }
        }
        DeepLinkMsg::Fetch => fetch(st, fx),
        DeepLinkMsg::CheckScheme => check_scheme(st, fx),
        DeepLinkMsg::Register => scheme(st, DeepLinkEffect::Register, fx),
        DeepLinkMsg::Unregister => scheme(st, DeepLinkEffect::Unregister, fx),
        DeepLinkMsg::SchemeChecked(result) => {
            st.deep_link.scheme = match result {
                Ok(s) => SchemeView::Known(s),
                Err(e) => SchemeView::Failed(e),
            }
        }
    }
}

fn copy(st: &mut AppState, t: &LinkTarget, now: Instant, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref() else { return };
    match dl::link_request(snap, t) {
        Ok((url, target, what)) => fx.push(Effect::DeepLink(DeepLinkEffect::Build { workdir: snap.workdir.clone(), url, target, what })),
        Err(why) => st.toast(false, format!("Can't make a link: {why}."), now),
    }
}

/// A link arrived: brings the window forward, then reads the link and starts looking for its
/// repository, unless another dialog is open (a toast offers to open it later).
fn open(st: &mut AppState, text: String, now: Instant, fx: &mut Vec<Effect>) {
    fx.push(Effect::Ui(UiEffect::FocusWindow));
    if busy(st) {
        return wait(st, text, now);
    }
    // A newer link replaces whatever an older one was doing.
    st.deep_link.seq += 1;
    st.deep_link.pending = None;
    match link::parse(&text) {
        Err(e) => st.modal = Some(Modal::DeepLink(LinkDialog::Invalid { text: dl::shown_link(&text), reason: e.to_string() })),
        Ok(link) => {
            if matches!(st.modal, Some(Modal::DeepLink(_))) {
                st.modal = None;
            }
            let candidates = dl::candidates(st);
            st.deep_link.pending = Some(Pending::new(link.clone()));
            let (seq, origin) = (st.deep_link.seq, FindOrigin::Search);
            fx.push(Effect::DeepLink(DeepLinkEffect::Find { link, candidates, seq, origin }));
        }
    }
}

/// Whether another dialog or Preferences is open, which a link's dialogs and tab switch must not
/// replace or happen under.
fn busy(st: &AppState) -> bool {
    st.prefs.is_some() || st.modal.as_ref().is_some_and(|m| !matches!(m, Modal::DeepLink(_)))
}

/// A toast offers to open link `text` once the open dialog is closed.
fn wait(st: &mut AppState, text: String, now: Instant) {
    let action = ToastAction { label: "Open link".into(), msg: Box::new(Msg::DeepLink(DeepLinkMsg::Open(text))), workdir: None };
    st.toast_action(true, dl::LINK_WAITING, action, now);
}

fn found(st: &mut AppState, seq: u64, origin: FindOrigin, result: Result<Vec<Found>, String>, now: Instant, fx: &mut Vec<Effect>) {
    if seq != st.deep_link.seq || st.deep_link.pending.is_none() {
        return; // superseded by a newer link, or given up
    }
    let matches = match result {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("looking for a linked repository failed: {e}");
            st.deep_link.pending = None;
            return st.toast(false, format!("Couldn't look for the linked repository: {e}"), now);
        }
    };
    if origin == FindOrigin::AfterFetch {
        return after_fetch(st, matches, now, fx);
    }
    // A dialog or Preferences opened during the search: the link waits rather than replacing
    // it or switching the tab under it.
    if busy(st) {
        let uri = st.deep_link.pending.take().map(|p| p.link.to_uri()).unwrap_or_default();
        return wait(st, uri, now);
    }
    match (dl::best(matches, st), origin) {
        (Best::One(f), _) => go(st, f, now, fx),
        (Best::Many(mut list), FindOrigin::Located | FindOrigin::AfterClone) => go(st, list.remove(0), now, fx),
        (Best::Many(list), _) => choose(st, list),
        (Best::None, FindOrigin::Located) => {
            let folder = st.deep_link.pending.as_ref().and_then(|p| p.located.clone()).unwrap_or_default();
            let shown = trellis_core::native_path::shown(&folder);
            not_found(st, Some(format!("{shown} isn't a copy of this repository: its history and remotes don't match the link.")));
        }
        (Best::None, FindOrigin::AfterClone) => {
            st.deep_link.pending = None;
            st.toast(false, "The cloned repository doesn't match the link.", now);
        }
        (Best::None, _) => not_found(st, None),
    }
}

/// Shows (or updates) the Repository not found dialog.
fn not_found(st: &mut AppState, problem: Option<String>) {
    let Some(p) = st.deep_link.pending.as_mut() else { return };
    p.awaiting = Awaiting::Dialog;
    if let Some(Modal::DeepLink(LinkDialog::NotFound { problem: shown, .. })) = st.modal.as_mut() {
        *shown = problem;
        return;
    }
    let l = &p.link;
    st.modal =
        Some(Modal::DeepLink(LinkDialog::NotFound { what: l.describe(), repo: l.repo_label(), can_clone: l.url.is_some(), problem }));
}

fn choose(st: &mut AppState, list: Vec<Found>) {
    let Some(p) = st.deep_link.pending.as_mut() else { return };
    p.awaiting = Awaiting::Dialog;
    let paths = list.iter().map(|f| f.workdir.clone()).collect();
    p.chosen = list;
    st.modal = Some(Modal::DeepLink(LinkDialog::Choose { what: p.link.describe(), paths, picked: 0 }));
}

/// The folder Locate on disk… picked: checked like the search's candidates.
pub(super) fn located(st: &mut AppState, path: PathBuf, fx: &mut Vec<Effect>) {
    let seq = st.deep_link.seq;
    let Some(p) = st.deep_link.pending.as_mut() else { return };
    p.located = Some(path.clone());
    let link = p.link.clone();
    fx.push(Effect::DeepLink(DeepLinkEffect::Find { link, candidates: vec![path], seq, origin: FindOrigin::Located }));
}

fn clone(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(p) = st.deep_link.pending.as_mut() else { return };
    let Some(url) = p.link.url.clone() else { return };
    p.awaiting = Awaiting::Clone { url: url.clone() };
    super::clone::open_with(st, Some(url), fx);
}

/// The toast's Fetch: the matched remote, or every remote when none had the link's URL.
fn fetch(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(p) = st.deep_link.pending.as_ref() else { return };
    let (workdir, remote) = match &p.awaiting {
        Awaiting::Ready { workdir, remote, .. } | Awaiting::Offered { workdir, remote } | Awaiting::Fetch { workdir, remote } => {
            (workdir.clone(), remote.clone())
        }
        Awaiting::Search | Awaiting::Dialog | Awaiting::Clone { .. } => return,
    };
    if !st.snap.as_ref().is_some_and(|s| st.names_repo(s, &workdir)) {
        return;
    }
    let (label, cmd) = match &remote {
        Some(r) => (format!("Fetch {r}"), trellis_core::remote_ops::fetch_cmd(r)),
        None => {
            let mut cmd = args(&["fetch", "--all"]);
            if st.settings.auto_prune {
                cmd.push("--prune".into());
            }
            ("Fetch all remotes".to_owned(), cmd)
        }
    };
    let spec = JobSpec { then_link: true, ..JobSpec::new(label, vec![cmd]) };
    if start_job(st, spec, fx) != Started::No
        && let Some(p) = st.deep_link.pending.as_mut()
    {
        p.awaiting = Awaiting::Fetch { workdir, remote };
    }
}

/// Reads which program opens links (Preferences opened).
pub(super) fn check_scheme(st: &mut AppState, fx: &mut Vec<Effect>) {
    scheme(st, DeepLinkEffect::SchemeStatus, fx);
}

fn scheme(st: &mut AppState, effect: DeepLinkEffect, fx: &mut Vec<Effect>) {
    st.deep_link.scheme = SchemeView::Checking;
    fx.push(Effect::DeepLink(effect));
}

/// A deep-link dialog was closed (Esc, Cancel, Close): the link is given up.
pub(super) fn modal_closed(st: &mut AppState) {
    st.deep_link.pending = None;
}
