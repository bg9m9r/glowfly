//! The runner's trust re-check, from update's side. Trust is judged against the scan of the last
//! full load, so work that passes the gate (a job, a released held job, the external or built-in
//! terminal, a conflict check or merge preview) is pushed with a `Seal` (`push`): what the
//! runner must find just before it starts (effect/trust_check.rs, runner/trust.rs). Before
//! update returns, each sealed effect is wrapped with its check (`wrap`, `Effect::Checked`), and
//! so is every read that lets git run in a repository (`read_in`: a status read by git, a
//! signature check, a diff or blame through git's filters). When the check finds the
//! repository no longer covered (a hook edited while the prompt was open, a pull that brings
//! other hooks into a hooks folder that follows the working tree), the work comes back here
//! (`stale`): the active tab takes the scan just made, and the work waits for the trust prompt
//! again, which names what changed.

use crate::effect::{Checked, Effect, LfsEffect, Seal, Sealed, SparseEffect, TrustCheck, WaitingKind};
use crate::msg::Stale;
use crate::state::AppState;
use crate::state::Modal;
use crate::state::tabs::is_repo_at;
use crate::state::terminal::SessionStatus;
use crate::state::trust::{TrustPrompt, WaitingFor, saved_for};
use crate::tools::ToolEffect;
use glowfly_core::paths::same_spelling;
use glowfly_core::trust::TrustScan;
use std::path::Path;

/// Pushes `effect`, which runs in `cwd`, with the re-check the runner makes just before it
/// starts; `sealed` says how to hold it again.
pub(super) fn push(st: &mut AppState, effect: Effect, sealed: Sealed, cwd: &Path, fx: &mut Vec<Effect>) {
    push_accepting(st, effect, sealed, cwd, false, fx);
}

/// `push`; `accept_unchecked`: the user chose Trust and run on a prompt that said what it
/// brings in couldn't be compared.
pub(super) fn push_accepting(st: &mut AppState, effect: Effect, sealed: Sealed, cwd: &Path, accept_unchecked: bool, fx: &mut Vec<Effect>) {
    let check = check_for(st, sealed, cwd, accept_unchecked);
    st.trust.seals.push(Seal { at: fx.len(), name: effect.name(), check });
    fx.push(effect);
}

/// The check for an effect in `cwd`: the decision of the repository there as it stands now.
fn check_for(st: &AppState, sealed: Sealed, cwd: &Path, accept_unchecked: bool) -> TrustCheck {
    let parked = || st.tabs.iter().find_map(|t| t.parked.as_ref()?.snap.as_ref().filter(|s| t.is(cwd) || is_repo_at(s, cwd)));
    let snap = st.snap.as_ref().filter(|s| is_repo_at(s, cwd)).or_else(parked);
    let (real, saved) = match snap {
        Some(s) => (s.real_workdir.clone(), saved_for(&st.settings, s)),
        None => {
            let real = st.tabs.iter().filter(|t| t.is(cwd)).find_map(|t| t.real.clone()).unwrap_or_else(|| cwd.to_path_buf());
            (real, st.saved_trust_for(cwd))
        }
    };
    TrustCheck { workdir: cwd.to_path_buf(), real, saved, sealed, accept_unchecked }
}

/// Wraps the effects one update returns with their checks: each sealed one (`st.trust.seals`
/// from `mark` on) by its place, and every read that lets git run in a repository (`read_in`). A seal whose
/// place now holds another effect (one was put before it) checks every effect of its name,
/// so none of them runs unchecked. A seal whose effect is gone (dropped by update) is dropped.
pub(crate) fn wrap(st: &mut AppState, fx: &mut [Effect], mark: usize) {
    let mut checks: Vec<Option<TrustCheck>> = fx.iter().map(|_| None).collect();
    let mark = mark.min(st.trust.seals.len());
    for seal in st.trust.seals.split_off(mark) {
        let same = |e: &Effect| e.name() == seal.name;
        if fx.get(seal.at).is_some_and(same) && checks[seal.at].is_none() {
            checks[seal.at] = Some(seal.check);
            continue;
        }
        tracing::debug!("the checked {} moved before it ran: every {} is checked", seal.name, seal.name);
        let mut any = false;
        for (c, e) in checks.iter_mut().zip(fx.iter()) {
            if c.is_none() && same(e) {
                *c = Some(seal.check.clone());
                any = true;
            }
        }
        if !any {
            tracing::debug!("the checked {} was dropped before it ran", seal.name);
        }
    }
    for (i, e) in fx.iter_mut().enumerate() {
        let read = read_in(e).map(Path::to_path_buf);
        let check = checks[i].take().or_else(|| read.map(|w| check_for(st, Sealed::Read, &w, false)));
        if let Some(check) = check {
            let effect = std::mem::replace(e, Effect::FetchAvatars(Vec::new()));
            *e = Effect::Checked(Box::new(Checked { effect, check }));
        }
    }
}

/// The repository a read lets git run in (its fsmonitor and filters, a signing program, a diff
/// or merge tool, git LFS): the runner checks it first (`Sealed::Read`).
fn read_in(e: &Effect) -> Option<&Path> {
    let w = match e {
        Effect::LoadStatus { workdir, git_status: true, .. } | Effect::LoadIndexStatus { workdir, git_status: true, .. } => workdir,
        Effect::VerifySignature { workdir, .. } => workdir,
        Effect::Lfs(LfsEffect::Load { workdir, .. }) => workdir,
        Effect::Sparse(SparseEffect::Load { workdir, run_git: true, .. }) => workdir,
        Effect::Tool(ToolEffect::OpenDiff(r)) if r.filters => &r.workdir,
        Effect::Tool(ToolEffect::RunMerge(r)) if r.filters => &r.workdir,
        Effect::Tool(ToolEffect::OpenGitDiff(r)) => &r.workdir,
        Effect::Tool(ToolEffect::RunGitMerge(r)) => &r.workdir,
        Effect::LoadFileText(r) if r.smudge => &r.workdir,
        Effect::LoadBlame(r) => &r.workdir,
        Effect::RangeDiff(j) => &j.workdir,
        Effect::CheckConflicts(j) => &j.workdir,
        _ => return None,
    };
    Some(w.as_path())
}

/// The runner didn't start checked work (`TrustMsg::Stale`): it waits for the trust prompt
/// again, made on what the runner just found.
pub(super) fn stale(st: &mut AppState, s: Stale) {
    let Stale { check, effect, now, incoming, unchecked } = s;
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
            let prompt = TrustPrompt {
                name,
                workdir,
                real: check.real,
                scan: now,
                refused: false,
                held: None,
                gained,
                terminal: false,
                incoming,
                unchecked: unchecked.map(Box::new),
            };
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
        // The runner reads without git, or fails the read, itself; the next load asks.
        Sealed::Read => {}
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
