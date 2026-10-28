//! Running an abort that keeps its work (`aborted.rs`): keep, abort, and read what the abort
//! left on disk (`left`, which Undo checks byte for byte). When an abort command fails partway
//! (a file locked on Windows, say), everything kept is put straight back (`put_back`), so the
//! operation is in progress again as it was; if that fails too, the output says what is where.

use super::Action;
use super::aborted::abort_saving;
use super::aborted_restore::{left, put_back};
use git2::Oid;
use std::path::Path;

/// How the output of a kept abort that was refused, with nothing changed, starts; the reason
/// follows. The app then offers the plain abort.
pub const NOTHING_ABORTED: &str = "Nothing was aborted: ";

/// How a kept abort went.
#[derive(Debug)]
pub struct AbortRun {
    /// The operation ended and its work is kept.
    pub ok: bool,
    /// The commands' output and what became of the work.
    pub output: String,
    /// What Undo reverses; `None` when the abort didn't happen or can't be undone.
    pub action: Option<Action>,
}

/// Aborts the merge, single cherry-pick or single revert in progress in `workdir`, keeping its
/// work first; `run` runs the abort's commands in order, stopping at the first that fails. Refs
/// of old kept aborts are dropped unless `prune` lists them (`None`: none are dropped). Nothing
/// is aborted when the work can't all be kept.
pub fn abort_keeping(workdir: &Path, prune: Option<&[Oid]>, run: impl FnOnce(&[Vec<String>]) -> (bool, String)) -> AbortRun {
    let failed = |output: String| AbortRun { ok: false, output, action: None };
    let saving = match abort_saving(workdir, prune) {
        Ok(s) => s,
        Err(e) => return failed(format!("{NOTHING_ABORTED}{e}.")),
    };
    let Action::AbortKept { op, saved, head, branch, .. } = saving.action else { unreachable!("a kept abort") };
    let (ok, output) = run(&saving.cmds);
    let kept = &saving.kept;
    if !ok {
        return failed(match put_back(workdir, saved, head) {
            Ok(()) => format!("{output}\nThe abort failed, so everything it had was put back: the {} is still in progress.", op.noun()),
            Err(e) => format!("{output}\nThe abort failed partway, and putting its work back failed too: {e}."),
        });
    }
    match left(workdir, saved) {
        Ok(left) => AbortRun {
            ok: true,
            output: format!("{output}\nIts work is kept in {kept} until Undo puts it back.").trim().to_owned(),
            action: Some(Action::AbortKept { op, saved, head, branch, left }),
        },
        // The abort happened, but Undo has nothing to check against: said as a failure, so the
        // ref that keeps the work is seen (it is pruned only once it is `KEEP_DAYS` old).
        Err(e) => failed(format!("{output}\nThe abort happened, but Undo can't put its work back ({e}). All of it is kept in {kept}.")),
    }
}
