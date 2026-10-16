//! Opening the repository a link matched and selecting what the link points at, once that
//! repository is the active tab: right away for an open or parked tab, after the load for a
//! new one (`ready`, called at the end of every snapshot load). A missing target offers Fetch,
//! and the repository is checked again when the fetch job ends (`fetched`).

use crate::deep_link::{DeepLinkEffect, DeepLinkMsg, FindOrigin};
use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::deep_link::{Awaiting, OLDER_THAN_LOADED, missing_text};
use crate::state::{AppState, Modal, ToastAction};
use git2::Oid;
use std::time::Instant;
use trellis_core::link::find::{Found, TargetState};
use trellis_core::link::{Target, same_url};

/// Opens the matched repository (selecting its tab, switching to a parked one, or loading it in
/// a new tab) and selects the target as soon as it is the active tab.
pub(super) fn go(st: &mut AppState, f: Found, now: Instant, fx: &mut Vec<Effect>) {
    let Some(p) = st.deep_link.pending.as_mut() else { return };
    p.awaiting = Awaiting::Ready { workdir: f.workdir.clone(), target: f.target, remote: f.remote };
    p.chosen.clear();
    if matches!(st.modal, Some(Modal::DeepLink(_))) {
        st.modal = None;
    }
    super::tabs::open(st, f.workdir, fx);
    ready(st, now, fx);
}

/// The active tab's snapshot is (re)loaded: selects a pending link's target if this is its
/// repository, or checks a repository cloned for a link.
pub(super) fn ready(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    let (Some(snap), Some(p)) = (st.snap.as_ref(), st.deep_link.pending.as_ref()) else { return };
    match p.awaiting.clone() {
        Awaiting::Ready { workdir, target, remote } if st.names_repo(snap, &workdir) => {
            let link = p.link.clone();
            match target {
                TargetState::Present { oid, .. } => {
                    st.deep_link.pending = None;
                    if link.target != Target::Repository {
                        select(st, oid, now, fx);
                    }
                }
                // An unborn HEAD: the repository is all there is to show.
                TargetState::Missing if link.target == Target::Repository => st.deep_link.pending = None,
                TargetState::Missing => {
                    let has_remotes = !snap.remote_urls.is_empty();
                    let text = missing_text(&link, &snap.name, remote.as_deref(), has_remotes);
                    if has_remotes {
                        let action = ToastAction {
                            label: "Fetch".into(),
                            msg: Box::new(Msg::DeepLink(DeepLinkMsg::Fetch)),
                            workdir: Some(snap.workdir.clone()),
                        };
                        st.toast_action(false, text, action, now);
                        if let Some(p) = st.deep_link.pending.as_mut() {
                            p.awaiting = Awaiting::Offered { workdir, remote };
                        }
                    } else {
                        st.toast(false, text, now);
                        st.deep_link.pending = None;
                    }
                }
            }
        }
        Awaiting::Clone { url } if snap.remote_urls.iter().any(|(_, u)| same_url(u, &url)) => {
            let (link, candidates, seq) = (p.link.clone(), vec![snap.workdir.clone()], st.deep_link.seq);
            if let Some(p) = st.deep_link.pending.as_mut() {
                p.awaiting = Awaiting::Search;
            }
            fx.push(Effect::DeepLink(DeepLinkEffect::Find { link, candidates, seq, origin: FindOrigin::AfterClone }));
        }
        _ => {}
    }
}

/// Selects commit `oid` in the active tab, scrolled into view; a commit past the loaded history
/// is shown in the details panel, with a toast saying how to see it in the graph.
fn select(st: &mut AppState, oid: Oid, now: Instant, fx: &mut Vec<Effect>) {
    super::repo::select_commit(st, oid, true, fx);
    if !st.snap.as_ref().is_some_and(|s| s.row_of.contains_key(&oid)) {
        st.toast(true, OLDER_THAN_LOADED, now);
    }
}

/// A job started by the toast's Fetch ended (`ops::finish_job`): `ok` it succeeded, `here` its
/// tab is still active. Then the repository is checked for the target again.
pub(super) fn fetched(st: &mut AppState, ok: bool, here: bool, fx: &mut Vec<Effect>) {
    let Some(p) = st.deep_link.pending.as_ref() else { return };
    let Awaiting::Fetch { workdir, .. } = &p.awaiting else { return };
    if ok && here {
        let (link, candidates, seq) = (p.link.clone(), vec![workdir.clone()], st.deep_link.seq);
        fx.push(Effect::DeepLink(DeepLinkEffect::Find { link, candidates, seq, origin: FindOrigin::AfterFetch }));
    } else {
        // A failed fetch has the job's own toast.
        st.deep_link.pending = None;
    }
}

/// The check after the fetch: selects the target, or says it still isn't there.
pub(super) fn after_fetch(st: &mut AppState, matches: Vec<Found>, now: Instant, fx: &mut Vec<Effect>) {
    let Some(mut p) = st.deep_link.pending.take() else { return };
    let Awaiting::Fetch { workdir, remote } = p.awaiting.clone() else { return };
    if !st.snap.as_ref().is_some_and(|s| st.names_repo(s, &workdir)) {
        return; // another tab is active now
    }
    match matches.into_iter().next().map(|f| f.target) {
        Some(target @ TargetState::Present { oid, .. }) => {
            if st.loading && !st.snap.as_ref().is_some_and(|s| s.row_of.contains_key(&oid)) {
                // The reload after the fetch will have it: select it then (`ready`).
                p.awaiting = Awaiting::Ready { workdir, target, remote };
                st.deep_link.pending = Some(p);
            } else {
                select(st, oid, now, fx);
            }
        }
        _ => {
            let from = remote.unwrap_or_else(|| "all remotes".to_owned());
            let what = p.link.describe();
            st.toast(false, format!("Fetched {from}, but {what} still isn't there; it may have been renamed or deleted."), now);
        }
    }
}

/// Opening the active tab's repository failed (`tabs::open_failed`): a link waiting for it is
/// given up.
pub(super) fn open_failed(st: &mut AppState) {
    let Some(path) = st.tabs.get(st.active_tab).and_then(|t| t.path.clone()) else { return };
    let waiting = st
        .deep_link
        .pending
        .as_ref()
        .is_some_and(|p| matches!(&p.awaiting, Awaiting::Ready { workdir, .. } if st.same_folder(workdir, &path)));
    if waiting {
        st.deep_link.pending = None;
        st.log(false, format!("Couldn't open {} for the link.", trellis_core::native_path::shown(&path)));
    }
}
