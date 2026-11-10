//! What waits for the trust prompt outside the job gate (`TrustSession::waiting`, held by
//! `trust::hold_waiting`) waits only while the prompt that names it is the open dialog: Trust
//! on that prompt runs it (`trust::released`), and anything else drops it (`drop_waiting`), so
//! a prompt never runs work it doesn't name. `prune` is called after every update and before a
//! decision or a new prompt.

use crate::state::trust::{Waiting, WaitingFor};
use crate::state::{AppState, Modal};
use glowfly_core::paths::same_spelling;

/// Whether the open dialog is the trust prompt that names `w`: its repository's, saying its
/// label, or asking for the terminal when it has none.
fn named(st: &AppState, w: &Waiting) -> bool {
    let Some(Modal::Trust(p)) = &st.modal else { return false };
    let says = if w.label.is_some() { p.held == w.label } else { p.terminal };
    says && same_spelling(&p.real, &w.real)
}

/// Drops what waits unless the open trust prompt names it.
pub(super) fn prune(st: &mut AppState) {
    if st.trust.waiting.as_ref().is_some_and(|w| !named(st, w)) {
        drop_waiting(st);
    }
}

/// Drops what waits, if anything: it doesn't run, and what its sender marked as in flight is
/// cleared.
pub(super) fn drop_waiting(st: &mut AppState) {
    let Some(w) = st.trust.waiting.take() else { return };
    tracing::debug!("dropped {} that waited for the trust prompt", w.label.as_deref().unwrap_or("the external terminal"));
    if let WaitingFor::Read(effect) = &w.what {
        super::file_trust::dropped(st, effect);
    }
}
