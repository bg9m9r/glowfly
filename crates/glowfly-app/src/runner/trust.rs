//! Repository trust's work in the runner: reading the repositories Glowfly just made from a
//! trusted one, so they can be trusted on the same terms (`Effect::Trust`), and the re-check
//! just before trust-gated work starts (`Effect::Checked`, `verify`): the repository is scanned
//! again with libgit2 (settings, hooks read whole, the commands file, where hooks follow the
//! working tree) and compared with the decision it was sent under, and where hooks follow the
//! working tree, what the work brings in is compared too (runner/trust_gate.rs): the commits an
//! effect names before it starts, and for a job of git commands each command's just before it
//! runs, a pull pinned to the commits its fetch brought. Nothing the repository names runs for
//! the check; a pull's fetch runs (under the settings just checked) to learn what it merges.
//! A scan or comparison that fails holds the work back: it never runs unchecked.

use super::execute::{Cx, Execute, execute_with};
use super::trust_gate::{self, Gate, Halt};
use crate::effect::{Accepted, Checked, Effect, Sealed, StashEffect, TrustCheck, TrustEffect};
use crate::msg::{InheritedRepo, Msg, Stale, TrustMsg};
use glowfly_core::trust::incoming::Unchecked;
use glowfly_core::trust::{self, TrustScan};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

pub(super) fn run(e: TrustEffect) -> Msg {
    match e {
        TrustEffect::Inherit { kind, from, paths, open } => {
            let repos = paths
                .into_iter()
                .map(|path| {
                    let real = glowfly_core::paths::resolve(&path);
                    let print = glowfly_core::trust::scan_at(&real).ok().map(|s| s.print());
                    InheritedRepo { path, real, print }
                })
                .collect();
            Msg::Trust(TrustMsg::Inherited { kind, from, repos, open })
        }
    }
}

impl Execute for TrustEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(run(self))
    }
}

impl Execute for Box<Checked> {
    fn execute(self, cx: &Cx<'_>) -> Option<Msg> {
        let Checked { effect, check } = *self;
        let fail = super::guard::Recovery::of(&effect, effect.name());
        checked(check, effect, |text| fail.failed(text), cx)
    }
}

/// What the re-check found.
#[derive(Debug)]
pub(super) enum Verdict {
    /// Still covered: the work starts. With the gate its git commands go through, one by one,
    /// where hooks follow the working tree.
    Pass(Option<Gate>),
    /// Not covered any more: the work is held again with what was found (the repository's own
    /// scan, and the incoming commit's when that is what isn't covered).
    Stale(Box<(TrustScan, Option<TrustScan>)>),
    /// What it brings in couldn't be compared: held again, saying why.
    Unchecked(Box<(TrustScan, Unchecked)>),
    /// The check itself failed (the repository can't be opened): the work doesn't run, for
    /// this reason.
    Failed(String),
}

/// The scans reused for reads while their stamp (`trust::stamp`) is unchanged, by folder.
static SCANS: Mutex<Option<HashMap<PathBuf, (u64, TrustScan)>>> = Mutex::new(None);

/// The repository's scan now: for a read (`Sealed::Read`, frequent and quick) the last one made
/// in its folder while nothing it read has changed size or time since; for anything else made
/// again.
fn scan(c: &TrustCheck, repo: &git2::Repository) -> TrustScan {
    if c.sealed != Sealed::Read {
        return trust::scan_now(repo);
    }
    let mut cache = SCANS.lock().unwrap_or_else(|e| e.into_inner());
    let scans = cache.get_or_insert_with(HashMap::new);
    if let Some((stamp, scan)) = scans.get(&c.workdir)
        && *stamp == trust::stamp(repo, scan)
    {
        return scan.clone();
    }
    let now = trust::scan_now(repo);
    scans.insert(c.workdir.clone(), (trust::stamp(repo, &now), now.clone()));
    now
}

/// Scans the repository `c` names again and compares it with `c.saved`, then what `effect`
/// brings in (see the module docs).
pub(super) fn verify(c: &TrustCheck, effect: &Effect) -> Verdict {
    let repo = match git2::Repository::open(&c.workdir) {
        Ok(r) => r,
        Err(e) => return Verdict::Failed(format!("Glowfly couldn't check what {} runs: {}", c.workdir.display(), e.message())),
    };
    let now = scan(c, &repo);
    if !trust::allows(c.saved.as_ref(), &now) {
        return Verdict::Stale(Box::new((now, None)));
    }
    if now.follows.is_empty() {
        return Verdict::Pass(None);
    }
    let accept = c.accepted.as_ref().filter(|a| a.at.is_none()).map(|a| &a.reason);
    match trust_gate::compare(&c.workdir, &now.follows, c.saved.as_ref(), &trust_gate::targets(effect), accept) {
        Ok(()) => {
            let commands =
                matches!(effect, Effect::RunGit { .. } | Effect::UndoRedo { .. } | Effect::Stash(StashEffect::AbortKeeping { .. }));
            let gate = || Gate::new(c.workdir.clone(), now.follows.clone(), c.saved.clone(), c.accepted.clone(), c.lang);
            Verdict::Pass(commands.then(gate))
        }
        Err(h) => halted(now, h),
    }
}

pub(super) fn halted(now: TrustScan, h: Halt) -> Verdict {
    match h {
        Halt::Held(found) => Verdict::Stale(Box::new((now, Some(*found)))),
        Halt::Unchecked(u) => Verdict::Unchecked(Box::new((now, u))),
        Halt::Changed(changed) => Verdict::Stale(Box::new((*changed, None))),
    }
}

/// The work held again (`TrustMsg::Stale`), with what the check found.
pub(super) fn stale(check: TrustCheck, effect: Effect, now: TrustScan, incoming: Option<TrustScan>, unchecked: Option<Accepted>) -> Msg {
    Msg::Trust(TrustMsg::Stale(Box::new(Stale { check, effect, now, incoming, unchecked })))
}

/// Runs `effect` if the re-check `c` passes; otherwise sends it back held (`TrustMsg::Stale`)
/// or failed (`fail`, the failure its sender waits for). A read that fails it reads without
/// git where it can (a status read), else fails.
pub(super) fn checked(c: TrustCheck, effect: Effect, fail: impl FnOnce(String) -> Msg, cx: &Cx<'_>) -> Option<Msg> {
    let verdict = verify(&c, &effect);
    if !matches!(verdict, Verdict::Pass(_)) {
        tracing::debug!("{} wasn't started: {} changed since it was trusted", effect.name(), c.workdir.display());
    }
    match verdict {
        Verdict::Pass(gate) => {
            let cx = Cx { gate: gate.as_ref().map(|g| (g, &c)), ..*cx };
            execute_with(effect, &cx)
        }
        Verdict::Failed(text) => Some(fail(text)),
        _ if c.sealed == Sealed::Read => match without_git(effect) {
            Some(read) => execute_with(read, cx),
            None => Some(fail(format!("Glowfly didn't run git in {}: it changed since it was trusted.", c.workdir.display()))),
        },
        Verdict::Stale(found) => {
            let (now, incoming) = *found;
            Some(stale(c, effect, now, incoming, None))
        }
        Verdict::Unchecked(found) => {
            let (now, u) = *found;
            Some(stale(c, effect, now, None, Some(Accepted { at: None, reason: u })))
        }
    }
}

/// The read `effect` without git, when it has such a form (a status read by libgit2).
fn without_git(effect: Effect) -> Option<Effect> {
    match effect {
        Effect::LoadStatus { workdir, generation, submodules, open, paths, .. } => {
            Some(Effect::LoadStatus { workdir, generation, submodules, open, git_status: false, paths })
        }
        Effect::LoadIndexStatus { workdir, generation, submodules, open, read, .. } => {
            Some(Effect::LoadIndexStatus { workdir, generation, submodules, open, git_status: false, read })
        }
        _ => None,
    }
}

/// A shell's re-check, in its session thread: None starts it; otherwise the message to send
/// instead (the shell `spawn` makes, held again) or why it can't start.
pub(super) fn before_shell(check: Option<TrustCheck>, spawn: impl FnOnce() -> Effect) -> Option<Result<Msg, String>> {
    let c = check?;
    let effect = spawn();
    match verify(&c, &effect) {
        Verdict::Pass(_) => None,
        Verdict::Stale(found) => Some(Ok(stale(c, effect, found.0, found.1, None))),
        Verdict::Unchecked(found) => Some(Ok(stale(c, effect, found.0, None, Some(Accepted { at: None, reason: found.1 })))),
        Verdict::Failed(text) => Some(Err(text)),
    }
}

/// `execute` with the trust re-check `c` first, for tests.
#[cfg(test)]
pub fn execute_checked(c: TrustCheck, effect: Effect) -> Option<Msg> {
    let cx = Cx { cancelled: &|| false, graph_cache: &glowfly_core::repo::GraphCache::default(), stop: None, gate: None, send: &|_| {} };
    Execute::execute(Box::new(Checked { effect, check: c }), &cx)
}

#[cfg(test)]
#[path = "trust_tests.rs"]
mod tests;
