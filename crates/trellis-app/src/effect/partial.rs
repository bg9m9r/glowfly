//! The effects of a shallow clone's size check and a partial clone's missing contents
//! (runner/partial.rs), and the keys of the git commands that can be stopped (`Effect::StopGit`).

use super::Effect;
use std::path::PathBuf;
use trellis_core::forge::{Auth, Target};

#[derive(Debug)]
pub enum PartialEffect {
    /// How much fetching the full history may download (`PartialMsg::HistorySized`).
    SizeHistory(HistorySizeRequest),
    /// Downloads a partial clone's missing contents in the background, without prompts
    /// (`PartialMsg::ContentFetched`); stopped by `StopGit(StopKey::Content(id))`.
    FetchContent(ContentRequest),
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

/// Which running git commands `Effect::StopGit` stops.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StopKey {
    /// The active job's (a `RunGit` that only fetches).
    Job,
    /// A background download of missing contents.
    Content(u64),
}

impl Effect {
    /// The key a runner registers this effect's stop under; None when it can't be stopped.
    pub fn stop_key(&self) -> Option<StopKey> {
        match self {
            Effect::RunGit { stoppable: true, .. } => Some(StopKey::Job),
            Effect::Partial(PartialEffect::FetchContent(r)) => Some(StopKey::Content(r.id)),
            _ => None,
        }
    }
}
