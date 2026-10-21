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
            | Effect::LoadDiffGaps { workdir, .. }
            | Effect::LoadBinarySides { workdir, .. }
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
            Effect::LoadStacked(r) => Some(&r.workdir),
            Effect::OpPreview(j) => Some(&j.workdir),
            Effect::Journal(e) => Some(e.scope()),
            Effect::RangeDiff(j) => Some(&j.workdir),
            Effect::LoadBlame(r) => Some(&r.workdir),
            Effect::LoadFileText(r) => Some(&r.workdir),
            Effect::CountWorktreeChanges(r) => Some(&r.workdir),
            Effect::Submodule(e) => e.scope(),
            Effect::Rebase(e) => e.scope(),
            Effect::PullRequest(e) => e.scope(),
            Effect::DeepLink(e) => e.scope(),
            Effect::Issue(e) => e.scope(),
            Effect::LineCounts(e) => e.scope(),
            Effect::Index(e) => e.scope(),
            // Unscoped: global answers, jobs whose result must reach `update` whichever tab is
            // active, and work outside any repository.
            Effect::LoadRepo { .. }
            | Effect::CheckWorktreeFolder(_)
            | Effect::CheckCloneParent { .. }
            | Effect::DetectLfs { .. }
            | Effect::RemoveWorktree(_)
            | Effect::MoveWorktree(_)
            | Effect::Stash(_)
            | Effect::SparseCheckout { .. }
            | Effect::RunGit { .. }
            | Effect::Pick(_)
            | Effect::Stack(_)
            | Effect::UndoRedo { .. }
            | Effect::Account(_)
            | Effect::FetchAvatars(_)
            | Effect::Prefs(_)
            | Effect::SaveSettings(_)
            | Effect::SaveRecent(_)
            | Effect::CheckRepoFolders(_)
            | Effect::ReadTabGroups(_)
            | Effect::SaveTabs { .. }
            | Effect::Os(_)
            | Effect::Ssh(_)
            | Effect::Profile(_)
            | Effect::Watch(_)
            | Effect::AnswerPrompt { .. }
            | Effect::Credential(_)
            | Effect::Terminal(_)
            | Effect::Patch(_)
            | Effect::Tool(_)
            // Jobs report to whichever tab is active, like RunGit.
            | Effect::CustomCommand(_)
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
            | Effect::Trust(_)
            // The size check and the download carry their own workdir and id.
            | Effect::Partial(_)
            | Effect::StopGit(_)
            | Effect::Ui(_) => None,
        }
    }
}
