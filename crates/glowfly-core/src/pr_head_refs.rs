//! Glowfly's refs that keep a pull request's head while its panel shows it. They are written by
//! the head fetch (`remote_ops::fetch_ref_cmd`'s destination), or by `keep_head` when the listed
//! head is already here. They are deleted once no panel shows the pull request, and leftovers
//! are swept when a repository opens. libgit2 only: no program runs.

use crate::error::Result;
use git2::{ErrorCode, Oid, Repository};
use std::path::Path;

/// Where a pull request's head is kept: `refs/glowfly/pr/<id>`. Every ref under refs/ is a root
/// for `git gc`, so the commits survive while the ref does.
pub const PR_HEAD_REF_PREFIX: &str = "refs/glowfly/pr/";

/// The ref that keeps pull request `id`'s head.
pub fn pr_head_ref(id: u64) -> String {
    format!("{PR_HEAD_REF_PREFIX}{id}")
}

/// Points `refs/glowfly/pr/<id>` at the commit `oid` (here already), unless it does so.
pub fn keep_head(workdir: &Path, id: u64, oid: Oid) -> Result<()> {
    let repo = Repository::open(workdir)?;
    let name = pr_head_ref(id);
    if repo.refname_to_id(&name).is_ok_and(|at| at == oid) {
        return Ok(());
    }
    repo.find_commit(oid)?;
    repo.reference(&name, oid, true, "glowfly: keep pull request head")?;
    Ok(())
}

/// Deletes the refs of pull requests `ids` (a missing one is fine) with their reflogs, which
/// would keep the objects alive too. Tries every id and returns the first error.
pub fn remove_heads(workdir: &Path, ids: &[u64]) -> Result<()> {
    let repo = Repository::open(workdir)?;
    let mut first = None;
    for id in ids {
        if let Err(e) = remove(&repo, &pr_head_ref(*id)) {
            first.get_or_insert(e);
        }
    }
    first.map_or(Ok(()), Err)
}

/// Deletes every ref under the prefix whose pull request isn't in `keep` (a name that isn't a
/// number goes too). Returns how many went; tries all and returns the first error.
pub fn sweep_heads(workdir: &Path, keep: &[u64]) -> Result<usize> {
    let repo = Repository::open(workdir)?;
    // Collected first, so nothing is deleted while the iterator is open.
    let mut names = Vec::new();
    for r in repo.references_glob(&format!("{PR_HEAD_REF_PREFIX}*"))? {
        if let Some(name) = r?.name() {
            names.push(name.to_owned());
        }
    }
    let (mut gone, mut first) = (0, None);
    for name in names {
        let id = name.strip_prefix(PR_HEAD_REF_PREFIX).and_then(|s| s.parse::<u64>().ok());
        if id.is_some_and(|id| keep.contains(&id)) {
            continue;
        }
        match remove(&repo, &name) {
            Ok(()) => gone += 1,
            Err(e) => {
                first.get_or_insert(e);
            }
        }
    }
    first.map_or(Ok(gone), Err)
}

/// Deletes the ref `name` and its reflog; a ref that isn't there is fine.
fn remove(repo: &Repository, name: &str) -> Result<()> {
    match repo.find_reference(name) {
        Ok(mut r) => r.delete()?,
        Err(e) if e.code() == ErrorCode::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    // With core.logAllRefUpdates=always the ref has a log, which protects objects until it expires.
    let _ = repo.reflog_delete(name);
    Ok(())
}

#[cfg(test)]
mod tests;
