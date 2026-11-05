//! Discarded work kept where git can't lose it, so it survives an unrelated job, a closed tab, a
//! restart or a crash, and the journal's Lost tab can list it under "Discarded changes". Besides
//! discards, deletes and dropped stashes, records also keep the working copy a restore from a
//! commit or a conflict view save replaced.
//!
//! Each record is a commit referenced by `refs/glowfly/discarded/<its id>`, written before the
//! discard runs (and removed again when the discard turned out to change nothing): for a file,
//! its tree holds the bytes as they were (`file`; a symbolic link's target as a mode-120000
//! entry); for a dropped stash, the stash commit is its parent. Either way the content stays
//! reachable, so `git gc` keeps it. The message's last line is the note itself, one JSON object
//! (`Note`): what was discarded, the path and blob, or the stash and its list message. A record
//! older than `KEEP_DAYS` is dropped at the next one, except those made since this app run
//! started: Undo's history is in memory only, so it can name nothing older. Restore from the
//! journal is `kept_restore.rs`.

pub use super::kept_restore::{Restored, copy_name, restore};
use super::{Discarded, Saved};
use crate::error::{Error, Result};
use git2::{Oid, Repository, Signature, Time};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::sync::OnceLock;

/// Where each record is referenced, by its commit's id.
pub const KEPT_REF_PREFIX: &str = "refs/glowfly/discarded/";
/// How long a record stays.
pub const KEEP_DAYS: i64 = 30;
/// The note's format version.
pub const VERSION: u32 = 1;
/// The format version of a record of a symbolic link: older builds skip those rather than
/// restore the target as a plain file.
pub const LINK_VERSION: u32 = 2;
/// The most records listed (the newest).
pub const MAX_LISTED: usize = 200;

/// What a record kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeptWhat {
    /// A file's unstaged changes discarded.
    File,
    Hunk,
    Lines,
    /// Part of a commit's change reverted over the working copy.
    Revert,
    /// A working-tree file deleted.
    Delete,
    /// An untracked file discarded.
    Untracked,
    /// A stash dropped from the stash list.
    Stash,
    /// A file written over while resolving a conflict, or by undoing or redoing that (Mark
    /// conflicted, a whole side taken); also the working copy a conflict view save (a
    /// resolution or a save with markers) wrote over.
    Conflict,
    /// The working copy a restore from a commit replaced.
    Restore,
}

impl KeptWhat {
    /// What the record's message says the bytes were kept before.
    pub(crate) fn before(self) -> &'static str {
        match self {
            KeptWhat::File | KeptWhat::Hunk | KeptWhat::Lines | KeptWhat::Untracked => "the discard",
            KeptWhat::Revert => "the revert",
            KeptWhat::Delete => "the delete",
            KeptWhat::Conflict => "the conflict change",
            KeptWhat::Restore => "the restore",
            KeptWhat::Stash => "the drop",
        }
    }
}

impl From<Discarded> for KeptWhat {
    fn from(d: Discarded) -> Self {
        match d {
            Discarded::File => KeptWhat::File,
            Discarded::Hunk => KeptWhat::Hunk,
            Discarded::Lines(_) => KeptWhat::Lines,
            Discarded::RevertedHunk | Discarded::RevertedLines(_) | Discarded::RevertedFile => KeptWhat::Revert,
        }
    }
}

/// The kept content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeptItem {
    /// `path`'s exact bytes as they were, in `blob` (a symbolic link's target when `link`).
    File { path: String, blob: Oid, executable: bool, link: bool },
    /// The dropped stash commit and its list message.
    Stash { stash: Oid, message: String },
}

/// One record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kept {
    /// The record's commit.
    pub id: Oid,
    /// When it was kept, Unix seconds.
    pub at: i64,
    pub what: KeptWhat,
    pub item: KeptItem,
}

/// The note on a record's last message line.
#[derive(Debug, Serialize, Deserialize)]
struct Note {
    v: u32,
    what: KeptWhat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    blob: Option<String>,
    #[serde(default)]
    executable: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    link: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// When this app run kept its first record: no undo entry can name a record older than that.
fn run_started() -> i64 {
    static STARTED: OnceLock<i64> = OnceLock::new();
    *STARTED.get_or_init(now)
}

/// Keeps `path`'s bytes `saved` (what a discard of `what` is about to throw away). Returns the
/// record's id, for `forget` when the discard changed nothing.
pub(crate) fn keep_file(workdir: &Path, path: &str, saved: Saved, what: KeptWhat) -> Result<Oid> {
    keep_file_dated(workdir, path, saved, what, None)
}

/// `keep_file` dated `at` (now when `None`).
fn keep_file_dated(workdir: &Path, path: &str, saved: Saved, what: KeptWhat, at: Option<i64>) -> Result<Oid> {
    let repo = Repository::open(workdir)?;
    let mut tree = repo.treebuilder(None)?;
    tree.insert("file", saved.blob, saved.mode())?;
    let tree = tree.write()?;
    let note = Note {
        v: if saved.link { LINK_VERSION } else { VERSION },
        what,
        path: Some(path.to_owned()),
        blob: Some(saved.blob.to_string()),
        executable: saved.executable,
        link: saved.link,
        stash: None,
        message: None,
    };
    record(&repo, tree, None, &note, &format!("{path} as it was before {}", what.before()), at)
}

/// Keeps the stash commit `stash` (list `message`), about to be dropped.
pub(crate) fn keep_stash(workdir: &Path, stash: Oid, message: &str) -> Result<Oid> {
    let repo = Repository::open(workdir)?;
    let tree = repo.treebuilder(None)?.write()?;
    let note = Note {
        v: VERSION,
        what: KeptWhat::Stash,
        path: None,
        blob: None,
        executable: false,
        link: false,
        stash: Some(stash.to_string()),
        message: Some(message.to_owned()),
    };
    record(&repo, tree, Some(stash), &note, &format!("the dropped stash '{message}'"), None)
}

/// Writes one record (dated `at`, now when `None`) and prunes the old ones.
fn record(repo: &Repository, tree: Oid, parent: Option<Oid>, note: &Note, label: &str, at: Option<i64>) -> Result<Oid> {
    let sig = match at {
        Some(at) => Signature::new("Glowfly", "glowfly@localhost", &Time::new(at, 0))?,
        None => Signature::now("Glowfly", "glowfly@localhost")?,
    };
    let json = serde_json::to_string(note).map_err(|e| Error::Other(e.to_string()))?;
    let first: String = label.lines().next().unwrap_or_default().chars().take(200).collect();
    let message = format!("Glowfly: kept {first}\n\nThe journal's Lost tab in Glowfly restores it.\n\n{json}\n");
    let tree = repo.find_tree(tree)?;
    let parents = match parent {
        Some(p) => vec![repo.find_commit(p)?],
        None => Vec::new(),
    };
    let parents: Vec<&git2::Commit> = parents.iter().collect();
    let id = repo.commit(None, &sig, &sig, &message, &tree, &parents)?;
    prune_old(repo, run_started());
    repo.reference(&format!("{KEPT_REF_PREFIX}{id}"), id, true, "glowfly: kept discarded work")?;
    Ok(id)
}

/// Drops record `id` (a discard that changed nothing). A failure only leaves it to be pruned.
pub(crate) fn forget(workdir: &Path, id: Oid) {
    if let Ok(repo) = Repository::open(workdir)
        && let Ok(mut r) = repo.find_reference(&format!("{KEPT_REF_PREFIX}{id}"))
    {
        let _ = r.delete();
    }
}

/// Drops the records older than `KEEP_DAYS` that were made before `protect_since`. A failure
/// only leaves them for later.
fn prune_old(repo: &Repository, protect_since: i64) {
    let cutoff = now() - KEEP_DAYS * 86_400;
    let Ok(refs) = repo.references_glob(&format!("{KEPT_REF_PREFIX}*")) else { return };
    for mut r in refs.flatten() {
        let Ok(c) = r.peel_to_commit() else { continue };
        let at = c.time().seconds();
        if at < cutoff && at < protect_since {
            let _ = r.delete();
        }
    }
}

/// Reads the record `c`, `None` when it isn't one this version understands.
fn parse(c: &git2::Commit) -> Option<Kept> {
    let message = String::from_utf8_lossy(c.message_bytes()).into_owned();
    let line = message.lines().rev().find(|l| !l.trim().is_empty())?;
    let note: Note = serde_json::from_str(line).ok()?;
    if note.v != VERSION && note.v != LINK_VERSION {
        return None;
    }
    let oid = |s: &Option<String>| s.as_deref().and_then(|s| Oid::from_str(s).ok());
    let item = match note.what {
        KeptWhat::Stash => KeptItem::Stash { stash: oid(&note.stash)?, message: note.message.unwrap_or_default() },
        _ => KeptItem::File { path: note.path?, blob: oid(&note.blob)?, executable: note.executable, link: note.link },
    };
    Some(Kept { id: c.id(), at: c.time().seconds(), what: note.what, item })
}

/// Every record of the repository, newest first, whatever is in the working tree now.
fn all(repo: &Repository) -> Result<Vec<Kept>> {
    let mut out = Vec::new();
    for r in repo.references_glob(&format!("{KEPT_REF_PREFIX}*"))?.flatten() {
        if let Some(k) = r.peel_to_commit().ok().as_ref().and_then(parse) {
            out.push(k);
        }
    }
    out.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| a.id.cmp(&b.id)));
    Ok(out)
}

/// The record `id`, `None` when it isn't one this version understands.
pub(super) fn find(repo: &Repository, id: Oid) -> Result<Option<Kept>> {
    Ok(parse(&repo.find_reference(&format!("{KEPT_REF_PREFIX}{id}"))?.peel_to_commit()?))
}

/// The records still worth listing, newest first: not a file whose path holds those bytes again
/// (undone, restored; a link only as a link, see `Saved::held_by`), nor a stash back in the
/// stash list; each path's bytes once.
pub fn list(workdir: &Path) -> Result<Vec<Kept>> {
    let repo = Repository::open(workdir)?;
    let stashed: HashSet<Oid> = match repo.reflog("refs/stash") {
        Ok(log) => log.iter().map(|e| e.id_new()).collect(),
        Err(_) => HashSet::new(),
    };
    let links = crate::repo::workfile_unrm::symlinks(&repo);
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for k in all(&repo)? {
        let shown = match &k.item {
            KeptItem::File { path, blob, link, .. } => {
                let now = super::saved::current(workdir, path).ok().flatten();
                let kept = Saved { blob: *blob, executable: false, link: *link };
                seen.insert((path.clone(), *blob)) && !kept.held_by(now, links)
            }
            KeptItem::Stash { stash, .. } => seen.insert((String::new(), *stash)) && !stashed.contains(stash),
        };
        if shown {
            out.push(k);
        }
        if out.len() == MAX_LISTED {
            break;
        }
    }
    Ok(out)
}

/// A record dated `at`, for tests of pruning.
#[cfg(test)]
pub(super) fn keep_file_at(workdir: &Path, path: &str, saved: Saved, at: i64) -> Result<Oid> {
    keep_file_dated(workdir, path, saved, KeptWhat::File, Some(at))
}

/// Every record, also those `list` hides, for tests.
#[cfg(test)]
pub(super) fn all_for_test(workdir: &Path) -> Vec<Kept> {
    all(&Repository::open(workdir).unwrap()).unwrap()
}
