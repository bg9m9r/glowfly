//! Repository trust's work in the runner: reading the repositories Glowfly just made from a
//! trusted one, so they can be trusted on the same terms (`Effect::Trust`), and the re-check
//! just before a trust-gated job, shell or read starts (`verify`): the repository is scanned
//! again with libgit2 (settings, hooks read whole, the commands file, where hooks follow the
//! working tree) and compared with the decision it was sent under, and for a checkout, merge,
//! pull or rebase the incoming commit's version of the hooks that follow the working tree is
//! read from its objects and compared too. Nothing the repository names runs for the check; a
//! pull's fetch runs first (under the settings just checked) to learn the commit it merges.
//! A scan or comparison that fails holds the work back: it never runs unchecked.

use super::execute::{Cx, Execute, execute_with};
use crate::effect::{Effect, TrustCheck, TrustEffect};
use crate::msg::{InheritedRepo, Msg, Stale, TrustMsg};
use glowfly_core::trust::{self, TrustScan, follow, incoming};

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

/// What the re-check found.
#[derive(Debug)]
pub(super) enum Verdict {
    /// Still covered: the work starts.
    Pass,
    /// Not covered any more: the work is held again with what was found (the repository's own
    /// scan, and the incoming commit's when that is what isn't covered).
    Stale(Box<(TrustScan, Option<TrustScan>)>),
    /// The check itself failed (the repository can't be opened, an object can't be read, a
    /// pull's fetch failed): the work doesn't run, for this reason.
    Failed(String),
}

/// Scans the repository `c` names again and compares it with `c.saved`, then the commits
/// its effect brings in (see the module docs).
pub(super) fn verify(c: &TrustCheck) -> Verdict {
    let repo = match git2::Repository::open(&c.workdir) {
        Ok(r) => r,
        Err(e) => return Verdict::Failed(format!("Glowfly couldn't check what {} runs: {}", c.workdir.display(), e.message())),
    };
    let now = trust::scan_now(&repo);
    if !trust::allows(c.saved.as_ref(), &now) {
        return Verdict::Stale(Box::new((now, None)));
    }
    if now.follows.is_empty() {
        return Verdict::Pass;
    }
    for target in &c.incoming {
        if let incoming::Target::Fetched { fetch, .. } = target {
            let (ok, output) = super::git_ops::run_git(&c.workdir, std::slice::from_ref(fetch), None, c.prompts);
            if !ok {
                return Verdict::Failed(output);
            }
        }
        // A revision that names no commit: git refuses the command too.
        let Some(oid) = incoming::resolve(&repo, target) else { continue };
        match follow::incoming(&repo, &now.follows, oid) {
            Ok(inc) if incoming::allows(c.saved.as_ref(), &inc) => {}
            Ok(inc) => return Verdict::Stale(Box::new((now, Some(inc)))),
            Err(e) => return Verdict::Failed(format!("Glowfly couldn't read the hooks commit {oid} brings in: {}", e.message())),
        }
    }
    Verdict::Pass
}

/// Runs `effect` if the re-check `c` passes; otherwise sends it back held (`TrustMsg::Stale`)
/// or failed (`fail`, the failure its sender waits for).
pub(super) fn checked(c: TrustCheck, effect: Effect, fail: impl FnOnce(String) -> Msg, cx: &Cx<'_>) -> Option<Msg> {
    match verify(&c) {
        Verdict::Pass => execute_with(effect, cx),
        Verdict::Stale(found) => {
            let (now, incoming) = *found;
            tracing::debug!("{} wasn't started: {} changed since it was trusted", effect.name(), c.workdir.display());
            Some(Msg::Trust(TrustMsg::Stale(Box::new(Stale { check: c, effect, now, incoming }))))
        }
        Verdict::Failed(text) => Some(fail(text)),
    }
}

/// A shell's re-check, in its session thread: None starts it; otherwise the message to send
/// instead (the shell `spawn` makes, held again) or why it can't start.
pub(super) fn before_shell(check: Option<TrustCheck>, spawn: impl FnOnce() -> Effect) -> Option<Result<Msg, String>> {
    let c = check?;
    match verify(&c) {
        Verdict::Pass => None,
        Verdict::Stale(found) => {
            Some(Ok(Msg::Trust(TrustMsg::Stale(Box::new(Stale { check: c, effect: spawn(), now: found.0, incoming: found.1 })))))
        }
        Verdict::Failed(text) => Some(Err(text)),
    }
}

/// `execute` with the trust re-check `c` first, for tests.
#[cfg(test)]
pub fn execute_checked(c: TrustCheck, effect: Effect) -> Option<Msg> {
    let cx = Cx { cancelled: &|| false, graph_cache: &glowfly_core::repo::GraphCache::default(), stop: None };
    let fail = super::guard::Recovery::of(&effect, effect.name());
    checked(c, effect, |text| fail.failed(text), &cx)
}

/// Whether the re-check `c` passes, for tests.
#[cfg(test)]
pub fn trust_passes(c: &TrustCheck) -> bool {
    matches!(verify(c), Verdict::Pass)
}

#[cfg(test)]
#[path = "trust_tests.rs"]
mod tests;
