//! Which repository an effect's result belongs to (`Effect::scope`), which repository's queue it
//! runs on (`Effect::queue`) and which it keeps out of the repository pool (`Effect::holds`),
//! each from the area's `EffectMeta`.

use super::areas::{effect_areas, effect_match};
use super::{Effect, EffectMeta};
use std::path::{Path, PathBuf};

impl Effect {
    /// The repository whose tab a result belongs to: results for a tab that is no longer
    /// active are dropped (the tab reloads when it is selected again).
    pub fn scope(&self) -> Option<&PathBuf> {
        effect_areas! { effect_match! { self, |e| e.scope() } }
    }

    /// The repository whose per-repository queue this effect runs on (runner/queue.rs), when it
    /// can change that repository.
    pub fn queue(&self) -> Option<&Path> {
        effect_areas! { effect_match! { self, |e| e.queue() } }
    }

    /// The repository this effect keeps out of the repository pool while it runs
    /// (`repo_pool::hold`): one it can change, and a fetch's or a push's too, after which git may
    /// repack. On Windows an open handle keeps the old packs from being deleted.
    pub fn holds(&self) -> Option<&Path> {
        effect_areas! { effect_match! { self, |e| e.holds() } }
    }
}
