//! Which repository an effect's result belongs to (`Effect::scope`), which repository's queue it
//! runs on (`Effect::queue`) and which it keeps out of the repository pool (`Effect::holds`). An
//! area's answers come from its `EffectMeta`; the flat variants' are here.

use super::areas::{effect_areas, effect_match};
use super::{Effect, EffectMeta};
use std::path::{Path, PathBuf};

impl Effect {
    /// The repository whose tab a result belongs to: results for a tab that is no longer
    /// active are dropped (the tab reloads when it is selected again).
    pub fn scope(&self) -> Option<&PathBuf> {
        effect_areas! { effect_match! { self, |e| e.scope(), {
            Effect::LoadStatus { workdir, .. }
            | Effect::LoadIndexStatus { workdir, .. }
            | Effect::LoadDetail { workdir, .. }
            | Effect::LoadDiff { workdir, .. }
            | Effect::HighlightDiff { workdir, .. }
            | Effect::LoadDiffGaps { workdir, .. }
            | Effect::LoadBinarySides { workdir, .. }
            | Effect::LoadFileHistory { workdir, .. }
            | Effect::LoadTrackedPaths { workdir, .. }
            | Effect::LocalOp { workdir, .. }
            | Effect::Revert { workdir, .. }
            | Effect::RestoreFile { workdir, .. }
            | Effect::WorkFile { workdir, .. }
            | Effect::Stats { workdir, .. }
            | Effect::SearchPaths { workdir, .. }
            | Effect::VerifySignature { workdir, .. } => Some(workdir),
            Effect::Forge { cwd, .. } => Some(cwd),
            // Unscoped: global answers, jobs whose result must reach `update` whichever tab is
            // active, and work outside any repository.
            Effect::LoadRepo { .. }
            | Effect::RunGit { .. }
            | Effect::UndoRedo { .. }
            | Effect::FetchAvatars(_)
            | Effect::SaveSettings(_)
            | Effect::SaveRecent(_)
            | Effect::CheckRepoFolders(_)
            | Effect::ReadTabGroups(_)
            | Effect::SaveTabs { .. }
            | Effect::Watch(_)
            | Effect::SweepWatch { .. }
            | Effect::AnswerPrompt { .. }
            | Effect::StopGit(_) => None,
        } } }
    }

    /// The repository whose per-repository queue this effect runs on (runner/queue.rs), when it
    /// can change that repository.
    pub fn queue(&self) -> Option<&Path> {
        effect_areas! { effect_match! { self, |e| e.queue(), {
            Effect::LocalOp { workdir, .. }
            | Effect::Revert { workdir, .. }
            | Effect::RestoreFile { workdir, .. }
            | Effect::WorkFile { workdir, .. } => Some(workdir),
            Effect::RunGit { cmds, .. } if cmds.iter().all(|c| network_only(c)) => None,
            Effect::RunGit { cwd, .. } | Effect::UndoRedo { cwd, .. } => Some(cwd),
            // Reads, network calls, Glowfly's own files (ordered by `saves`) and work outside the
            // process. Listed one by one so a new effect has to decide.
            Effect::LoadRepo { .. }
            | Effect::LoadStatus { .. }
            | Effect::LoadIndexStatus { .. }
            | Effect::LoadDetail { .. }
            | Effect::LoadDiff { .. }
            | Effect::LoadDiffGaps { .. }
            | Effect::LoadBinarySides { .. }
            | Effect::HighlightDiff { .. }
            | Effect::LoadFileHistory { .. }
            | Effect::LoadTrackedPaths { .. }
            | Effect::CheckRepoFolders(_)
            | Effect::ReadTabGroups(_)
            | Effect::Forge { .. }
            | Effect::Stats { .. }
            | Effect::SearchPaths { .. }
            | Effect::VerifySignature { .. }
            | Effect::FetchAvatars(_)
            | Effect::SaveSettings(_)
            | Effect::SaveRecent(_)
            | Effect::SaveTabs { .. }
            | Effect::Watch(_)
            | Effect::SweepWatch { .. }
            | Effect::AnswerPrompt { .. }
            | Effect::StopGit(_) => None,
        } } }
    }

    /// The repository this effect keeps out of the repository pool while it runs
    /// (`repo_pool::hold`): one it can change, and a fetch's or a push's too, after which git may
    /// repack. On Windows an open handle keeps the old packs from being deleted.
    pub fn holds(&self) -> Option<&Path> {
        effect_areas! { effect_match! { self, |e| e.holds(), {
            Effect::RunGit { cwd, .. } => Some(cwd),
            flat => flat.queue(),
        } } }
    }
}

/// Fetch and push update remote-tracking refs at most; they never touch the index or files.
fn network_only(cmd: &[String]) -> bool {
    matches!(glowfly_core::gitcli::subcommand(cmd), Some("fetch" | "push")) && !cmd.iter().any(|a| a == ".")
}
