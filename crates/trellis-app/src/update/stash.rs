//! Apply, pop and drop of one stash (sidebar STASHES menu and palette). The stash list is
//! numbered from the newest entry, so a stash made or dropped elsewhere since the snapshot
//! shifts every `stash@{N}`: the job names the entry by position for git but carries the
//! commit the user picked, and the runner refuses it if that position now holds another stash.

use super::ops::{JobSpec, start_job};
use crate::effect::Effect;
use crate::state::AppState;
use crate::state::palette::StashAction;
use git2::Oid;
use trellis_core::gitcli::args;

pub(super) fn run(st: &mut AppState, action: StashAction, index: usize, oid: Oid, fx: &mut Vec<Effect>) {
    let refname = format!("stash@{{{index}}}");
    let cmd = args(&["stash", action.git_verb(), &refname]);
    // git's "Dropped stash@{N} (<sha>)" line lands in the log, so a dropped stash can be found
    // again (`git stash apply <sha>`) until it is garbage-collected.
    let spec = JobSpec { expect: Some((refname, oid)), ..JobSpec::new(action.job_label().into(), vec![cmd]) };
    start_job(st, spec, fx);
}
