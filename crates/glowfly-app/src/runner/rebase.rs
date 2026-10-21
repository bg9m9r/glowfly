//! Interactive rebase effects: loading the list, preparing a graph edit, running a rebase with
//! Trellis as the sequence editor, resuming a stopped one (each recording what undo needs), and
//! reading and editing the remaining steps of a running one.

use super::err;
use crate::effect::{RebaseEffect, UndoResult};
use crate::msg::Msg;
use crate::msg::RebaseMsg;
use glowfly_core::rebase::{self, Outcome, SequenceEditor};
use glowfly_core::undo::{self, RebaseStart};
use std::path::Path;

/// The command git runs to install the prepared todo: this executable in `--write-todo` mode.
/// Unit tests run inside the test binary, which has no such mode, so they copy the file.
fn sequence_editor() -> SequenceEditor {
    if cfg!(test) {
        return SequenceEditor::Copy;
    }
    std::env::current_exe().map(SequenceEditor::Helper).unwrap_or(SequenceEditor::Copy)
}

pub(super) fn run(e: RebaseEffect) -> Msg {
    Msg::Rebase(match e {
        RebaseEffect::LoadPlan { workdir, base, generation } => {
            RebaseMsg::RebasePlanLoaded { generation, result: rebase::plan(&workdir, &base).map_err(err) }
        }
        RebaseEffect::Start { cwd, request, detail } => {
            let before = undo::rebase_start(&cwd);
            let op = Some(glowfly_core::journal::Op::Rewrite);
            let result = super::journal::around(&cwd, op, || rebase::start(&cwd, &request, &sequence_editor()));
            let undo = recorded(&cwd, before, &detail, &result);
            RebaseMsg::RebaseFinished { result: result.map_err(err), undo }
        }
        RebaseEffect::PrepareEdit { workdir, edit } => {
            let result = rebase::prepare_edit(&workdir, &edit).map_err(err);
            RebaseMsg::EditPrepared { workdir, result }
        }
        RebaseEffect::Resume { cwd, how } => {
            let before = undo::rebase_start(&cwd);
            let detail = before.as_ref().map(RebaseStart::describe).unwrap_or_default();
            let result = rebase::resume(&cwd, how);
            let undo = recorded(&cwd, before, &detail, &result);
            RebaseMsg::RebaseFinished { result: result.map_err(err), undo }
        }
        RebaseEffect::LoadRemaining { workdir, generation } => {
            RebaseMsg::RemainingLoaded { generation, result: rebase::remaining(&workdir).map_err(err) }
        }
        RebaseEffect::Restack { cwd, job } => return super::stack::restack(cwd, job),
        RebaseEffect::ResumeRestack { cwd, how, job } => return super::stack::resume(cwd, how, job),
        RebaseEffect::EditTodo { cwd, request } => {
            RebaseMsg::RemainingEdited { result: rebase::edit_todo(&cwd, &request, &sequence_editor()).map_err(err) }
        }
    })
}

/// What a rebase run means for the undo history: a finished rebase is one action back to where
/// its branch was before (`None`, clearing the history, when that can't be known); a stopped
/// one, or one that failed without moving HEAD, keeps the history as it is.
fn recorded(cwd: &Path, before: Option<RebaseStart>, detail: &str, result: &glowfly_core::error::Result<Outcome>) -> Option<UndoResult> {
    match result {
        Ok(Outcome::Finished { .. }) => before.and_then(|b| undo::rebased(cwd, &b, detail)).map(|a| UndoResult::Recorded(Some(a))),
        Ok(Outcome::Stopped { .. }) => Some(UndoResult::Recorded(None)),
        Ok(Outcome::Failed { .. }) | Err(_) => match before {
            Some(b) if !undo::unmoved(cwd, &b) => None,
            _ => Some(UndoResult::Recorded(None)),
        },
    }
}
