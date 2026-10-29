//! Undo and redo as the runner carries them out: the plan for an action as the repository is
//! now, its git commands run through the trust gate (#273) when there is one, and what it means
//! for the history. "Stash changes and undo" wraps this (`undo_stash.rs`).

use super::git_ops::{Gated, run_git, run_git_gated};
use crate::effect::{UndoOpts, UndoResult};
use crate::msg::{JobMsg, Msg};
use crate::state::files::TRASH;
use glowfly_core::error::Error;
use glowfly_core::undo::{self, Action, Plan, Refusal};
use std::path::Path;

/// Checks that `action` still applies, then runs the git commands that undo or redo it. With
/// `opts.stash_first`, local changes are stashed first (`undo_stash::stash_and_undo`). Where
/// hooks follow the working tree, every git command (the stash's too) goes through `gate`, and
/// a command it holds fails the undo.
pub(super) fn undo_redo(cwd: &Path, action: Action, redo: bool, opts: UndoOpts, gate: Option<&super::trust_gate::Gate>) -> Msg {
    let git = UndoGit { gate };
    let (ok, output, undo) = if opts.stash_first {
        super::undo_stash::stash_and_undo(cwd, action, redo, opts, git)
    } else {
        let ran = undo_run(cwd, action, redo, opts, git);
        (ran.ok, ran.output, ran.undo)
    };
    Msg::Job(JobMsg::JobDone { ok, output, undo: Some(undo) })
}

/// How undo runs its git commands: through the trust gate (#273) when there is one.
#[derive(Clone, Copy)]
pub(super) struct UndoGit<'a> {
    pub gate: Option<&'a super::trust_gate::Gate>,
}

impl UndoGit<'_> {
    /// Runs `cmds` in order, stopping at the first failure: whether all worked and the output;
    /// `held` when the gate stopped one, with whether a command before it could have changed
    /// anything.
    pub fn run(&self, cwd: &Path, cmds: &[Vec<String>]) -> GitRun {
        match self.gate {
            Some(g) => match run_git_gated(cwd, cmds, None, true, g) {
                Gated::Done(ok, output) => GitRun { ok, output, held: None },
                Gated::Halted { halt, output, moved, .. } => {
                    let output = format!("{output}\n{}", halt.describe(g.lang)).trim().to_owned();
                    GitRun { ok: false, output, held: Some(moved) }
                }
            },
            None => {
                let (ok, output) = run_git(cwd, cmds, None, true);
                GitRun { ok, output, held: None }
            }
        }
    }
}

/// What `UndoGit::run` did.
pub(super) struct GitRun {
    pub ok: bool,
    pub output: String,
    /// The gate held a command: `Some(true)` when one before it could have changed anything.
    pub held: Option<bool>,
}

/// How an undo or redo went: whether it worked, what it printed, what it means for the history,
/// and whether it may have changed anything (`false`: refused, not planned at all, or held by the
/// gate before any command ran).
pub(super) struct UndoRun {
    pub ok: bool,
    pub output: String,
    pub undo: UndoResult,
    pub acted: bool,
}

/// Plans the undo (or redo) of `action` as the repository is now and carries it out.
pub(super) fn undo_run(cwd: &Path, action: Action, redo: bool, opts: UndoOpts, git: UndoGit) -> UndoRun {
    let plan = undo::plan_with(cwd, &action, redo, undo::Options { delete_created: opts.delete_created });
    let planned = matches!(plan, Ok(ref p) if !matches!(p, Plan::Refuse(_)));
    let mut held = None;
    let (ok, output, undo) = carry_out(cwd, action, redo, opts, plan, git, &mut held);
    UndoRun { ok, output, undo, acted: planned && held != Some(false) }
}

fn carry_out(
    cwd: &Path,
    action: Action,
    redo: bool,
    opts: UndoOpts,
    plan: glowfly_core::error::Result<Plan>,
    git: UndoGit,
    held: &mut Option<bool>,
) -> (bool, String, UndoResult) {
    let done = |output: String, before: Action, after: Action| {
        (true, output, UndoResult::Done { redo, before, after: Box::new(after), stash: None })
    };
    let failed = |output: String| (false, output, UndoResult::Failed { redo, stash: None });
    match plan {
        Err(e) => failed(e.to_string()),
        Ok(Plan::Refuse(r)) => {
            let offer_stash = matches!(r, Refusal::Dirty(_)) && action.offers_stash(redo) && !opts.stash_first;
            (false, r.message().into(), UndoResult::Refused { redo, stale: r.is_stale(), offer_stash, stash: None })
        }
        Ok(Plan::Write { path, content }) => match undo::write_saved(cwd, &path, content) {
            Ok(()) => done(action.wrote(redo), action.clone(), action),
            Err(e) => failed(format!("Could not write {path}: {e}")),
        },
        Ok(Plan::Run(cmds)) => {
            let GitRun { ok, output, held: h } = git.run(cwd, &cmds);
            *held = h;
            if !ok {
                return failed(output);
            }
            let after = if redo { undo::redone(cwd, action.clone()).unwrap_or_else(|_| action.clone()) } else { action.clone() };
            done(output, action, after)
        }
        Ok(Plan::RunThenWrite { cmds, path, content }) => {
            let GitRun { ok, output, held: h } = git.run(cwd, &cmds);
            *held = h;
            if !ok {
                return failed(output);
            }
            match undo::write_saved(cwd, &path, content) {
                Ok(()) => done(format!("{output}\n{}", action.wrote(redo)), action.clone(), action),
                Err(e) => {
                    let kept = content.map(|s| format!(". Its contents are kept as blob {} (git cat-file -p {})", s.blob, s.blob));
                    failed(format!("{output}\nCould not write {path}: {e}{}", kept.unwrap_or_default()))
                }
            }
        }
        Ok(Plan::PutBack { path, content, staged, via }) => {
            match undo::put_back(cwd, &path, content, staged.as_ref(), via, super::files::TRASHERS) {
                Ok(how) => done(how.wrote(&path, staged.is_some(), via), action.clone(), action),
                Err(e) => failed(format!("Could not put back {path}: {e}")),
            }
        }
        Ok(Plan::Restage { path, staged }) => match undo::restage(cwd, &path, &staged) {
            Ok(()) => done(action.wrote(redo), action.clone(), action),
            Err(e) => failed(format!("Could not stage {path} again: {e}")),
        },
        Ok(Plan::UnstageAgain { path }) => match undo::unstage_again(cwd, &path) {
            Ok(()) => done(action.wrote(redo), action.clone(), action),
            Err(e) => failed(format!("Could not unstage {path}: {e}")),
        },
        Ok(Plan::DeleteAgain { path, via, staged }) => match undo::delete_again(cwd, &path, via, staged, super::files::TRASHERS) {
            Ok(_) => done(action.wrote(redo), action.clone(), action),
            Err(Error::TrashRefused(r)) => failed(format!("The {TRASH} couldn't take {path}: {}", r.detail)),
            Err(e) => failed(format!("Could not delete {path}: {e}")),
        },
        Ok(Plan::RestoreAbort) => match undo::restore_abort(cwd, &action) {
            Ok(output) => done(output, action.clone(), action),
            Err(e) => failed(format!("Could not put the {} back: {e}", action.describe())),
        },
        Ok(Plan::PutConflicts) => match undo::put_conflicts(cwd, &action, redo) {
            Ok(output) => done(output, action.clone(), action),
            Err(e) => failed(format!("Could not put back the files of the {}: {e}", action.describe())),
        },
        Ok(Plan::AbortAgain) => match super::stashes::abort_keeping(cwd, None) {
            (true, output, Some(again)) => done(output, action, again),
            (_, output, _) => failed(output),
        },
    }
}
