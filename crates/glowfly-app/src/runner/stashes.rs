//! Stash list jobs (`Effect::Stash`): renaming an entry, putting an auto-stash back, reading
//! and writing the auto-stashes waiting to go back across restarts, reading the files a stash
//! changes for the question before applying it, and the abort that keeps an operation's work
//! (also run by redoing one).

use super::execute::{Cx, Execute};
use super::expect;
use crate::effect::{PutBackRequest, StashEffect, UndoResult};
use crate::msg::{AutoStashMsg, JobMsg, Msg, StashMsg};
use git2::Oid;
use glowfly_core::stash_ops::{self, PutBack};
use glowfly_core::undo::{self, UndoKind};
use std::path::Path;

pub(super) fn run(e: StashEffect) -> Option<Msg> {
    match e {
        StashEffect::Rename { cwd, index, oid, message } => Some(rename(&cwd, index, oid, &message)),
        StashEffect::PutBack(req) => Some(put_back(req)),
        StashEffect::ReadAutoStashes { cwd } => {
            let saved = stash_ops::read_saved(&cwd);
            (!saved.is_empty()).then(|| Msg::AutoStash(AutoStashMsg::SavedRead { workdir: cwd, saved }))
        }
        StashEffect::SaveAutoStashes { cwd, saved } => stash_ops::write_saved(&cwd, &saved)
            .err()
            .map(|e| Msg::Logged { ok: false, text: format!("Could not save the stashes waiting to go back: {e}") }),
        StashEffect::ReadPaths { cwd, ask } => {
            let paths = stash_ops::changed_paths(&cwd, ask.oid).map_err(|e| e.to_string());
            Some(Msg::Stash(StashMsg::PathsRead { ask, paths }))
        }
        StashEffect::AbortKeeping { cwd } => {
            let (ok, output, action) = abort_keeping(&cwd);
            Some(Msg::Job(JobMsg::JobDone { ok, output, undo: Some(UndoResult::Recorded(action)) }))
        }
    }
}

/// Keeps the work of the operation in progress in `cwd`, then aborts it: whether that worked,
/// the output, and the abort to record for Undo. When the work can't all be kept, nothing is
/// aborted; when an abort command fails, the output says where the work is kept.
pub(super) fn abort_keeping(cwd: &Path) -> (bool, String, Option<undo::Action>) {
    let saving = match undo::abort_saving(cwd) {
        Ok(s) => s,
        Err(e) => return (false, format!("Nothing was aborted: {e}."), None),
    };
    let (ok, output) = super::git_ops::run_git(cwd, &saving.cmds, None, true);
    if !ok {
        return (false, format!("{output}\nThe operation's work is kept in {}.", saving.kept), None);
    }
    (true, output, Some(saving.action))
}

impl Execute for StashEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        run(self)
    }
}

/// Gives `stash@{index}` the list text `message`, if that still is commit `oid`, and ends the
/// active job with the result. A shifted list is refused in the same words as apply, pop and drop.
fn rename(cwd: &Path, index: usize, oid: Oid, message: &str) -> Msg {
    if let Err(output) = expect::check(cwd, &format!("stash@{{{index}}}"), oid) {
        return Msg::Job(JobMsg::JobDone { ok: false, output, undo: None });
    }
    let r = stash_ops::rename(cwd, index, oid, message);
    Msg::Job(JobMsg::JobDone { ok: r.ok, output: r.output, undo: None })
}

/// Pops the auto-stash `req.oid` once the repository has nothing in progress, recording the pop
/// for undo as a plain `git stash pop stash@{n}` would be.
fn put_back(req: PutBackRequest) -> Msg {
    let PutBackRequest { cwd, oid } = req;
    let done = |outcome, output, undo| Msg::AutoStash(AutoStashMsg::PutBack { workdir: cwd.clone(), oid, outcome, output, undo });
    let n = match stash_ops::prepare_put_back(&cwd, oid) {
        Ok(n) => n,
        Err((outcome, output)) => return done(outcome, output, None),
    };
    let kind = UndoKind::StashApply { pop: true, rev: Some(format!("stash@{{{n}}}")) };
    let before = undo::before(&cwd, &kind).ok();
    // A pop onto local changes mixes them with the stash's, so it can't be undone.
    let irreversible = before.as_ref().is_some_and(undo::irreversible);
    let (outcome, output) = stash_ops::pop_put_back(&cwd, n, oid);
    let recorded = if matches!(outcome, PutBack::Restored | PutBack::RestoredUnstaged) && !irreversible {
        before.and_then(|b| undo::after(&cwd, kind, b).ok().flatten())
    } else {
        None
    };
    done(outcome, output, Some(Box::new(UndoResult::Recorded(recorded))))
}

#[cfg(test)]
mod tests {
    use super::super::guard::{Recovery, guarded};
    use crate::effect::{Effect, PutBackRequest, StashEffect};
    use crate::msg::{AutoStashMsg, Msg};
    use glowfly_core::stash_ops::PutBack;

    #[test]
    fn a_panicking_put_back_ends_its_job() {
        let cwd = glowfly_core::paths::temp_dir().join("put-back-panic");
        let effect = Effect::Stash(StashEffect::PutBack(PutBackRequest { cwd: cwd.clone(), oid: git2::Oid::zero() }));
        assert!(effect.scope().is_none(), "unscoped: the job ends whichever tab is active");
        let msg = guarded(Recovery::of(&effect, effect.name()), || panic!("boom")).expect("a failure result");
        let Msg::AutoStash(AutoStashMsg::PutBack { workdir, outcome: PutBack::Failed, output, undo: None, .. }) = &msg else {
            panic!("not a failed put-back")
        };
        assert_eq!(workdir, &cwd);
        assert!(output.contains("boom") && output.contains("put-back-stash"), "{output}");
    }
}
