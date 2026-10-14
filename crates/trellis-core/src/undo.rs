//! Undo and redo for Trellis's own git operations. `before`/`after` record the repository state
//! around an operation and turn it into an `Action`; `plan` checks that the repository still
//! matches it and returns the git commands that reverse (or re-apply) it. Reads use libgit2; the
//! planned commands are meant to run through `gitcli`, so hooks and user config apply.

use crate::error::Result;
use crate::gitcli::args;
use git2::{BranchType, Oid, Repository, RepositoryState};
use std::path::Path;

mod saved;
pub use saved::{Saved, discard_saving, write as write_saved};

/// The message of the stash that "Discard all" moves uncommitted changes into.
pub const DISCARD_STASH_MESSAGE: &str = "Trellis: discarded changes";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
}

impl ResetMode {
    pub fn flag(self) -> &'static str {
        match self {
            ResetMode::Soft => "--soft",
            ResetMode::Mixed => "--mixed",
            ResetMode::Hard => "--hard",
        }
    }

    fn name(self) -> &'static str {
        &self.flag()[2..]
    }
}

/// Which kind of undoable operation a git command line is.
#[derive(Clone, Debug, PartialEq)]
pub enum UndoKind {
    Checkout,
    Commit { amend: bool },
    Reset(ResetMode),
    DeleteBranch(String),
    DiscardAll,
}

/// Where HEAD points.
#[derive(Clone, Debug, PartialEq)]
pub enum HeadPos {
    Branch(String),
    Detached(Oid),
}

/// A recorded operation that can be undone (and then redone). `Discard` is one file's unstaged
/// changes (all, a hunk or some lines) discarded: `before` is its content then and `after` what
/// the discard left (`None`: absent).
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Checkout { from: HeadPos, to: HeadPos },
    Commit { branch: Option<String>, from: Oid, to: Oid, message: String, amend: bool },
    Reset { mode: ResetMode, branch: Option<String>, from: Oid, to: Oid },
    DeleteBranch { name: String, oid: Oid, upstream: Option<String> },
    DiscardAll { stash: Oid, head: Oid },
    Discard { path: String, before: Option<Saved>, after: Option<Saved> },
}

/// Repository state captured before an operation runs (opaque to callers).
#[derive(Debug)]
pub struct Before {
    head: Option<HeadPos>,
    head_oid: Option<Oid>,
    branch: Option<(Oid, Option<String>)>,
    stash_top: Option<Oid>,
}

/// Why an undo or redo can't run now.
#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    /// A merge, rebase or similar is in progress.
    Busy(String),
    /// Running it would overwrite uncommitted work; the entry stays usable.
    Dirty(String),
    /// The repository no longer matches the history, which should be cleared.
    Stale(String),
}

impl Refusal {
    pub fn message(&self) -> &str {
        match self {
            Refusal::Busy(m) | Refusal::Dirty(m) | Refusal::Stale(m) => m,
        }
    }

    pub fn is_stale(&self) -> bool {
        matches!(self, Refusal::Stale(_))
    }
}

#[derive(Debug, PartialEq)]
pub enum Plan {
    /// Git commands to run in order.
    Run(Vec<Vec<String>>),
    /// Make the working-tree file `path` hold `content` exactly (`None`: delete it).
    Write {
        path: String,
        content: Option<Saved>,
    },
    Refuse(Refusal),
}

impl UndoKind {
    /// Recognises the command lines Trellis builds for undoable operations.
    pub fn classify(argv: &[String]) -> Option<UndoKind> {
        let a: Vec<&str> = argv.iter().map(String::as_str).collect();
        match a.as_slice() {
            ["checkout", rest @ ..] => {
                let mut targets = 0;
                for (i, arg) in rest.iter().enumerate() {
                    match *arg {
                        "--track" | "--detach" => {}
                        // A trailing `--` only stops the target being read as a path.
                        "--" if i + 1 == rest.len() => {}
                        s if s.starts_with('-') => return None,
                        _ => targets += 1,
                    }
                }
                (targets == 1).then_some(UndoKind::Checkout)
            }
            ["commit", ..] => Some(UndoKind::Commit { amend: a.contains(&"--amend") }),
            ["reset", mode, rev] if !rev.starts_with('-') => match *mode {
                "--soft" => Some(UndoKind::Reset(ResetMode::Soft)),
                "--mixed" => Some(UndoKind::Reset(ResetMode::Mixed)),
                "--hard" => Some(UndoKind::Reset(ResetMode::Hard)),
                _ => None,
            },
            ["branch", "-d" | "-D", name] if !name.starts_with('-') => Some(UndoKind::DeleteBranch((*name).to_owned())),
            ["stash", "push", "-u", "-m", m] if *m == DISCARD_STASH_MESSAGE => Some(UndoKind::DiscardAll),
            _ => None,
        }
    }
}

fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

impl Action {
    /// What the action did, for tooltips and job labels ("commit 'Fix build'").
    pub fn describe(&self) -> String {
        match self {
            Action::Checkout { to, .. } => match to {
                HeadPos::Branch(b) => format!("checkout of {b}"),
                HeadPos::Detached(o) => format!("checkout of {}", short(*o)),
            },
            Action::Commit { message, amend, .. } => {
                let summary = message.lines().next().unwrap_or_default();
                let cut: String = summary.chars().take(50).collect();
                let cut = if summary.chars().count() > 50 { format!("{cut}…") } else { cut };
                format!("{} '{cut}'", if *amend { "amend" } else { "commit" })
            }
            Action::Reset { mode, to, .. } => format!("{} reset to {}", mode.name(), short(*to)),
            Action::DeleteBranch { name, .. } => format!("delete of branch {name}"),
            Action::DiscardAll { .. } => "Discard all changes".to_owned(),
            Action::Discard { path, .. } => format!("discard in {path}"),
        }
    }
}

fn head(repo: &Repository) -> (Option<HeadPos>, Option<Oid>) {
    match repo.head() {
        Ok(h) => {
            let oid = h.target();
            let pos = if h.is_branch() { h.shorthand().map(|b| HeadPos::Branch(b.to_owned())) } else { oid.map(HeadPos::Detached) };
            (pos, oid)
        }
        Err(_) => (None, None), // unborn
    }
}

fn stash_top(repo: &Repository) -> Option<Oid> {
    repo.find_reference("refs/stash").ok().and_then(|r| r.target())
}

fn ref_exists(repo: &Repository, name: &str) -> bool {
    repo.find_reference(name).is_ok()
}

fn branch_name(pos: &Option<HeadPos>) -> Option<String> {
    match pos {
        Some(HeadPos::Branch(b)) => Some(b.clone()),
        _ => None,
    }
}

/// Records what an operation of `kind` may change, before it runs.
pub fn before(workdir: &Path, kind: &UndoKind) -> Result<Before> {
    let repo = Repository::open(workdir)?;
    let (head, head_oid) = head(&repo);
    let branch = match kind {
        UndoKind::DeleteBranch(name) => repo.find_branch(name, BranchType::Local).ok().and_then(|b| {
            let oid = b.get().target()?;
            let upstream = b.upstream().ok().and_then(|u| u.name().ok().flatten().map(str::to_owned));
            Some((oid, upstream))
        }),
        _ => None,
    };
    let stash_top = if *kind == UndoKind::DiscardAll { stash_top(&repo) } else { None };
    Ok(Before { head, head_oid, branch, stash_top })
}

/// The action an operation of `kind` performed, or `None` if it changed nothing undoable.
pub fn after(workdir: &Path, kind: UndoKind, before: Before) -> Result<Option<Action>> {
    let repo = Repository::open(workdir)?;
    let (pos, oid) = head(&repo);
    Ok(match kind {
        UndoKind::Checkout => match (before.head, pos) {
            (Some(from), Some(to)) if from != to => Some(Action::Checkout { from, to }),
            _ => None,
        },
        UndoKind::Commit { amend } => match (before.head_oid, oid) {
            (Some(from), Some(to)) if from != to => {
                let message = repo.find_commit(to)?.message().unwrap_or_default().trim().to_owned();
                Some(Action::Commit { branch: branch_name(&pos), from, to, message, amend })
            }
            _ => None,
        },
        UndoKind::Reset(mode) => match (before.head_oid, oid) {
            (Some(from), Some(to)) if from != to => Some(Action::Reset { mode, branch: branch_name(&pos), from, to }),
            _ => None,
        },
        UndoKind::DeleteBranch(name) => match before.branch {
            Some((oid, upstream)) if !ref_exists(&repo, &format!("refs/heads/{name}")) => {
                Some(Action::DeleteBranch { name, oid, upstream })
            }
            _ => None,
        },
        UndoKind::DiscardAll => match stash_top(&repo) {
            Some(stash) if Some(stash) != before.stash_top => {
                let head = repo.find_commit(stash)?.parent_id(0)?;
                Some(Action::DiscardAll { stash, head })
            }
            _ => None,
        },
    })
}

fn busy_name(state: RepositoryState) -> Option<&'static str> {
    use RepositoryState::*;
    Some(match state {
        Clean => return None,
        Merge => "merge",
        Revert | RevertSequence => "revert",
        CherryPick | CherryPickSequence => "cherry-pick",
        Bisect => "bisect",
        Rebase | RebaseInteractive | RebaseMerge => "rebase",
        ApplyMailbox | ApplyMailboxOrRebase => "patch application",
    })
}

fn has_staged(workdir: &Path) -> Result<bool> {
    let (_, staged, _) = crate::repo::status_at(workdir)?;
    Ok(!staged.is_empty())
}

fn is_clean(workdir: &Path) -> Result<bool> {
    let (unstaged, staged, _) = crate::repo::status_at(workdir)?;
    Ok(unstaged.is_empty() && staged.is_empty())
}

fn stale(msg: impl Into<String>) -> Plan {
    Plan::Refuse(Refusal::Stale(format!("{}; undo history cleared", msg.into())))
}

fn gone() -> Plan {
    stale("The commit to go back to is gone (probably garbage-collected)")
}

/// The git commands that undo `action` (or redo it, after an undo), or why they can't run.
/// Checks the repository as it is now, not as a snapshot showed it.
pub fn plan(workdir: &Path, action: &Action, redo: bool) -> Result<Plan> {
    if let Action::Discard { path, before, after } = action {
        // Only this file's bytes change, so a merge or rebase in progress doesn't matter.
        return saved::plan(workdir, path, *before, *after, redo);
    }
    let mut repo = Repository::open(workdir)?;
    if let Some(op) = busy_name(repo.state()) {
        return Ok(Plan::Refuse(Refusal::Busy(format!("Finish or abort the {op} in progress first."))));
    }
    let verb = if redo { "redoing" } else { "undoing" };
    let dirty =
        || Plan::Refuse(Refusal::Dirty(format!("Commit or stash your changes first: {verb} {} would overwrite them.", action.describe())));
    let exists = |oid: Oid| repo.find_commit(oid).is_ok();
    let (pos, head_oid) = head(&repo);
    let moved = || stale(format!("HEAD has moved since the {}", action.describe()));
    let at = |branch: &Option<String>, oid: Oid| {
        head_oid == Some(oid)
            && match (&pos, branch) {
                (Some(HeadPos::Branch(b)), Some(want)) => b == want,
                (Some(HeadPos::Detached(_)), None) => true,
                _ => false,
            }
    };
    Ok(match action {
        Action::Checkout { from, to } => {
            let (expect, target) = if redo { (from, to) } else { (to, from) };
            if pos.as_ref() != Some(expect) {
                return Ok(moved());
            }
            match target {
                HeadPos::Branch(b) if !ref_exists(&repo, &format!("refs/heads/{b}")) => stale(format!("Branch {b} no longer exists")),
                HeadPos::Branch(b) => Plan::Run(vec![args(&["checkout", b, "--"])]),
                HeadPos::Detached(o) if !exists(*o) => gone(),
                HeadPos::Detached(o) => Plan::Run(vec![args(&["checkout", "--detach", &o.to_string(), "--"])]),
            }
        }
        Action::Commit { branch, from, to, .. } => {
            let (expect, target) = if redo { (*from, *to) } else { (*to, *from) };
            if !at(branch, expect) {
                moved()
            } else if !exists(target) {
                gone()
            } else {
                Plan::Run(vec![args(&["reset", "--soft", &target.to_string()])])
            }
        }
        Action::Reset { mode, branch, from, to } => {
            let (expect, target) = if redo { (*from, *to) } else { (*to, *from) };
            if !at(branch, expect) {
                moved()
            } else if !exists(target) {
                gone()
            } else if *mode == ResetMode::Hard && !is_clean(workdir)? {
                dirty()
            } else if *mode == ResetMode::Mixed && has_staged(workdir)? {
                // A mixed reset rewrites the index, so a staged version that differs from the
                // working copy would be lost.
                Plan::Refuse(Refusal::Dirty(format!(
                    "Commit or unstage your staged changes first: {verb} {} would replace them.",
                    action.describe()
                )))
            } else {
                Plan::Run(vec![args(&["reset", mode.flag(), &target.to_string()])])
            }
        }
        Action::DeleteBranch { name, oid, upstream } => {
            let tip = repo.find_reference(&format!("refs/heads/{name}")).ok().map(|r| r.target());
            if redo {
                match tip {
                    Some(t) if t == Some(*oid) => Plan::Run(vec![args(&["branch", "-D", name])]),
                    Some(_) => stale(format!("Branch {name} has moved since it was restored")),
                    None => stale(format!("Branch {name} no longer exists")),
                }
            } else if tip.is_some() {
                stale(format!("A branch named {name} exists again"))
            } else if !exists(*oid) {
                gone()
            } else {
                let mut cmds = vec![args(&["branch", name, &oid.to_string()])];
                if let Some(u) = upstream.as_deref().filter(|u| ref_exists(&repo, &format!("refs/remotes/{u}"))) {
                    cmds.push(args(&["branch", "--set-upstream-to", u, name]));
                }
                Plan::Run(cmds)
            }
        }
        Action::DiscardAll { stash, head } => {
            if head_oid != Some(*head) {
                return Ok(moved());
            }
            let clean = is_clean(workdir)?;
            if redo {
                return Ok(if clean { stale("No changes to discard") } else { Plan::Run(vec![discard_command()]) });
            }
            if !clean {
                return Ok(dirty());
            }
            let mut index = None;
            repo.stash_foreach(|i, _, oid| {
                if oid == stash {
                    index = Some(i);
                }
                index.is_none()
            })?;
            match index {
                Some(n) => Plan::Run(vec![args(&["stash", "pop", "--index", &format!("stash@{{{n}}}")])]),
                None => stale("The discarded-changes stash was dropped"),
            }
        }
        Action::Discard { .. } => unreachable!("planned above"),
    })
}

/// The command Trellis runs for "Discard all".
pub fn discard_command() -> Vec<String> {
    args(&["stash", "push", "-u", "-m", DISCARD_STASH_MESSAGE])
}

/// `action` as it stands after a successful redo: a re-done discard lives in a new stash.
pub fn redone(workdir: &Path, action: Action) -> Result<Action> {
    Ok(match action {
        Action::DiscardAll { head, .. } => {
            let repo = Repository::open(workdir)?;
            match stash_top(&repo) {
                Some(stash) => Action::DiscardAll { stash, head },
                None => return Err(crate::error::Error::NotFound("the stash for the redone discard".into())),
            }
        }
        other => other,
    })
}

#[cfg(test)]
mod saved_tests;
#[cfg(test)]
mod tests;
