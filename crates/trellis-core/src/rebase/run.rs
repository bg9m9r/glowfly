//! Running an interactive rebase through the git CLI with a prepared todo list, and reading
//! where a stopped rebase is.

use super::plan::{checked_head, range};
use super::todo::{build, editor_command, problem};
use super::{CHANGED, Outcome, STALE, SequenceEditor, StartRequest, Stop, StopKind, Upstream};
use crate::error::{Error, Result};
use crate::gitcli;
use git2::{Oid, Repository, RepositoryState};
use std::collections::HashSet;
use std::path::Path;

/// Runs `git rebase -i` with `req.items` as the todo list. `Err` means nothing was changed:
/// HEAD moved since the list was made, the commits no longer match, the list has a problem, or
/// the sequence editor refused the todo.
pub fn start(workdir: &Path, req: &StartRequest, editor: &SequenceEditor) -> Result<Outcome> {
    let repo = Repository::open(workdir)?;
    let (head, _) = checked_head(&repo)?;
    if head != req.head {
        return Err(Error::Unsupported(STALE.into()));
    }
    let expected: HashSet<Oid> = range(&repo, head, req.upstream)?.into_iter().collect();
    let listed: HashSet<Oid> = req.items.iter().map(|i| i.oid).collect();
    if expected.is_empty() || listed.len() != req.items.len() || listed != expected {
        return Err(Error::Unsupported(CHANGED.into()));
    }
    if let Some(p) = problem(&req.items) {
        return Err(Error::Unsupported(p));
    }

    // Message files from an earlier rebase are removed only now: exec lines read them until a
    // stopped rebase finishes.
    let git_dir = repo.path().to_path_buf();
    let msg_dir = git_dir.join("trellis-rebase");
    if msg_dir.exists() {
        std::fs::remove_dir_all(&msg_dir)?;
    }
    std::fs::create_dir_all(&msg_dir)?;
    let (todo, files) = build(&req.items, &msg_dir);
    for (path, text) in files {
        crate::persist::atomic_write(&path, text.as_bytes())?;
    }
    let prepared = msg_dir.join("todo");
    crate::persist::atomic_write(&prepared, todo.as_bytes())?;

    let upstream = match req.upstream {
        Upstream::Commit(o) => o.to_string(),
        Upstream::Root => "--root".to_owned(),
    };
    // Our list replaces git's, so git must list the same commits: keep the ones it would
    // otherwise drop up front as already applied upstream.
    let args = gitcli::args(&["rebase", "-i", "--reapply-cherry-picks", &upstream]);
    let command = editor_command(editor, &prepared);
    let (ok, output) = gitcli::run_with_input(workdir, &args, "", &[("GIT_SEQUENCE_EDITOR", &command)]);

    let refused = msg_dir.join("todo.error");
    if refused.exists() {
        abort_unstarted(workdir, &git_dir);
        let repo = Repository::open(workdir)?;
        let unchanged = repo.head().ok().and_then(|h| h.target()) == Some(head);
        if unchanged && repo.state() == RepositoryState::Clean {
            let text = std::fs::read_to_string(&refused).unwrap_or_default();
            return Err(Error::Unsupported(text.trim().to_owned()));
        }
    } else if !ok {
        abort_unstarted(workdir, &git_dir);
    }
    Ok(match stopped_at(workdir) {
        Some(stop) => Outcome::Stopped { ok, output, stop },
        None if ok => Outcome::Finished { output },
        None => Outcome::Failed { output },
    })
}

/// A rebase that git set up but never began (no step done): abort it, so nothing is left in
/// progress.
fn abort_unstarted(workdir: &Path, git_dir: &Path) {
    let dir = git_dir.join("rebase-merge");
    if !dir.is_dir() {
        return;
    }
    let done = std::fs::read_to_string(dir.join("done")).unwrap_or_default();
    if done.trim().is_empty() {
        let _ = gitcli::run(workdir, &gitcli::args(&["rebase", "--abort"]));
    }
}

/// Where a stopped interactive rebase is: the last step done, and whether it conflicted.
/// `None` unless an interactive (or merge-based) rebase is in progress.
pub fn stopped_at(workdir: &Path) -> Option<Stop> {
    let repo = Repository::open(workdir).ok()?;
    if !matches!(repo.state(), RepositoryState::RebaseInteractive | RepositoryState::RebaseMerge) {
        return None;
    }
    let done = std::fs::read_to_string(repo.path().join("rebase-merge").join("done")).unwrap_or_default();
    let steps: Vec<(&str, Option<&str>)> = done
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let mut words = l.split_whitespace();
            let cmd = words.next().unwrap_or_default();
            (cmd, words.find(|w| !w.starts_with('-')))
        })
        .collect();
    let resolve = |sha: Option<&str>| sha.and_then(|s| repo.revparse_single(s).ok()).map(|o| o.id());
    let conflicted = || repo.index().map(|i| i.has_conflicts()).unwrap_or(false);
    let Some(&(cmd, sha)) = steps.last() else { return Some(Stop { kind: StopKind::Other, oid: None }) };
    Some(match cmd {
        "exec" | "x" => {
            let commit = steps.iter().rev().find(|(c, _)| matches!(*c, "pick" | "p" | "edit" | "e" | "reword" | "r"));
            Stop { kind: StopKind::Exec, oid: resolve(commit.and_then(|(_, s)| *s)) }
        }
        "edit" | "e" => Stop { kind: StopKind::Edit, oid: resolve(sha) },
        "pick" | "p" | "reword" | "r" | "squash" | "s" | "fixup" | "f" => {
            let kind = if conflicted() { StopKind::Conflict } else { StopKind::Other };
            Stop { kind, oid: resolve(sha) }
        }
        _ => Stop { kind: StopKind::Other, oid: resolve(sha) },
    })
}
