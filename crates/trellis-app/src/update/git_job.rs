//! Running git command lines as the active job: `JobMsg::Git`, and `JobMsg::Journaled` for the
//! journal's Restore, which names its own journal note and revision check. The undo record and
//! the journal note are worked out from the command as given, before the auto-stash gate may
//! put a stash in front of it and before a hard reset gets its own stash, so neither rewrite
//! hides what the operation is.

use super::auto_stash::{self, Gate};
use super::jobs::{JobSpec, start_job};
use crate::effect::Effect;
use crate::msg::{JobMsg, Msg};
use crate::state::AppState;
use git2::Oid;
use trellis_core::journal::Op;
use trellis_core::undo::{ResetMode, UndoKind};

/// Runs `cmds` labelled `label`, with the journal note `journal` (else the one the single
/// command's kind calls for) and the revision check `expect`.
pub(super) fn run(
    st: &mut AppState,
    label: String,
    mut cmds: Vec<Vec<String>>,
    journal: Option<Op>,
    expect: Option<(String, Oid)>,
    fx: &mut Vec<Effect>,
) {
    let record = if cmds.len() == 1 { UndoKind::classify(&cmds[0]) } else { None };
    let given = journal.clone();
    let journal = journal.or_else(|| match (&record, cmds.as_slice()) {
        (Some(k), [c]) => Op::of_kind(k, c),
        _ => None,
    });
    // Uncommitted changes in the way: ask, or stash them first (update/auto_stash.rs).
    let mut retry = None;
    if let [cmd] = cmds.as_slice() {
        let again = || match &given {
            Some(op) => Msg::Job(JobMsg::Journaled { label: label.clone(), cmd: cmd.clone(), op: op.clone(), expect: expect.clone() }),
            None => Msg::Job(JobMsg::Git { label: label.clone(), cmds: cmds.clone() }),
        };
        match auto_stash::gate(st, &cmd.clone(), again) {
            Gate::Asked => return,
            Gate::Stash(op) => {
                let mut spec = JobSpec { record, journal, expect, ..JobSpec::new(label, cmds) };
                auto_stash::wrap(st, &mut spec, op);
                start_job(st, spec, fx);
                return;
            }
            Gate::Run(r) => retry = r,
        }
    }
    // A hard reset stashes uncommitted changes first, so undoing it brings them back.
    if record == Some(UndoKind::Reset(ResetMode::Hard)) {
        cmds = trellis_core::undo::hard_reset_commands(&cmds[0][2]);
    }
    let started = start_job(st, JobSpec { record, journal, expect, ..JobSpec::new(label.clone(), cmds) }, fx);
    auto_stash::ran(st, &label, started, retry);
}
