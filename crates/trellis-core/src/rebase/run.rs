//! Running an interactive rebase through the git CLI with a prepared todo list, and resuming a
//! stopped one (Continue / Skip).

use super::plan::{checked_head, range};
use super::progress::progress;
use super::todo::{MSG_DIR, build, editor_command, problem};
use super::{CHANGED, NOT_REBASING, Outcome, STALE, SequenceEditor, StartRequest, TodoStep, Upstream};
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
    let commits: Vec<Oid> = req.items.iter().filter_map(TodoStep::oid).collect();
    let listed: HashSet<Oid> = commits.iter().copied().collect();
    if expected.is_empty() || listed.len() != commits.len() || listed != expected {
        return Err(Error::Unsupported(CHANGED.into()));
    }
    if let Some(p) = problem(&req.items) {
        return Err(Error::Unsupported(p));
    }

    // Message files from an earlier rebase are removed only now: exec lines read them until a
    // stopped rebase finishes.
    let git_dir = repo.path().to_path_buf();
    let msg_dir = git_dir.join(MSG_DIR);
    if msg_dir.exists() {
        std::fs::remove_dir_all(&msg_dir)?;
    }
    std::fs::create_dir_all(&msg_dir)?;
    let (todo, files) = build(&req.items, &msg_dir, false);
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
    let mut args = gitcli::args(&["rebase", "-i"]);
    if req.autostash {
        args.push("--autostash".into());
    }
    args.extend(gitcli::args(&["--reapply-cherry-picks", &upstream]));
    let stash_before = stash_tip(&repo);
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
    outcome(workdir, ok, output, req.autostash, stash_before)
}

/// How to go on with a stopped rebase.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Resume {
    /// `git rebase --continue`: commit what is staged (keeping the message) and go on.
    Continue,
    /// `git rebase --skip`: leave the stopped commit out and go on.
    Skip,
}

/// Continues or skips the rebase in progress, then reads where it stands: finished, stopped
/// again (at an edit, a conflict, or because git refused to go on), or failed. Refused when no
/// rebase is in progress.
pub fn resume(workdir: &Path, how: Resume) -> Result<Outcome> {
    let repo = Repository::open(workdir)?;
    use RepositoryState::*;
    if !matches!(repo.state(), Rebase | RebaseInteractive | RebaseMerge | ApplyMailboxOrRebase) {
        return Err(Error::Unsupported(NOT_REBASING.into()));
    }
    let git_dir = repo.path();
    let had_stash = git_dir.join("rebase-merge").join("autostash").exists() || git_dir.join("rebase-apply").join("autostash").exists();
    let stash_before = stash_tip(&repo);
    let flag = match how {
        Resume::Continue => "--continue",
        Resume::Skip => "--skip",
    };
    let (ok, output) = gitcli::run(workdir, &gitcli::args(&["rebase", flag]));
    outcome(workdir, ok, output, had_stash, stash_before)
}

/// The stash list's newest entry, if any.
fn stash_tip(repo: &Repository) -> Option<Oid> {
    repo.refname_to_id("refs/stash").ok()
}

/// What a finished git run left: a rebase still in progress, none (with whether git had to
/// keep an autostash it couldn't apply in the stash list), or a failure.
fn outcome(workdir: &Path, ok: bool, output: String, autostash: bool, stash_before: Option<Oid>) -> Result<Outcome> {
    if let Some(progress) = progress(workdir) {
        return Ok(Outcome::Stopped { ok, output, progress });
    }
    let stash_kept = autostash && Repository::open(workdir).ok().and_then(|r| stash_tip(&r)) != stash_before;
    Ok(if ok { Outcome::Finished { output, stash_kept } } else { Outcome::Failed { output } })
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
