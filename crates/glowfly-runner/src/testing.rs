//! The runner's entry points for tests: running one effect to completion (`execute`,
//! `execute_all` keeping the messages it sends on the way, and `execute_checked` with a trust
//! re-check first), the sandboxed runner of the whole-app tests (`sandboxed`, `sandbox.rs`), the
//! diff decoration a loaded diff gets, and the working-file helpers. Everything here runs with
//! the stand-ins of `SEAMS`: a folder in the temp dir as the trash (`test_bin`), `cp` as the
//! sequence editor, long path-search chunks and a folder in the temp dir for saves of Glowfly's
//! own files (`own_files`), so a test never touches the real Recycle Bin or Trash, never runs the
//! test binary as git's editor and never writes over the developer's settings, recent
//! repositories or tabs. `execute` refuses the effects that reach outside the test (those the
//! whole-app sandbox drops: the user's global git config, the credential store, Glowfly's own
//! folders, the network, the desktop), bar a few that only read.

use super::execute::{Cx, Execute, execute_with};
use super::seams::{Seams, SeamsKind};
use super::{Runner, Waker};
use glowfly_core::rebase::SequenceEditor;
use glowfly_core::{repo, undo};
use glowfly_model::effect::{Checked, Effect, FileOp, SandboxRule, SaveEffect, TrustCheck};
use glowfly_model::msg::Msg;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::time::Duration;

pub use super::highlight::{decorate, decorate_in_context};

/// The tests' stand-ins. A path search's chunk may take far longer than the app's, so a busy
/// machine (the whole suite running in parallel) cannot split a small search into several.
pub const SEAMS: Seams = Seams {
    kind: SeamsKind::StandIn,
    trashers: undo::Trashers { system: test_trash, home: test_trash, restore: test_untrash },
    sequence_editor: copy_editor,
    chunk: Duration::from_secs(30),
    save: save_for_test,
};

/// Runs one effect to completion (blocking) and returns the message to send back, if any.
/// Results that belong to one repository come back as `Msg::Scoped`.
pub fn execute(effect: Effect) -> Option<Msg> {
    refuse_outside(&effect);
    let graph_cache = repo::GraphCache::default();
    execute_with(effect, &cx(&graph_cache))
}

/// `execute`, keeping what the effect sends while it runs (`Cx::send`, such as an auto-stash's
/// `AutoStashMsg::Made`): those messages in order, then its result.
pub fn execute_all(effect: Effect) -> Vec<Msg> {
    refuse_outside(&effect);
    let graph_cache = repo::GraphCache::default();
    let sent = std::cell::RefCell::new(Vec::new());
    let send = |m: Msg| sent.borrow_mut().push(m);
    let result = execute_with(effect, &Cx { send: &send, ..cx(&graph_cache) });
    let mut all = sent.into_inner();
    all.extend(result);
    all
}

/// `execute` with the trust re-check `c` first.
pub fn execute_checked(c: TrustCheck, effect: Effect) -> Option<Msg> {
    refuse_outside(&effect);
    let graph_cache = repo::GraphCache::default();
    Execute::execute(Box::new(Checked { effect, check: c }), &cx(&graph_cache))
}

/// Fails the test if `effect` reaches outside the test's own repositories and folders: every
/// effect the whole-app tests' sandbox drops (the network, the OS shell, the desktop, file
/// pickers, the credential store, the global git config, Glowfly's own folders; see
/// `Effect::sandbox`), with `ALLOWED`'s exceptions. Saves are not refused: `SEAMS` redirects them.
#[track_caller]
fn refuse_outside(effect: &Effect) {
    assert!(
        !refused(effect),
        "{} reaches outside the test (the user's git config, Glowfly's own folders, the network or the desktop); tests must not run it",
        effect.name()
    );
}

/// Whether `refuse_outside` fails a test that runs `effect`.
fn refused(effect: &Effect) -> bool {
    effect.sandbox() == SandboxRule::Drop && !allowed(effect)
}

/// The effects the sandbox drops that a test may still run: they only read the test's
/// repositories and folders, write a test repository's own config, fetch from a test
/// repository's (local) remote, or read the user's git config and signing keys without changing
/// them. An effect wrapped in a trust re-check (`Effect::Checked`) is allowed when the effect
/// inside it is.
fn allowed(effect: &Effect) -> bool {
    use glowfly_model::effect::partial::PartialEffect;
    if let Effect::Checked(c) = effect {
        return allowed(&c.effect);
    }
    use glowfly_model::deep_link::DeepLinkEffect;
    use glowfly_model::effect::{PrefsEffect, ReposEffect};
    use glowfly_model::pull_request::PrEffect;
    use glowfly_model::ssh::SshEffect;
    matches!(
        effect,
        Effect::DeepLink(DeepLinkEffect::Build { .. } | DeepLinkEffect::Find { .. })
            | Effect::PullRequest(PrEffect::Files(_) | PrEffect::LineMaps(_) | PrEffect::FetchHead(_))
            | Effect::Repos(ReposEffect::Scan { .. } | ReposEffect::Sync { .. })
            | Effect::Ssh(SshEffect::ReadChoice { .. } | SshEffect::SetKeyChoice { repo: Some(_), .. })
            | Effect::Prefs(PrefsEffect::ListSigningKeys { .. })
    ) || matches!(effect, Effect::Partial(PartialEffect::SizeHistory(r)) if r.service.is_none())
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

/// Where `SEAMS` writes saves of Glowfly's own files (settings.json, recent.txt, tabs.txt): a
/// folder in the temp dir, one per process, emptied on its first use. Never the config folder.
pub fn own_files() -> PathBuf {
    glowfly_core::testkit::bin::own("glowfly-test-files")
}

/// `saves::save_now` into `own_files()`.
fn save_for_test(effect: SaveEffect) -> Option<Msg> {
    let dir = own_files();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return Some(Msg::Logged { ok: false, text: format!("Could not create {}: {e}", dir.display()) });
    }
    super::sandbox::save_into(&dir, effect)
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

/// `FilesEffect::Work`'s work with the stand-in trash.
pub fn work_file_for_test(workdir: &Path, op: FileOp) -> Msg {
    super::files::work_file(workdir, op, SEAMS.trashers)
}

/// `FilesEffect::Work`'s work with a trash that refuses every file (`TrashProblem::NoTrash`).
pub fn refused_for_test(workdir: &Path, op: FileOp) -> Msg {
    fn refuse(_: &Path) -> Result<(), repo::TrashRefusal> {
        Err(repo::TrashRefusal::new(repo::TrashProblem::NoTrash, "no bin"))
    }
    super::files::work_file(workdir, op, undo::Trashers { system: refuse, home: refuse, ..SEAMS.trashers })
}

#[cfg(test)]
#[path = "testing_tests.rs"]
mod tests;
