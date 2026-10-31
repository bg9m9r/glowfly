//! Passing trust on to the repositories Glowfly makes from a trusted one: a worktree it creates
//! (whose config and hooks are its repository's) and submodules it checks out for the first time
//! (like a clone Glowfly made). The job's caller says what it will make (`expect`); when the job
//! succeeds and the source is still trusted, the runner reads the new repositories
//! (`Effect::Trust`) and `inherited` records them, never over a decision already saved. Also
//! keeps a `git lfs install` from Glowfly from asking again about the settings and hooks it adds.

use crate::effect::{Effect, TrustEffect};
use crate::msg::InheritedRepo;
use crate::state::AppState;
use crate::state::trust::{Inherit, InheritKind, Standing, record_trusted, standing};
use glowfly_core::paths::same_spelling;
use std::path::{Path, PathBuf};

/// The job labelled `label` that just started in the active repository makes the repositories
/// `paths`: they are trusted once it succeeds, if the active repository is trusted (not Clean,
/// not changed since).
pub(super) fn expect(st: &mut AppState, label: &str, kind: InheritKind, paths: Vec<PathBuf>) {
    st.trust.inherit = match st.snap.as_ref() {
        Some(snap) if !paths.is_empty() && standing(&st.settings, snap) == Standing::Trusted => {
            Some(Inherit { label: label.to_owned(), kind, from: snap.real_workdir.clone(), paths })
        }
        _ => None,
    };
}

/// The `git lfs install` job labelled `label` just started in the active repository: if that
/// repository is trusted, what it adds is adopted at the next load instead of asking.
pub(super) fn expect_lfs_install(st: &mut AppState, label: &str) {
    st.trust.lfs_install =
        st.snap.as_ref().filter(|s| standing(&st.settings, s) == Standing::Trusted).map(|s| (label.to_owned(), s.real_workdir.clone()));
}

/// The active job labelled `label` ended (`ok`: it succeeded). When it made repositories from a
/// repository that is still trusted, asks the runner to read them, opening `then_open` after
/// they are recorded when it is one of them; returns whether it took over that open.
pub(super) fn job_ended(st: &mut AppState, label: &str, ok: bool, then_open: Option<&Path>, fx: &mut Vec<Effect>) -> bool {
    if let Some((l, real)) = st.trust.lfs_install.take()
        && ok
        && l == label
    {
        st.trust.adopt_next = Some(real);
    }
    let Some(inherit) = st.trust.inherit.take() else { return false };
    if !ok || inherit.label != label || !still_trusted(st, &inherit.from) {
        return false;
    }
    let open = then_open.filter(|p| inherit.paths.iter().any(|q| same_spelling(q, p))).map(Path::to_path_buf);
    let took = open.is_some();
    fx.push(Effect::Trust(TrustEffect::Inherit { kind: inherit.kind, from: inherit.from, paths: inherit.paths, open }));
    took
}

/// Whether the repository at `from` (its real path) is still trusted: Trusted, and not changed
/// since when it is the active one.
fn still_trusted(st: &AppState, from: &Path) -> bool {
    match st.snap.as_ref().filter(|s| same_spelling(&s.real_workdir, from)) {
        Some(snap) => standing(&st.settings, snap) == Standing::Trusted,
        None => st.settings.saved_trust(from).is_some_and(|t| t.is_trusted()),
    }
}

/// The runner read the repositories a job made from `from`: each one that is a repository and
/// has no decision yet is trusted on its own print, a worktree only while everything it runs is
/// in the source's print. Then `open` opens.
pub(super) fn inherited(
    st: &mut AppState,
    kind: InheritKind,
    from: &Path,
    repos: Vec<InheritedRepo>,
    open: Option<PathBuf>,
    fx: &mut Vec<Effect>,
) {
    let source = st.settings.saved_trust(from).filter(|_| still_trusted(st, from));
    let mut names = Vec::new();
    for repo in repos {
        let (Some(source), Some(print)) = (source.as_ref(), repo.print) else { continue };
        if st.saved_trust_for(&repo.path).is_some() || st.settings.saved_trust(&repo.real).is_some() {
            continue;
        }
        if kind == InheritKind::Worktree && !source.print.as_ref().is_none_or(|p| p.covers(&print)) {
            tracing::debug!("not passing trust to {}: it runs more than its repository", repo.real.display());
            continue;
        }
        record_trusted(&mut st.settings, &repo.real, print);
        names.push(folder_name(&repo.real));
    }
    if !names.is_empty() {
        fx.push(super::prefs::save_settings(st));
        st.log_quiet(true, format!("Trusted {}: made from {}, which you trust.", names.join(", "), folder_name(from)));
    }
    if let Some(p) = open {
        super::tabs::open(st, p, fx);
    }
}

fn folder_name(p: &Path) -> String {
    p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned())
}
