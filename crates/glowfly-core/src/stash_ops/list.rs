//! Reading the stash list from the `refs/stash` reflog, newest first, each message decoded
//! lossily: git keeps the bytes it was given, and git2's `stash_foreach` panics on one that isn't
//! UTF-8. Each entry carries its stash commit's time (None when the commit is unreadable), not the
//! reflog entry's: a rename writes the entries again, the commits stay.

use crate::error::Result;
use crate::repo::StashInfo;
use git2::{ErrorCode, Repository};
use std::path::Path;

/// The reference whose reflog is the stash list.
const STASH_REF: &str = "refs/stash";

/// The stash list of the repository at `workdir`, newest first (as a repository load reads it).
pub fn list(workdir: &Path) -> Result<Vec<StashInfo>> {
    list_in(&Repository::open(workdir)?)
}

/// The stash list of `repo`: the same index (`stash@{n}`, 0 the newest), message and id that
/// libgit2's `git_stash_foreach` gives, except that a message that isn't UTF-8 has U+FFFD for
/// its bad bytes and a missing message is "". Empty without `refs/stash` (or with one that has
/// no reflog); an error when the ref or its log can't be read. `time` is the stash commit's
/// committer time, None when the log names an object that is gone or isn't a commit (Rename
/// rewrites the reflog entries, so their own times can't be used).
pub fn list_in(repo: &Repository) -> Result<Vec<StashInfo>> {
    match repo.find_reference(STASH_REF) {
        Ok(_) => {}
        Err(e) if e.code() == ErrorCode::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    }
    let log = repo.reflog(STASH_REF)?;
    Ok(log
        .iter()
        .enumerate()
        .map(|(index, e)| {
            let oid = e.id_new();
            StashInfo {
                index,
                message: e.message_bytes().map(|m| String::from_utf8_lossy(m).into_owned()).unwrap_or_default(),
                oid,
                time: repo.find_commit(oid).ok().map(|c| c.time().seconds()),
            }
        })
        .collect())
}
