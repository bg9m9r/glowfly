//! Shallow clones: which remote more history comes from, whether it can be fetched now, and the
//! graph header's explanation.

use super::AppState;
use trellis_core::repo::Snapshot;

/// The remote to fetch more history from: the checked-out branch's upstream remote, else
/// origin, else the first configured remote. None without remotes.
pub fn history_remote(snap: &Snapshot) -> Option<String> {
    if let Some(t) = snap.head_info().and_then(|b| b.tracking.as_ref()).filter(|t| t.is_remote())
        && snap.remote_info.iter().any(|r| r.name == t.remote)
    {
        return Some(t.remote.clone());
    }
    let names = || snap.remote_info.iter().map(|r| r.name.as_str());
    names().find(|n| *n == "origin").or_else(|| names().next()).map(str::to_owned)
}

impl AppState {
    /// Whether the open repository is shallow and has a remote to fetch the rest from.
    pub fn can_fetch_history(&self) -> bool {
        self.snap.as_ref().is_some_and(|s| s.is_shallow() && history_remote(s).is_some())
    }
}

/// The tooltip of the graph header's "Shallow clone" note and "Fetch full history" link.
pub fn header_tooltip(snap: &Snapshot) -> String {
    let n = snap.shallow_roots.len();
    let commits = if n == 1 { "1 commit".to_owned() } else { format!("{n} commits") };
    let rest = match history_remote(snap) {
        Some(r) => format!("Fetch full history downloads the rest from {r}. On a large repository that can take a while."),
        None => "There is no remote to download the rest from.".to_owned(),
    };
    format!("This is a shallow clone: its history stops at {commits} whose parents weren't downloaded. {rest}")
}
