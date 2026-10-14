//! Interactive rebase effects: loading the list, and running it with Trellis as the sequence
//! editor.

use super::err;
use crate::msg::Msg;
use std::path::Path;
use trellis_core::rebase::{self, RebaseBase, SequenceEditor, StartRequest};

/// The command git runs to install the prepared todo: this executable in `--write-todo` mode.
/// Unit tests run inside the test binary, which has no such mode, so they copy the file.
fn sequence_editor() -> SequenceEditor {
    if cfg!(test) {
        return SequenceEditor::Copy;
    }
    std::env::current_exe().map(SequenceEditor::Helper).unwrap_or(SequenceEditor::Copy)
}

pub(super) fn load_plan(workdir: &Path, base: &RebaseBase, generation: u64) -> Msg {
    Msg::RebasePlanLoaded { generation, result: rebase::plan(workdir, base).map_err(err) }
}

pub(super) fn start(cwd: &Path, request: &StartRequest) -> Msg {
    Msg::RebaseFinished { result: rebase::start(cwd, request, &sequence_editor()).map_err(err) }
}
