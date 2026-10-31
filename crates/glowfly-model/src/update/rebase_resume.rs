//! The rebase banner's Continue and Skip commit: run as a rebase job whose result
//! (`RebaseMsg::RebaseFinished`) says where the rebase stopped next, and the headlines for
//! stopped rebases.

use super::ops::Started;
use super::rebase::refuse;
use crate::effect::{Effect, RebaseEffect};
use crate::state::AppState;
use crate::state::rebase::has_conflicts;
use git2::RepositoryState;
use glowfly_core::rebase::{self, RebaseProgress, Resume, StopKind};
use std::time::Instant;

/// After "<label> ": git kept an autostash it couldn't apply.
pub const STASH_KEPT: &str = "finished, but your stashed changes didn't apply cleanly. They are kept in the stash list.";

pub(super) fn resume(st: &mut AppState, how: Resume, now: Instant, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    use RepositoryState::*;
    if !matches!(snap.state, Rebase | RebaseInteractive | RebaseMerge | ApplyMailboxOrRebase) {
        refuse(st, rebase::NOT_REBASING.into(), now);
        return;
    }
    if how == Resume::Continue && has_conflicts(snap) {
        let why = st.settings.lang().tr("details.banner.resolve_first.tip").to_owned();
        refuse(st, why, now);
        return;
    }
    // A stopped restack goes on with its remaining steps.
    if super::stack::resume(st, how, fx) {
        return;
    }
    let Some(snap) = &st.snap else { return };
    let label = if snap.state == RebaseInteractive { "Interactive rebase" } else { "Rebase" };
    if super::ops::begin_job(st, label, false, move |cwd| Effect::Rebase(RebaseEffect::Resume { cwd, how }), fx) != Started::No {
        super::submodule_batch::job_started(st, label, true);
    }
}

/// The headline for a rebase that stopped, with the step when the count is known. `ok` is
/// git's exit status: a failed run that stopped at an edit or elsewhere means git refused to go
/// on (nothing staged after resolving, say), in its own words.
pub fn stopped_headline(label: &str, p: &RebaseProgress, ok: bool, output: &str) -> String {
    if !ok && matches!(p.stop.kind, StopKind::Edit | StopKind::Other) {
        return format!("Couldn't continue the rebase: {}", super::ops::last_line(output));
    }
    let at = p.stop.oid.map(|o| o.to_string()[..7].to_owned()).unwrap_or_else(|| "a commit".into());
    let step = if p.total > 0 { format!(" (step {} of {})", p.step, p.total) } else { String::new() };
    match p.stop.kind {
        StopKind::Edit => format!("{label} paused at {at}{step} so you can edit it. Use Continue when you're done."),
        StopKind::Conflict => format!("{label} stopped on a conflict at {at}{step}. Resolve it, then Continue, or Abort."),
        StopKind::Exec => format!("{label} stopped: rewording {at}{step} failed."),
        StopKind::Command => format!("{label} stopped: the command after {at}{step} failed."),
        StopKind::Break => format!("{label} paused at a break step after {at}{step}. Use Continue to go on."),
        StopKind::Other => format!("{label} stopped at {at}{step}. Continue, Skip commit or Abort."),
    }
}
