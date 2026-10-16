//! Short names of effects, for the worker threads and the debug log.

use super::Effect;

impl Effect {
    /// A short name for the worker thread and the debug log.
    pub fn name(&self) -> &'static str {
        match self {
            Effect::LoadRepo { .. } => "load-repo",
            Effect::LoadStatus { .. } => "load-status",
            Effect::LoadDetail { .. } => "load-detail",
            Effect::LoadCompare { .. } => "load-compare",
            Effect::LoadDiff { .. } => "load-diff",
            Effect::HighlightDiff { .. } => "highlight",
            Effect::LoadConflict { .. } => "load-conflict",
            Effect::LoadFileHistory { .. } => "file-history",
            Effect::LoadBlame(_) => "blame",
            Effect::LocalOp { .. } => "local-op",
            Effect::RevertHunk { .. } => "revert-hunk",
            Effect::RestoreFile { .. } => "restore-file",
            Effect::WorkFile { .. } => "work-file",
            Effect::LoadTrackedPaths { .. } => "tracked-paths",
            Effect::Submodule(e) => e.name(),
            Effect::CheckWorktreeFolder(_) => "check-worktree-folder",
            Effect::CheckCloneParent { .. } => "check-clone-parent",
            Effect::DetectLfs { .. } => "lfs-detect",
            Effect::RemoveWorktree(_) => "remove-worktree",
            Effect::RenameStash { .. } => "rename-stash",
            Effect::LoadLfs { .. } => "lfs-status",
            Effect::LoadSparse { .. } => "sparse-status",
            Effect::SparseCheckout { .. } => "sparse-checkout",
            Effect::RunGit { .. } => "git",
            Effect::UndoRedo { .. } => "undo-redo",
            Effect::LoadRebasePlan { .. } => "rebase-plan",
            Effect::InteractiveRebase { .. } => "rebase",
            Effect::Forge { .. } => "forge",
            Effect::Connect { .. } => "connect",
            Effect::RepoOwners { .. } => "repo-owners",
            Effect::CreateRepo { .. } => "create-repo",
            Effect::SaveToken { .. } => "save-token",
            Effect::DeleteToken(_) => "delete-token",
            Effect::ForgetCredentialManager { .. } => "forget-gcm",
            Effect::TrelloBoards { .. } => "trello-boards",
            Effect::Stats { .. } => "stats",
            Effect::SearchPaths { .. } => "search-paths",
            Effect::VerifySignature { .. } => "verify-signature",
            Effect::FetchAvatars(_) => "avatars",
            Effect::ReadGitConfig { .. } => "read-config",
            Effect::SetGitConfig { .. } => "set-config",
            Effect::SaveSettings(_) => "save-settings",
            Effect::SaveRecent(_) => "save-recent",
            Effect::CheckRepoFolders(_) => "check-repo-folders",
            Effect::SaveTabs { .. } => "save-tabs",
            Effect::OpenUrl(_) => "open-url",
            Effect::OpenTerminal(_) => "open-terminal",
            Effect::OpenFolder(_) => "open-folder",
            Effect::OpenPath { .. } => "open-path",
            Effect::RevealPath(_) => "reveal-path",
            Effect::PickFolder { .. } => "pick-folder",
            Effect::PickFile { .. } => "pick-file",
            Effect::ListSigningKeys { .. } => "list-keys",
            Effect::GenerateGpgKey { .. } => "generate-key",
            Effect::ExportPublicKey { .. } => "export-key",
            Effect::Ssh(e) => e.name(),
            Effect::Profile(e) => e.name(),
            Effect::Watch(_) => "watch",
            Effect::AnswerPrompt { .. } => "answer-prompt",
            Effect::Terminal(_) => "terminal",
            Effect::Patch(p) => p.name(),
            Effect::Tool(t) => t.name(),
            Effect::GitEnv(e) => e.name(),
            Effect::Onboarding(e) => e.name(),
            Effect::Theme(e) => e.name(),
            Effect::CheckConflicts(_) => "check-conflicts",
            Effect::Remote(e) => e.name(),
            Effect::Clone(e) => e.name(),
            Effect::Init(e) => e.name(),
            Effect::Notify(_) => "desktop-note",
            Effect::Logs(_) => "logs",
            Effect::PullRequest(e) => e.name(),
            Effect::DeepLink(e) => e.name(),
            Effect::Dashboard(e) => e.name(),
            Effect::Ui(_) => "ui",
        }
    }

    /// Whether the runner times this effect for the performance log (runner/mod.rs).
    pub fn timed(&self) -> bool {
        match self {
            Effect::LoadRepo { .. }
            | Effect::LoadStatus { .. }
            | Effect::LoadDetail { .. }
            | Effect::LoadCompare { .. }
            | Effect::LoadDiff { .. }
            | Effect::HighlightDiff { .. }
            | Effect::LoadConflict { .. }
            | Effect::LoadFileHistory { .. }
            | Effect::LoadBlame(_)
            | Effect::LocalOp { .. }
            | Effect::RevertHunk { .. }
            | Effect::RestoreFile { .. }
            | Effect::WorkFile { .. }
            | Effect::LoadTrackedPaths { .. }
            | Effect::Submodule(_)
            | Effect::CheckWorktreeFolder(_)
            | Effect::CheckCloneParent { .. }
            | Effect::DetectLfs { .. }
            | Effect::RemoveWorktree(_)
            | Effect::RenameStash { .. }
            | Effect::LoadLfs { .. }
            | Effect::LoadSparse { .. }
            | Effect::SparseCheckout { .. }
            | Effect::RunGit { .. }
            | Effect::UndoRedo { .. }
            | Effect::LoadRebasePlan { .. }
            | Effect::InteractiveRebase { .. }
            | Effect::Forge { .. }
            | Effect::Connect { .. }
            | Effect::RepoOwners { .. }
            | Effect::CreateRepo { .. }
            | Effect::SaveToken { .. }
            | Effect::DeleteToken(_)
            | Effect::ForgetCredentialManager { .. }
            | Effect::TrelloBoards { .. }
            | Effect::Stats { .. }
            | Effect::SearchPaths { .. }
            | Effect::VerifySignature { .. }
            | Effect::FetchAvatars(_)
            | Effect::ReadGitConfig { .. }
            | Effect::SetGitConfig { .. }
            | Effect::SaveSettings(_)
            | Effect::SaveRecent(_)
            | Effect::CheckRepoFolders(_)
            | Effect::SaveTabs { .. }
            | Effect::ListSigningKeys { .. }
            | Effect::ExportPublicKey { .. }
            | Effect::Profile(_)
            | Effect::GitEnv(_)
            | Effect::Onboarding(_)
            | Effect::Theme(_)
            | Effect::CheckConflicts(_)
            | Effect::Remote(_)
            | Effect::Clone(_)
            | Effect::Init(_)
            | Effect::PullRequest(_)
            | Effect::DeepLink(_) => true,
            // Some of their variants wait on the user (a file dialog, a passphrase prompt).
            Effect::Ssh(e) => e.timed(),
            Effect::Patch(p) => p.timed(),
            Effect::Dashboard(e) => e.timed(),
            // They wait on the user, hand work to the system, or never run on a worker thread.
            Effect::OpenUrl(_)
            | Effect::OpenTerminal(_)
            | Effect::OpenFolder(_)
            | Effect::OpenPath { .. }
            | Effect::RevealPath(_)
            | Effect::PickFolder { .. }
            | Effect::PickFile { .. }
            | Effect::GenerateGpgKey { .. }
            | Effect::Watch(_)
            | Effect::AnswerPrompt { .. }
            | Effect::Terminal(_)
            | Effect::Tool(_)
            | Effect::Notify(_)
            | Effect::Logs(_)
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
        let pick_save = PatchEffect::PickSave { workdir: dir.clone(), oids: oids.clone(), name: "a.patch".into() };
        assert!(!Effect::Patch(pick_save).timed());
        assert!(!Effect::Patch(PatchEffect::PickOpen { workdir: dir.clone() }).timed());
        assert!(Effect::Patch(PatchEffect::Export { workdir: dir.clone(), oids, dest: dir.join("a.patch") }).timed());
        assert!(!Effect::Ssh(SshEffect::AddToAgent { ssh_add: "ssh-add".into(), key: dir.join("id") }).timed());
        assert!(Effect::Ssh(SshEffect::Agent { generation: 1 }).timed());
        assert!(Effect::Ssh(SshEffect::List { generation: 1 }).timed());
    }
}
