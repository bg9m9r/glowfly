//! What the whole-app tests' runner (runner/sandbox.rs) does with each effect: runs it, writes
//! a save into the test's folder, or drops it.

use super::areas::{effect_areas, effect_match};
use super::{Effect, EffectMeta, SandboxRule};

impl Effect {
    /// What the whole-app tests' runner does with it (runner/sandbox.rs), from the area's
    /// `EffectMeta`.
    pub fn sandbox(&self) -> SandboxRule {
        effect_areas! { effect_match! { self, |e| e.sandbox() } }
    }
}
