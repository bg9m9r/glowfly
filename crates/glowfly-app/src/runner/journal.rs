//! The operation journal's reads (`Effect::Journal`), and writing its notes around the
//! operations that rewrite history: `around` captures the refs an op may move, runs it, and
//! appends a note only when one of them moved, whether git succeeded or not. The clock is read
//! here, never in `update`. A note that can't be written is logged and never fails the run.

use super::execute::err;
use super::execute::{Cx, Execute};
use crate::journal::{JournalEffect, JournalMsg, LostMsg};
use crate::msg::Msg;
use glowfly_core::journal::{self, Op, notes};
use std::path::Path;

pub(super) fn run(e: JournalEffect) -> Msg {
    match e {
        JournalEffect::Load { workdir, generation } => {
            let result = journal::load(&workdir).map(Box::new).map_err(err);
            Msg::Journal(JournalMsg::Loaded { generation, result })
        }
        JournalEffect::Preview { workdir, key, oid, against, generation } => {
            let result = journal::preview(&workdir, oid, against).map_err(err);
            Msg::Journal(JournalMsg::PreviewLoaded { generation, key, result })
        }
        JournalEffect::Lost { workdir, generation } => {
            let result = journal::lost::find(&workdir).map(Box::new).map_err(err);
            Msg::Journal(JournalMsg::Lost(LostMsg::Loaded { generation, result }))
        }
        JournalEffect::LostPreview { workdir, oid, from, generation } => {
            let result = journal::preview(&workdir, from, None).map_err(err);
            Msg::Journal(JournalMsg::Lost(LostMsg::PreviewLoaded { generation, oid, result }))
        }
    }
}

impl Execute for JournalEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        Some(run(self))
    }
}

/// Unix seconds now.
fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// Runs `f` in the repository at `cwd`; with `op`, writes its journal note when `f` moved
/// HEAD or the op's branch.
pub(super) fn around<T>(cwd: &Path, op: Option<Op>, f: impl FnOnce() -> T) -> T {
    let Some(op) = op else { return f() };
    let captured = notes::capture(cwd, op, now());
    let out = f();
    match captured {
        Ok(c) if c.moved(cwd) => {
            let note = c.into_note();
            if !note.refs.is_empty()
                && let Err(e) = notes::append(cwd, &note)
            {
                tracing::warn!("could not write the journal note: {e}");
            }
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("could not read the refs for the journal note: {e}"),
    }
    out
}

#[cfg(test)]
#[path = "journal_tests.rs"]
mod tests;
