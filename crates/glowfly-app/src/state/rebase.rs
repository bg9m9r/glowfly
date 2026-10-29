//! The interactive rebase panel: the list being edited, where it came from (a plan for a new
//! rebase, or the remaining steps of one in progress), and the rules that decide whether it can
//! start. Step and group helpers are in `rebase_steps`, the remaining-steps texts in
//! `rebase_remaining`, the subtitle and the rules of merge, label and reset rows in
//! `rebase_merges`, the update-ref rows that travel with their commit in `rebase_update_refs`.

use git2::RepositoryState;
use glowfly_core::i18n::Lang;
use glowfly_core::rebase::{self, MergeStep, RebaseAction, RebaseBase, RebasePlan, Remaining, TodoStep};
use glowfly_core::repo::Snapshot;

#[path = "rebase_update_refs.rs"]
mod update_refs;

pub const LOADING: &str = "The commit list is still loading.";
pub const STARTING: &str = "The interactive rebase is already running.";
pub const JOB_RUNNING: &str = "Another operation is still running.";
pub const CONFLICTED_FILES: &str = "Resolve the conflicted files before rebasing.";
/// Said before a rebase stashes the uncommitted changes (`--autostash`).
pub const STASH_LINE: &str = "You have uncommitted changes. Glowfly stashes them first and puts them back when the rebase ends.";
pub const UNCHANGED: &str = "Nothing to change yet.";
pub const HINT: &str = "Commits are applied from top to bottom.";

/// What the panel edits.
#[derive(Clone, Debug, PartialEq)]
pub enum PanelSource {
    /// A new rebase of the current branch from this base.
    Start(RebaseBase),
    /// The remaining steps of the interactive rebase in progress.
    Remaining,
}

/// The inline editor that is open, on the row it belongs to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Editing {
    /// A commit's new message.
    Message(usize),
    /// The message of the commit the group headed by this row combines into.
    Combined(usize),
    /// An exec step's command.
    Command(usize),
}

impl Editing {
    pub fn row(self) -> usize {
        match self {
            Self::Message(i) | Self::Combined(i) | Self::Command(i) => i,
        }
    }

    pub(super) fn with_row(self, i: usize) -> Self {
        match self {
            Self::Message(_) => Self::Message(i),
            Self::Combined(_) => Self::Combined(i),
            Self::Command(_) => Self::Command(i),
        }
    }
}

pub struct RebasePanel {
    pub source: PanelSource,
    /// Where a new rebase's list came from (its `items` are not used); `None` while loading.
    pub plan: Option<RebasePlan>,
    /// The rebase in progress the list came from (its `steps` are in `original` and `items`);
    /// `None` while loading.
    pub remaining: Option<Remaining>,
    /// The list as loaded, for Reset.
    pub original: Vec<TodoStep>,
    /// The list as edited, in the order git runs it.
    pub items: Vec<TodoStep>,
    pub loading: bool,
    /// The rebase (or the update of the steps) is running; the list can't change until its
    /// result arrives.
    pub starting: bool,
    /// The list no longer matches the repository: Start is refused until Reset reloads it.
    pub stale: bool,
    /// Why the last start failed.
    pub error: Option<String>,
    /// The selected row (keys act on it).
    pub selected: Option<usize>,
    /// The open inline editor.
    pub editing: Option<Editing>,
    /// The editor's text (edited in place by the view).
    pub draft: String,
}

impl RebasePanel {
    fn empty(source: PanelSource) -> Self {
        Self {
            source,
            plan: None,
            remaining: None,
            original: Vec::new(),
            items: Vec::new(),
            loading: true,
            starting: false,
            stale: false,
            error: None,
            selected: None,
            editing: None,
            draft: String::new(),
        }
    }

    pub fn loading(base: RebaseBase) -> Self {
        Self::empty(PanelSource::Start(base))
    }

    pub fn loading_remaining() -> Self {
        Self::empty(PanelSource::Remaining)
    }

    /// The list arrived (a plan or the remaining steps).
    pub fn loaded(&self) -> bool {
        self.plan.is_some() || self.remaining.is_some()
    }

    fn filled(&mut self, steps: Vec<TodoStep>) {
        self.original = steps;
        self.items = self.original.clone();
        self.loading = false;
        self.stale = false;
        self.error = None;
        self.selected = None;
        self.editing = None;
        self.draft.clear();
    }

    /// Shows a newly loaded list.
    pub fn fill(&mut self, mut plan: RebasePlan) {
        let steps = std::mem::take(&mut plan.steps);
        self.plan = Some(plan);
        self.filled(steps);
    }

    /// Shows the remaining steps of the rebase in progress.
    pub fn fill_remaining(&mut self, mut remaining: Remaining) {
        let steps = std::mem::take(&mut remaining.steps);
        self.remaining = Some(remaining);
        self.filled(steps);
    }

    /// Back to loading (Reset of a stale list).
    pub fn reload(&mut self) {
        *self = Self::empty(self.source.clone());
    }

    /// The list can be edited (loaded, not running).
    pub fn editable(&self) -> bool {
        !self.loading && !self.starting
    }

    /// The first row may squash or fix up into a commit the rebase in progress already made.
    pub fn first_may_combine(&self) -> bool {
        self.remaining.as_ref().is_some_and(|r| r.first_may_combine)
    }

    /// Moves row `from` so it ends up at `to` (removed, then inserted), together with the
    /// update-ref rows that belong to it (`update_refs::moved_order`). The selection and
    /// the open editor follow their row.
    pub fn move_item(&mut self, from: usize, to: usize) {
        if !self.movable(from) {
            return;
        }
        let Some(order) = update_refs::moved_order(&self.items, from, to) else { return };
        let before = super::rebase_steps::groups(&self.items);
        let old = std::mem::take(&mut self.items);
        self.items = order.iter().map(|&k| old[k].clone()).collect();
        let place = |i: usize| order.iter().position(|&k| k == i).unwrap_or(i);
        self.selected = self.selected.map(place);
        self.editing = self.editing.map(|e| e.with_row(place(e.row())));
        self.prune_combined(&before);
    }

    /// Sets commit row `i`'s action. Choosing reword opens its message editor (the combined
    /// one on a group head); leaving reword keeps the typed message (switching back restores
    /// it) and closes the row's editor.
    pub fn set_action(&mut self, i: usize, action: RebaseAction) {
        if !self.actions(i).contains(&action) {
            return;
        }
        let before = super::rebase_steps::groups(&self.items);
        let Some(item) = self.items.get_mut(i).and_then(TodoStep::item_mut) else { return };
        item.action = action;
        if action != RebaseAction::Reword && self.editing == Some(Editing::Message(i)) {
            self.close_editor();
        }
        self.prune_combined(&before);
        if action == RebaseAction::Reword {
            self.open_editor(i);
        }
    }

    /// Opens row `i`'s editor: a group head's combined message (typed before, or git's), a
    /// commit's or merge's message (typed before, or the original), or an exec step's command.
    pub fn open_editor(&mut self, i: usize) {
        let (editing, draft) = match self.items.get(i) {
            Some(TodoStep::Commit(item)) if self.heads_group(i) => {
                let draft = item.combined_message.clone().unwrap_or_else(|| rebase::default_combined(&self.items, i));
                (Editing::Combined(i), draft)
            }
            Some(TodoStep::Commit(item) | TodoStep::Merge(MergeStep { item, .. })) => {
                (Editing::Message(i), item.new_message.clone().unwrap_or_else(|| item.message.trim_end().to_owned()))
            }
            Some(TodoStep::Exec(cmd)) => (Editing::Command(i), cmd.clone()),
            Some(TodoStep::Break | TodoStep::Label { .. } | TodoStep::Reset(_) | TodoStep::UpdateRef(_)) | None => return,
        };
        self.draft = draft;
        self.editing = Some(editing);
    }

    /// The row after which the open editor is drawn: a combined message's under its group's
    /// last row.
    pub fn editor_row(&self) -> Option<usize> {
        self.editing.map(|e| match e {
            Editing::Combined(h) => rebase::group_end(&self.items, h).unwrap_or(h),
            e => e.row(),
        })
    }

    /// The editor's save button: a message makes the row a reword; a combined message equal
    /// to git's is stored as none; a command is stored trimmed (an empty one stays open).
    pub fn save_message(&mut self) {
        let Some(editing) = self.editing else { return };
        match editing {
            Editing::Message(i) => {
                let Some(item) = self.items.get_mut(i).and_then(TodoStep::item_mut) else { return };
                item.new_message = Some(self.draft.trim_end().to_owned());
                item.action = RebaseAction::Reword;
            }
            Editing::Combined(h) => {
                let default = rebase::default_combined(&self.items, h);
                let typed = self.draft.trim_end().to_owned();
                let Some(item) = self.items.get_mut(h).and_then(TodoStep::commit_mut) else { return };
                item.combined_message = (typed.trim() != default.trim()).then_some(typed);
            }
            Editing::Command(i) => {
                let cmd = self.draft.trim().to_owned();
                if cmd.is_empty() {
                    return;
                }
                if let Some(TodoStep::Exec(c)) = self.items.get_mut(i) {
                    *c = cmd;
                }
            }
        }
        self.close_editor();
    }

    /// Closes the editor; an exec step whose command is still empty goes away with it.
    pub fn close_editor(&mut self) {
        if let Some(Editing::Command(i)) = self.editing
            && matches!(self.items.get(i), Some(TodoStep::Exec(c)) if c.trim().is_empty())
        {
            self.editing = None;
            self.remove_step(i);
        }
        self.editing = None;
        self.draft.clear();
    }

    /// The original order and actions, without typed messages or added steps.
    pub fn reset(&mut self) {
        self.editing = None;
        self.items = self.original.clone();
        self.error = None;
        self.selected = None;
        self.close_editor();
    }

    /// The branch being rebased ("HEAD" for a running rebase of a detached HEAD).
    pub fn branch(&self) -> &str {
        match (&self.plan, &self.remaining) {
            (Some(p), _) => &p.branch,
            (None, Some(r)) => r.branch.as_deref().unwrap_or("HEAD"),
            (None, None) => "",
        }
    }

    /// The list no longer matches the repository: HEAD or its branch moved, or the rebase in
    /// progress moved on (or ended).
    pub fn is_stale(&self, snap: &Snapshot) -> bool {
        if let Some(r) = &self.remaining {
            return self.stale || snap.state != RepositoryState::RebaseInteractive || snap.rebase.as_ref().map(|p| p.step) != Some(r.done);
        }
        self.stale || self.plan.as_ref().is_some_and(|p| snap.head_oid != Some(p.head) || snap.head_branch.as_deref() != Some(&p.branch))
    }

    /// Why Start (or Update steps) is unavailable, if it is (the drop confirmation aside). The
    /// remaining steps don't mind uncommitted changes or conflicted files.
    pub fn start_blocker(&self, snap: &Snapshot, job_running: bool) -> Option<String> {
        if self.loading || !self.loaded() {
            return Some(LOADING.into());
        }
        if self.starting {
            return Some(STARTING.into());
        }
        if job_running {
            return Some(JOB_RUNNING.into());
        }
        if self.remaining.is_some() {
            if self.is_stale(snap) {
                return Some(rebase::TODO_MOVED.into());
            }
            if let Some(p) = rebase::problem_with(&self.items, self.first_may_combine()) {
                return Some(p);
            }
            return (self.items == self.original).then(|| UNCHANGED.into());
        }
        if self.is_stale(snap) {
            return Some(rebase::STALE.into());
        }
        if let Some(h) = self.plan.as_ref().and_then(|p| p.held.first()) {
            return Some(h.text());
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
        let from_commit = matches!(self.source, PanelSource::Start(RebaseBase::FromCommit(_)));
        (from_commit && rebase::is_unchanged(&self.items, &self.original)).then(|| UNCHANGED.into())
    }

    /// Why Start (or Update steps) can't write the typed messages: the repository's commit
    /// encoding (`i18n.commitEncoding`) can't hold some of their characters.
    pub fn message_blocker(&self, snap: &Snapshot, t: Lang) -> Option<String> {
        let items = self.items.iter().filter_map(|s| s.commit().or_else(|| s.merge().map(|m| &m.item)));
        let typed = items.flat_map(|i| [i.new_message.as_deref(), i.combined_message.as_deref()]).flatten();
        snap.commit_encoding.check(typed).err().map(|e| super::commit::unencodable_text(t, &e))
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
            body = format!("{body}\n\n{STASH_LINE}");
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
