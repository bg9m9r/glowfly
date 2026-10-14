//! A cheap fingerprint of everything a full load reads besides the working-tree status: refs,
//! HEAD, stashes, the repository state, config and linked worktrees. When it hasn't changed
//! since the last load, re-reading the status is enough and the history needn't be walked again.

use crate::error::Result;
use git2::Repository;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::time::SystemTime;

/// The fingerprint of the repository at `workdir` (see the module docs). Only meaningful when
/// compared with another taken in the same run of the app.
pub fn refs_key(workdir: &Path) -> Result<u64> {
    key_of(&Repository::discover(workdir)?)
}

pub(super) fn key_of(repo: &Repository) -> Result<u64> {
    let mut h = DefaultHasher::new();
    (repo.state() as u32).hash(&mut h);
    match repo.find_reference("HEAD") {
        Ok(head) => {
            head.symbolic_target_bytes().hash(&mut h);
            head.target().hash(&mut h);
            repo.head().ok().and_then(|r| r.target()).hash(&mut h);
        }
        Err(_) => 0u8.hash(&mut h),
    }
    for r in repo.references()? {
        let r = r?;
        r.name_bytes().hash(&mut h);
        r.target().hash(&mut h);
        r.symbolic_target_bytes().hash(&mut h);
    }
    let common = repo.commondir();
    // Dropping stash@{1} only rewrites the stash's reflog.
    file_stamp(&common.join("logs/refs/stash")).hash(&mut h);
    // Remotes, upstreams, gitflow and the commit template.
    file_stamp(&common.join("config")).hash(&mut h);
    if let Some(w) = repo.workdir() {
        // The root .gitattributes' LFS patterns.
        file_stamp(&w.join(".gitattributes")).hash(&mut h);
    }
    // Linked worktrees: added, removed, locked or switched to another branch.
    if let Ok(dir) = std::fs::read_dir(common.join("worktrees")) {
        let mut names: Vec<_> = dir.flatten().map(|e| e.path()).collect();
        names.sort();
        for p in names {
            p.hash(&mut h);
            std::fs::read(p.join("HEAD")).ok().hash(&mut h);
            let gitdir = std::fs::read_to_string(p.join("gitdir")).ok();
            // A worktree whose folder is gone shows as missing.
            gitdir.as_deref().map(|g| Path::new(g.trim()).exists()).hash(&mut h);
            gitdir.hash(&mut h);
            p.join("locked").exists().hash(&mut h);
        }
    }
    Ok(h.finish())
}

/// The size and modification time of the file at `p`, or None when there is none.
fn file_stamp(p: &Path) -> Option<(u64, Option<SystemTime>)> {
    let m = std::fs::metadata(p).ok()?;
    Some((m.len(), m.modified().ok()))
}
