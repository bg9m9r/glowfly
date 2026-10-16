//! The interactive rebase panel: the list being edited, the plan it came from, and the rules
//! that decide whether it can start.

use super::tabs::index_after_move;
use git2::RepositoryState;
use trellis_core::rebase::{self, RebaseAction, RebaseBase, RebasePlan, TodoItem, Upstream};
use trellis_core::repo::Snapshot;

pub const LOADING: &str = "The commit list is still loading.";
pub const STARTING: &str = "The interactive rebase is already running.";
pub const JOB_RUNNING: &str = "Another operation is still running.";
pub const CONFLICTED_FILES: &str = "Resolve the conflicted files before rebasing.";
/// Said before a rebase stashes the uncommitted changes (`--autostash`).
pub const STASH_LINE: &str = "You have uncommitted changes. Trellis stashes them first and puts them back when the rebase ends.";
pub const UNCHANGED: &str = "Nothing to change yet.";
pub const HINT: &str = "Commits are applied from top to bottom.";

pub struct RebasePanel {
    pub base: RebaseBase,
    /// Where the list came from (its `items` are not used); `None` while loading.
    pub plan: Option<RebasePlan>,
    /// The list as loaded, for Reset.
    pub original: Vec<TodoItem>,
    /// The list as edited, oldest first.
    pub items: Vec<TodoItem>,
    pub loading: bool,
    /// The rebase is running; the list can't change until its result arrives.
    pub starting: bool,
    /// HEAD moved since the list was made: Start is refused until Reset reloads it.
    pub stale: bool,
    /// Why the last start failed.
    pub error: Option<String>,
    /// The row whose message editor is open.
    pub editing: Option<usize>,
    /// The message editor's text (edited in place by the view).
    pub draft: String,
}

impl RebasePanel {
    pub fn loading(base: RebaseBase) -> Self {
        Self {
            base,
            plan: None,
            original: Vec::new(),
            items: Vec::new(),
            loading: true,
            starting: false,
            stale: false,
            error: None,
            editing: None,
            draft: String::new(),
        }
    }

    /// Shows a newly loaded list.
    pub fn fill(&mut self, mut plan: RebasePlan) {
        self.original = std::mem::take(&mut plan.items);
        self.items = self.original.clone();
        self.plan = Some(plan);
        self.loading = false;
        self.stale = false;
        self.error = None;
        self.editing = None;
        self.draft.clear();
    }

    /// Back to loading (Reset of a stale list).
    pub fn reload(&mut self) {
        *self = Self::loading(self.base.clone());
    }

    /// The list can be edited (loaded, not running).
    pub fn editable(&self) -> bool {
        !self.loading && !self.starting
    }

    /// Moves row `from` so it ends up at `to` (removed, then inserted). The open message
    /// editor follows its row.
    pub fn move_item(&mut self, from: usize, to: usize) {
        let n = self.items.len();
        if from >= n || to >= n || from == to {
            return;
        }
        let item = self.items.remove(from);
        self.items.insert(to, item);
        self.editing = self.editing.map(|e| index_after_move(e, from, to));
    }

    /// Sets row `i`'s action. Choosing reword opens its message editor; leaving reword keeps
    /// the typed message (switching back restores it) and closes the row's editor.
    pub fn set_action(&mut self, i: usize, action: RebaseAction) {
        let Some(item) = self.items.get_mut(i) else { return };
        item.action = action;
        if action == RebaseAction::Reword {
            self.open_editor(i);
        } else if self.editing == Some(i) {
            self.close_editor();
        }
    }

    /// Opens the message editor on row `i`, with the message typed before or the original.
    pub fn open_editor(&mut self, i: usize) {
        let Some(item) = self.items.get(i) else { return };
        self.draft = item.new_message.clone().unwrap_or_else(|| item.message.trim_end().to_owned());
        self.editing = Some(i);
    }

    /// Update message: stores the draft and makes the row a reword.
    pub fn save_message(&mut self) {
        let Some(item) = self.editing.and_then(|i| self.items.get_mut(i)) else { return };
        item.new_message = Some(self.draft.trim_end().to_owned());
        item.action = RebaseAction::Reword;
        self.close_editor();
    }

    pub fn close_editor(&mut self) {
        self.editing = None;
        self.draft.clear();
    }

    /// The original order and actions, without typed messages.
    pub fn reset(&mut self) {
        self.items = self.original.clone();
        self.error = None;
        self.close_editor();
    }

    pub fn drops(&self) -> usize {
        self.items.iter().filter(|i| i.action == RebaseAction::Drop).count()
    }

    pub fn branch(&self) -> &str {
        self.plan.as_ref().map_or("", |p| p.branch.as_str())
    }

    /// "Rebasing 4 commits of feature onto main", or "… from the first commit" for a rebase of
    /// the root.
    pub fn subtitle(&self) -> String {
        let Some(plan) = &self.plan else { return "Loading commits…".into() };
        let n = self.items.len();
        let commits = if n == 1 { "1 commit".to_owned() } else { format!("{n} commits") };
        match plan.upstream {
            Upstream::Root => format!("Rebasing {commits} of {} from the first commit", plan.branch),
            Upstream::Commit(_) => format!("Rebasing {commits} of {} onto {}", plan.branch, plan.onto_label),
        }
    }

    /// The list no longer matches the repository: HEAD or its branch moved.
    pub fn is_stale(&self, snap: &Snapshot) -> bool {
        self.stale || self.plan.as_ref().is_some_and(|p| snap.head_oid != Some(p.head) || snap.head_branch.as_deref() != Some(&p.branch))
    }

    /// Why Start is unavailable, if it is (the drop confirmation aside).
    pub fn start_blocker(&self, snap: &Snapshot, job_running: bool) -> Option<String> {
        if self.loading || self.plan.is_none() {
            return Some(LOADING.into());
        }
        if self.starting {
            return Some(STARTING.into());
        }
        if job_running {
            return Some(JOB_RUNNING.into());
        }
        if self.is_stale(snap) {
            return Some(rebase::STALE.into());
        }
        if let Some(text) = rebase::in_progress_text(snap.state) {
            return Some(text);
        }
        if has_conflicts(snap) {
            return Some(CONFLICTED_FILES.into());
        }
        if let Some(p) = rebase::problem(&self.items) {
            return Some(p);
        }
        let from_commit = matches!(self.base, RebaseBase::FromCommit(_));
        (from_commit && rebase::is_unchanged(&self.items, &self.original)).then(|| UNCHANGED.into())
    }
}

/// The work tree has conflicted files (also without an operation in progress, after a stash
/// apply).
pub fn has_conflicts(snap: &Snapshot) -> bool {
    snap.unstaged.iter().any(|f| f.status == '!')
}

/// A rebase must stash first: something is staged, or a tracked file changed (untracked files
/// don't get in git's way).
pub fn needs_stash(snap: &Snapshot) -> bool {
    !snap.staged.is_empty() || snap.unstaged.iter().any(|f| f.status != '?')
}

/// The question asked before starting a rebase, if one is needed: about dropping `drops`
/// commits of `branch`, about stashing the uncommitted changes (`stash`), or both at once.
pub fn start_question(drops: usize, branch: &str, stash: bool) -> Option<(String, String)> {
    if drops > 0 {
        let commits = if drops == 1 { "1 commit".to_owned() } else { format!("{drops} commits") };
        let mut body = format!("Drop {commits} from {branch}? Their changes will be removed.");
        if stash {
            body = format!(
                "{body}

{STASH_LINE}"
            );
        }
        return Some(("Drop commits".into(), body));
    }
    stash.then(|| ("Stash changes and rebase".into(), STASH_LINE.into()))
}

/// Why a rebase can't be opened in this state, if it can't.
pub fn open_blocker(snap: &Snapshot) -> Option<String> {
    if snap.state != RepositoryState::Clean {
        return rebase::in_progress_text(snap.state);
    }
    snap.head_branch.is_none().then(|| rebase::DETACHED.into())
}
