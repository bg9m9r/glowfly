//! Interactive rebase effects: loading the list, preparing a graph edit, running a rebase with
//! Trellis as the sequence editor, and resuming a stopped one.

use super::err;
use crate::effect::RebaseEffect;
use crate::msg::Msg;
use crate::msg::RebaseMsg;
use trellis_core::rebase::{self, SequenceEditor};

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
        RebaseEffect::Start { cwd, request } => {
            RebaseMsg::RebaseFinished { result: rebase::start(&cwd, &request, &sequence_editor()).map_err(err) }
        }
        RebaseEffect::PrepareEdit { workdir, edit } => {
            let result = rebase::prepare_edit(&workdir, &edit).map_err(err);
            RebaseMsg::EditPrepared { workdir, result }
        }
        RebaseEffect::Resume { cwd, how } => RebaseMsg::RebaseFinished { result: rebase::resume(&cwd, how).map_err(err) },
    })
}
