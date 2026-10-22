//! The runner of the whole-app tests (`src/journey/`). Repository and file effects run as in the
//! app; saves of Glowfly's own files go to the test's folder instead of the user's config
//! folder, written before `run` returns; and effects that would reach the network, the OS
//! shell, the desktop, file pickers, the credential store or the global git config are dropped
//! and recorded by name. The tally counts effects started and still in flight, so a test can
//! tell when the app is idle.

use super::{Runner, Waker};
use crate::effect::{CloneEffect, Effect, PrefsEffect};
use crate::git_env::GitEnvEffect;
use crate::msg::Msg;
use crate::patch::PatchEffect;
use crate::profile::ProfileEffect;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

pub(super) struct Sandbox {
    /// Where settings.json, recent.txt and tabs.txt are written.
    files: PathBuf,
    /// The names of the effects dropped, in order.
    dropped: Arc<Mutex<Vec<&'static str>>>,
}

/// Effects started, and the names of those still running or queued.
#[derive(Default)]
pub(super) struct Tally {
    started: AtomicU64,
    in_flight: Mutex<Vec<&'static str>>,
}

/// One effect in flight; dropping it (also while a panicking effect unwinds) ends it.
pub(super) struct Busy {
    tally: Arc<Tally>,
    name: &'static str,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
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

impl<W: Waker + Clone> Runner<W> {
    /// `Runner::new` for a whole-app test: saves land in `files`, and outside effects are dropped.
    pub(crate) fn sandboxed(tx: Sender<Msg>, waker: W, files: PathBuf) -> Self {
        let mut runner = Self::new(tx, waker);
        runner.sandbox = Some(Sandbox { files, dropped: Default::default() });
        runner
    }

    pub(crate) fn started(&self) -> u64 {
        self.tally.started.load(Ordering::SeqCst)
    }

    pub(crate) fn in_flight_names(&self) -> Vec<&'static str> {
        lock(&self.tally.in_flight).clone()
    }

    pub(crate) fn dropped(&self) -> Vec<&'static str> {
        self.sandbox.as_ref().map(|s| lock(&s.dropped).clone()).unwrap_or_default()
    }

    /// The effect to run as usual, or None when the sandbox handled it (a save it wrote, or an
    /// effect it dropped). Without a sandbox every effect runs.
    pub(super) fn admit(&self, effect: Effect) -> Option<Effect> {
        let Some(sandbox) = &self.sandbox else { return Some(effect) };
        // Every variant is listed, so a new effect has to be placed in one of these groups.
        let run = match &effect {
            Effect::LoadRepo { .. }
            | Effect::LoadStatus { .. }
            | Effect::CountWorktreeChanges(_)
            | Effect::LoadDetail { .. }
            | Effect::LoadCompare { .. }
            | Effect::LoadStacked(_)
            | Effect::LoadWorkdirCompare { .. }
            | Effect::LoadDiff { .. }
            | Effect::HighlightDiff { .. }
            | Effect::LoadDiffGaps { .. }
            | Effect::LoadBinarySides { .. }
            | Effect::LoadConflict { .. }
            | Effect::LocalOp { .. }
            | Effect::ReadConflictFiles { .. }
            | Effect::Revert { .. }
            | Effect::RestoreFile { .. }
            | Effect::WorkFile { .. }
            | Effect::LoadTrackedPaths { .. }
            | Effect::Submodule(_)
            | Effect::CheckWorktreeFolder(_)
            | Effect::CheckCloneParent { .. }
            | Effect::DetectLfs { .. }
            | Effect::RemoveWorktree(_)
            | Effect::MoveWorktree(_)
            | Effect::Stash(_)
            | Effect::LoadLfs { .. }
            | Effect::LoadSparse { .. }
            | Effect::SparseCheckout { .. }
            | Effect::LoadFileHistory { .. }
            | Effect::LoadFileText(_)
            | Effect::LoadBlame(_)
            | Effect::RunGit { .. }
            | Effect::Pick(_)
            | Effect::Rebase(_)
            // Journeys push to local bare repositories.
            | Effect::Stack(_)
            | Effect::UndoRedo { .. }
            | Effect::Stats { .. }
            | Effect::LineCounts(_)
            | Effect::SearchPaths { .. }
            | Effect::Index(_)
            | Effect::VerifySignature { .. }
            | Effect::Prefs(PrefsEffect::ReadGitConfig { .. } | PrefsEffect::SetGitConfig { global: false, .. })
            | Effect::CheckRepoFolders(_)
            | Effect::ReadTabGroups(_)
            | Effect::Patch(PatchEffect::Export { .. } | PatchEffect::Inspect { .. } | PatchEffect::Copy { .. })
            | Effect::CheckConflicts(_)
            | Effect::OpPreview(_)
            | Effect::Journal(_)
            | Effect::RangeDiff(_)
            | Effect::Clone(CloneEffect::Sparse { .. })
            | Effect::Init(_)
            | Effect::Trust(_)
            | Effect::GitEnv(GitEnvEffect::Detect { .. } | GitEnvEffect::Discover { .. } | GitEnvEffect::CheckPathGit { .. })
            | Effect::Watch(_)
            | Effect::AnswerPrompt { .. }
            | Effect::Partial(crate::effect::partial::PartialEffect::FetchContent(_))
            | Effect::StopGit(_)
            // A repository's own config, read or (asked first) its identity removed.
            | Effect::Profile(ProfileEffect::ReadIdentity { .. } | ProfileEffect::ClearLocalIdentity { .. })
            // A no-op: the log worker is never attached in a test.
            | Effect::Logs(_)
            // Handled by the shell; it never gets here.
            | Effect::Ui(_) => true,
            Effect::SaveSettings(_) | Effect::SaveRecent(_) | Effect::SaveTabs { .. } => {
                if let Some(msg) = save_into(&sandbox.files, effect)
                    && self.tx.send(msg).is_ok()
                {
                    self.waker.wake();
                }
                return None;
            }
            Effect::Forge { .. }
            | Effect::Account(_)
            | Effect::Credential(_)
            | Effect::FetchAvatars(_)
            | Effect::Prefs(
                PrefsEffect::SetGitConfig { global: true, .. }
                | PrefsEffect::ListSigningKeys { .. }
                | PrefsEffect::GenerateGpgKey { .. }
                | PrefsEffect::ExportPublicKey { .. },
            )
            | Effect::Os(_)
            | Effect::Patch(PatchEffect::PickSave { .. } | PatchEffect::PickOpen { .. })
            | Effect::Clone(CloneEffect::ListRepos { .. })
            | Effect::Ssh(_)
            // The global git config, Glowfly's config folder or a picked folder.
            | Effect::Profile(
                ProfileEffect::Apply { .. }
                | ProfileEffect::Rewrite { .. }
                | ProfileEffect::CheckFolder { .. }
                | ProfileEffect::SyncIncludes { .. },
            )
            | Effect::Tool(_)
            // Reads the user's config folder; runs the OS shell.
            | Effect::CustomCommand(_)
            | Effect::Theme(_)
            | Effect::Onboarding(_)
            | Effect::Notify(_)
            | Effect::PullRequest(_)
            | Effect::DeepLink(_)
            | Effect::Dashboard(_)
            | Effect::Issue(_)
            | Effect::Updates(_)
            | Effect::Remote(_)
            | Effect::Repos(_)
            // May ask a hosting service.
            | Effect::Partial(crate::effect::partial::PartialEffect::SizeHistory(_))
            | Effect::Terminal(_)
            | Effect::GitEnv(GitEnvEffect::AddSafeDirectory(_) | GitEnvEffect::Use { .. } | GitEnvEffect::PickProgram) => false,
        };
        if run {
            return Some(effect);
        }
        tracing::debug!("sandbox: dropped {}", effect.name());
        lock(&sandbox.dropped).push(effect.name());
        None
    }
}

/// `saves::save_now`, writing into `folder`.
fn save_into(folder: &Path, effect: Effect) -> Option<Msg> {
    let (what, result) = match effect {
        Effect::SaveSettings(settings) => ("settings", settings.save_to(&folder.join("settings.json"))),
        Effect::SaveRecent(list) => ("the recent repositories", crate::platform::save_recent_to(&folder.join("recent.txt"), &list)),
        Effect::SaveTabs { paths, labels, active } => {
            ("the open tabs", crate::platform::save_tabs_to(&folder.join("tabs.txt"), &paths, &labels, active))
        }
        _ => return None,
    };
    let e = result.err()?;
    Some(Msg::Logged { ok: false, text: format!("Could not save {what}: {e}") })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect::{DesktopNote, NoteKind, OsEffect};
    use std::sync::mpsc;

    #[derive(Clone)]
    struct NoWake;
    impl Waker for NoWake {
        fn wake(&self) {}
    }

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
        let runner = Runner::sandboxed(tx, NoWake, dir.clone());
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
    fn patch_pickers_and_the_service_repo_list_are_dropped() {
        let dir = folder("split");
        let (tx, _rx) = mpsc::channel();
        let runner = Runner::sandboxed(tx, NoWake, dir.clone());
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
        let sparse = CloneEffect::Sparse { parent: dir.clone(), cmd: Vec::new(), dest: dir.join("c"), folders: Vec::new() };
        assert!(runner.admit(Effect::Clone(sparse)).is_some());
        assert_eq!(runner.dropped(), ["pick-patch-save", "pick-patch", "list-my-repos"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn repository_identity_runs_and_folder_rules_are_dropped() {
        use glowfly_core::profile_includes::IncludePlan;
        let dir = folder("profiles");
        let (tx, _rx) = mpsc::channel();
        let runner = Runner::sandboxed(tx, NoWake, dir.clone());
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
        use crate::custom_commands::{CustomCommandEffect, ShellRun};
        let dir = folder("custom");
        let (tx, _rx) = mpsc::channel();
        let runner = Runner::sandboxed(tx, NoWake, dir.clone());
        assert!(runner.admit(Effect::CustomCommand(CustomCommandEffect::Load { generation: 1, workdir: dir.clone() })).is_none());
        let shell = CustomCommandEffect::RunShell(ShellRun { cwd: dir.clone(), script: "true".into() });
        assert!(runner.admit(Effect::CustomCommand(shell)).is_none());
        assert_eq!(runner.dropped(), ["load-custom-commands", "run-custom-command"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn saves_are_written_into_the_folder_before_admit_returns() {
        let dir = folder("saves");
        let (tx, rx) = mpsc::channel();
        let runner = Runner::sandboxed(tx, NoWake, dir.clone());
        let repo = dir.join("repo");
        assert!(runner.admit(Effect::SaveRecent(vec![repo.clone()])).is_none());
        let tabs = Effect::SaveTabs { paths: vec![repo.clone()], labels: vec![Some("Web".into())], active: 0 };
        assert!(runner.admit(tabs).is_none());
        let settings = glowfly_core::settings::Settings { auto_fetch_minutes: 7, ..Default::default() };
        assert!(runner.admit(Effect::SaveSettings(Box::new(settings))).is_none());
        let recent = std::fs::read_to_string(dir.join("recent.txt")).unwrap();
        assert_eq!(crate::platform::parse_recent(&recent), std::slice::from_ref(&repo));
        let tabs = std::fs::read_to_string(dir.join("tabs.txt")).unwrap();
        assert_eq!(crate::platform::parse_tabs(&tabs), (vec![repo], vec![Some("Web".to_owned())], 0));
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
