//! Which repository an effect's result belongs to (`Effect::scope`).

use super::Effect;
use std::path::PathBuf;

impl Effect {
    /// The repository whose tab a result belongs to: results for a tab that is no longer
    /// active are dropped (the tab reloads when it is selected again).
    pub fn scope(&self) -> Option<&PathBuf> {
        match self {
            Effect::LoadStatus { workdir, .. }
            | Effect::LoadDetail { workdir, .. }
            | Effect::LoadCompare { workdir, .. }
            | Effect::LoadWorkdirCompare { workdir, .. }
            | Effect::LoadDiff { workdir, .. }
            | Effect::HighlightDiff { workdir, .. }
            | Effect::LoadConflict { workdir, .. }
            | Effect::LoadFileHistory { workdir, .. }
            | Effect::LoadTrackedPaths { workdir, .. }
            | Effect::LoadLfs { workdir, .. }
            | Effect::LoadSparse { workdir, .. }
            | Effect::LocalOp { workdir, .. }
            | Effect::ReadConflictFiles { workdir, .. }
            | Effect::Revert { workdir, .. }
            | Effect::RestoreFile { workdir, .. }
            | Effect::WorkFile { workdir, .. }
            | Effect::Stats { workdir, .. }
            | Effect::SearchPaths { workdir, .. }
            | Effect::VerifySignature { workdir, .. } => Some(workdir),
            Effect::Forge { cwd, .. } => Some(cwd),
            Effect::CheckConflicts(j) => Some(&j.workdir),
            Effect::LoadBlame(r) => Some(&r.workdir),
            Effect::LoadFileText(r) => Some(&r.workdir),
            Effect::Submodule(e) => e.scope(),
            Effect::Rebase(e) => e.scope(),
            Effect::PullRequest(e) => e.scope(),
            Effect::DeepLink(e) => e.scope(),
            Effect::Issue(e) => e.scope(),
            // Unscoped: global answers, jobs whose result must reach `update` whichever tab is
            // active, and work outside any repository.
            Effect::LoadRepo { .. }
            | Effect::CheckWorktreeFolder(_)
            | Effect::CheckCloneParent { .. }
            | Effect::DetectLfs { .. }
            | Effect::RemoveWorktree(_)
            | Effect::MoveWorktree(_)
            | Effect::RenameStash { .. }
            | Effect::SparseCheckout { .. }
            | Effect::RunGit { .. }
            | Effect::UndoRedo { .. }
            | Effect::RepoOwners { .. }
            | Effect::CreateRepo { .. }
            | Effect::Connect { .. }
            | Effect::SaveToken { .. }
            | Effect::DeleteToken(_)
            | Effect::ForgetCredentialManager { .. }
            | Effect::TrelloBoards { .. }
            | Effect::FetchAvatars(_)
            | Effect::ReadGitConfig { .. }
            | Effect::SetGitConfig { .. }
            | Effect::SaveSettings(_)
            | Effect::SaveRecent(_)
            | Effect::CheckRepoFolders(_)
            | Effect::ReadTabGroups(_)
            | Effect::SaveTabs { .. }
            | Effect::OpenUrl(_)
            | Effect::OpenTerminal(_)
            | Effect::OpenFolder(_)
            | Effect::OpenPath { .. }
            | Effect::RevealPath(_)
            | Effect::PickFolder { .. }
            | Effect::PickFile { .. }
            | Effect::ListSigningKeys { .. }
            | Effect::GenerateGpgKey { .. }
            | Effect::ExportPublicKey { .. }
            | Effect::Ssh(_)
            | Effect::Profile(_)
            | Effect::Watch(_)
            | Effect::AnswerPrompt { .. }
            | Effect::Terminal(_)
            | Effect::Patch(_)
            | Effect::Tool(_)
            | Effect::GitEnv(_)
            | Effect::Onboarding(_)
            | Effect::Theme(_)
            | Effect::Remote(_)
            | Effect::Clone(_)
            | Effect::Init(_)
            | Effect::Notify(_)
            | Effect::Logs(_)
            | Effect::Dashboard(_)
            | Effect::Updates(_)
            | Effect::Repos(_)
            | Effect::Ui(_) => None,
        }
    }
}
