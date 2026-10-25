//! Open repository tabs. The active tab's data lives directly in `AppState` (where views read
//! it); every other tab keeps its data parked here until it is selected again.

use super::conflict::ConflictView;
use super::history::FileHistoryView;
use super::pull_request::PrView;
use super::rebase::RebasePanel;
use super::undo::UndoHistory;
use super::{CommitDraft, ForgeState, Selection};
use git2::Oid;
use glowfly_core::paths::{FolderId, same_spelling};
use glowfly_core::repo::{CommitDetail, CommonDir, DiffTarget, FileDiff, Snapshot};
use glowfly_core::settings::{DiffLayout, DiffMode};
use glowfly_core::whole_file::Expanded;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Closed tabs remembered for "Reopen closed tab".
pub const MAX_CLOSED_TABS: usize = 10;

/// The longest name a tab can be given (characters).
pub const MAX_TAB_LABEL: usize = 64;

/// A tab's identity for as long as it is open, whatever index it moves to: what a request
/// waiting for an answer (the close confirmation, the rename dialog) names it by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TabId(u64);

impl TabId {
    pub fn next() -> TabId {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        TabId(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

pub struct Tab {
    pub id: TabId,
    /// The repository shown in this tab; `None` for a new tab (the welcome screen).
    pub path: Option<PathBuf>,
    pub name: String,
    /// Where `path` really is on disk (`Snapshot::real_workdir`), once the repository has loaded
    /// in this tab. Another name for the same folder (a link, a junction, a short 8.3 name)
    /// matches this rather than `path`.
    pub real: Option<PathBuf>,
    /// The folder's identity on disk (`Snapshot::folder`), once loaded in this tab: what matches
    /// any other spelling of it, case included where the volume ignores case.
    pub folder: Option<FolderId>,
    /// The common dir of the tab's repository, from its snapshot or the startup read
    /// (`Effect::ReadTabGroups`): what groups worktree tabs. None for a new tab, or before either.
    pub group: Option<CommonDir>,
    /// Everything this tab showed when it was last active. `None` for the active tab, and for
    /// a tab restored at startup that hasn't been loaded yet.
    pub parked: Option<Box<Parked>>,
    /// The name the user gave the tab (Rename tab); `None` shows the folder name.
    pub label: Option<String>,
    /// This tab's activity log while it isn't the active tab; the active tab's is
    /// `AppState::log`. Not in `Parked`: a tab restored but not loaded yet still gets lines.
    pub log: Vec<super::LogEntry>,
}

impl Tab {
    pub fn empty() -> Self {
        Self {
            id: TabId::next(),
            path: None,
            name: "New tab".into(),
            real: None,
            folder: None,
            group: None,
            parked: None,
            label: None,
            log: Vec::new(),
        }
    }

    /// A tab for `path` whose repository loads when the tab is first selected.
    pub fn unloaded(path: PathBuf) -> Self {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string());
        Self {
            id: TabId::next(),
            path: Some(path),
            name,
            real: None,
            folder: None,
            group: None,
            parked: None,
            label: None,
            log: Vec::new(),
        }
    }

    /// The name the tab shows: its label, else the folder name.
    pub fn shown_name(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }

    /// Whether this tab shows the repository at `path` (as given, or where it really is), by
    /// spelling. `place().is` also matches by identity.
    pub fn is(&self, path: &Path) -> bool {
        [&self.path, &self.real].into_iter().flatten().any(|p| same_spelling(p, path))
    }

    /// Whether this tab shows the repository `snap` was loaded from: the same folder on disk, or
    /// (before either was looked at) the same spelling.
    pub fn shows(&self, snap: &Snapshot) -> bool {
        (self.folder.is_some() && self.folder == snap.folder) || [&self.path, &self.real].into_iter().flatten().any(|p| is_repo_at(snap, p))
    }

    /// The names and identity this tab's folder is known by.
    pub fn place(&self) -> Place {
        Place { paths: [&self.path, &self.real].into_iter().flatten().cloned().collect(), folder: self.folder }
    }
}

/// `text` as a tab label: control characters removed, trimmed, at most `MAX_TAB_LABEL`
/// characters. `None` (the folder name is shown) when nothing is left or it is the folder name.
pub fn clean_label(text: &str, folder: &str) -> Option<String> {
    let kept: String = text.chars().filter(|c| !c.is_control()).collect();
    let capped: String = kept.trim().chars().take(MAX_TAB_LABEL).collect();
    let label = capped.trim_end();
    (!label.is_empty() && label != folder).then(|| label.to_owned())
}

/// A closed tab remembered for reopening, with the label it had.
#[derive(Clone, Debug, PartialEq)]
pub struct ClosedTab {
    pub path: PathBuf,
    pub label: Option<String>,
}

impl From<PathBuf> for ClosedTab {
    fn from(path: PathBuf) -> Self {
        Self { path, label: None }
    }
}

impl ClosedTab {
    /// The label, else the folder name.
    pub fn name(&self) -> String {
        match (&self.label, self.path.file_name()) {
            (Some(label), _) => label.clone(),
            (None, Some(name)) => name.to_string_lossy().into_owned(),
            (None, None) => self.path.display().to_string(),
        }
    }
}

/// The Rename tab dialog.
pub struct RenameTabForm {
    pub tab: TabId,
    pub path: PathBuf,
    /// The folder name: the hint, and what an empty field goes back to.
    pub folder: String,
    pub current: Option<String>,
    pub text: String,
}

impl RenameTabForm {
    /// For a tab that shows a repository.
    pub fn new(tab: &Tab) -> Self {
        Self {
            tab: tab.id,
            path: tab.path.clone().unwrap_or_default(),
            folder: tab.name.clone(),
            current: tab.label.clone(),
            text: tab.shown_name().to_owned(),
        }
    }

    /// The label the field gives (`clean_label`).
    pub fn label(&self) -> Option<String> {
        clean_label(&self.text, &self.folder)
    }

    /// Whether Rename would change the label.
    pub fn changed(&self) -> bool {
        self.label() != self.current
    }
}

/// A folder as Glowfly knows it without touching the disk: the spellings it was seen by and,
/// once it was looked at on a worker thread, its identity.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Place {
    pub paths: Vec<PathBuf>,
    pub folder: Option<FolderId>,
}

impl Place {
    /// Whether this is the folder at `path`, whose identity is `folder` when known.
    pub fn is(&self, path: &Path, folder: Option<FolderId>) -> bool {
        (folder.is_some() && folder == self.folder) || self.paths.iter().any(|p| same_spelling(p, path))
    }
}

/// The places of the tabs that show a repository.
pub fn open_places(tabs: &[Tab]) -> Vec<Place> {
    tabs.iter().filter(|t| t.path.is_some()).map(Tab::place).collect()
}

/// The per-repository part of `AppState`, set aside while another tab is active.
#[derive(Default)]
pub struct Parked {
    pub snap: Option<Snapshot>,
    pub selection: Selection,
    pub detail: Option<CommitDetail>,
    pub diff: Option<(DiffTarget, std::sync::Arc<FileDiff>)>,
    /// The binary diff's sizes and previews (one still loading is asked for again on return).
    pub diff_preview: Option<super::diff_preview::DiffPreview>,
    /// The open conflict with its picks (a load in flight is requested again on return).
    pub conflict: Option<ConflictView>,
    /// The Conflicted Files counts and Resolved Files (read again on return).
    pub conflict_files: crate::state::conflict_files::ConflictFiles,
    /// The open file history (a request in flight is sent again on return).
    pub history: Option<FileHistoryView>,
    /// The open blame view (a load in flight is requested again on return).
    pub blame: Option<super::blame::BlameView>,
    /// The pull request panel (a file list in flight is asked for again on return).
    pub pull_request: Option<PrView>,
    /// The issue panel (its details load again on return).
    pub issue: Option<super::issue::IssueView>,
    /// The Create issue dialog, unless an issue was being created (its choices load again on
    /// return).
    pub issue_form: Option<Box<super::issue_form::IssueForm>>,
    /// The interactive rebase panel (a list still loading is requested again on return).
    pub rebase: Option<RebasePanel>,
    /// The Git Flow panel is open.
    pub gitflow_panel: bool,
    /// The operation journal (a load or preview in flight is asked for again on return).
    pub journal: Option<crate::state::journal::JournalView>,
    /// The Range-diff view (a comparison in flight is asked for again on return).
    pub range_diff: Option<crate::state::range_diff::RangeDiffView>,
    /// A Finish stopped at a merge conflict, with its steps still to run.
    pub gitflow_resume: Option<crate::state::gitflow_resume::StoppedFinish>,
    pub draft: CommitDraft,
    pub undo: UndoHistory,
    pub forge: Option<ForgeState>,
    pub stats: HashMap<Oid, (usize, usize, usize)>,
    pub line_counts: crate::state::line_counts::LineCounts,
    pub signatures: crate::state::signatures::Signatures,
    /// The last conflict check (one in flight is asked for again on return).
    pub conflict_checks: crate::state::conflict_check::ConflictChecks,
    pub search: String,
    pub commit_search: crate::state::search::CommitSearch,
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
    /// The open diff's LFS summary and whether it shows the pointer text instead.
    pub lfs_diff: Option<super::lfs::LfsDiffNote>,
    /// The folders collapsed in the changed-file lists' tree.
    pub file_folds: crate::state::file_folds::FileFolds,
}

/// Where the tab at index `i` ends up after moving the tab at `from` to `to`: what
/// `tab_groups::move_order` gives when every tab is a block of its own (its tests compare).
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

/// Whether `path` names the repository `snap` loaded, under any spelling it was seen by: as
/// libgit2 reports it, resolved on disk (links and short 8.3 names followed), or as it was
/// opened. Pure, by spelling only: `AppState::names_repo` also matches by identity.
pub fn is_repo_at(snap: &Snapshot, path: &Path) -> bool {
    [Some(&snap.workdir), Some(&snap.real_workdir), snap.opened_as.as_ref()].into_iter().flatten().any(|p| same_spelling(p, path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u64) -> Option<FolderId> {
        Some(FolderId::for_tests(n))
    }

    #[test]
    fn places_match_by_identity_or_spelling() {
        let place = Place { paths: vec![PathBuf::from("/w/repo")], folder: id(1) };
        assert!(place.is(Path::new("/w/./repo/"), None), "spelling");
        assert!(place.is(Path::new("/elsewhere/link"), id(1)), "identity");
        assert!(!place.is(Path::new("/w/Repo"), None), "case is never folded by spelling");
        assert!(!place.is(Path::new("/w/Repo"), id(2)), "another folder");
        let unseen = Place { paths: vec![PathBuf::from("/w/repo")], folder: None };
        assert!(!unseen.is(Path::new("/x"), None), "no identity matches no identity");
    }

    #[test]
    fn labels_are_cleaned() {
        assert_eq!(clean_label("  Web app \t", "repo"), Some("Web app".into()));
        assert_eq!(clean_label("Web\napp\u{7}", "repo"), Some("Webapp".into()));
        assert_eq!(clean_label("", "repo"), None);
        assert_eq!(clean_label("   ", "repo"), None);
        assert_eq!(clean_label("\t\n\r", "repo"), None);
        assert_eq!(clean_label(" repo ", "repo"), None, "the folder name is no label");
        let long = "é".repeat(MAX_TAB_LABEL + 10);
        assert_eq!(clean_label(&long, "repo").map(|l| l.chars().count()), Some(MAX_TAB_LABEL));
    }

    #[test]
    fn tab_ids_are_distinct() {
        let (a, b, c) = (Tab::empty(), Tab::unloaded(PathBuf::from("/w/a")), Tab::empty());
        assert!(a.id != b.id && b.id != c.id && a.id != c.id);
        assert_eq!(b.shown_name(), "a");
        let named = Tab { label: Some("API".into()), ..Tab::unloaded(PathBuf::from("/w/a")) };
        assert_eq!(named.shown_name(), "API");
    }
}
