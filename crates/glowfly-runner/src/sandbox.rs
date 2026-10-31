//! The runner of the whole-app tests (`src/journey/`). Repository and file effects run as in the
//! app; saves of Glowfly's own files go to the test's folder instead of the user's config
//! folder, written before `run` returns; and effects that would reach the network, the OS
//! shell, the desktop, file pickers, the credential store or the global git config are dropped
//! and recorded by name. The tally counts effects started and still in flight, so a test can
//! tell when the app is idle.

use super::Runner;
use crate::effect::{Effect, SandboxRule};
use crate::msg::Msg;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// Built by `testing::sandboxed` only.
pub(super) struct Sandbox {
    /// Where settings.json, recent.txt and tabs.txt are written.
    pub(super) files: PathBuf,
    /// The names of the effects dropped, in order.
    pub(super) dropped: Arc<Mutex<Vec<&'static str>>>,
}

/// Effects started, and the names of those still running or queued.
#[derive(Default)]
pub(super) struct Tally {
    pub(super) started: AtomicU64,
    pub(super) in_flight: Mutex<Vec<&'static str>>,
}

/// One effect in flight; dropping it (also while a panicking effect unwinds) ends it.
pub(super) struct Busy {
    tally: Arc<Tally>,
    name: &'static str,
}

pub(super) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Tally {
    pub(super) fn begin(self: &Arc<Self>, name: &'static str) -> Busy {
        self.started.fetch_add(1, Ordering::SeqCst);
        lock(&self.in_flight).push(name);
        Busy { tally: Arc::clone(self), name }
    }
}

impl Drop for Busy {
    fn drop(&mut self) {
        let mut names = lock(&self.tally.in_flight);
        if let Some(k) = names.iter().position(|n| *n == self.name) {
            names.remove(k);
        }
    }
}

impl Runner {
    /// The effect to run as usual, or None when the sandbox handled it (a save it wrote, or an
    /// effect it dropped). Without a sandbox every effect runs.
    pub(super) fn admit(&self, effect: Effect) -> Option<Effect> {
        let Some(sandbox) = &self.sandbox else { return Some(effect) };
        match effect.sandbox() {
            SandboxRule::Run => Some(effect),
            SandboxRule::Save => {
                if let Some(msg) = save_into(&sandbox.files, effect)
                    && self.tx.send(msg).is_ok()
                {
                    self.waker.wake();
                }
                None
            }
            SandboxRule::Drop => {
                tracing::debug!("sandbox: dropped {}", effect.name());
                lock(&sandbox.dropped).push(effect.name());
                None
            }
        }
    }
}

/// `saves::save_now`, writing into `folder`.
fn save_into(folder: &Path, effect: Effect) -> Option<Msg> {
    let (what, result) = match effect {
        Effect::SaveSettings(settings) => ("settings", settings.save_to(&folder.join("settings.json"))),
        Effect::SaveRecent(list) => ("the recent repositories", super::platform::save_recent_to(&folder.join("recent.txt"), &list)),
        Effect::SaveTabs { paths, labels, active } => {
            ("the open tabs", super::platform::save_tabs_to(&folder.join("tabs.txt"), &paths, &labels, active))
        }
        _ => return None,
    };
    let e = result.err()?;
    Some(Msg::Logged { ok: false, text: format!("Could not save {what}: {e}") })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect::{CloneEffect, DesktopNote, NoteKind, OsEffect};
    use crate::patch::PatchEffect;
    use crate::profile::ProfileEffect;
    use crate::runner::Waker;
    use std::sync::mpsc;

    fn folder(name: &str) -> PathBuf {
        let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-sandbox-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn runs_repository_effects_and_drops_outside_ones() {
        let dir = folder("admit");
        let (tx, _rx) = mpsc::channel();
        let runner = super::super::testing::sandboxed(tx, Waker::none(), dir.clone());
        let status =
            Effect::LoadStatus { workdir: dir.clone(), generation: 1, submodules: false, open: None, git_status: false, paths: None };
        assert!(matches!(runner.admit(status), Some(Effect::LoadStatus { .. })));
        assert!(runner.admit(Effect::Os(OsEffect::OpenUrl("https://example.invalid".into()))).is_none());
        let note = DesktopNote { kind: NoteKind::Job, title: "t".into(), body: "b".into(), workdir: None };
        assert!(runner.admit(Effect::Notify(note)).is_none());
        assert_eq!(runner.dropped(), ["open-url", "desktop-note"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_sweep_without_a_watcher_reads_the_whole_status() {
        let dir = folder("sweep");
        let (tx, rx) = mpsc::channel();
        let runner = super::super::testing::sandboxed(tx, Waker::none(), dir.clone());
        runner.run(Effect::SweepWatch { workdir: dir.clone() });
        let msg = rx.recv_timeout(std::time::Duration::from_secs(5)).expect("a message");
        let Msg::Scoped { workdir, msg } = msg else { panic!("scoped to the repository") };
        assert_eq!(workdir, dir);
        assert!(matches!(*msg, Msg::Watch(crate::msg::WatchMsg::RepoChanged { full: false, paths: None, .. })));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn patch_pickers_and_the_service_repo_list_are_dropped() {
        let dir = folder("split");
        let (tx, _rx) = mpsc::channel();
        let runner = super::super::testing::sandboxed(tx, Waker::none(), dir.clone());
        let what = glowfly_core::patch_ops::PatchSource::Commits { oids: Vec::new(), paths: Vec::new() };
        let pick_save = PatchEffect::PickSave { workdir: dir.clone(), what: what.clone(), name: "a.patch".into() };
        assert!(runner.admit(Effect::Patch(pick_save)).is_none());
        assert!(runner.admit(Effect::Patch(PatchEffect::PickOpen { workdir: dir.clone() })).is_none());
        let export = PatchEffect::Export { workdir: dir.clone(), what, dest: dir.join("a.patch") };
        assert!(runner.admit(Effect::Patch(export)).is_some());
        let inspect = PatchEffect::Inspect { workdir: dir.clone(), path: dir.join("a.patch") };
        assert!(runner.admit(Effect::Patch(inspect)).is_some());
        let account =
            glowfly_core::forge::Account { provider: glowfly_core::forge::Provider::GitHub, host: "github.com".into(), org: String::new() };
        assert!(runner.admit(Effect::Clone(CloneEffect::ListRepos { account, cwd: dir.clone() })).is_none());
        let sparse =
            CloneEffect::Sparse { parent: dir.clone(), cmd: Vec::new(), dest: dir.join("c"), folders: Vec::new(), submodules: Vec::new() };
        assert!(runner.admit(Effect::Clone(sparse)).is_some());
        assert_eq!(runner.dropped(), ["pick-patch-save", "pick-patch", "list-my-repos"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn repository_identity_runs_and_folder_rules_are_dropped() {
        use glowfly_core::profile_includes::IncludePlan;
        let dir = folder("profiles");
        let (tx, _rx) = mpsc::channel();
        let runner = super::super::testing::sandboxed(tx, Waker::none(), dir.clone());
        let (workdir, git_dir) = (dir.clone(), dir.join(".git"));
        let read = ProfileEffect::ReadIdentity { workdir: workdir.clone(), git_dir: git_dir.clone(), rules: Vec::new() };
        assert!(runner.admit(Effect::Profile(read)).is_some());
        assert!(runner.admit(Effect::Profile(ProfileEffect::ClearLocalIdentity { workdir, git_dir, rules: Vec::new() })).is_some());
        let id = glowfly_core::settings::ProfileId(1);
        let plan = IncludePlan { files: Vec::new(), entries: Vec::new() };
        assert!(runner.admit(Effect::Profile(ProfileEffect::SyncIncludes { cwd: dir.clone(), plan, place: true })).is_none());
        assert!(runner.admit(Effect::Profile(ProfileEffect::CheckFolder { id, path: dir.clone(), repository: false })).is_none());
        assert_eq!(runner.dropped(), ["sync-profile-includes", "check-profile-folder"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn custom_commands_are_dropped() {
        use glowfly_model::custom_commands::{CustomCommandEffect, ShellRun};
        let dir = folder("custom");
        let (tx, _rx) = mpsc::channel();
        let runner = super::super::testing::sandboxed(tx, Waker::none(), dir.clone());
        assert!(runner.admit(Effect::CustomCommand(CustomCommandEffect::Load { generation: 1, workdir: dir.clone() })).is_none());
        let shell = CustomCommandEffect::RunShell(ShellRun { cwd: dir.clone(), script: "true".into() });
        assert!(runner.admit(Effect::CustomCommand(shell)).is_none());
        assert_eq!(runner.dropped(), ["load-custom-commands", "run-custom-command"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn moved_effects_keep_their_sandbox_rule() {
        use crate::effect::{ConflictEffect, ConflictRead, LfsEffect, MonitorJob, SelectionEffect, SparseEffect, WorktreeEffect};
        let dir = folder("moved");
        let (tx, _rx) = mpsc::channel();
        let runner = super::super::testing::sandboxed(tx, Waker::none(), dir.clone());
        assert!(runner.admit(Effect::TurnOnMonitor(MonitorJob { cwd: dir.clone(), then: Vec::new() })).is_none());
        let oid = git2::Oid::zero();
        let count = crate::worktree::CountRequest { workdir: dir.clone(), refs_key: 1, targets: Vec::new() };
        let runs = [
            Effect::Lfs(LfsEffect::Detect),
            Effect::Lfs(LfsEffect::Load { workdir: dir.clone(), generation: 1 }),
            Effect::Sparse(SparseEffect::Load { workdir: dir.clone(), generation: 1, run_git: false }),
            Effect::Sparse(SparseEffect::Checkout { cwd: dir.clone(), command: glowfly_core::sparse_ops::SparseCommand::Reapply }),
            Effect::Conflict(ConflictEffect::Load { workdir: dir.clone(), path: "a.txt".into(), generation: 1 }),
            Effect::Conflict(ConflictEffect::ReadFiles {
                workdir: dir.clone(),
                paths: Vec::new(),
                purpose: ConflictRead::BeforeMarkResolved,
            }),
            Effect::Selection(SelectionEffect::Compare { workdir: dir.clone(), from: oid, to: oid }),
            Effect::Worktree(WorktreeEffect::CountChanges(count)),
            Effect::Clone(CloneEffect::CheckParent { parent: "parent".into(), delay: std::time::Duration::ZERO }),
        ];
        for effect in runs {
            let name = effect.name();
            assert!(runner.admit(effect).is_some(), "{name} runs");
        }
        assert_eq!(runner.dropped(), ["turn-on-monitor"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn saves_are_written_into_the_folder_before_admit_returns() {
        let dir = folder("saves");
        let (tx, rx) = mpsc::channel();
        let runner = super::super::testing::sandboxed(tx, Waker::none(), dir.clone());
        let repo = dir.join("repo");
        assert!(runner.admit(Effect::SaveRecent(vec![repo.clone()])).is_none());
        let tabs = Effect::SaveTabs { paths: vec![repo.clone()], labels: vec![Some("Web".into())], active: 0 };
        assert!(runner.admit(tabs).is_none());
        let settings = glowfly_core::settings::Settings { auto_fetch_minutes: 7, ..Default::default() };
        assert!(runner.admit(Effect::SaveSettings(Box::new(settings))).is_none());
        let recent = std::fs::read_to_string(dir.join("recent.txt")).unwrap();
        assert_eq!(crate::runner::platform::parse_recent(&recent), std::slice::from_ref(&repo));
        let tabs = std::fs::read_to_string(dir.join("tabs.txt")).unwrap();
        assert_eq!(crate::runner::platform::parse_tabs(&tabs), (vec![repo], vec![Some("Web".to_owned())], 0));
        let saved = std::fs::read_to_string(dir.join("settings.json")).unwrap();
        assert!(saved.contains("\"auto_fetch_minutes\": 7"), "{saved}");
        assert!(rx.try_recv().is_err(), "nothing failed");
        assert!(runner.dropped().is_empty() && runner.started() == 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_effect_is_in_flight_until_its_guard_drops() {
        let tally = Arc::new(Tally::default());
        let a = tally.begin("git");
        let b = tally.begin("git");
        assert_eq!(*lock(&tally.in_flight), ["git", "git"]);
        drop(a);
        assert_eq!(*lock(&tally.in_flight), ["git"]);
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _b = b;
            panic!("an effect panicked");
        }));
        assert!(lock(&tally.in_flight).is_empty());
        assert_eq!(tally.started.load(Ordering::SeqCst), 2);
    }
}
