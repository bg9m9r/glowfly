//! History edits from the graph's menus (reword, squash, drop): why a menu entry is unavailable,
//! the Reword dialog's form, and the questions asked before an edit runs. The menu rules mirror
//! `trellis_core::rebase::prepare_edit` on the loaded snapshot; core checks them again.

use super::rebase::STASH_LINE;
use git2::{Oid, RepositoryState};
use std::path::PathBuf;
use trellis_core::rebase::{self, QuickEdit, QuickPlan};
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

/// HEAD's first-parent line in the loaded history, newest first, ending at the first merge
/// (included), at the root, or where the loaded history stops. `true` when it reached the root.
fn line(snap: &Snapshot) -> (Vec<&CommitInfo>, bool) {
    let mut out = Vec::new();
    let mut at = snap.head_oid;
    while let Some(c) = at.and_then(|o| snap.row_of.get(&o)).and_then(|&r| snap.commits.get(r)) {
        out.push(c);
        if c.parents.len() > 1 {
            return (out, false);
        }
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

/// Why `oid` can't be found on the line: a merge between it and HEAD, or it isn't on the
/// branch. `None` when the loaded history ends before it (core decides).
fn missing(snap: &Snapshot, line: &[&CommitInfo], rooted: bool, oid: Oid, many: bool) -> Option<String> {
    let merge_last = line.last().is_some_and(|c| c.parents.len() > 1);
    let reachable = || snap.head_oid.is_some_and(|h| snap.commits_between(h, None).iter().any(|c| c.oid == oid));
    if merge_last && reachable() {
        return Some(rebase::MERGE_AFTER.into());
    }
    (merge_last || rooted).then(|| if many { rebase::SOME_NOT_ON_BRANCH } else { rebase::NOT_ON_BRANCH }.into())
}

/// The blockers of the single-commit menu entries for `oid`.
pub fn single(snap: &Snapshot, oid: Oid) -> EditBlockers {
    if let Some(why) = general(snap) {
        return EditBlockers::all(why);
    }
    let (line, rooted) = line(snap);
    let Some(pos) = line.iter().position(|c| c.oid == oid) else {
        return missing(snap, &line, rooted, oid, false).map(EditBlockers::all).unwrap_or_default();
    };
    let commit = line[pos];
    if commit.parents.len() > 1 {
        return EditBlockers::all(rebase::MERGE_SELECTED.into());
    }
    let parent = commit.parents.first().and_then(|p| snap.row_of.get(p)).map(|&r| &snap.commits[r]);
    let squash = match (commit.parents.is_empty(), parent) {
        (true, _) => Some(rebase::ROOT_SQUASH.into()),
        (false, Some(p)) if p.parents.len() > 1 => Some(rebase::PARENT_MERGE.into()),
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
            None => return missing(snap, &line, rooted, o, true).map(EditBlockers::all).unwrap_or_default(),
        }
    }
    rows.sort_unstable();
    rows.dedup();
    let consecutive = rows.len() > 1 && rows.windows(2).all(|w| w[1] == w[0] + 1);
    let squash = (!consecutive).then(|| rebase::NOT_CONSECUTIVE.into());
    let drop = (rooted && rows.len() == line.len()).then(|| rebase::DROPS_EVERYTHING.into());
    EditBlockers { reword: None, squash, drop }
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

/// The Reword dialog (`Modal::RewordCommit`): the prepared plan and the message being typed.
#[derive(Clone, Debug)]
pub struct RewordForm {
    pub workdir: PathBuf,
    pub plan: QuickPlan,
    pub draft: String,
    /// The message field takes the keyboard focus once.
    pub focus: bool,
}

impl RewordForm {
    pub fn new(workdir: PathBuf, plan: QuickPlan) -> Self {
        let mut form = Self { workdir, plan, draft: String::new(), focus: true };
        form.draft = form.original().trim_end().to_owned();
        form
    }

    /// The reworded row of the plan's list.
    pub fn index(&self) -> usize {
        self.plan.edited.first().copied().unwrap_or(0)
    }

    pub fn oid(&self) -> Oid {
        self.plan.request.items.get(self.index()).map(|i| i.oid).unwrap_or_else(Oid::zero)
    }

    /// The commit's message as it is now.
    pub fn original(&self) -> &str {
        self.plan.request.items.get(self.index()).map(|i| i.message.as_str()).unwrap_or("")
    }

    /// Commits after it, rewritten too.
    pub fn later(&self) -> usize {
        self.plan.later
    }

    /// "1a2b3c4 on main · rewrites this commit and 2 later ones".
    pub fn subtitle(&self) -> String {
        let what = match self.later() {
            0 => "rewrites only this commit".to_owned(),
            1 => "rewrites this commit and 1 later one".to_owned(),
            n => format!("rewrites this commit and {n} later ones"),
        };
        format!("{} on {} · {what}", short(self.oid()), self.plan.branch)
    }

    /// Why the primary button is unavailable: a blank or unchanged message, or a job running
    /// (`idle` false).
    pub fn blocker(&self, idle: bool) -> Option<&'static str> {
        if self.draft.trim().is_empty() {
            return Some(BLANK);
        }
        if self.draft.trim() == self.original().trim() {
            return Some(SAME_MESSAGE);
        }
        (!idle).then_some(super::rebase::JOB_RUNNING)
    }
}

/// The drop confirmation: (title, body, button).
pub fn drop_question(plan: &QuickPlan, stash: bool) -> (String, String, String) {
    let n = plan.edited.len();
    if n == 1 {
        let item = &plan.request.items[plan.edited[0]];
        let after = match plan.later {
            0 => "and nothing after it changes.".to_owned(),
            1 => "and the commit after it is rewritten.".to_owned(),
            k => format!("and the {k} commits after it are rewritten."),
        };
        let body = format!("Drop {} \"{}\" from {}? Its changes are removed {after}", short(item.oid), item.summary, plan.branch);
        return ("Drop commit".into(), with_stash(body, stash), "Drop commit".into());
    }
    let body = format!("Drop {} from {}? Their changes are removed and the commits after them are rewritten.", commits(n), plan.branch);
    ("Drop commits".into(), with_stash(body, stash), format!("Drop {n} commits"))
}

/// The question before a squash on a dirty tree: (title, body, button).
pub fn squash_question(plan: &QuickPlan) -> (String, String, String) {
    let body = match &plan.edit {
        QuickEdit::SquashIntoParent(o) => format!("Squash {} into its parent?", short(*o)),
        _ => format!("Squash {} into one?", commits(plan.edited.len())),
    };
    ("Stash changes and squash".into(), with_stash(body, true), "Squash".into())
}
