//! Undo and redo of a conflict resolution saved in the conflict view, and of a save with
//! markers. The conflict's stages come back from git's resolve-undo record (`git checkout -m`)
//! and go away again by staging the saved blob (`git update-index --cacheinfo`, which records
//! resolve-undo once more); the working file's exact bytes come from the blobs `saved.rs`
//! keeps. Every refusal is stale and drops only this entry: each older entry checks the
//! repository again when it is planned.

use super::record::head;
use super::{Action, Plan, Refusal, Saved, Staged, saved};
use crate::error::{Error, Result};
use crate::gitcli::args;
use crate::repo::{self, ConflictIds};
use git2::Repository;
use std::path::Path;

/// Runs `write` (a conflict view save of `path`, whose conflict has the stages `ids`) with the
/// working copy saved before and after it. Returns the action that undoes it, or `None` when a
/// save with markers changed nothing, or the file isn't one whose contents can be saved (a
/// symbolic link or a folder: `write` still runs). Any other failure to save first refuses the
/// write; a write that fails after changing the file says where its earlier contents are kept.
pub fn resolve_saving(workdir: &Path, path: &str, ids: &ConflictIds, write: impl FnOnce() -> Result<()>) -> Result<Option<Action>> {
    let before = match saved::save(workdir, path) {
        Ok(b) => b,
        Err(Error::Unsupported(_)) => {
            write()?;
            return Ok(None);
        }
        Err(e) => return Err(e),
    };
    // Closed before `write` runs git: Windows locks files that are held open.
    let at = head(&Repository::open(workdir)?).1;
    if let Err(e) = write() {
        return Err(saved::failed_after_saving(workdir, path, before, e).0);
    }
    let Ok(after) = saved::save(workdir, path) else { return Ok(None) };
    let staged = staged_entry(workdir, path)?;
    if staged.is_none() && before == after {
        return Ok(None);
    }
    Ok(Some(Action::Resolve { path: path.to_owned(), ids: ids.clone(), head: at, staged, before, after }))
}

/// The stage-0 entry of `path` when it is no longer conflicted.
fn staged_entry(workdir: &Path, path: &str) -> Result<Option<Staged>> {
    let repo = Repository::open(workdir)?;
    let index = repo.index()?;
    if index.conflict_get(Path::new(path)).is_ok() {
        return Ok(None);
    }
    Ok(index.get_path(Path::new(path), 0).map(|e| Staged { mode: e.mode, blob: e.id }))
}

fn expired(m: impl std::fmt::Display) -> Plan {
    Plan::Refuse(Refusal::Stale(format!("{m}; removed from the undo history")))
}

/// Checks the working file holds `expect`; on undo the refusal names the blob keeping `before`.
fn working_check(workdir: &Path, path: &str, expect: Option<Saved>, before: Option<Saved>, redo: bool) -> Result<Option<Plan>> {
    let now = match saved::current(workdir, path) {
        Ok(n) => Some(n),
        Err(Error::Unsupported(_)) => None,
        Err(e) => return Err(e),
    };
    if now.is_some_and(|n| n.map(|s| s.blob) == expect.map(|s| s.blob)) {
        return Ok(None);
    }
    let kept = match before {
        Some(b) if !redo => format!("; the earlier version is kept as blob {} (git cat-file -p {})", b.blob, b.blob),
        _ => String::new(),
    };
    Ok(Some(expired(format!("{path} has changed since{kept}"))))
}

/// Plans undoing (or redoing) a `Resolve` action. An operation in progress is expected, so it
/// doesn't refuse; anything else that changed since refuses as stale, with nothing written.
pub(super) fn plan(workdir: &Path, action: &Action, redo: bool) -> Result<Plan> {
    let Action::Resolve { path, ids, head: at, staged, before, after } = action else { unreachable!("a resolution") };
    let (head_now, conflict, stage0) = {
        let repo = Repository::open(workdir)?;
        let conflict = repo::conflict_ids(&repo, path);
        let stage0 = repo.index()?.get_path(Path::new(path), 0).map(|e| Staged { mode: e.mode, blob: e.id });
        (head(&repo).1, conflict, stage0)
    };
    if head_now != *at {
        return Ok(expired(format!("HEAD has moved since the {}", action.describe())));
    }
    let changed = || expired(format!("The conflict in {path} changed since it was saved"));
    let (path, before, after) = (path.clone(), *before, *after);
    match staged {
        Some(s) if !redo => {
            if conflict.is_some() || stage0 != Some(*s) {
                return Ok(expired(format!("{path} has changed in the index since")));
            }
            if let Some(refused) = working_check(workdir, &path, after, before, redo)? {
                return Ok(refused);
            }
            if repo::resolve_undo_ids(workdir, &path)?.as_ref() != Some(ids) {
                return Ok(expired(format!("git no longer keeps the conflict of {path}, so it can't be brought back")));
            }
            let cmds = vec![args(&["--literal-pathspecs", "checkout", "-m", "--", &path])];
            Ok(Plan::RunThenWrite { cmds, path, content: before })
        }
        Some(s) => {
            if conflict.as_ref() != Some(ids) {
                return Ok(changed());
            }
            if let Some(refused) = working_check(workdir, &path, before, before, redo)? {
                return Ok(refused);
            }
            let cmds = vec![
                args(&["update-index", "--cacheinfo", &format!("{:o},{},{}", s.mode, s.blob, path)]),
                args(&["--literal-pathspecs", "checkout-index", "--force", "-u", "--", &path]),
            ];
            Ok(Plan::RunThenWrite { cmds, path, content: after })
        }
        None => {
            if conflict.as_ref() != Some(ids) {
                return Ok(changed());
            }
            let expect = if redo { before } else { after };
            if let Some(refused) = working_check(workdir, &path, expect, before, redo)? {
                return Ok(refused);
            }
            Ok(Plan::Write { path, content: if redo { after } else { before } })
        }
    }
}

/// The output line for carrying out an undo (or redo) of a resolution (`full`) or of a save
/// with markers.
pub(super) fn wrote(path: &str, full: bool, redo: bool) -> String {
    match (full, redo) {
        (true, false) => format!("Brought back the conflict in {path}"),
        (true, true) => format!("Resolved {path} again"),
        (false, false) => format!("Put back the earlier working copy of {path}"),
        (false, true) => format!("Saved {path} with markers again"),
    }
}
