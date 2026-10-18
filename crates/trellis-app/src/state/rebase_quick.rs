//! History edits from the graph's menus (reword, squash, drop): why a menu entry is unavailable,
//! the Reword and Squash dialog's form, and the question asked before a drop runs. The menu rules mirror
//! `trellis_core::rebase::prepare_edit` on the loaded snapshot; core checks them again.

use super::rebase::STASH_LINE;
use git2::{Oid, RepositoryState};
use std::path::PathBuf;
use trellis_core::rebase::{self, QuickEdit, QuickPlan, TodoItem, TodoStep};
use trellis_core::repo::{CommitInfo, Snapshot};

pub const REWORD_LABEL: &str = "Reword commit";
pub const SQUASH_LABEL: &str = "Squash commits";
pub const DROP_LABEL: &str = "Drop commits";
/// Why an edit didn't run: HEAD moved after it was chosen.
pub const QUICK_STALE: &str = "The branch changed since you chose this; nothing was changed. Try again.";
pub const BLANK: &str = "Enter a message.";
pub const SAME_MESSAGE: &str = "The message is unchanged.";
pub const STASH_NOTE: &str = "Your uncommitted changes are stashed first and put back when the rebase ends.";

/// Why each menu entry is unavailable; `None` when it can be used.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EditBlockers {
    pub reword: Option<String>,
    pub squash: Option<String>,
    pub drop: Option<String>,
}

impl EditBlockers {
    fn all(why: String) -> Self {
        Self { reword: Some(why.clone()), squash: Some(why.clone()), drop: Some(why) }
    }
}

/// HEAD's first-parent line in the loaded history, newest first, through merges, ending at the
/// root or where the loaded history stops. `true` when it reached the root.
fn line(snap: &Snapshot) -> (Vec<&CommitInfo>, bool) {
    let mut out = Vec::new();
    let mut at = snap.head_oid;
    while let Some(c) = at.and_then(|o| snap.row_of.get(&o)).and_then(|&r| snap.commits.get(r)) {
        out.push(c);
        at = c.parents.first().copied();
        if at.is_none() {
            return (out, true);
        }
    }
    (out, false)
}

/// Why nothing can be edited in this state: an operation in progress, or a detached HEAD.
fn general(snap: &Snapshot) -> Option<String> {
    if snap.state != RepositoryState::Clean {
        return rebase::in_progress_text(snap.state);
    }
    snap.head_branch.is_none().then(|| rebase::DETACHED.into())
}

/// The loaded commits reachable from HEAD.
fn reachable(snap: &Snapshot) -> Vec<&CommitInfo> {
    snap.head_oid.map(|h| snap.commits_between(h, None)).unwrap_or_default()
}

/// Why `oid` can't be found on the line: it came in through a merge, or it isn't on the
/// branch. `None` when the loaded history ends before it (core decides).
fn missing(snap: &Snapshot, rooted: bool, oid: Oid, many: bool) -> Option<String> {
    if reachable(snap).iter().any(|c| c.oid == oid) {
        return Some(if many { rebase::SOME_SIDE_LINE } else { rebase::SIDE_LINE }.into());
    }
    rooted.then(|| if many { rebase::SOME_NOT_ON_BRANCH } else { rebase::NOT_ON_BRANCH }.into())
}

/// Whether two or more loaded commits reachable from HEAD have `oid` as a parent: another
/// line of history starts there.
fn splits(snap: &Snapshot, oid: Oid) -> bool {
    reachable(snap).iter().filter(|c| c.parents.contains(&oid)).count() >= 2
}

/// The blockers of the single-commit menu entries for `oid`.
pub fn single(snap: &Snapshot, oid: Oid) -> EditBlockers {
    if let Some(why) = general(snap) {
        return EditBlockers::all(why);
    }
    let (line, rooted) = line(snap);
    let Some(pos) = line.iter().position(|c| c.oid == oid) else {
        return missing(snap, rooted, oid, false).map(EditBlockers::all).unwrap_or_default();
    };
    let commit = line[pos];
    if commit.parents.len() > 1 {
        return EditBlockers::all(rebase::MERGE_SELECTED.into());
    }
    let parent = commit.parents.first().and_then(|p| snap.row_of.get(p)).map(|&r| &snap.commits[r]);
    let squash = match (commit.parents.is_empty(), parent) {
        (true, _) => Some(rebase::ROOT_SQUASH.into()),
        (false, Some(p)) if p.parents.len() > 1 => Some(rebase::PARENT_MERGE.into()),
        (false, Some(p)) if splits(snap, p.oid) => Some(rebase::SPLIT_BY_BRANCH.into()),
        _ => None,
    };
    let drop = (rooted && line.len() == 1).then(|| rebase::DROPS_EVERYTHING.into());
    EditBlockers { reword: None, squash, drop }
}

/// The blockers of the multi-selection entries for `oids` (`reword` is unused).
pub fn multi(snap: &Snapshot, oids: &[Oid]) -> EditBlockers {
    if let Some(why) = general(snap) {
        return EditBlockers::all(why);
    }
    let merge = |o: &Oid| snap.row_of.get(o).is_some_and(|&r| snap.commits[r].parents.len() > 1);
    if oids.iter().any(merge) {
        return EditBlockers::all(rebase::SOME_MERGE_SELECTED.into());
    }
    let (line, rooted) = line(snap);
    let mut rows = Vec::with_capacity(oids.len());
    for &o in oids {
        match line.iter().position(|c| c.oid == o) {
            Some(p) => rows.push(p),
            None => return missing(snap, rooted, o, true).map(EditBlockers::all).unwrap_or_default(),
        }
    }
    rows.sort_unstable();
    rows.dedup();
    let consecutive = rows.len() > 1 && rows.windows(2).all(|w| w[1] == w[0] + 1);
    let squash = if !consecutive {
        Some(rebase::NOT_CONSECUTIVE.into())
    } else if rows[1..].iter().any(|&r| splits(snap, line[r].oid)) {
        Some(rebase::SPLIT_BY_BRANCH.into())
    } else {
        None
    };
    let drop = (rooted && rows.len() == line.len()).then(|| rebase::DROPS_EVERYTHING.into());
    EditBlockers { reword: None, squash, drop }
}

/// What an edit is called in the undo history ("reword of 1a2b3c4", "squash of 3 commits").
pub fn undo_detail(edit: &QuickEdit) -> String {
    match edit {
        QuickEdit::Reword(o) => format!("reword of {}", short(*o)),
        QuickEdit::SquashIntoParent(o) => format!("squash of {} into its parent", short(*o)),
        QuickEdit::Squash(v) => format!("squash of {}", commits(v.len())),
        QuickEdit::Drop(v) if v.len() == 1 => format!("drop of {}", short(v[0])),
        QuickEdit::Drop(v) => format!("drop of {}", commits(v.len())),
    }
}

/// The job label of an edit.
pub fn label(edit: &QuickEdit) -> &'static str {
    match edit {
        QuickEdit::Reword(_) => REWORD_LABEL,
        QuickEdit::SquashIntoParent(_) | QuickEdit::Squash(_) => SQUASH_LABEL,
        QuickEdit::Drop(_) => DROP_LABEL,
    }
}

fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

/// "1 commit" / "3 commits".
fn commits(n: usize) -> String {
    if n == 1 { "1 commit".into() } else { format!("{n} commits") }
}

fn with_stash(body: String, stash: bool) -> String {
    if stash { format!("{body}\n\n{STASH_LINE}") } else { body }
}

/// The Reword dialog (`Modal::RewordCommit`), which is also the Squash dialog: the prepared
/// plan and the message being typed (for a squash, the combined commit's).
#[derive(Clone, Debug)]
pub struct RewordForm {
    pub workdir: PathBuf,
    pub plan: QuickPlan,
    pub draft: String,
    /// The message field takes the keyboard focus once.
    pub focus: bool,
}

impl RewordForm {
    /// The form for a prepared reword or squash; a squash starts from the joined messages.
    pub fn new(workdir: PathBuf, plan: QuickPlan) -> Self {
        let mut form = Self { workdir, plan, draft: String::new(), focus: true };
        let joined = form.row().and_then(|i| i.new_message.clone()).filter(|_| form.squashes());
        form.draft = joined.unwrap_or_else(|| form.original().to_owned()).trim_end().to_owned();
        form
    }

    /// The dialog combines commits (a squash) rather than rewording one.
    pub fn squashes(&self) -> bool {
        matches!(self.plan.edit, QuickEdit::Squash(_) | QuickEdit::SquashIntoParent(_))
    }

    /// The reworded row of the plan's list (a squash's group head).
    pub fn index(&self) -> usize {
        self.plan.edited.first().copied().unwrap_or(0)
    }

    fn row(&self) -> Option<&TodoItem> {
        self.plan.request.items.get(self.index()).and_then(TodoStep::commit)
    }

    pub fn oid(&self) -> Oid {
        self.row().map(|i| i.oid).unwrap_or_else(Oid::zero)
    }

    /// The commit's message as it is now.
    pub fn original(&self) -> &str {
        self.row().map(|i| i.message.as_str()).unwrap_or("")
    }

    /// Commits after it, rewritten too.
    pub fn later(&self) -> usize {
        self.plan.later
    }

    pub fn title(&self) -> &'static str {
        if self.squashes() { SQUASH_LABEL } else { REWORD_LABEL }
    }

    /// The message field's hint.
    pub fn hint(&self) -> &'static str {
        if self.squashes() { "Message of the combined commit" } else { "Commit message" }
    }

    /// The primary button, saying so when the changes are stashed first.
    pub fn button(&self, stash: bool) -> &'static str {
        match (self.squashes(), stash) {
            (true, true) => "Stash and squash",
            (true, false) => "Squash",
            (false, true) => "Stash and reword",
            (false, false) => "Reword",
        }
    }

    /// "1a2b3c4 on main · rewrites this commit and 2 later ones"; for a squash "3 commits on
    /// main become one · rewrites 2 later commits" or "1a2b3c4 into its parent 9f8e7d6 on main
    /// · …".
    pub fn subtitle(&self) -> String {
        let branch = &self.plan.branch;
        if self.squashes() {
            let later = match self.later() {
                0 => "no later commits change".to_owned(),
                1 => "rewrites 1 later commit".to_owned(),
                n => format!("rewrites {n} later commits"),
            };
            let what = match &self.plan.edit {
                QuickEdit::SquashIntoParent(o) => format!("{} into its parent {} on {branch}", short(*o), short(self.oid())),
                _ => format!("{} on {branch} become one", commits(self.plan.edited.len())),
            };
            return format!("{what} · {later}");
        }
        let what = match self.later() {
            0 => "rewrites only this commit".to_owned(),
            1 => "rewrites this commit and 1 later one".to_owned(),
            n => format!("rewrites this commit and {n} later ones"),
        };
        format!("{} on {branch} · {what}", short(self.oid()))
    }

    /// Why the primary button is unavailable: a blank message, a reword's unchanged message, or
    /// a job running (`idle` false).
    pub fn blocker(&self, idle: bool) -> Option<&'static str> {
        if self.draft.trim().is_empty() {
            return Some(BLANK);
        }
        if !self.squashes() && self.draft.trim() == self.original().trim() {
            return Some(SAME_MESSAGE);
        }
        (!idle).then_some(super::rebase::JOB_RUNNING)
    }
}

/// The drop confirmation: (title, body, button).
pub fn drop_question(plan: &QuickPlan, stash: bool) -> (String, String, String) {
    let n = plan.edited.len();
    if n == 1 {
        let (sha, summary) = match plan.request.items[plan.edited[0]].commit() {
            Some(item) => (short(item.oid), item.summary.as_str()),
            None => (String::new(), ""),
        };
        let after = match plan.later {
            0 => "and nothing after it changes.".to_owned(),
            1 => "and the commit after it is rewritten.".to_owned(),
            k => format!("and the {k} commits after it are rewritten."),
        };
        let body = format!("Drop {sha} \"{summary}\" from {}? Its changes are removed {after}", plan.branch);
        return ("Drop commit".into(), with_stash(body, stash), "Drop commit".into());
    }
    let body = format!("Drop {} from {}? Their changes are removed and the commits after them are rewritten.", commits(n), plan.branch);
    ("Drop commits".into(), with_stash(body, stash), format!("Drop {n} commits"))
}
