//! The runner's trust re-check, from update's side. Trust is judged against the scan of the last
//! full load, so work that passes the gate (a job, a released held job, the external or built-in
//! terminal, a conflict check or merge preview) is pushed with a `Seal` (`push`): what the
//! runner must find just before it starts (effect/trust_check.rs, runner/trust.rs). The shell
//! pairs the seals with the effects before running them (`pair`). When the check finds the
//! repository no longer covered (a hook edited while the prompt was open, a pull that brings
//! other hooks into a hooks folder that follows the working tree), the work comes back here
//! (`stale`): the active tab takes the scan just made, and the work waits for the trust prompt
//! again, which names what changed.

use crate::effect::{Effect, RebaseEffect, Seal, Sealed, TrustCheck, WaitingKind};
use crate::msg::Stale;
use crate::state::AppState;
use crate::state::Modal;
use crate::state::tabs::is_repo_at;
use crate::state::terminal::SessionStatus;
use crate::state::trust::{TrustPrompt, WaitingFor, saved_for};
use glowfly_core::paths::same_spelling;
use glowfly_core::rebase::Upstream;
use glowfly_core::trust::TrustScan;
use glowfly_core::trust::incoming::{Target, targets};
use std::path::Path;

/// Pushes `effect`, which runs in `cwd`, with the re-check the runner makes just before it
/// starts; `sealed` says how to hold it again.
pub(super) fn push(st: &mut AppState, effect: Effect, sealed: Sealed, cwd: &Path, fx: &mut Vec<Effect>) {
    let check = check_for(st, &effect, sealed, cwd);
    st.trust.seals.push(Seal { at: fx.len(), name: effect.name(), check });
    fx.push(effect);
}

/// The check for `effect` in `cwd`: the decision of the repository there as it stands now.
fn check_for(st: &AppState, effect: &Effect, sealed: Sealed, cwd: &Path) -> TrustCheck {
    let parked = || st.tabs.iter().find_map(|t| t.parked.as_ref()?.snap.as_ref().filter(|s| t.is(cwd) || is_repo_at(s, cwd)));
    let snap = st.snap.as_ref().filter(|s| is_repo_at(s, cwd)).or_else(parked);
    let (real, saved) = match snap {
        Some(s) => (s.real_workdir.clone(), saved_for(&st.settings, s)),
        None => {
            let real = st.tabs.iter().filter(|t| t.is(cwd)).find_map(|t| t.real.clone()).unwrap_or_else(|| cwd.to_path_buf());
            (real, st.saved_trust_for(cwd))
        }
    };
    let (incoming, prompts) = match effect {
        Effect::RunGit { cmds, prompts, .. } => (cmds.iter().flat_map(|c| targets(c)).collect(), *prompts),
        Effect::Pick(j) => (j.cmds.iter().flat_map(|c| targets(c)).collect(), j.prompts),
        Effect::Rebase(RebaseEffect::Start { request, .. }) => match request.upstream {
            Upstream::Commit(oid) => (vec![Target::Commit(oid)], true),
            Upstream::Root => (Vec::new(), true),
        },
        _ => (Vec::new(), true),
    };
    TrustCheck { workdir: cwd.to_path_buf(), real, saved, incoming, prompts, sealed }
}

/// The check for each of `fx` (the effects one update returned), from the seals recorded since
/// the last call: a seal goes with the effect at its place when that one has its name, else
/// with the last effect that has. A seal whose effect is gone (dropped by update) is dropped.
pub fn pair(st: &mut AppState, fx: &[Effect]) -> Vec<Option<TrustCheck>> {
    let mut out: Vec<Option<TrustCheck>> = fx.iter().map(|_| None).collect();
    for seal in std::mem::take(&mut st.trust.seals) {
        let free: Vec<bool> = out.iter().zip(fx).map(|(c, e)| c.is_none() && e.name() == seal.name).collect();
        let at = if free.get(seal.at).copied().unwrap_or(false) { Some(seal.at) } else { free.iter().rposition(|f| *f) };
        match at {
            Some(i) => out[i] = Some(seal.check),
            None => tracing::debug!("the checked {} was dropped before it ran", seal.name),
        }
    }
    out
}

/// The runner didn't start checked work (`TrustMsg::Stale`): it waits for the trust prompt
/// again, made on what the runner just found.
pub(super) fn stale(st: &mut AppState, s: Stale) {
    let Stale { check, effect, now, incoming } = s;
    let active = st.snap.as_ref().is_some_and(|s| same_spelling(&s.real_workdir, &check.real));
    let now = if active { adopt(st, now) } else { now };
    let print = check.saved.as_ref().and_then(|t| t.print.as_ref());
    let t = st.settings.lang();
    match check.sealed {
        Sealed::Job { label } => {
            let Some(job) = st.job.take_if(|j| j.label == label) else { return };
            st.job_stop = None;
            // A quiet job is dropped by `hold`, which says so in the quiet log.
            if !job.quiet {
                st.log(false, t.trf("trust.stale.held", &[("label", &label)]));
            }
            let (name, workdir) = match st.snap.as_ref().filter(|_| active) {
                Some(s) => (s.name.clone(), s.workdir.clone()),
                None => (folder_name(&check.workdir), check.workdir.clone()),
            };
            let gained = print.filter(|_| incoming.is_none()).map(|p| now.gained_since(p));
            let incoming = incoming.map(|i| print.map_or_else(|| i.clone(), |p| i.gained_since(p)));
            let prompt =
                TrustPrompt { name, workdir, real: check.real, scan: now, refused: false, held: None, gained, terminal: false, incoming };
            super::trust::hold(st, job, effect, prompt, !active);
        }
        Sealed::Waiting { kind, label } => {
            if !active {
                return;
            }
            let what = match (kind, effect) {
                (WaitingKind::Preview, Effect::OpPreview(job)) => {
                    if matches!(st.modal, Some(Modal::OpPreview(_))) {
                        st.modal = None;
                    }
                    WaitingFor::Preview(job)
                }
                (WaitingKind::Terminal, e) => WaitingFor::Terminal(e),
                (_, e) => WaitingFor::Read(e),
            };
            st.log(
                false,
                t.trf("trust.stale.held", &[("label", &label.clone().unwrap_or_else(|| t.tr("trust.stale.terminal").to_owned()))]),
            );
            super::trust::hold_waiting(st, what, label);
        }
        Sealed::Shell { id } => {
            if let Some(s) = st.terminal.sessions.iter_mut().find(|s| s.id == id && s.status == SessionStatus::Starting) {
                s.status = SessionStatus::Held;
            }
            if active {
                st.log(false, t.trf("trust.stale.held", &[("label", &t.tr("trust.stale.terminal"))]));
                super::trust::ask(st, &check.real, true);
            }
        }
    }
}

/// The active tab takes the runner's scan of its own settings and hooks (keeping the
/// submodules its last full load found), so its standing, the Untrusted chip and the prompt
/// follow what is on disk now; the next full load reads it all again. Returns that scan.
fn adopt(st: &mut AppState, mut now: TrustScan) -> TrustScan {
    if let Some(snap) = st.snap.as_mut() {
        now.submodules = snap.trust_scan.submodules.clone();
        snap.trust_print = now.print();
        snap.trust_scan = now.clone();
    }
    now
}

fn folder_name(p: &Path) -> String {
    p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned())
}
