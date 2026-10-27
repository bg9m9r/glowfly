//! Fetch full history's size check: before downloading a shallow clone's whole history, its
//! size is asked of the hosting service (when the repository is trusted and the service may be
//! contacted) or measured on disk for a source on this computer. A large one (500 MiB or more)
//! asks first (state/partial_confirm.rs); anything else, an unknown size included, fetches
//! straight away. The status bar shows the check while it runs.

use crate::effect::Effect;
use crate::effect::partial::{HistorySizeRequest, PartialEffect, SizeService};
use crate::msg::{JobMsg, Msg};
use crate::state::partial::Sizing;
use crate::state::{AppState, Modal};
use glowfly_core::forge;
use glowfly_core::history_size::HistorySize;
use glowfly_core::paths::same_spelling;
use glowfly_core::shallow_ops::Deepen;
use std::path::Path;

/// Starts sizing the full history that `remote` would send to the active repository.
pub(super) fn start(st: &mut AppState, remote: String, fx: &mut Vec<Effect>) {
    if super::jobs::busy(st, false) {
        return;
    }
    let Some(snap) = &st.snap else { return };
    let workdir = snap.workdir.clone();
    if st.partial.sizing.as_ref().is_some_and(|s| same_spelling(&s.workdir, &workdir)) {
        return; // a second click while the first check runs
    }
    let url = snap.remote_info.iter().find(|r| r.name == remote).map(|r| r.fetch_url().to_owned()).filter(|u| !u.is_empty());
    let connections = &st.settings.connections;
    // Signing in may ask the repository's credential helper: only for a trusted repository.
    let service = url
        .as_deref()
        .filter(|_| st.runs_git_unasked())
        .and_then(|u| forge::parse_remote(u, connections))
        .filter(|t| forge::may_contact(&t.account(), connections))
        .map(|target| SizeService {
            auth: st.session.forge.as_ref().and_then(|f| f.auth.get(&target.account().key())).cloned(),
            connected: forge::is_connected(&target.account(), connections),
            target,
        });
    let id = st.partial.take_id();
    st.partial.sizing = Some(Sizing { id, workdir: workdir.clone(), remote });
    fx.push(Effect::Partial(PartialEffect::SizeHistory(HistorySizeRequest { workdir, id, url, service })));
}

/// Size check `id` ended: asks before a large download, else fetches the full history now.
/// Dropped when another check replaced it, its repository is no longer the active tab, or the
/// history is complete by now.
pub(super) fn sized(st: &mut AppState, workdir: &Path, id: u64, size: HistorySize, fx: &mut Vec<Effect>) {
    let Some(sizing) = st.partial.sizing.take_if(|s| s.id == id) else { return };
    let Some(snap) = st.snap.as_ref().filter(|s| same_spelling(&s.workdir, workdir) && s.is_shallow()) else { return };
    let Some((title, body, button)) = crate::state::partial_confirm::confirm(&size, &sizing.remote, snap.commits.len(), snap.truncated)
    else {
        return super::shallow::deepen_now(st, Deepen::Full, fx);
    };
    if st.modal.is_some() {
        st.log(false, "Fetch full history didn't start: close the open dialog and try again.");
        return;
    }
    let then = Box::new(Msg::Job(JobMsg::DeepenNow(Deepen::Full)));
    st.modal = Some(Modal::Confirm { title, body, button, then });
}
