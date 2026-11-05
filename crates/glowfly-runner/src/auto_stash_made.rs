//! An auto-stash job's saved record, written on the job's own thread as soon as its first
//! command (`git stash push -m <message>`) made the stash, before the operation's next command
//! starts: a crash, a kill or the session ending from then on still leaves the stash offered at
//! the next open. A save the model asks for meanwhile queues behind the job, so the record can't
//! wait for it. Then `AutoStashMsg::Made` tells the model, which keeps the record in every later
//! rewrite until the put-back ends. A push that stashed nothing ("No local changes to save")
//! writes and sends nothing.

use glowfly_core::stash_ops::{self, SavedAutoStash};
use glowfly_model::msg::{AutoStashMsg, Msg};
use glowfly_model::state::auto_stash::AutoStashJob;
use std::path::Path;

/// Writes the record of the stash `job` made in `cwd`, when the list shows one, and sends `Made`.
pub(super) fn note(cwd: &Path, job: &AutoStashJob, send: &dyn Fn(Msg)) {
    let Ok(list) = stash_ops::list(cwd) else { return };
    let Some(oid) = stash_ops::made_in(&list, &job.message, &job.before) else { return };
    if let Err(e) = stash_ops::add_saved(cwd, SavedAutoStash::new(job.op.clone(), job.message.clone(), oid)) {
        send(super::stashes::save_failed(e));
    }
    send(Msg::AutoStash(AutoStashMsg::Made { workdir: cwd.to_path_buf(), oid }));
}

/// The hook a job calls before each command index: before command 1 (so command 0, the stash
/// push, succeeded) it notes `job`'s stash, once. Without a job it does nothing.
pub(super) fn starting<'a>(cwd: &'a Path, job: Option<&'a AutoStashJob>, send: &'a dyn Fn(Msg)) -> impl FnMut(usize) + 'a {
    let mut job = job;
    move |at| {
        if at == 1
            && let Some(j) = job.take()
        {
            note(cwd, j, send);
        }
    }
}

#[cfg(test)]
#[path = "auto_stash_made_tests.rs"]
mod tests;
