//! Messages of a shallow clone's size check before Fetch full history (update/partial_size.rs)
//! and of a partial clone's missing file contents (update/partial_content.rs).

use super::Inactive;
use crate::effect::Effect;
use glowfly_core::history_size::HistorySize;
use glowfly_core::partial::MissingContent;
use std::path::PathBuf;

pub enum PartialMsg {
    /// The size check `id` for the repository at `workdir` ended.
    HistorySized { workdir: PathBuf, id: u64, size: HistorySize },
    /// A diff, File view or blame read needs contents the partial clone hasn't downloaded;
    /// `retry` is the same read, run again once they are downloaded.
    ContentMissing { missing: MissingContent, retry: Box<Effect> },
    /// A waiting pane's button: download the missing contents as a job (through the trust gate).
    DownloadContent,
    /// The downloading pane's Cancel (a background download; a job is cancelled with
    /// `JobMsg::Cancel`).
    StopContent,
    /// Background download `id` ended.
    ContentFetched { id: u64, result: Result<(), ContentFailure> },
}

/// Why a background download didn't finish.
#[derive(Clone, Debug, PartialEq)]
pub enum ContentFailure {
    Stopped,
    /// git's last meaningful line.
    Failed(String),
}

impl PartialMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`): nothing,
    /// the views read again (and ask again) when their tab returns.
    pub fn on_inactive(self) -> Option<Inactive> {
        match self {
            Self::HistorySized { .. }
            | Self::ContentMissing { .. }
            | Self::DownloadContent
            | Self::StopContent
            | Self::ContentFetched { .. } => None,
        }
    }
}
