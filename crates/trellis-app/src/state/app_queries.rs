//! `AppState`'s queries: whether hunk and line operations are blocked, the working folder, load
//! options. The activity log's `log` and `log_quiet` are in `logs.rs`.

use super::{AppState, WHITESPACE_BLOCKS_HUNK_OPS, WHITESPACE_BLOCKS_REVERT, WHOLE_FILE_BLOCKS_LINE_OPS};
use std::path::{Path, PathBuf};
use trellis_core::repo::LoadOptions;
use trellis_core::settings::{BranchVisibility, DiffMode};

impl AppState {
    /// The generation of the diff on screen when its hunk indexes are current, i.e. no reload
    /// or newer diff is on its way. Hunk operations must carry it.
    pub fn hunk_ops_gen(&self) -> Option<u64> {
        (self.diff.is_some() && self.diff_shown_gen == self.diff_gen).then_some(self.diff_gen)
    }

    /// Why hunk operations on the open diff (stage/unstage/discard on a working-tree diff,
    /// revert on a commit or comparison diff) are refused, apart from a pending reload: they
    /// count hunks of the unfiltered diff, so with whitespace hidden the shown hunks don't
    /// match. Every change to the setting reopens the diff (a new `diff_gen`), so a diff loaded
    /// under the other setting never has `hunk_ops_gen` set.
    pub fn hunk_ops_blocked(&self) -> Option<&'static str> {
        let (target, _) = self.diff.as_ref()?;
        let why = if target.is_working_tree() { WHITESPACE_BLOCKS_HUNK_OPS } else { WHITESPACE_BLOCKS_REVERT };
        self.settings.diff_ignore_whitespace.then_some(why)
    }

    /// Why selecting and applying single lines is refused: whatever blocks hunk operations, or
    /// the whole-file view.
    pub fn line_ops_blocked(&self) -> Option<&'static str> {
        self.hunk_ops_blocked().or((self.settings.diff_mode == DiffMode::WholeFile).then_some(WHOLE_FILE_BLOCKS_LINE_OPS))
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

    /// How to load the repository at `workdir`, with the hidden and soloed refs saved for it.
    pub fn load_options(&self, workdir: &Path) -> LoadOptions {
        let (smart, trusted) = (self.settings.branch_visibility == BranchVisibility::Smart, self.trusts(workdir));
        // Smart visibility also keeps branches that have open pull requests.
        let extra_tips = self.forge.as_ref().map(|f| f.prs.iter().map(|p| p.source.clone()).collect()).unwrap_or_default();
        let (f, worktree_wips) = (self.settings.graph_filter_for(&self.repo_key(workdir)), self.settings.worktree_wips);
        LoadOptions { max_commits: self.commits_wanted(), smart, extra_tips, hidden: f.hidden, solo: f.solo, trusted, worktree_wips }
    }

    /// Whether the commit graph is the central view: the same order as `ui/central.rs::show`
    /// (keep the two in step).
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
