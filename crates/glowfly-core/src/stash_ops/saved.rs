//! The auto-stashes waiting to go back, kept across restarts in
//! `<worktree git dir>/glowfly/auto-stashes.json` (beside the journal notes, so each linked
//! worktree has its own: the stash list is shared, but the put-back targets one working tree).
//!
//! The file is `{"v":1,"stashes":[...]}`, each entry the operation, the stash's message and its
//! commit. Entries are read one by one: an unknown operation or a bad commit id skips that
//! entry, and a garbled file or another version reads as empty.

use super::AutoStashFor;
use crate::error::Result;
use crate::persist;
use git2::Oid;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The file's format version.
const VERSION: u32 = 1;

/// One auto-stash waiting to go back.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedAutoStash {
    pub op: AutoStashFor,
    /// The stash's message (`AutoStashFor::message`).
    pub message: String,
    /// The stash commit, as full hex.
    pub oid: String,
}

impl SavedAutoStash {
    pub fn new(op: AutoStashFor, message: String, oid: Oid) -> Self {
        Self { op, message, oid: oid.to_string() }
    }

    /// The stash commit, when `oid` is a full commit id.
    pub fn oid(&self) -> Option<Oid> {
        (self.oid.len() == 40).then(|| Oid::from_str(&self.oid).ok()).flatten()
    }
}

#[derive(Serialize)]
struct FileOut<'a> {
    v: u32,
    stashes: &'a [SavedAutoStash],
}

#[derive(Deserialize)]
struct FileIn {
    v: u32,
    #[serde(default)]
    stashes: Vec<serde_json::Value>,
}

/// Where the records of the repository (worktree) at `workdir` are kept.
pub fn path(workdir: &Path) -> Result<PathBuf> {
    Ok(crate::journal::notes::path(workdir)?.with_file_name("auto-stashes.json"))
}

/// The records kept for `workdir`; a missing, unreadable or garbled file reads as none.
pub fn read_saved(workdir: &Path) -> Vec<SavedAutoStash> {
    let Ok(file) = path(workdir) else { return Vec::new() };
    let Ok(text) = std::fs::read(&file) else { return Vec::new() };
    match serde_json::from_slice::<FileIn>(&text) {
        Ok(f) if f.v == VERSION => {
            f.stashes.into_iter().filter_map(|v| serde_json::from_value::<SavedAutoStash>(v).ok()).filter(|s| s.oid().is_some()).collect()
        }
        _ => Vec::new(),
    }
}

/// Replaces the records kept for `workdir` with `saved`; none removes the file.
pub fn write_saved(workdir: &Path, saved: &[SavedAutoStash]) -> std::io::Result<()> {
    let file = path(workdir).map_err(std::io::Error::other)?;
    if saved.is_empty() {
        return persist::remove_file(&file);
    }
    let text = serde_json::to_vec_pretty(&FileOut { v: VERSION, stashes: saved }).map_err(std::io::Error::other)?;
    persist::atomic_write(&file, &text)
}
