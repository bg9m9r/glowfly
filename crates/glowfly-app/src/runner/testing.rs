//! The runner's entry points for tests: running one effect to completion (`execute`, and
//! `execute_checked` with a trust re-check first), the sandboxed runner of the whole-app tests
//! (`sandboxed`, `sandbox.rs`), the diff decoration a loaded diff gets, and the working-file
//! helpers. Everything here runs with the stand-ins of `SEAMS`: a folder in the temp dir as the
//! trash (`test_bin`), `cp` as the sequence editor and long path-search chunks, so a test never
//! touches the real Recycle Bin or Trash and never runs the test binary as git's editor.

use super::execute::{Cx, Execute, execute_with};
use super::seams::{Seams, SeamsKind};
use super::{Runner, Waker};
use crate::effect::{Checked, Effect, FileOp, TrustCheck};
use crate::msg::Msg;
use glowfly_core::rebase::SequenceEditor;
use glowfly_core::{repo, undo};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::time::Duration;

pub(crate) use super::highlight::{decorate, decorate_in_context};

/// The tests' stand-ins. A path search's chunk may take far longer than the app's, so a busy
/// machine (the whole suite running in parallel) cannot split a small search into several.
pub const SEAMS: Seams = Seams {
    kind: SeamsKind::StandIn,
    trashers: undo::Trashers { system: test_trash, home: test_trash, restore: test_untrash },
    sequence_editor: copy_editor,
    chunk: Duration::from_secs(30),
};

/// Runs one effect to completion (blocking) and returns the message to send back, if any.
/// Results that belong to one repository come back as `Msg::Scoped`.
pub fn execute(effect: Effect) -> Option<Msg> {
    let graph_cache = repo::GraphCache::default();
    execute_with(effect, &cx(&graph_cache))
}

/// `execute` with the trust re-check `c` first.
pub fn execute_checked(c: TrustCheck, effect: Effect) -> Option<Msg> {
    let graph_cache = repo::GraphCache::default();
    Execute::execute(Box::new(Checked { effect, check: c }), &cx(&graph_cache))
}

/// The `Cx` of an effect run on its own: never cancelled, no stop, no gate, other messages
/// dropped, a fresh graph cache, and the stand-ins.
fn cx(graph_cache: &repo::GraphCache) -> Cx<'_> {
    Cx { cancelled: &|| false, graph_cache, stop: None, gate: None, send: &|_| {}, seams: SEAMS }
}

/// `Runner::new` for a whole-app test: saves land in `files`, outside effects are dropped
/// (`sandbox.rs`), and the work runs with the stand-ins.
pub fn sandboxed(tx: Sender<Msg>, waker: Waker, files: PathBuf) -> Runner {
    let mut runner = Runner::new(tx, waker);
    runner.seams = SEAMS;
    runner.sandbox = Some(super::sandbox::Sandbox { files, dropped: Default::default() });
    runner
}

impl Runner {
    /// How many effects the runner has started.
    pub fn started(&self) -> u64 {
        self.tally.started.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// The names of the effects still running or queued.
    pub fn in_flight_names(&self) -> Vec<&'static str> {
        super::sandbox::lock(&self.tally.in_flight).clone()
    }

    /// The names of the effects the sandbox dropped, in order.
    pub fn dropped(&self) -> Vec<&'static str> {
        self.sandbox.as_ref().map(|s| super::sandbox::lock(&s.dropped).clone()).unwrap_or_default()
    }
}

/// The stand-in trash: a folder in the temp dir, one per process.
pub fn test_bin() -> PathBuf {
    glowfly_core::testkit::bin::own("glowfly-app-test-bin")
}

/// Moves `full` into `test_bin()` under a unique name (`glowfly_core::testkit::bin`).
fn test_trash(full: &Path) -> Result<(), repo::TrashRefusal> {
    glowfly_core::testkit::bin::move_in(&test_bin(), full)
}

/// Takes `full` back out of `test_bin()` for Undo.
fn test_untrash(full: &Path, ours: &dyn Fn(&Path) -> bool) -> repo::Untrashed {
    glowfly_core::testkit::bin::take_out(&test_bin(), full, ours)
}

/// `cp`: installs the prepared todo without checking it (the test binary has no
/// `--write-todo` mode).
fn copy_editor() -> SequenceEditor {
    SequenceEditor::Copy
}

/// `Effect::WorkFile`'s work with the stand-in trash.
pub fn work_file_for_test(workdir: &Path, op: FileOp) -> Msg {
    super::files::work_file(workdir, op, SEAMS.trashers)
}

/// `Effect::WorkFile`'s work with a trash that refuses every file (`TrashProblem::NoTrash`).
pub fn refused_for_test(workdir: &Path, op: FileOp) -> Msg {
    fn refuse(_: &Path) -> Result<(), repo::TrashRefusal> {
        Err(repo::TrashRefusal::new(repo::TrashProblem::NoTrash, "no bin"))
    }
    super::files::work_file(workdir, op, undo::Trashers { system: refuse, home: refuse, ..SEAMS.trashers })
}

#[cfg(test)]
#[path = "testing_tests.rs"]
mod tests;
