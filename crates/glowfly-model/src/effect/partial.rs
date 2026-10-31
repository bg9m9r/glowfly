//! The effects of a shallow clone's size check and a partial clone's missing contents
//! (runner/partial.rs), and the keys of the git commands that can be stopped (`Effect::StopGit`).

use super::SandboxRule;
use super::areas::{effect_areas, effect_match};
use super::{Effect, EffectMeta};
use glowfly_core::forge::{Auth, Target};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum PartialEffect {
    /// How much fetching the full history may download (`PartialMsg::HistorySized`).
    SizeHistory(HistorySizeRequest),
    /// Downloads a partial clone's missing contents in the background, without prompts
    /// (`PartialMsg::ContentFetched`); stopped by `StopGit(StopKey::Content(id))`.
    FetchContent(ContentRequest),
}

impl EffectMeta for PartialEffect {
    fn name(&self) -> &'static str {
        match self {
            PartialEffect::SizeHistory(_) => "partial-size",
            PartialEffect::FetchContent(_) => "partial-content",
        }
    }

    /// The size check and the download carry their own workdir and id.
    fn scope(&self) -> Option<&PathBuf> {
        None
    }

    /// A network call and a download the user may stop.
    fn timed(&self) -> bool {
        false
    }

    /// A network call, and a fetch that only adds objects.
    fn queue(&self) -> Option<&Path> {
        None
    }

    fn stop_key(&self) -> Option<StopKey> {
        match self {
            PartialEffect::FetchContent(r) => Some(StopKey::Content(r.id)),
            PartialEffect::SizeHistory(_) => None,
        }
    }

    /// The size check may ask a hosting service.
    fn sandbox(&self) -> SandboxRule {
        match self {
            PartialEffect::FetchContent(_) => SandboxRule::Run,
            PartialEffect::SizeHistory(_) => SandboxRule::Drop,
        }
    }
}

/// The size check behind Fetch full history.
#[derive(Debug)]
pub struct HistorySizeRequest {
    pub workdir: PathBuf,
    pub id: u64,
    /// The history remote's first URL (a folder on this computer is measured on disk).
    pub url: Option<String>,
    /// The hosting service to ask, when the repository is trusted and it may be contacted.
    pub service: Option<SizeService>,
}

/// The hosting service a size is asked of.
#[derive(Debug)]
pub struct SizeService {
    pub target: Target,
    pub auth: Option<Auth>,
    /// The user connected this account in Preferences.
    pub connected: bool,
}

/// The fetch commands that download a partial clone's missing contents.
#[derive(Debug)]
pub struct ContentRequest {
    pub workdir: PathBuf,
    pub id: u64,
    pub cmds: Vec<Vec<String>>,
}

/// Which running git commands (or downloads) `Effect::StopGit` stops.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StopKey {
    /// The active job's (a `RunGit` that only fetches).
    Job,
    /// A background download of missing contents.
    Content(u64),
    /// The download of Glowfly's own update (`UpdateEffect::Download`).
    UpdateDownload,
}

impl Effect {
    /// The key a runner registers this effect's stop under; None when it can't be stopped. An
    /// area's comes from its `EffectMeta`.
    pub fn stop_key(&self) -> Option<StopKey> {
        effect_areas! { effect_match! { self, |e| e.stop_key(), {
            Effect::RunGit { stoppable: true, .. } => Some(StopKey::Job),
            _ => None,
        } } }
    }
}
