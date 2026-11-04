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
            Effect::Forge { cwd, .. } => Some(cwd),
            // Unscoped: global answers, jobs whose result must reach `update` whichever tab is
            // active, and work outside any repository.
            Effect::FetchAvatars(_)
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
            // Network calls and work outside the process. Listed one by one so a new effect has
            // to decide.
            Effect::Forge { .. }
            | Effect::FetchAvatars(_)
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
            flat => flat.queue(),
        } } }
    }
}
