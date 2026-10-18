//! Results for a tab that is no longer active: what each area keeps (`Msg::on_inactive`).

use crate::msg::{Inactive, Msg};
use crate::state::AppState;
use std::path::Path;
use std::time::Instant;

/// Records `action` in the undo history of the parked tab for `workdir`.
fn record_parked(st: &mut AppState, workdir: &Path, action: trellis_core::undo::Action) {
    if let Some(p) = st.tabs.iter_mut().find(|t| t.is(workdir)).and_then(|t| t.parked.as_mut()) {
        p.undo.record(action);
    }
}

/// A result for the repository at `workdir`, whose tab is no longer active: only what its area
/// says still matters (`Msg::on_inactive`) is applied.
pub(super) fn inactive(st: &mut AppState, workdir: &Path, msg: Msg, now: Instant) {
    match msg.on_inactive() {
        Some(Inactive::Log(text)) => st.log(false, text),
        Some(Inactive::Undo(action)) => record_parked(st, workdir, action),
        Some(Inactive::LogAndUndo { text, undo }) => {
            st.log(false, text);
            record_parked(st, workdir, undo);
        }
        Some(Inactive::WorkFile { op, result }) => {
            // A delete that can be undone goes into that tab's history.
            if let Ok(crate::effect::WorkFileOk::Deleted { undo: Some(a), .. }) = &result {
                record_parked(st, workdir, (**a).clone());
            }
            super::work_files::inactive_result(st, op, result, now);
        }
        None => tracing::debug!("dropped a result for {}, which is no longer the active tab", workdir.display()),
    }
}
