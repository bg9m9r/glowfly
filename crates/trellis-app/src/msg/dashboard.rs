//! Messages of the cross-repository dashboard (update/dashboard.rs).

use crate::effect::FetchReply;
pub use crate::state::dashboard::DashShow;
use std::path::PathBuf;
use trellis_core::forge::dashboard::ScannedRepo;
use trellis_core::forge::{Account, Target};

/// Opening and closing the dashboard, its switch, refreshes and row actions, and the runner's
/// results (update/dashboard.rs). Never sent scoped: the dashboard belongs to no tab.
pub enum DashboardMsg {
    Open,
    Close,
    /// The Refresh button: always asks again.
    Refresh,
    /// The Mine / Review requested / Assigned switch.
    Show(DashShow),
    /// Clear filter, when nothing matches it.
    ClearFilter,
    /// Sign in to one account interactively and ask its targets again.
    SignIn(Account),
    /// Open this local clone in a tab.
    OpenRepo(PathBuf),
    /// Open the clone at `path` in a tab and show pull request `id` of `target` there.
    OpenPr {
        path: PathBuf,
        target: Target,
        id: u64,
    },
    /// The folders read for refresh `generation`, each with what was found or why it couldn't be read.
    Scanned {
        generation: u64,
        repos: Vec<(PathBuf, Result<ScannedRepo, String>)>,
    },
    Fetched(Box<FetchReply>),
}

impl DashboardMsg {
    /// What this result still needs when its tab is no longer active (`Msg::Scoped`): nothing,
    /// since none is ever scoped.
    pub fn on_inactive(self) -> Option<crate::msg::Inactive> {
        match self {
            Self::Open
            | Self::Close
            | Self::Refresh
            | Self::Show(_)
            | Self::ClearFilter
            | Self::SignIn(_)
            | Self::OpenRepo(_)
            | Self::OpenPr { .. }
            | Self::Scanned { .. }
            | Self::Fetched(_) => None,
        }
    }
}
