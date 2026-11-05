//! Restore of a kept file or symbolic link from the journal (`kept.rs`), never over anything:
//! at its own path when nothing is there, or as a copy next to it.

use super::kept::{Kept, KeptItem, find};
use super::saved;
use crate::error::{Error, Result};
use git2::{Oid, Repository};
use std::path::Path;

/// How a Restore of a kept file went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Restored {
    /// Written at `path` (its own path, or the copy's next to it).
    Wrote { path: String },
    /// Its path already holds exactly those bytes; nothing was written.
    AlreadyThere { path: String },
    /// Something else is at its path, which Restore never replaces; nothing was written.
    Occupied { path: String },
}

/// Writes the kept file `id` back: at its own path when nothing is there (`copy` false), or as a
/// new file next to it ("name (restored).ext"). Never writes over anything. A kept link is made
/// as a link again (a plain file holding its target when `core.symlinks` is false).
pub fn restore(workdir: &Path, id: Oid, copy: bool) -> Result<Restored> {
    let repo = Repository::open(workdir)?;
    let Some(Kept { item: KeptItem::File { path, blob, executable, link }, .. }) = find(&repo, id)? else {
        return Err(Error::Unsupported("This record isn't a kept file".into()));
    };
    let full = workdir.join(crate::repo::contained(&path)?);
    let target = if copy {
        copy_name(workdir, &path)?
    } else if saved::current(workdir, &path).ok().flatten().is_some_and(|s| s.content() == (blob, link)) {
        return Ok(Restored::AlreadyThere { path });
    } else if full.symlink_metadata().is_ok() {
        return Ok(Restored::Occupied { path });
    } else {
        path
    };
    let bytes = repo.find_blob(blob)?.content().to_vec();
    let file_mode = repo.config().and_then(|c| c.get_bool("core.fileMode")).unwrap_or(true);
    drop(repo);
    if link {
        crate::repo::workfile_unrm::undelete_link(workdir, &target, &bytes, None)?;
    } else {
        crate::repo::undelete_file(workdir, &target, &bytes, file_mode.then_some(executable), None)?;
    }
    Ok(Restored::Wrote { path: target })
}

/// The first free "name (restored).ext", "name (restored 2).ext", … next to `path`.
pub fn copy_name(workdir: &Path, path: &str) -> Result<String> {
    let (dir, name) = path.rsplit_once('/').map_or(("", path), |(d, n)| (d, n));
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    for n in 1..=100 {
        let tag = if n == 1 { "restored".to_owned() } else { format!("restored {n}") };
        let file = format!("{stem} ({tag}){ext}");
        let candidate = if dir.is_empty() { file } else { format!("{dir}/{file}") };
        if workdir.join(crate::repo::contained(&candidate)?).symlink_metadata().is_err() {
            return Ok(candidate);
        }
    }
    Err(Error::Other(format!("Too many restored copies of {path} are already next to it")))
}
