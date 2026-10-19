//! `AppState`'s queries: whether hunk indexes are current and line operations blocked, the
//! working folder, load options. The activity log's `log` and `log_quiet` are in `logs.rs`.

use super::{AppState, WHOLE_FILE_BLOCKS_LINE_OPS};
use std::path::{Path, PathBuf};
use trellis_core::repo::LoadOptions;
use trellis_core::settings::{BranchVisibility, DiffMode};

impl AppState {
    /// The generation of the diff on screen when its hunk indexes are current, i.e. no reload
    /// or newer diff is on its way. Hunk operations must carry it. Every change to the
    /// whitespace filter reopens the diff (a new `diff_gen`), so an operation that matches it
    /// counts hunks of the diff under the current setting.
    pub fn hunk_ops_gen(&self) -> Option<u64> {
        (self.diff.is_some() && self.diff_shown_gen == self.diff_gen).then_some(self.diff_gen)
    }

    /// Why selecting and applying single lines is refused: the whole-file view.
    pub fn line_ops_blocked(&self) -> Option<&'static str> {
        (self.settings.diff_mode == DiffMode::WholeFile).then_some(WHOLE_FILE_BLOCKS_LINE_OPS)
    }

    pub fn workdir(&self) -> Option<PathBuf> {
        self.snap.as_ref().map(|s| s.workdir.clone())
    }

    /// Working directory for commands that don't need a repository (git config, sign-in).
    pub fn any_dir(&self) -> PathBuf {
        self.workdir().unwrap_or_else(Self::outside_dir)
    }

    /// A folder outside any repository (the temp folder), for git calls that must not read the
    /// active repository's own config, e.g. its credential helper while it isn't trusted.
    pub fn outside_dir() -> PathBuf {
        std::env::temp_dir()
    }

    /// How to load the repository at `workdir`, with the hidden and shown-only refs saved for it.
    pub fn load_options(&self, workdir: &Path) -> LoadOptions {
        let (smart, trusted) = (self.settings.branch_visibility == BranchVisibility::Smart, self.trusts(workdir));
        // Relevant branches also keeps branches that have open pull requests.
        let extra_tips = self.forge.as_ref().map(|f| f.prs.iter().map(|p| p.source.clone()).collect()).unwrap_or_default();
        let (f, worktree_wips) = (self.settings.graph_filter_for(&self.repo_key(workdir)), self.settings.worktree_wips);
        LoadOptions { max_commits: self.commits_wanted(), smart, extra_tips, hidden: f.hidden, solo: f.solo, trusted, worktree_wips }
    }

    /// Whether the commit graph is the central view: the same order as `ui/central.rs::show`
    /// and `state::layout::tool_view` (keep the three in step).
    pub fn graph_shown(&self) -> bool {
        !self.dashboard.open
            && self.snap.is_some()
            && self.conflict.is_none()
            && self.blame.is_none()
            && self.history.is_none()
            && self.pull_request.is_none()
            && self.issue.is_none()
            && self.diff.is_none()
            && self.rebase.is_none()
            && !self.gitflow_panel
    }
}
