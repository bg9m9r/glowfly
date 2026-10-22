//! Index paths that a file system can't keep apart: `a.txt` and `A.txt` on a case-insensitive
//! one (NTFS, APFS and HFS+ by default, FAT, exFAT, an ext4 casefold folder), or `café` spelled
//! with a composed and a decomposed é on one that normalizes Unicode (APFS, HFS+). Git keeps
//! both entries, but only one file is on disk for them, so discarding, deleting, renaming or
//! restoring one of them changes "the other" too.
//!
//! Whether this work tree folds case or normalization is asked of the file system itself (a
//! probe in the git dir, once per git dir per run), never assumed from the OS: Windows and macOS
//! folders can be case-sensitive, and Linux mounts can fold.

use super::{FileEntry, Hazard};
use crate::error::{Error, Result};
use git2::Repository;
use icu_normalizer::{ComposingNormalizerBorrowed, DecomposingNormalizerBorrowed};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Finding the collisions without scanning an unchanged index again.
#[path = "collide_scan.rs"]
mod scan;
pub(super) use scan::collisions;

/// What the work tree's file system treats as the same name, as probed; `None` when the probe
/// couldn't run (a read-only git dir).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Folding {
    /// `A.txt` opens `a.txt`.
    pub case: Option<bool>,
    /// A decomposed `é` (e + U+0301) opens the composed one.
    pub unicode: Option<bool>,
}

/// Index paths that are one name to some file system.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Collision {
    /// The spellings, sorted, at least two.
    pub paths: Vec<String>,
    /// Some of them differ by more than Unicode normalization (case).
    pub case: bool,
    /// Some of them differ only by Unicode normalization.
    pub unicode: bool,
}

impl Collision {
    /// Whether one file on this work tree stands for all of them, so acting on one reaches the
    /// others: the file system folds the way they differ (or the probe couldn't tell), or, for
    /// case, git itself is set to fold (`core.ignorecase`).
    pub fn shares_a_file(&self, folding: Folding, ignorecase: bool) -> bool {
        (self.case && (ignorecase || folding.case != Some(false))) || (self.unicode && folding.unicode != Some(false))
    }
}

pub(super) fn nfc(s: &str) -> String {
    ComposingNormalizerBorrowed::new_nfc().normalize(s).into_owned()
}

/// The name a case- and normalization-insensitive file system would see.
pub(super) fn key(path: &str) -> String {
    nfc(&nfc(path).to_lowercase())
}

/// The name a case- and normalization-insensitive volume sees for `path`: two paths with the
/// same key can name one file there.
pub fn name_key(path: &str) -> String {
    key(path)
}

/// Marks each of `rows` that is in a collision sharing one file here with `Hazard::Twins`.
pub(super) fn mark(rows: &mut [FileEntry], found: &[Collision], folding: Folding, ignorecase: bool) {
    if found.is_empty() {
        return;
    }
    let mut twins: HashMap<&str, &Collision> = HashMap::new();
    for c in found.iter().filter(|c| c.shares_a_file(folding, ignorecase)) {
        for p in &c.paths {
            twins.insert(p, c);
        }
    }
    for row in rows.iter_mut().filter(|r| r.hazard.is_none()) {
        if let Some(c) = twins.get(row.path.as_str()) {
            row.hazard = Some(Hazard::Twins(c.paths.iter().filter(|p| **p != row.path).cloned().collect()));
        }
    }
}

/// `core.ignorecase` as libgit2 reads it (false when unset).
pub(super) fn ignorecase(repo: &Repository) -> bool {
    repo.config().and_then(|c| c.get_bool("core.ignorecase")).unwrap_or(false)
}

/// Refuses `doing` (e.g. "discard") on `path` when another index path shares its file here.
/// Runs again at the moment of acting, so a stale view can't get past it.
pub(crate) fn refuse(workdir: &Path, path: &str, doing: &str) -> Result<()> {
    let repo = Repository::open(workdir)?;
    let mut index = super::exact::index(&repo)?;
    let k = key(path);
    let found = collisions(&mut index);
    let Some(c) = found.iter().find(|c| c.paths.iter().any(|p| key(p) == k)) else {
        // Not tracked under any spelling of this name, or no other spelling.
        return Ok(());
    };
    if !c.shares_a_file(folding(repo.path()), ignorecase(&repo)) {
        return Ok(());
    }
    let others: Vec<&str> = c.paths.iter().map(String::as_str).filter(|p| *p != path).collect();
    Err(Error::Other(format!(
        "can't {doing} {path}: this file system can't tell it from {} (one file on disk holds both). Rename one of them in git first",
        others.join(", ")
    )))
}

/// The probe for `git_dir`, run once per git dir per run and remembered.
pub(super) fn folding(git_dir: &Path) -> Folding {
    static SEEN: Mutex<BTreeMap<PathBuf, Folding>> = Mutex::new(BTreeMap::new());
    if let Some(f) = SEEN.lock().ok().and_then(|m| m.get(git_dir).copied()) {
        return f;
    }
    let f = probe(git_dir);
    if let Ok(mut m) = SEEN.lock() {
        m.insert(git_dir.to_path_buf(), f);
    }
    f
}

/// Creates a scratch file in `git_dir` and looks it up by its case-flipped and decomposed
/// names. Named `*.lock` so the file watcher ignores it, like git's own lock files.
pub fn probe(git_dir: &Path) -> Folding {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default();
    let name = format!("glowfly-probe-{}-{stamp}-\u{e9}.lock", std::process::id());
    let made = git_dir.join(&name);
    if std::fs::OpenOptions::new().write(true).create_new(true).open(&made).is_err() {
        return Folding::default();
    }
    let exists = |n: String| git_dir.join(n).symlink_metadata().is_ok();
    let f = Folding {
        case: Some(exists(name.to_uppercase())),
        unicode: Some(exists(DecomposingNormalizerBorrowed::new_nfd().normalize(&name).into_owned())),
    };
    let _ = std::fs::remove_file(&made);
    f
}
