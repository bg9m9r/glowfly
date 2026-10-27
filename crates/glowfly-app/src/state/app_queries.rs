//! `AppState`'s queries: whether hunk indexes are current and line operations blocked, the
//! working folder, load options. The activity log's `log` and `log_quiet` are in `logs.rs`.

use super::{AppState, WHOLE_FILE_BLOCKS_LINE_OPS};
use glowfly_core::repo::LoadOptions;
use glowfly_core::settings::{BranchVisibility, DiffMode};
use std::path::{Path, PathBuf};

impl AppState {
    /// The generation of the diff on screen when its hunk indexes are current, i.e. no reload
    /// or newer diff is on its way (`DiffSlot::hunk_ops_gen`). Hunk operations must carry it.
    /// Every change to the whitespace filter reopens the diff (a new `DiffSlot::generation`), so
    /// an operation that matches it counts hunks of the diff under the current setting.
    pub fn hunk_ops_gen(&self) -> Option<u64> {
        self.session.diff.hunk_ops_gen()
    }

    /// Why selecting and applying single lines is refused: the whole-file view.
    pub fn line_ops_blocked(&self) -> Option<&'static str> {
        (self.settings.diff_mode == DiffMode::WholeFile).then_some(WHOLE_FILE_BLOCKS_LINE_OPS)
    }

    /// The git banner's text: [`super::git_env::GitState::banner`] with the version hidden by
    /// Don't show again.
    pub fn git_banner(&self) -> Option<String> {
        self.git.banner(self.settings.git_banner_hidden_for.as_deref())
    }

    pub fn workdir(&self) -> Option<PathBuf> {
        self.snap.as_ref().map(|s| s.workdir.clone())
    }

    /// Working directory for commands that don't need a repository (git config, sign-in).
    pub fn any_dir(&self) -> PathBuf {
        self.workdir().unwrap_or_else(Self::outside_dir)
    }

    /// The active repository while git runs there unasked, else a folder outside any repository
    /// (`outside_dir`): for git calls that need no repository and must not read a held one's
    /// config.
    pub fn unasked_dir(&self) -> PathBuf {
        if self.runs_git_unasked() { self.any_dir() } else { Self::outside_dir() }
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
        let extra_tips = self.session.forge.as_ref().map(|f| f.prs.iter().map(|p| p.source.clone()).collect()).unwrap_or_default();
        let (f, worktree_wips) = (self.settings.graph_filter_for(&self.repo_key(workdir)), self.settings.worktree_wips);
        // What the decision was made on: git reads the status only while that covers the scan.
        let saved = self.saved_trust_for(workdir).or_else(|| self.settings.saved_trust(&self.repo_key(workdir)));
        let trusted_print = saved.filter(|t| t.is_trusted()).and_then(|t| t.print);
        LoadOptions {
            max_commits: self.commits_wanted(),
            smart,
            extra_tips,
            hidden: f.hidden,
            solo: f.solo,
            trusted,
            trusted_print,
            worktree_wips,
        }
    }

    /// Whether the commit graph is the central view: the same order as `ui/central.rs::show`
    /// and `state::layout::tool_view` (keep the three in step). A maximised terminal panel
    /// covers it.
    pub fn graph_shown(&self) -> bool {
        !self.dashboard.open
            && self.snap.is_some()
            && self.session.conflict.view.is_none()
            && self.session.blame.view.is_none()
            && self.session.history.view.is_none()
            && self.session.pull_request.view.is_none()
            && self.session.issue.view.is_none()
            && !self.diff_or_download()
            && self.session.rebase.view.is_none()
            && self.session.range_diff.view.is_none()
            && self.session.journal.view.is_none()
            && !self.session.gitflow_panel
            && !super::terminal::maximized_shown(self)
    }
}
