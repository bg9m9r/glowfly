//! Discarded work kept where git can't lose it, so it survives an unrelated job, a closed tab, a
//! restart or a crash, and the journal's Lost tab can list it under "Discarded changes".
//!
//! Each record is a commit referenced by `refs/glowfly/discarded/<its id>`, written before the
//! discard runs (and removed again when the discard turned out to change nothing): for a file,
//! its tree holds the bytes as they were (`file`); for a dropped stash, the stash commit is its
//! parent. Either way the content stays reachable, so `git gc` keeps it. The message's last line
//! is the note itself, one JSON object (`Note`): what was discarded, the path and blob, or the
//! stash and its list message. A record older than `KEEP_DAYS` is dropped at the next one,
//! except those made since this app run started: Undo's history is in memory only, so it can
//! name nothing older.

use super::{Discarded, Saved};
use crate::error::{Error, Result};
use git2::{ObjectType, Oid, Repository, Signature, Time};
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
    /// `path`'s exact bytes as they were, in `blob`.
    File { path: String, blob: Oid, executable: bool },
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
    let repo = Repository::open(workdir)?;
    let mut tree = repo.treebuilder(None)?;
    tree.insert("file", saved.blob, if saved.executable { 0o100755 } else { 0o100644 })?;
    let tree = tree.write()?;
    let note = Note {
        v: VERSION,
        what,
        path: Some(path.to_owned()),
        blob: Some(saved.blob.to_string()),
        executable: saved.executable,
        stash: None,
        message: None,
    };
    record(&repo, tree, None, &note, &format!("{path} as it was before the discard"), None)
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
    if note.v != VERSION {
        return None;
    }
    let oid = |s: &Option<String>| s.as_deref().and_then(|s| Oid::from_str(s).ok());
    let item = match note.what {
        KeptWhat::Stash => KeptItem::Stash { stash: oid(&note.stash)?, message: note.message.unwrap_or_default() },
        _ => KeptItem::File { path: note.path?, blob: oid(&note.blob)?, executable: note.executable },
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

/// The blob `path` would be now (`None`: absent or not a regular file).
fn blob_at(workdir: &Path, path: &str) -> Option<Oid> {
    let full = workdir.join(crate::repo::contained(path).ok()?);
    let meta = full.symlink_metadata().ok()?;
    if !meta.is_file() {
        return None;
    }
    Oid::hash_object(ObjectType::Blob, &std::fs::read(full).ok()?).ok()
}

/// The records still worth listing, newest first: not a file whose path holds those bytes again
/// (undone, restored), nor a stash back in the stash list; each path's bytes once.
pub fn list(workdir: &Path) -> Result<Vec<Kept>> {
    let repo = Repository::open(workdir)?;
    let stashed: HashSet<Oid> = match repo.reflog("refs/stash") {
        Ok(log) => log.iter().map(|e| e.id_new()).collect(),
        Err(_) => HashSet::new(),
    };
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for k in all(&repo)? {
        let shown = match &k.item {
            KeptItem::File { path, blob, .. } => seen.insert((path.clone(), *blob)) && blob_at(workdir, path) != Some(*blob),
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
/// new file next to it ("name (restored).ext"). Never writes over anything.
pub fn restore(workdir: &Path, id: Oid, copy: bool) -> Result<Restored> {
    let repo = Repository::open(workdir)?;
    let record = parse(&repo.find_reference(&format!("{KEPT_REF_PREFIX}{id}"))?.peel_to_commit()?);
    let Some(Kept { item: KeptItem::File { path, blob, executable }, .. }) = record else {
        return Err(Error::Unsupported("This record isn't a kept file".into()));
    };
    let full = workdir.join(crate::repo::contained(&path)?);
    let target = if copy {
        copy_name(workdir, &path)?
    } else if blob_at(workdir, &path) == Some(blob) {
        return Ok(Restored::AlreadyThere { path });
    } else if full.symlink_metadata().is_ok() {
        return Ok(Restored::Occupied { path });
    } else {
        path
    };
    let bytes = repo.find_blob(blob)?.content().to_vec();
    let file_mode = repo.config().and_then(|c| c.get_bool("core.fileMode")).unwrap_or(true);
    drop(repo);
    crate::repo::undelete_file(workdir, &target, &bytes, file_mode.then_some(executable), None)?;
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

/// A record dated `at`, for tests of pruning.
#[cfg(test)]
pub(super) fn keep_file_at(workdir: &Path, path: &str, saved: Saved, at: i64) -> Result<Oid> {
    let repo = Repository::open(workdir)?;
    let mut tree = repo.treebuilder(None)?;
    tree.insert("file", saved.blob, 0o100644)?;
    let tree = tree.write()?;
    let note = Note {
        v: VERSION,
        what: KeptWhat::File,
        path: Some(path.into()),
        blob: Some(saved.blob.to_string()),
        executable: false,
        stash: None,
        message: None,
    };
    record(&repo, tree, None, &note, path, Some(at))
}

/// Every record, also those `list` hides, for tests.
#[cfg(test)]
pub(super) fn all_for_test(workdir: &Path) -> Vec<Kept> {
    all(&Repository::open(workdir).unwrap()).unwrap()
}
