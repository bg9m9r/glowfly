//! What waits for the trust prompt outside the job gate (`TrustSession::waiting`): it is held
//! here (`hold`), waits only while the prompt that names it is the open dialog, and leaves in
//! one of two ways. Trust on that prompt takes it to run (`take_for_active`, from
//! `trust::released`); anything else ends it without a result (`not_run`): the prompt closed or
//! was answered Don't trust (`drop_waiting`), another dialog took its place (`prune`, called
//! after every update and before a decision), it belongs to another repository, or the
//! runner's re-check sent it back and no prompt could take it again (`trust_seal::stale`).
//! `not_run` is the one place that clears what its sender marked as in flight.

use crate::state::trust::{Waiting, WaitingFor};
use crate::state::{AppState, Modal};
use glowfly_core::paths::same_spelling;
use std::path::Path;

/// Whether the open dialog is the trust prompt that names `w`: its repository's, saying its
/// label, or asking for the terminal when it has none.
fn named(st: &AppState, w: &Waiting) -> bool {
    let Some(Modal::Trust(p)) = &st.modal else { return false };
    let says = if w.label.is_some() { p.held == w.label } else { p.terminal };
    says && same_spelling(&p.real, &w.real)
}

/// Holds `what` for the trust prompt of the active repository, which asks on the terminal's
/// behalf or names `label`: a new prompt, or the open one when it is that repository's and
/// names nothing yet. Any other open dialog, a prompt that names something included, takes
/// nothing more: then `what` comes back and nothing waits.
pub(super) fn hold(st: &mut AppState, what: WaitingFor, label: Option<String>) -> Option<WaitingFor> {
    let Some(real) = st.snap.as_ref().map(|s| s.real_workdir.clone()) else { return Some(what) };
    let terminal = matches!(what, WaitingFor::Terminal(_));
    let free = st.trust.held.is_none() && st.trust.waiting.is_none();
    match &mut st.modal {
        None => super::trust::ask(st, &real, terminal),
        Some(Modal::Trust(p)) if free && p.held.is_none() && !p.terminal && same_spelling(&p.real, &real) => p.terminal = terminal,
        Some(_) => return Some(what),
    }
    let Some(Modal::Trust(p)) = &mut st.modal else { return Some(what) };
    if label.is_some() {
        p.held.clone_from(&label);
    }
    tracing::debug!("holding {} until {} is trusted", p.held.as_deref().unwrap_or("the external terminal"), real.display());
    st.trust.waiting = Some(Waiting { real, what, label });
    None
}

/// Drops what waits unless the open trust prompt names it.
pub(super) fn prune(st: &mut AppState) {
    if st.trust.waiting.as_ref().is_some_and(|w| !named(st, w)) {
        drop_waiting(st);
    }
}

/// Drops what waits, if anything: it doesn't run.
pub(super) fn drop_waiting(st: &mut AppState) {
    let Some(w) = st.trust.waiting.take() else { return };
    tracing::debug!("dropped {} that waited for the trust prompt", w.label.as_deref().unwrap_or("the external terminal"));
    not_run(st, &w.what, &w.real);
}

/// What waits, to run now that the active repository is trusted, when it is that repository's;
/// another repository's is dropped.
pub(super) fn take_for_active(st: &mut AppState) -> Option<Waiting> {
    let w = st.trust.waiting.take()?;
    if st.snap.as_ref().is_some_and(|s| same_spelling(&s.real_workdir, &w.real)) {
        return Some(w);
    }
    not_run(st, &w.what, &w.real);
    None
}

/// `what`, which waited for the repository at `real`, ends without having run to a result:
/// what its sender marked as in flight is cleared.
pub(super) fn not_run(st: &mut AppState, what: &WaitingFor, real: &Path) {
    if let WaitingFor::Read(effect) = what {
        super::file_trust::dropped(st, effect, real);
    }
}

/// Whether a conflict change of the active repository waits for the trust prompt: until it
/// runs, another operation's result is not its result (update/local_done.rs).
pub(super) fn conflict_change_waits(st: &AppState) -> bool {
    let active = |w: &Waiting| st.snap.as_ref().is_some_and(|s| same_spelling(&s.real_workdir, &w.real));
    st.trust
        .waiting
        .as_ref()
        .is_some_and(|w| active(w) && matches!(&w.what, WaitingFor::Read(e) if super::file_trust::is_conflict_change(e)))
}
