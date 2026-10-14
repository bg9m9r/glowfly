//! Open repository tabs. The active tab's data lives directly in `AppState` (where views read
//! it); every other tab keeps its data parked here until it is selected again.

use super::conflict::ConflictView;
use super::history::FileHistoryView;
use super::rebase::RebasePanel;
use super::undo::UndoHistory;
use super::{CommitDraft, ForgeState, Selection};
use git2::Oid;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use trellis_core::repo::{CommitDetail, DiffTarget, FileDiff, Snapshot};
use trellis_core::settings::{DiffLayout, DiffMode};
use trellis_core::whole_file::Expanded;

/// Closed tabs remembered for "Reopen closed tab".
pub const MAX_CLOSED_TABS: usize = 10;

pub struct Tab {
    /// The repository shown in this tab; `None` for a new tab (the welcome screen).
    pub path: Option<PathBuf>,
    pub name: String,
    /// Where `path` really is on disk (`Snapshot::real_workdir`), once the repository has loaded
    /// in this tab. Another name for the same folder (a link, a junction, a short 8.3 name)
    /// matches this rather than `path`.
    pub real: Option<PathBuf>,
    /// Everything this tab showed when it was last active. `None` for the active tab, and for
    /// a tab restored at startup that hasn't been loaded yet.
    pub parked: Option<Box<Parked>>,
}

impl Tab {
    pub fn empty() -> Self {
        Self { path: None, name: "New tab".into(), real: None, parked: None }
    }

    /// A tab for `path` whose repository loads when the tab is first selected.
    pub fn unloaded(path: PathBuf) -> Self {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string());
        Self { path: Some(path), name, real: None, parked: None }
    }

    /// Whether this tab shows the repository at `path` (as given, or where it really is).
    pub fn is(&self, path: &Path) -> bool {
        [&self.path, &self.real].into_iter().flatten().any(|p| same_path(p, path))
    }

    /// Whether this tab shows the repository `snap` was loaded from.
    pub fn shows(&self, snap: &Snapshot) -> bool {
        self.is(&snap.workdir) || self.real.as_deref().is_some_and(|r| same_path(r, &snap.real_workdir))
    }
}

/// The per-repository part of `AppState`, set aside while another tab is active.
#[derive(Default)]
pub struct Parked {
    pub snap: Option<Snapshot>,
    pub selection: Selection,
    pub detail: Option<CommitDetail>,
    pub diff: Option<(DiffTarget, std::sync::Arc<FileDiff>)>,
    /// The open conflict with its picks (a load in flight is requested again on return).
    pub conflict: Option<ConflictView>,
    /// The open file history (a request in flight is sent again on return).
    pub history: Option<FileHistoryView>,
    /// The interactive rebase panel (a list still loading is requested again on return).
    pub rebase: Option<RebasePanel>,
    pub draft: CommitDraft,
    pub undo: UndoHistory,
    pub forge: Option<ForgeState>,
    pub stats: HashMap<Oid, (usize, usize, usize)>,
    pub search: String,
    pub search_cursor: usize,
    pub ref_filter: String,
    pub graph_scroll: f32,
    /// First visible row of the open diff, counted in `diff_layout` (the layout setting is
    /// shared by all tabs and may change while this one is parked) with `diff_expanded`'s folds
    /// open.
    pub diff_top_row: usize,
    pub diff_layout: DiffLayout,
    /// The view mode `diff_top_row` was counted in (shared by all tabs, like the layout).
    pub diff_mode: DiffMode,
    /// The whole-file folds opened in the open diff.
    pub diff_expanded: Expanded,
}

/// Where the tab at index `i` ends up after moving the tab at `from` to `to`.
pub fn index_after_move(i: usize, from: usize, to: usize) -> usize {
    if i == from {
        to
    } else if from < i && i <= to {
        i - 1
    } else if to <= i && i < from {
        i + 1
    } else {
        i
    }
}

/// Whether two paths are spelled as the same folder, ignoring separator style, `.` parts,
/// trailing separators, the verbatim `\\?\` prefix of resolved Windows paths and (on Windows
/// and macOS, whose file systems ignore it by default) case. Pure: nothing is resolved on disk, so callers
/// compare paths that were resolved when they were loaded (`Snapshot::real_workdir`, `Tab::real`).
pub fn same_path(a: &Path, b: &Path) -> bool {
    parts(a) == parts(b)
}

fn parts(path: &Path) -> Vec<String> {
    use std::path::{Component, Prefix};
    let fold = |s: String| if cfg!(any(windows, target_os = "macos")) { s.to_lowercase() } else { s };
    path.components()
        .filter(|c| !matches!(c, Component::CurDir))
        .map(|c| match c {
            Component::Prefix(p) => match p.kind() {
                Prefix::Disk(d) | Prefix::VerbatimDisk(d) => format!("{}:", (d as char).to_ascii_lowercase()),
                Prefix::UNC(s, h) | Prefix::VerbatimUNC(s, h) => fold(format!("//{}/{}", s.to_string_lossy(), h.to_string_lossy())),
                _ => fold(p.as_os_str().to_string_lossy().into_owned()),
            },
            other => fold(other.as_os_str().to_string_lossy().into_owned()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::same_path;
    use std::path::Path;

    #[test]
    fn same_path_compares_spelling_only() {
        let p = |s: &str| Path::new(s).to_path_buf();
        assert!(same_path(&p("work/repo/"), &p("work/./repo")));
        assert!(!same_path(&p("work/repo"), &p("work/other")));
        // Case is ignored where the file system ignores it by default.
        assert_eq!(same_path(&p("work/Repo"), &p("work/repo")), cfg!(any(windows, target_os = "macos")));
        #[cfg(windows)]
        {
            assert!(same_path(&p(r"C:\work\repo"), &p("c:/work/repo")));
            assert!(same_path(&p(r"\\?\C:\work\repo"), &p(r"C:\work\repo")), "a resolved path matches its plain form");
            assert!(same_path(&p(r"\\?\UNC\server\share\repo"), &p(r"\\Server\share\repo")));
        }
    }
}
