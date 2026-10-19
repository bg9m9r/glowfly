//! Stash list jobs (`Effect::Stash`): renaming an entry.

use super::expect;
use crate::effect::StashEffect;
use crate::msg::{JobMsg, Msg};
use git2::Oid;
use std::path::Path;
use trellis_core::stash_ops;

pub(super) fn run(e: StashEffect) -> Msg {
    match e {
        StashEffect::Rename { cwd, index, oid, message } => rename(&cwd, index, oid, &message),
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
