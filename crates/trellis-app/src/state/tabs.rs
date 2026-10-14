//! Open repository tabs. The active tab's data lives directly in `AppState` (where views read
//! it); every other tab keeps its data parked here until it is selected again.

use super::undo::UndoHistory;
use super::{CommitDraft, ForgeState, Selection};
use git2::Oid;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use trellis_core::repo::{CommitDetail, DiffTarget, FileDiff, Snapshot};
use trellis_core::settings::DiffLayout;

/// Closed tabs remembered for "Reopen closed tab".
pub const MAX_CLOSED_TABS: usize = 10;

pub struct Tab {
    /// The repository shown in this tab; `None` for a new tab (the welcome screen).
    pub path: Option<PathBuf>,
    pub name: String,
    /// Everything this tab showed when it was last active. `None` for the active tab, and for
    /// a tab restored at startup that hasn't been loaded yet.
    pub parked: Option<Box<Parked>>,
}

impl Tab {
    pub fn empty() -> Self {
        Self { path: None, name: "New tab".into(), parked: None }
    }

    /// A tab for `path` whose repository loads when the tab is first selected.
    pub fn unloaded(path: PathBuf) -> Self {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string());
        Self { path: Some(path), name, parked: None }
    }

    pub fn is(&self, path: &Path) -> bool {
        self.path.as_deref().is_some_and(|p| same_path(p, path))
    }
}

/// The per-repository part of `AppState`, set aside while another tab is active.
#[derive(Default)]
pub struct Parked {
    pub snap: Option<Snapshot>,
    pub selection: Selection,
    pub detail: Option<CommitDetail>,
    pub diff: Option<(DiffTarget, FileDiff)>,
    pub draft: CommitDraft,
    pub undo: UndoHistory,
    pub forge: Option<ForgeState>,
    pub stats: HashMap<Oid, (usize, usize, usize)>,
    pub search: String,
    pub search_cursor: usize,
    pub ref_filter: String,
    pub graph_scroll: f32,
    /// First visible row of the open diff, counted in `diff_layout` (the layout setting is
    /// shared by all tabs and may change while this one is parked).
    pub diff_top_row: usize,
    pub diff_layout: DiffLayout,
}

/// Whether two paths name the same folder, ignoring separator style, trailing separators and
/// (on Windows) case. Folders that exist are compared by their real location, so a short (8.3)
/// name, a junction or a symlink matches the path libgit2 reports for the same repository.
pub fn same_path(a: &Path, b: &Path) -> bool {
    if let (Ok(a), Ok(b)) = (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        return a == b;
    }
    if cfg!(windows) {
        let norm = |p: &Path| p.components().map(|c| c.as_os_str().to_string_lossy().to_lowercase()).collect::<Vec<_>>();
        norm(a) == norm(b)
    } else {
        a.components().eq(b.components())
    }
}
