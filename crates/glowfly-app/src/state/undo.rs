//! Per-tab undo history: Glowfly's own recent operations that can be reversed, most recent last.
//! In memory only, so it ends with the tab or the app (discarded work is also kept on disk for
//! the journal's Lost tab, `undo::kept`). Also the form of the dialog that asks
//! whether undoing a checkout should delete the branch it created (`Modal::UndoCreated`).

use glowfly_core::i18n::Lang;
use glowfly_core::undo::{Action, OpStart};

/// Undo steps kept per tab; older ones are dropped.
pub const MAX_UNDO: usize = 20;

/// The dialog shown before undoing a checkout that created `branch`, which still points where
/// the checkout made it: switch back to `back_to`, and also delete the branch when `delete`.
#[derive(Clone, Debug, PartialEq)]
pub struct UndoCreatedForm {
    pub action: Action,
    pub branch: String,
    pub back_to: String,
    pub delete: bool,
}

impl UndoCreatedForm {
    pub fn new(action: Action, back_to: String) -> Self {
        let branch = match &action {
            Action::Checkout { created: Some(c), .. } => c.name.clone(),
            _ => String::new(),
        };
        Self { action, branch, back_to, delete: true }
    }
}

#[derive(Default, Debug)]
pub struct UndoHistory {
    pub undo: Vec<Action>,
    pub redo: Vec<Action>,
    /// Label of the most recent operation when it can't be undone (and cleared the history).
    pub blocked: Option<String>,
    /// The operation in progress that a Glowfly job started and that stopped for conflicts.
    pub stopped: Option<Stopped>,
}

/// Where a stopped operation started (`undo::op_start`), and how many undo entries were older.
#[derive(Clone, Debug, PartialEq)]
pub struct Stopped {
    pub start: OpStart,
    pub depth: usize,
}

/// What a stopped operation's own steps record (resolving its conflicts, discards), which leave
/// it in progress; anything else recorded means it has ended.
fn during_operation(a: &Action) -> bool {
    matches!(
        a,
        Action::Resolve { .. }
            | Action::Conflicts { .. }
            | Action::Discard { .. }
            | Action::Restore { .. }
            | Action::DeleteFile { .. }
            | Action::UnstageGone { .. }
    )
}

impl UndoHistory {
    /// A new undoable action succeeded. A kept abort of the stopped operation drops what the
    /// operation's own steps recorded, as a plain abort does (`aborted`).
    pub fn record(&mut self, a: Action) {
        if let (Action::AbortKept { .. }, Some(s)) = (&a, &self.stopped) {
            let since = self.undo.split_off(s.depth.min(self.undo.len()));
            self.undo.extend(since.into_iter().filter(Action::is_content));
        }
        if !during_operation(&a) {
            self.stopped = None;
        }
        self.push_undo(a);
        self.redo.clear();
        self.blocked = None;
    }

    /// An operation that can't be undone succeeded: older commits and ref moves can't be undone
    /// past it, but discarded work (`Action::is_content`) stays undoable, since its undo checks
    /// the files or stash list it writes to first.
    pub fn invalidate(&mut self, label: &str) {
        self.undo.retain(Action::is_content);
        self.redo.clear();
        self.blocked = Some(label.to_owned());
        self.stopped = None;
    }

    /// A merge, pull, cherry-pick, revert or rebase that started at `start` stopped in progress.
    pub fn stopped_at(&mut self, start: OpStart) {
        self.stopped = Some(Stopped { start, depth: self.undo.len() });
    }

    /// A plain abort (labelled `label`) succeeded, leaving the repository at `back`. When that
    /// is exactly where the stopped operation started (HEAD, its branch and the index), the
    /// entries older than the operation stay, with any discarded work since; what the
    /// operation's own steps recorded goes. Otherwise it is an operation that can't be undone.
    pub fn aborted(&mut self, label: &str, back: Option<&OpStart>) {
        match self.stopped.take() {
            Some(s) if back == Some(&s.start) => {
                let since = self.undo.split_off(s.depth.min(self.undo.len()));
                self.undo.extend(since.into_iter().filter(Action::is_content));
                self.redo.clear();
                self.blocked = None;
            }
            _ => self.invalidate(label),
        }
    }

    /// The repository no longer matches the history.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Drops the top of the undo stack (or the redo stack, with `redo`) alone, keeping a stopped
    /// operation's count of older entries true.
    pub fn pop_top(&mut self, redo: bool) {
        if redo {
            self.redo.pop();
        } else {
            self.undo.pop();
            if let Some(s) = &mut self.stopped {
                s.depth = s.depth.min(self.undo.len());
            }
        }
    }

    /// `a` (the top of the undo stack) was undone.
    pub fn undone(&mut self, a: Action) {
        if self.undo.last() == Some(&a) {
            self.undo.pop();
        }
        if let Some(s) = &mut self.stopped {
            s.depth = s.depth.min(self.undo.len());
        }
        self.redo.push(a);
    }

    /// `original` (the top of the redo stack) was redone; `refreshed` is how it stands now.
    pub fn redone(&mut self, original: &Action, refreshed: Action) {
        if self.redo.last() == Some(original) {
            self.redo.pop();
        }
        self.push_undo(refreshed);
    }

    fn push_undo(&mut self, a: Action) {
        self.undo.push(a);
        if self.undo.len() > MAX_UNDO {
            let extra = self.undo.len() - MAX_UNDO;
            self.undo.drain(..extra);
            if let Some(s) = &mut self.stopped {
                s.depth = s.depth.saturating_sub(extra);
            }
        }
    }

    /// Edit > Undo: its label ("Undo checkout of main", or "Undo" with nothing to undo) and why
    /// it is disabled, if it is: a job is running (`idle` false) or there is nothing to undo.
    pub fn undo_entry(&self, t: Lang, idle: bool) -> (String, Option<String>) {
        entry(t, false, self.undo.last(), idle, || self.undo_reason(t))
    }

    /// Edit > Redo, like `undo_entry`.
    pub fn redo_entry(&self, t: Lang, idle: bool) -> (String, Option<String>) {
        entry(t, true, self.redo.last(), idle, || self.redo_reason(t))
    }

    /// Why there is nothing to undo.
    fn undo_reason(&self, t: Lang) -> String {
        match &self.blocked {
            Some(label) => t.trf("menu.edit.cant_undo", &[("label", label)]),
            None => t.tr("menu.edit.nothing_to_undo").to_owned(),
        }
    }

    fn redo_reason(&self, t: Lang) -> String {
        t.tr("menu.edit.nothing_to_redo").to_owned()
    }
}

/// An Undo (`redo` false) or Redo entry for `top`, in `t`.
fn entry(t: Lang, redo: bool, top: Option<&Action>, idle: bool, reason: impl FnOnce() -> String) -> (String, Option<String>) {
    let label = match top {
        Some(a) if redo => t.trf("menu.edit.redo_action", &[("action", &a.describe())]),
        Some(a) => t.trf("menu.edit.undo_action", &[("action", &a.describe())]),
        None if redo => t.tr("menu.edit.redo").to_owned(),
        None => t.tr("menu.edit.undo").to_owned(),
    };
    let blocked = if !idle {
        Some(t.tr("commit.blocked.job_running").to_owned())
    } else if top.is_none() {
        Some(reason())
    } else {
        None
    };
    (label, blocked)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::rebase::JOB_RUNNING;
    use glowfly_core::undo::HeadPos;

    fn checkout() -> Action {
        Action::Checkout { from: HeadPos::Branch("dev".into()), to: HeadPos::Branch("main".into()), created: None }
    }

    #[test]
    fn entries_name_the_action_or_say_why_not() {
        let mut h = UndoHistory::default();
        assert_eq!(h.undo_entry(Lang::En, true), ("Undo".to_owned(), Some("Nothing to undo".to_owned())));
        assert_eq!(h.redo_entry(Lang::En, true), ("Redo".to_owned(), Some("Nothing to redo".to_owned())));
        h.record(checkout());
        assert_eq!(h.undo_entry(Lang::En, true), ("Undo checkout of main".to_owned(), None));
        assert_eq!(h.undo_entry(Lang::En, false), ("Undo checkout of main".to_owned(), Some(JOB_RUNNING.to_owned())));
        h.undone(checkout());
        assert_eq!(h.redo_entry(Lang::En, true), ("Redo checkout of main".to_owned(), None));
        assert_eq!(h.redo_entry(Lang::En, false).1.as_deref(), Some(JOB_RUNNING));
        h.invalidate("Rename branch");
        let (label, why) = h.undo_entry(Lang::En, true);
        assert_eq!(label, "Undo");
        assert!(why.as_deref().is_some_and(|w| w.starts_with("Can't undo Rename branch: only checkout, commit")), "{why:?}");
        assert_eq!(why, Some(Lang::En.trf("menu.edit.cant_undo", &[("label", &"Rename branch")])));
    }

    fn discard() -> Action {
        Action::Discard { path: "a.txt".into(), what: glowfly_core::undo::Discarded::File, before: None, after: None }
    }

    #[test]
    fn discarded_work_survives_a_job_that_cant_be_undone() {
        let mut h = UndoHistory::default();
        let drop = Action::StashDrop { stash: git2::Oid::zero(), message: "On main: x".into() };
        let abort = Action::AbortKept {
            op: glowfly_core::undo::MoveOp::Merge,
            saved: git2::Oid::zero(),
            head: git2::Oid::zero(),
            branch: None,
            left: glowfly_core::undo::Changes(Vec::new()),
        };
        for a in [discard(), checkout(), abort.clone(), drop.clone(), checkout()] {
            h.record(a);
        }
        h.undone(checkout());
        h.invalidate("Rename branch");
        // A kept abort stays named, so `kept_aborts` still keeps its ref from being pruned.
        assert_eq!(h.undo, vec![discard(), abort, drop], "the commits and ref moves go; the content stays, in order");
        assert!(h.redo.is_empty());
        assert_eq!(h.blocked.as_deref(), Some("Rename branch"));
        assert_eq!(h.undo_entry(Lang::En, true), ("Undo drop of stash 'On main: x'".to_owned(), None));
    }

    fn start(n: u8) -> OpStart {
        let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-app-test-undo-start-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        glowfly_core::testkit::git(&dir, &["init", "-q", "-b", "main"]);
        glowfly_core::testkit::set_identity(&dir);
        std::fs::write(dir.join("a.txt"), [b'a' + n]).unwrap();
        glowfly_core::testkit::git(&dir, &["add", "."]);
        glowfly_core::testkit::git(&dir, &["commit", "-q", "-m", "c"]);
        let s = glowfly_core::undo::op_start(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        s
    }

    #[test]
    fn an_abort_back_at_the_start_keeps_what_came_before_the_operation() {
        let (s, other) = (start(0), start(1));
        assert_ne!(s, other);
        let mut h = UndoHistory::default();
        h.record(checkout());
        h.stopped_at(s.clone());
        let resolve = Action::Conflicts {
            change: glowfly_core::undo::ConflictChange::MarkResolved,
            head: None,
            state: git2::RepositoryState::Merge,
            heads: Vec::new(),
            files: Vec::new(),
        };
        for a in [resolve.clone(), discard()] {
            h.record(a);
        }
        assert!(h.stopped.is_some(), "the operation's own steps leave it stopped");
        h.aborted("Abort merge", Some(&s));
        assert_eq!(h.undo, vec![checkout(), discard()], "the operation's own step goes; discarded work stays");
        assert!(h.blocked.is_none() && h.stopped.is_none());

        // Undone below where it started, then a step of its own: that step isn't counted as older.
        h.stopped_at(s.clone());
        h.undone(discard());
        h.undone(checkout());
        h.record(resolve.clone());
        h.aborted("Abort merge", Some(&s));
        assert!(h.undo.is_empty(), "{:?}", h.undo);

        // Anywhere else, or nothing known: an operation that can't be undone.
        for back in [Some(&other), None] {
            h.record(checkout());
            h.stopped_at(s.clone());
            h.aborted("Abort merge", back);
            assert!(h.undo.is_empty() && h.blocked.as_deref() == Some("Abort merge"));
        }
        // Without a stopped operation, and once anything else ended it.
        h.record(checkout());
        h.aborted("Abort rebase", Some(&s));
        assert_eq!(h.blocked.as_deref(), Some("Abort rebase"));
        h.record(checkout());
        h.stopped_at(s.clone());
        h.record(checkout());
        assert!(h.stopped.is_none(), "a new action ends it");

        // A kept abort drops the operation's own steps too, and ends it.
        let mut h = UndoHistory::default();
        h.record(checkout());
        h.stopped_at(s.clone());
        h.record(resolve.clone());
        h.record(discard());
        let kept = Action::AbortKept {
            op: glowfly_core::undo::MoveOp::Merge,
            saved: git2::Oid::zero(),
            head: git2::Oid::zero(),
            branch: None,
            left: glowfly_core::undo::Changes(Vec::new()),
        };
        h.record(kept.clone());
        assert_eq!(h.undo, vec![checkout(), discard(), kept]);
        assert!(h.stopped.is_none());
    }

    #[test]
    fn dropping_the_top_alone_keeps_the_stopped_depth_true() {
        let mut h = UndoHistory::default();
        h.record(checkout());
        h.record(discard());
        h.stopped_at(start(2));
        h.pop_top(false);
        assert_eq!(h.stopped.as_ref().map(|s| s.depth), Some(1));
        h.redo.push(checkout());
        h.pop_top(true);
        assert!(h.redo.is_empty());
        assert_eq!(h.undo, vec![checkout()]);
    }

    #[test]
    fn tooltips_are_unchanged() {
        // The toolbar's tooltips take their title and reason from the entries.
        let mut h = UndoHistory::default();
        assert_eq!(h.undo_entry(Lang::En, true).1.as_deref(), Some("Nothing to undo"));
        assert_eq!(h.redo_entry(Lang::En, true).1.as_deref(), Some("Nothing to redo"));
        h.record(checkout());
        assert_eq!(h.undo_entry(Lang::En, true), ("Undo checkout of main".to_owned(), None));
        h.invalidate("Rename branch");
        let tip = h.undo_entry(Lang::En, true).1.unwrap();
        assert_eq!(tip, Lang::En.trf("menu.edit.cant_undo", &[("label", &"Rename branch")]));
        assert!(tip.starts_with("Can't undo Rename branch: only checkout, commit"), "{tip}");
        assert!(tip.contains("merge, pull, rebase, cherry-pick, revert") && tip.contains("stash, pop, apply, drop"));
        assert!(Lang::En.tr("menu.edit.cant_undo").contains("drop"));
    }

    #[test]
    fn entries_follow_the_language() {
        let mut h = UndoHistory::default();
        h.record(checkout());
        let (label, _) = h.undo_entry(Lang::De, true);
        assert_eq!(label, Lang::De.trf("menu.edit.undo_action", &[("action", &"checkout of main")]));
        assert_ne!(label, "Undo checkout of main");
        assert_eq!(h.undo_entry(Lang::Es, false).1, Some(Lang::Es.tr("commit.blocked.job_running").to_owned()));
    }
}
