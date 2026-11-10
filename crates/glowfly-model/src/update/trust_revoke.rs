//! Preferences > Trusted repositories: checking which decided folders still exist when the page
//! opens (`TrustEffect::CheckFolders`), revoking or forgetting one decision with every decision
//! passed on from it, and forgetting every decision whose folder the check found missing. A
//! revoked repository open in a tab stands Undecided at once (standing is read from the
//! settings); it isn't asked about over Preferences, but the next job, terminal or read that
//! needs git there asks through the gate, and the active one reloads without git's status read.

use crate::effect::{Effect, TrustEffect};
use crate::msg::FolderCheck;
use crate::state::AppState;
use crate::state::trust_list::folder_name;
use glowfly_core::paths::same_spelling;
use glowfly_core::settings::RepoTrust;
use std::path::{Path, PathBuf};

/// The page opened: check which of the decided folders are still there. With no decisions
/// there is nothing to check.
pub(super) fn page_opened(st: &mut AppState, fx: &mut Vec<Effect>) {
    let paths: Vec<PathBuf> = st.settings.trust_decisions().into_iter().map(|d| d.path).collect();
    let Some(p) = st.prefs.as_mut() else { return };
    if paths.is_empty() {
        p.trust.missing = Some(Vec::new());
        return;
    }
    p.trust.missing = None;
    fx.push(Effect::Trust(TrustEffect::CheckFolders(paths)));
}

/// The folder check reported: the folders that aren't there are marked missing, while
/// Preferences is open.
pub(super) fn folders_checked(st: &mut AppState, checks: &[FolderCheck]) {
    let Some(p) = st.prefs.as_mut() else { return };
    p.trust.missing = Some(checks.iter().filter(|c| !c.present).map(|c| c.path.clone()).collect());
}

/// The row's Revoke or Forget: removes the decision for `real` and every one passed on from it.
pub(super) fn revoke(st: &mut AppState, real: &Path, fx: &mut Vec<Effect>) {
    let trusted = st.settings.saved_trust(real).is_some_and(|t| t.trust == RepoTrust::Trusted);
    let gone = st.settings.revoke_trust(real);
    let Some(first) = gone.first() else { return };
    let t = st.settings.lang();
    let (name, heirs) = (folder_name(first), gone.len() - 1);
    let text = match (trusted, heirs) {
        (true, 0) => t.trf("trust.revoked", &[("name", &name)]),
        (true, n) => t.trn("trust.revoked_with", n, &[("name", &name)]),
        (false, 0) => t.trf("trust.forgot", &[("name", &name)]),
        (false, n) => t.trn("trust.forgot_with", n, &[("name", &name)]),
    };
    removed(st, &gone, fx);
    st.log(true, text);
}

/// Forget missing folders: removes the decision of every folder the last check found missing
/// that still has one, with the decisions passed on from them.
pub(super) fn forget_missing(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(missing) = st.prefs.as_mut().and_then(|p| p.trust.missing.as_mut()) else { return };
    let listed = std::mem::take(missing);
    let mut gone = Vec::new();
    let mut forgotten = 0;
    for path in &listed {
        let removed = st.settings.revoke_trust(path);
        forgotten += usize::from(!removed.is_empty());
        gone.extend(removed);
    }
    if gone.is_empty() {
        return;
    }
    removed(st, &gone, fx);
    let t = st.settings.lang();
    st.log(true, t.trn("trust.forgot_missing", forgotten, &[]));
}

/// The decisions for `gone` were just removed: nothing set up under them goes on, the settings
/// are saved, and the active repository reloads if it is one of them.
fn removed(st: &mut AppState, gone: &[PathBuf], fx: &mut Vec<Effect>) {
    let among = |p: &Path| gone.iter().any(|g| same_spelling(g, p));
    // Asked about already: a reload doesn't bring the prompt up over Preferences.
    for g in gone {
        if !st.trust.asked.iter().any(|p| same_spelling(p, g)) {
            st.trust.asked.push(g.clone());
        }
    }
    st.trust.inherit.take_if(|i| among(&i.from));
    st.trust.lfs_install.take_if(|(_, p)| among(p));
    st.trust.adopt_next.take_if(|p| among(p));
    fx.extend(super::prefs::save_settings(st));
    let active = st.snap.as_ref().is_some_and(|s| among(&s.real_workdir) || gone.iter().any(|g| st.names_repo(s, g)));
    if active {
        // The load options changed (no longer trusted): it reloads without git's status read.
        super::repo::refresh(st, fx);
    }
}
