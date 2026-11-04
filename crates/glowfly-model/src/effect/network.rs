//! Calls that reach the network (`Effect::Network`, runner/network.rs): a hosting service's
//! calls (`ForgeCall`, forge.rs) and avatar downloads. The whole-app tests' runner drops both.

use super::{EffectMeta, ForgeCall, SandboxRule};
use glowfly_core::forge::{Auth, Target};
use std::path::{Path, PathBuf};

/// Large because of the call it carries, as `Effect::Forge` was before the area: an effect is
/// moved once, into its worker, so boxing it would only add an allocation.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum NetworkEffect {
    /// Asks the hosting service at `target` (`ForgeCall`), reported as `ForgeMsg::ForgeReplied`
    /// and scoped to `cwd`.
    Forge {
        target: Target,
        cwd: PathBuf,
        auth: Option<Auth>,
        interactive: bool,
        /// The user connected this account in Preferences (lets GCM sign in to a self-hosted server).
        connected: bool,
        call: ForgeCall,
    },
    /// Downloads (or reads from the disk cache) avatars one after another; unscoped.
    Avatars(Vec<String>),
}

impl EffectMeta for NetworkEffect {
    fn name(&self) -> &'static str {
        match self {
            NetworkEffect::Forge { .. } => "forge",
            NetworkEffect::Avatars(_) => "avatars",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        match self {
            NetworkEffect::Forge { cwd, .. } => Some(cwd),
            NetworkEffect::Avatars(_) => None,
        }
    }

    fn timed(&self) -> bool {
        true
    }

    /// Network calls never change a repository.
    fn queue(&self) -> Option<&Path> {
        None
    }

    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Drop
    }
}
