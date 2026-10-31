//! What the whole-app tests' runner (runner/sandbox.rs) does with each effect: runs it, writes
//! a save into the test's folder, or drops it.

use super::areas::{effect_areas, effect_match};
use super::{Effect, EffectMeta, SandboxRule};

impl Effect {
    /// What the whole-app tests' runner does with it (runner/sandbox.rs): an area's rule comes
    /// from its `EffectMeta`; every flat variant is listed, so a new one has to be placed.
    pub fn sandbox(&self) -> SandboxRule {
        effect_areas! { effect_match! { self, |e| e.sandbox(), {
            Effect::LoadRepo { .. }
            | Effect::LoadStatus { .. }
            | Effect::LoadIndexStatus { .. }
            | Effect::LoadDetail { .. }
            | Effect::LoadDiff { .. }
            | Effect::HighlightDiff { .. }
            | Effect::LoadDiffGaps { .. }
            | Effect::LoadBinarySides { .. }
            | Effect::LocalOp { .. }
            | Effect::Revert { .. }
            | Effect::RestoreFile { .. }
            | Effect::WorkFile { .. }
            | Effect::LoadTrackedPaths { .. }
            | Effect::LoadFileHistory { .. }
            | Effect::RunGit { .. }
            | Effect::UndoRedo { .. }
            | Effect::Stats { .. }
            | Effect::SearchPaths { .. }
            | Effect::VerifySignature { .. }
            | Effect::CheckRepoFolders(_)
            | Effect::ReadTabGroups(_)
            | Effect::Watch(_)
            | Effect::SweepWatch { .. }
            | Effect::AnswerPrompt { .. }
            | Effect::StopGit(_) => SandboxRule::Run,
            Effect::SaveSettings(_) | Effect::SaveRecent(_) | Effect::SaveTabs { .. } => SandboxRule::Save,
            Effect::Forge { .. } | Effect::FetchAvatars(_) => SandboxRule::Drop,
        } } }
    }
}
