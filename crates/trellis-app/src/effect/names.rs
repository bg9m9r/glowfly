//! Short names of effects, for the worker threads and the debug log.

use super::{Effect, OpPreviewJob};

impl Effect {
    /// A short name for the worker thread and the debug log.
    pub fn name(&self) -> &'static str {
        match self {
            Effect::LoadRepo { .. } => "load-repo",
            Effect::LoadStatus { .. } => "load-status",
            Effect::LoadDetail { .. } => "load-detail",
            Effect::LoadCompare { .. } => "load-compare",
            Effect::LoadStacked(_) => "load-stacked",
            Effect::LoadWorkdirCompare { .. } => "load-workdir-compare",
            Effect::LoadDiff { .. } => "load-diff",
            Effect::HighlightDiff { .. } => "highlight",
            Effect::LoadDiffGaps { .. } => "load-diff-gaps",
            Effect::LoadBinarySides { .. } => "binary-sides",
            Effect::LoadConflict { .. } => "load-conflict",
            Effect::LoadFileHistory { .. } => "file-history",
            Effect::LoadBlame(_) => "blame",
            Effect::LoadFileText(_) => "file-text",
            Effect::LocalOp { .. } => "local-op",
            Effect::ReadConflictFiles { .. } => "conflict-files",
            Effect::Revert { .. } => "revert",
            Effect::RestoreFile { .. } => "restore-file",
            Effect::WorkFile { .. } => "work-file",
            Effect::LoadTrackedPaths { .. } => "tracked-paths",
            Effect::Submodule(e) => e.name(),
            Effect::CheckWorktreeFolder(_) => "check-worktree-folder",
            Effect::CheckCloneParent { .. } => "check-clone-parent",
            Effect::DetectLfs { .. } => "lfs-detect",
            Effect::RemoveWorktree(_) => "remove-worktree",
            Effect::MoveWorktree(_) => "move-worktree",
            Effect::CountWorktreeChanges(_) => "count-worktree-changes",
            Effect::Stash(e) => e.name(),
            Effect::LoadLfs { .. } => "lfs-status",
            Effect::LoadSparse { .. } => "sparse-status",
            Effect::SparseCheckout { .. } => "sparse-checkout",
            Effect::RunGit { .. } => "git",
            Effect::Pick(_) => "pick",
            Effect::UndoRedo { .. } => "undo-redo",
            Effect::Rebase(e) => e.name(),
            Effect::Forge { .. } => "forge",
            Effect::Account(e) => e.name(),
            Effect::Stats { .. } => "stats",
            Effect::LineCounts(e) => e.name(),
            Effect::SearchPaths { .. } => "search-paths",
            Effect::Index(e) => e.name(),
            Effect::VerifySignature { .. } => "verify-signature",
            Effect::FetchAvatars(_) => "avatars",
            Effect::SaveSettings(_) => "save-settings",
            Effect::SaveRecent(_) => "save-recent",
            Effect::CheckRepoFolders(_) => "check-repo-folders",
            Effect::ReadTabGroups(_) => "read-tab-groups",
            Effect::SaveTabs { .. } => "save-tabs",
            Effect::Stack(e) => e.name(),
            Effect::Os(e) => e.name(),
            Effect::Prefs(e) => e.name(),
            Effect::Ssh(e) => e.name(),
            Effect::Profile(e) => e.name(),
            Effect::Watch(_) => "watch",
            Effect::AnswerPrompt { .. } => "answer-prompt",
            Effect::Credential(e) => e.name(),
            Effect::Terminal(_) => "terminal",
            Effect::Patch(p) => p.name(),
            Effect::Tool(t) => t.name(),
            Effect::CustomCommand(e) => e.name(),
            Effect::GitEnv(e) => e.name(),
            Effect::Onboarding(e) => e.name(),
            Effect::Theme(e) => e.name(),
            Effect::CheckConflicts(_) => "check-conflicts",
            Effect::OpPreview(_) => OpPreviewJob::name(),
            Effect::Journal(e) => e.name(),
            Effect::RangeDiff(_) => "range-diff",
            Effect::Remote(e) => e.name(),
            Effect::Clone(e) => e.name(),
            Effect::Init(e) => e.name(),
            Effect::Notify(_) => "desktop-note",
            Effect::Logs(_) => "logs",
            Effect::PullRequest(e) => e.name(),
            Effect::DeepLink(e) => e.name(),
            Effect::Dashboard(e) => e.name(),
            Effect::Issue(e) => e.name(),
            Effect::Updates(e) => e.name(),
            Effect::Repos(e) => e.name(),
            Effect::Trust(e) => e.name(),
            Effect::Partial(super::partial::PartialEffect::SizeHistory(_)) => "partial-size",
            Effect::Partial(super::partial::PartialEffect::FetchContent(_)) => "partial-content",
            Effect::StopGit(_) => "stop-git",
            Effect::Ui(_) => "ui",
        }
    }

    /// Whether the runner times this effect for the performance log (runner/mod.rs).
    pub fn timed(&self) -> bool {
        match self {
            Effect::Index(e) => e.timed(),
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
            | Effect::LoadFileHistory { .. }
            | Effect::LoadBlame(_)
            | Effect::LoadFileText(_)
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
            | Effect::RunGit { .. }
            | Effect::Pick(_)
            | Effect::UndoRedo { .. }
            | Effect::Rebase(_)
            | Effect::Stack(_)
            | Effect::Forge { .. }
            | Effect::Account(_)
            | Effect::Stats { .. }
            | Effect::LineCounts(_)
            | Effect::SearchPaths { .. }
            | Effect::VerifySignature { .. }
            | Effect::FetchAvatars(_)
            | Effect::SaveSettings(_)
            | Effect::SaveRecent(_)
            | Effect::CheckRepoFolders(_)
            | Effect::ReadTabGroups(_)
            | Effect::SaveTabs { .. }
            | Effect::Profile(_)
            | Effect::GitEnv(_)
            | Effect::Onboarding(_)
            | Effect::Theme(_)
            | Effect::CheckConflicts(_)
            | Effect::OpPreview(_)
            | Effect::Journal(_)
            | Effect::RangeDiff(_)
            | Effect::Remote(_)
            | Effect::Clone(_)
            | Effect::Init(_)
            | Effect::Trust(_)
            | Effect::PullRequest(_)
            | Effect::DeepLink(_)
            | Effect::Issue(_) => true,
            // Some of their variants wait on the user (a file dialog, a passphrase prompt).
            Effect::Ssh(e) => e.timed(),
            Effect::Patch(p) => p.timed(),
            Effect::Dashboard(e) => e.timed(),
            Effect::Updates(e) => e.timed(),
            Effect::Repos(e) => e.timed(),
            Effect::CustomCommand(e) => e.timed(),
            Effect::Prefs(e) => e.timed(),
            // They wait on the user, hand work to the system, or never run on a worker thread.
            Effect::Os(_)
            | Effect::Watch(_)
            | Effect::AnswerPrompt { .. }
            // A keychain may ask the user to allow saving the sign-in.
            | Effect::Credential(_)
            | Effect::Terminal(_)
            | Effect::Tool(_)
            | Effect::Notify(_)
            | Effect::Logs(_)
            // A network call and a download the user may stop; the stop never runs on a worker.
            | Effect::Partial(_)
            | Effect::StopGit(_)
            | Effect::Ui(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Effect;
    use crate::patch::PatchEffect;
    use crate::ssh::SshEffect;
    use std::path::PathBuf;

    #[test]
    fn effects_that_wait_on_the_user_are_not_timed() {
        let dir = PathBuf::from("repo");
        let oids = vec![git2::Oid::zero()];
        let what = trellis_core::patch_ops::PatchSource::Commits { oids: oids.clone(), paths: Vec::new() };
        let pick_save = PatchEffect::PickSave { workdir: dir.clone(), what: what.clone(), name: "a.patch".into() };
        assert!(!Effect::Patch(pick_save).timed());
        assert!(!Effect::Patch(PatchEffect::PickOpen { workdir: dir.clone() }).timed());
        assert!(Effect::Patch(PatchEffect::Export { workdir: dir.clone(), what, dest: dir.join("a.patch") }).timed());
        let inspect = PatchEffect::Inspect { workdir: dir.clone(), path: dir.join("a.patch") };
        assert_eq!(inspect.name(), "inspect-patch");
        assert!(Effect::Patch(inspect).timed());
        let copy = PatchEffect::Copy { workdir: dir.clone(), oids };
        assert_eq!(copy.name(), "copy-patch");
        assert!(Effect::Patch(copy).timed());
        assert!(!Effect::Ssh(SshEffect::AddToAgent { ssh_add: "ssh-add".into(), key: dir.join("id") }).timed());
        assert!(Effect::Ssh(SshEffect::Agent { generation: 1 }).timed());
        let windows = SshEffect::SetWindowsSsh { program: None };
        assert_eq!(windows.name(), "ssh-windows");
        assert!(Effect::Ssh(windows).timed());
        assert!(Effect::Ssh(SshEffect::List { generation: 1, extra: Vec::new() }).timed());
        let pick = SshEffect::PickKey { title: "Choose a private key".into(), start: dir.clone(), keygen: None };
        assert_eq!(pick.name(), "ssh-pick-key");
        assert!(!Effect::Ssh(pick).timed(), "waits on the user");
    }

    #[test]
    fn custom_command_effects_are_named_and_timed() {
        use crate::custom_commands::{CustomCommandEffect, ShellRun};
        let load = Effect::CustomCommand(CustomCommandEffect::Load { generation: 1, workdir: PathBuf::from("repo") });
        assert_eq!((load.name(), load.timed()), ("load-custom-commands", true));
        let shell = Effect::CustomCommand(CustomCommandEffect::RunShell(ShellRun { cwd: PathBuf::from("repo"), script: "make".into() }));
        assert_eq!((shell.name(), shell.timed()), ("run-custom-command", true));
    }
}
