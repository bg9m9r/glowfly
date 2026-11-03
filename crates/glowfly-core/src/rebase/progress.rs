//! Where a rebase in progress stands: how many of its commits are done, the commit it stopped
//! at and why, and whether git holds an autostash. Read from git's own state files, so it also
//! describes rebases started outside Glowfly.

use super::todo::{listed_oids, own_line};
use super::{Stop, StopKind};
use crate::commit_text;
use git2::{Oid, Repository, RepositoryState};
use std::path::Path;

#[derive(Clone, PartialEq, Debug)]
pub struct RebaseProgress {
    /// Commits done, the one it stopped at included.
    pub step: usize,
    /// Commits in the list (the `exec` lines Glowfly adds aren't counted); 0 when unknown.
    pub total: usize,
    pub stop: Stop,
    /// The stopped commit's summary; empty when the commit is unknown.
    pub summary: String,
    /// Git stashed uncommitted changes before the rebase and puts them back when it ends.
    pub autostash: bool,
    /// An interactive rebase (rather than a plain one).
    pub interactive: bool,
    /// The command of a `Command` stop: the exec step that failed.
    pub command: Option<String>,
    /// Git would amend HEAD on Continue: it stopped cleanly to edit a commit, which is still
    /// HEAD (`rebase-merge/amend` names it). False at an `edit` step that conflicted, and once
    /// HEAD moved off the commit (reset to split it).
    pub amends: bool,
}

/// Where the rebase in progress in `workdir` stands; `None` when no rebase is in progress.
pub fn progress(workdir: &Path) -> Option<RebaseProgress> {
    progress_of(&Repository::open(workdir).ok()?)
}

/// Where a stopped rebase is: the last step done, and whether it conflicted. `None` unless a
/// rebase is in progress.
pub fn stopped_at(workdir: &Path) -> Option<Stop> {
    progress(workdir).map(|p| p.stop)
}

/// Whether `amend` (the contents of `rebase-merge/amend`) names HEAD.
fn amends_head(repo: &Repository, amend: &str) -> bool {
    let head = repo.head().ok().and_then(|h| h.target());
    Oid::from_str(amend.trim()).ok().is_some_and(|o| Some(o) == head)
}

pub(crate) fn progress_of(repo: &Repository) -> Option<RebaseProgress> {
    let state = repo.state();
    let read = |p: &Path| std::fs::read_to_string(p).unwrap_or_default();
    let mut p = match state {
        RepositoryState::RebaseInteractive | RepositoryState::RebaseMerge => {
            let dir = repo.path().join("rebase-merge");
            let done = read(&dir.join("done"));
            let step = counted(&done);
            let total = step + counted(&read(&dir.join("git-rebase-todo")));
            let (stop, command) = stop_of(repo, &done);
            RebaseProgress {
                step,
                total,
                stop,
                summary: String::new(),
                autostash: dir.join("autostash").exists(),
                interactive: state == RepositoryState::RebaseInteractive,
                command,
                amends: amends_head(repo, &read(&dir.join("amend"))),
            }
        }
        RepositoryState::Rebase | RepositoryState::ApplyMailboxOrRebase => {
            let dir = repo.path().join("rebase-apply");
            let number = |name: &str| read(&dir.join(name)).trim().parse::<usize>().unwrap_or(0);
            let (step, total) = (number("next"), number("last"));
            let kind = if conflicted(repo) { StopKind::Conflict } else { StopKind::Other };
            RebaseProgress {
                step: step.min(total),
                total,
                stop: Stop { kind, oid: None },
                summary: String::new(),
                autostash: dir.join("autostash").exists(),
                interactive: false,
                command: None,
                amends: false,
            }
        }
        _ => return None,
    };
    if let Some(c) = p.stop.oid.and_then(|o| repo.find_commit(o).ok()) {
        p.summary = commit_text::summary(&c).into_owned();
    }
    Some(p)
}

/// The commits a todo text lists: its pick-like lines and its `merge` lines.
fn counted(text: &str) -> usize {
    let merges = text.lines().filter(|l| matches!(l.split_whitespace().next(), Some("merge" | "m"))).count();
    listed_oids(text).len() + merges
}

fn conflicted(repo: &Repository) -> bool {
    repo.index().map(|i| i.has_conflicts()).unwrap_or(false)
}

/// The stop described by the last line of a merge-backend rebase's `done` file, with the
/// command of an exec step the user added that failed.
fn stop_of(repo: &Repository, done: &str) -> (Stop, Option<String>) {
    let lines: Vec<&str> = done.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).collect();
    let words = |l: &str| {
        let mut words = l.split_whitespace();
        let cmd = words.next().unwrap_or_default().to_owned();
        (cmd, words.find(|w| !w.starts_with('-')).map(str::to_owned))
    };
    let resolve = |sha: Option<&str>| sha.and_then(|s| repo.revparse_single(s).ok()).map(|o| o.id());
    // The commit of the last of these commands before the stop.
    let last_commit = |kinds: &[&str]| {
        let found = lines.iter().rev().skip(1).map(|l| words(l)).find(|(c, _)| kinds.contains(&c.as_str()));
        resolve(found.and_then(|(_, s)| s).as_deref())
    };
    const PICKS: [&str; 8] = ["pick", "p", "edit", "e", "reword", "r", "merge", "m"];
    const COMMITS: [&str; 12] = ["pick", "p", "edit", "e", "reword", "r", "squash", "s", "fixup", "f", "merge", "m"];
    let Some(&last) = lines.last() else { return (Stop { kind: StopKind::Other, oid: None }, None) };
    let (cmd, sha) = words(last);
    let stop = |kind, oid| (Stop { kind, oid }, None);
    match cmd.as_str() {
        "exec" | "x" if own_line(last).is_some() => stop(StopKind::Exec, last_commit(&PICKS)),
        "exec" | "x" => {
            let command = last.split_once(char::is_whitespace).map(|(_, rest)| rest.trim().to_owned()).unwrap_or_default();
            (Stop { kind: StopKind::Command, oid: last_commit(&COMMITS) }, Some(command))
        }
        "break" | "b" => stop(StopKind::Break, last_commit(&COMMITS)),
        "edit" | "e" => stop(StopKind::Edit, resolve(sha.as_deref())),
        // A merge redone with `merge -C <oid> …`: the oid is the first word that isn't a flag.
        "pick" | "p" | "reword" | "r" | "squash" | "s" | "fixup" | "f" | "merge" | "m" => {
            let kind = if conflicted(repo) { StopKind::Conflict } else { StopKind::Other };
            stop(kind, resolve(sha.as_deref()))
        }
        _ => stop(StopKind::Other, resolve(sha.as_deref())),
    }
}
