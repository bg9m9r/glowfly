//! Repository trust: asking once whether to trust a repository whose own config or hooks make
//! git run programs, recording the answer, bringing the prompt back on request (the toolbar's
//! Untrusted chip, the palette), and trusting repositories Trellis cloned or created. What is
//! held until then is checked where it starts (`AppState::runs_git_unasked`).

use crate::effect::{Effect, LoadKind};
use crate::msg::TrustMsg;
use crate::state::tabs::same_path;
use crate::state::trust::{Created, Standing, prompt_for, record, standing};
use crate::state::{AppState, Modal};
use std::path::Path;
use std::time::Instant;
use trellis_core::settings::RepoTrust;

pub(super) fn handle(st: &mut AppState, msg: TrustMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        TrustMsg::Decide { real, trusted } => decide(st, &real, trusted, now, fx),
        TrustMsg::Ask { real } => ask(st, &real),
    }
}

/// Shows the prompt again for the active repository at `real`, if git is held there (Undecided
/// or NotTrusted) and no other dialog is open. Closes the palette, which never covers a dialog.
fn ask(st: &mut AppState, real: &Path) {
    let Some(snap) = st.snap.as_ref().filter(|s| same_path(&s.real_workdir, real)) else { return };
    if !matches!(standing(&st.settings, snap), Standing::Undecided | Standing::NotTrusted) || st.modal.is_some() {
        return;
    }
    let prompt = prompt_for(&st.settings, snap);
    if !st.trust.asked.iter().any(|p| same_path(p, &prompt.real)) {
        st.trust.asked.push(prompt.real.clone());
    }
    st.palette = None;
    st.modal = Some(Modal::Trust(prompt));
}

fn decide(st: &mut AppState, real: &Path, trusted: bool, now: Instant, fx: &mut Vec<Effect>) {
    record(&mut st.settings, real, if trusted { RepoTrust::Trusted } else { RepoTrust::NotTrusted });
    fx.push(super::prefs::save_settings(st));
    if matches!(st.modal, Some(Modal::Trust(_))) {
        st.modal = None;
    }
    let Some(snap) = st.snap.as_ref().filter(|s| same_path(&s.real_workdir, real)) else { return };
    let name = snap.name.clone();
    if trusted {
        st.toast(true, format!("{name} is trusted. Background fetch is on."), now);
        // The load options changed (trusted), so this is a full load whose status git reads.
        super::repo::refresh(st, fx);
        super::forge::refresh(st, true, false, now, fx);
    } else {
        st.log(true, format!("{name} isn't trusted. Trellis won't fetch or contact its hosting service for it on its own."));
    }
}

/// A job that made the repository at `path` succeeded and Trellis is about to open it: a clone,
/// or a `git init` whose `output` says it made a new repository, is trusted when it loads.
pub(super) fn created(st: &mut AppState, path: &Path, how: Created, output: &str) {
    let new = match how {
        Created::Clone => true,
        Created::Init => trellis_core::trust::init_created_new(output),
        Created::Worktree => false,
    };
    if new {
        st.trust.created.push(path.to_path_buf());
    }
}

/// The active tab's snapshot was just replaced by a full load of `kind`.
pub(super) fn loaded(st: &mut AppState, kind: LoadKind, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    let made = st.trust.created.iter().position(|p| same_path(p, &snap.workdir) || same_path(p, &snap.real_workdir));
    if let Some(i) = made {
        st.trust.created.remove(i);
        let real = snap.real_workdir.clone();
        record(&mut st.settings, &real, RepoTrust::Trusted);
        fx.push(super::prefs::save_settings(st));
    }
    maybe_ask(st, kind == LoadKind::Open);
}

/// Shows the trust prompt for the active repository if it is undecided, no other dialog is
/// open, and it wasn't asked about this session (a `fresh_open` asks again).
pub(super) fn maybe_ask(st: &mut AppState, fresh_open: bool) {
    let Some(snap) = &st.snap else { return };
    if standing(&st.settings, snap) != Standing::Undecided || st.modal.is_some() {
        return;
    }
    let asked = st.trust.asked.iter().any(|p| same_path(p, &snap.real_workdir));
    if asked && !fresh_open {
        return;
    }
    tracing::debug!("asking whether to trust {}", snap.real_workdir.display());
    let prompt = prompt_for(&st.settings, snap);
    if !asked {
        st.trust.asked.push(prompt.real.clone());
    }
    st.modal = Some(Modal::Trust(prompt));
}
