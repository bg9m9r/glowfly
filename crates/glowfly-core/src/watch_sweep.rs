//! A cheap sweep of a watched repository for changes a file watcher may have missed (Windows'
//! watcher drops events without saying so when its buffer overflows). The first sweep records
//! every non-ignored folder's listing; each later one stats those folders, lists again the ones
//! whose stamp changed (a file created, removed or renamed in a folder changes its time on NTFS,
//! ext4 and APFS), the ones that were too recent to trust last time (`RACY_WINDOW`) and a
//! rotating `1 / SWEEP_ROUNDS` of the rest (an edit to an existing file changes only the file),
//! and compares the git dir's refs, the index, exclude and attributes files, and the config files
//! with those they include (`config_includes`). It only names paths that may have changed: the
//! caller classifies them as watcher events, and the status read decides what really did.
//!
//! Submodules' and nested repositories' work trees are walked too, within the same budget, each
//! with its own ignore rules, and their HEAD and refs compared (`nested.rs`): a change there is
//! named as `<folder>/.git/HEAD`, which classification takes as the nested repository moving.
//!
//! Never walked: a `.git` entry (the repository's or a nested one's), the git dir, symbolic
//! links and junctions (kept as entries, never followed), and ignored folders that hold no
//! tracked files. Names are kept as the OS gives them, so a name that isn't UTF-8 still reaches
//! classification (which reads the whole status for it).

use crate::repo::{RACY_WINDOW, refs_key};
use crate::watch::{Ignores, Touch};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

mod nested;
use nested::Nested;

/// A content-only edit is found within this many sweeps: each lists a share of the folders.
pub const SWEEP_ROUNDS: usize = 10;

/// The most entries a sweep remembers; past it every sweep asks for a whole read.
pub const SWEEP_MAX_ENTRIES: usize = 1_000_000;

/// The sweeps of one repository (see the module doc).
pub struct Sweep {
    workdir: PathBuf,
    git_dir: PathBuf,
    common: Option<PathBuf>,
    max_entries: usize,
    seen: Option<Seen>,
    gave_up: bool,
}

/// What the last sweep found.
struct Seen {
    refs: Option<u64>,
    git_files: Vec<(PathBuf, Stamp)>,
    /// Every walked folder by its path relative to the work tree ("" for the root).
    folders: BTreeMap<PathBuf, Folder>,
    /// The repositories nested in the tree, by their folder relative to the work tree.
    nested: BTreeMap<PathBuf, Nested>,
    /// Where the next rotating share starts, as a position in `folders`.
    cursor: usize,
}

struct Folder {
    stamp: Stamp,
    /// Its own time or an entry's was too recent to trust: listed again next time.
    racy: bool,
    /// It holds a `.git` entry: a submodule or nested repository (never the root).
    repo: bool,
    /// Sorted by name.
    entries: Vec<Entry>,
}

#[derive(Clone, PartialEq)]
struct Entry {
    name: OsString,
    dir: bool,
    len: u64,
    mtime: Option<SystemTime>,
}

#[derive(Clone, Copy, PartialEq)]
struct Stamp {
    exists: bool,
    dir: bool,
    len: u64,
    mtime: Option<SystemTime>,
}

impl Stamp {
    fn of(p: &Path) -> Stamp {
        match std::fs::symlink_metadata(p) {
            Ok(m) => Stamp { exists: true, dir: m.is_dir(), len: m.len(), mtime: m.modified().ok() },
            Err(_) => Stamp { exists: false, dir: false, len: 0, mtime: None },
        }
    }
}

/// Whether a time is too close to `now` (or after it) for an unchanged stamp to prove anything.
fn recent(mtime: Option<SystemTime>, now: SystemTime) -> bool {
    mtime.is_some_and(|m| !now.duration_since(m).is_ok_and(|age| age >= RACY_WINDOW))
}

impl Sweep {
    /// `common`: a linked worktree's common git dir, whose `info/attributes` it reads too.
    pub fn new(workdir: &Path, git_dir: &Path, common: Option<&Path>) -> Self {
        Self::with_limit(workdir, git_dir, common, SWEEP_MAX_ENTRIES)
    }

    pub(crate) fn with_limit(workdir: &Path, git_dir: &Path, common: Option<&Path>, max_entries: usize) -> Self {
        let (workdir, git_dir) = (workdir.to_path_buf(), git_dir.to_path_buf());
        Self { workdir, git_dir, common: common.map(Path::to_path_buf), max_entries, seen: None, gave_up: false }
    }

    /// The paths (absolute) that may have changed since the last sweep, or None for "read
    /// everything": the first sweep, which only records, and every sweep of a tree past the limit.
    pub fn sweep(&mut self, ignores: &mut Ignores, now: SystemTime) -> Option<Vec<PathBuf>> {
        if self.gave_up {
            return None;
        }
        let Some(mut seen) = self.seen.take() else {
            self.seen = self.baseline(ignores, now);
            self.gave_up = self.seen.is_none();
            return None;
        };
        let mut found = Vec::new();
        let refs = refs_key(&self.workdir).ok();
        if refs != seen.refs {
            seen.refs = refs;
            found.push(self.git_dir.join("HEAD"));
        }
        for (p, stamp) in &mut seen.git_files {
            let now_stamp = Stamp::of(p);
            if now_stamp != *stamp {
                *stamp = now_stamp;
                found.push(p.clone());
            }
        }
        if !found.is_empty() {
            // The config or the branch changed: it may include other files now.
            seen.git_files = self.git_files_after(&seen.git_files);
        }
        nested::compare(&self.workdir, &mut seen.nested, &mut found);
        let fits = self.sweep_tree(&mut seen, ignores, now, &mut found);
        if !fits || entries(&seen.folders) > self.max_entries {
            self.gave_up = true;
            return None;
        }
        self.seen = Some(seen);
        Some(found)
    }

    /// The git dir's files a sweep compares: the index, exclude and attributes files, and the
    /// config files with the files they include.
    fn git_file_paths(&self) -> Vec<PathBuf> {
        let mut files = vec![self.git_dir.join("index"), self.git_dir.join("info/exclude"), self.git_dir.join("info/attributes")];
        files.extend(self.common.iter().map(|c| c.join("info/attributes")));
        files.extend(crate::config_includes::repo_roots(&self.git_dir, self.common.as_deref()));
        for f in crate::config_includes::of_repo(&self.git_dir, self.common.as_deref()).files {
            // A linked file is stamped as the link and as what it points to.
            let real = crate::paths::resolve(&f);
            if real != f {
                files.push(real);
            }
            files.push(f);
        }
        files
    }

    fn git_files(&self) -> Vec<(PathBuf, Stamp)> {
        self.git_file_paths().into_iter().map(|p| (p.clone(), Stamp::of(&p))).collect()
    }

    /// The files to compare from now on, keeping the stamps of those compared before.
    fn git_files_after(&self, before: &[(PathBuf, Stamp)]) -> Vec<(PathBuf, Stamp)> {
        let stamp = |p: &PathBuf| before.iter().find(|(q, _)| q == p).map_or_else(|| Stamp::of(p), |(_, s)| *s);
        self.git_file_paths().into_iter().map(|p| (p.clone(), stamp(&p))).collect()
    }

    fn baseline(&self, ignores: &mut Ignores, now: SystemTime) -> Option<Seen> {
        let refs = refs_key(&self.workdir).ok();
        let mut seen = Seen { refs, git_files: self.git_files(), folders: BTreeMap::new(), nested: BTreeMap::new(), cursor: 0 };
        self.walk(PathBuf::new(), &mut seen, ignores, now)?;
        Some(seen)
    }

    /// Lists `rel` and every folder below it that is walked into `seen`, with the repositories
    /// nested there; None past the limit.
    fn walk(&self, rel: PathBuf, seen: &mut Seen, ignores: &mut Ignores, now: SystemTime) -> Option<()> {
        let mut todo = vec![rel];
        let mut count = entries(&seen.folders);
        while let Some(rel) = todo.pop() {
            let Some(folder) = self.list(&rel, now) else { continue };
            count += folder.entries.len();
            if count > self.max_entries {
                return None;
            }
            if folder.repo {
                nested::add(&self.workdir, &mut seen.nested, &rel);
            }
            let dirs: Vec<PathBuf> = folder.entries.iter().filter(|e| e.dir).map(|e| rel.join(&e.name)).collect();
            todo.extend(dirs.into_iter().filter(|c| self.walked(c, ignores, &mut seen.nested)));
            seen.folders.insert(rel, folder);
        }
        Some(())
    }

    /// Whether the folder `rel` is walked: not the git dir, and not ignored (by the rules of the
    /// innermost repository it is in, a nested one's too) unless that repository's index holds
    /// something below it.
    fn walked(&self, rel: &Path, ignores: &mut Ignores, nested: &mut BTreeMap<PathBuf, Nested>) -> bool {
        if self.workdir.join(rel) == self.git_dir {
            return false;
        }
        let (ignores, rel) = match nested::owner(nested, rel) {
            Some((n, inner)) => (&mut n.ignores, inner),
            None => (ignores, rel),
        };
        let parts: Option<Vec<&str>> = rel.components().map(|c| c.as_os_str().to_str()).collect();
        // A name that isn't UTF-8 can't be matched against the patterns: walked.
        parts.is_none_or(|p| !ignores.is_ignored(&p.join("/"), Touch::Other))
    }

    /// The folder `rel` as it is now; None when it can't be listed.
    fn list(&self, rel: &Path, now: SystemTime) -> Option<Folder> {
        let abs = self.workdir.join(rel);
        let stamp = Stamp::of(&abs);
        let mut entries = Vec::new();
        let mut repo = false;
        for e in std::fs::read_dir(&abs).ok()?.flatten() {
            let name = e.file_name();
            if name == ".git" || e.path() == self.git_dir {
                repo |= name == ".git" && e.path() != self.git_dir && !rel.as_os_str().is_empty();
                continue;
            }
            let Ok(kind) = e.file_type() else { continue };
            let dir = kind.is_dir() && !kind.is_symlink();
            // Windows gives these from the listing itself; elsewhere it is one lstat.
            let meta = if dir { None } else { e.metadata().ok() };
            let len = meta.as_ref().map_or(0, |m| m.len());
            let mtime = meta.and_then(|m| m.modified().ok());
            entries.push(Entry { name, dir, len, mtime });
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        let racy = recent(stamp.mtime, now) || entries.iter().any(|e| recent(e.mtime, now));
        Some(Folder { stamp, racy, repo, entries })
    }

    /// Lists the folders due this time and walks the new ones; false once a new folder would take
    /// the tree past the limit (it isn't recorded, so the caller must give up).
    fn sweep_tree(&self, seen: &mut Seen, ignores: &mut Ignores, now: SystemTime, found: &mut Vec<PathBuf>) -> bool {
        let keys: Vec<PathBuf> = seen.folders.keys().cloned().collect();
        let n = keys.len();
        let share = n.div_ceil(SWEEP_ROUNDS);
        let start = if n == 0 { 0 } else { seen.cursor % n };
        let rotating = |i: usize| (i + n - start) % n.max(1) < share;
        for (i, rel) in keys.iter().enumerate() {
            let Some(folder) = seen.folders.get(rel) else { continue }; // dropped with a parent
            let abs = self.workdir.join(rel);
            let stamp = Stamp::of(&abs);
            if !stamp.exists || !stamp.dir {
                found.push(abs);
                drop_subtree(&mut seen.folders, rel);
                nested::drop_under(&mut seen.nested, rel);
                // Its parent wasn't listed this time (that would have dropped it first): its
                // own time can be unchanged (Linux stamps a folder from a coarse clock tick), so
                // record the change there too, or the parent's next listing reports it again.
                forget_in_parent(&mut seen.folders, rel, stamp);
                continue;
            }
            if stamp == folder.stamp && !folder.racy && !rotating(i) {
                continue;
            }
            let Some(fresh) = self.list(rel, now) else { continue };
            let (old, was_repo) = seen.folders.remove(rel).map(|f| (f.entries, f.repo)).unwrap_or_default();
            if fresh.repo != was_repo {
                // A repository made or removed here: the parent's status shows the folder anew.
                found.push(abs.clone());
                if fresh.repo {
                    nested::add(&self.workdir, &mut seen.nested, rel);
                } else {
                    seen.nested.remove(rel);
                }
            }
            let mut walk = Vec::new();
            diff(&old, &fresh.entries, |name, before, after| {
                let child = rel.join(name);
                match (before, after) {
                    (Some(b), Some(a)) if b.dir == a.dir => {
                        if !a.dir && (b.len, b.mtime) != (a.len, a.mtime) {
                            found.push(self.workdir.join(&child));
                        } else if a.dir && !seen.folders.contains_key(&child) {
                            walk.push(child); // not walked before: maybe tracked files now
                        }
                    }
                    (b, a) => {
                        found.push(self.workdir.join(&child));
                        if b.is_some_and(|b| b.dir) {
                            drop_subtree(&mut seen.folders, &child);
                            nested::drop_under(&mut seen.nested, &child);
                        }
                        if a.is_some_and(|a| a.dir) {
                            walk.push(child); // its own path covers what is below it
                        }
                    }
                }
            });
            seen.folders.insert(rel.clone(), fresh);
            for child in walk {
                if self.walked(&child, ignores, &mut seen.nested) && self.walk(child, seen, ignores, now).is_none() {
                    return false;
                }
            }
        }
        seen.cursor = start + share;
        true
    }
}

/// Calls `f` for each name in either sorted listing that differs or is in both, with its entry
/// before and after.
fn diff(old: &[Entry], new: &[Entry], mut f: impl FnMut(&OsString, Option<&Entry>, Option<&Entry>)) {
    let (mut i, mut j) = (0, 0);
    while i < old.len() || j < new.len() {
        match (old.get(i), new.get(j)) {
            (Some(a), Some(b)) if a.name == b.name => {
                f(&a.name, Some(a), Some(b));
                (i, j) = (i + 1, j + 1);
            }
            (Some(a), Some(b)) if a.name < b.name => {
                f(&a.name, Some(a), None);
                i += 1;
            }
            (Some(a), None) => {
                f(&a.name, Some(a), None);
                i += 1;
            }
            (_, Some(b)) => {
                f(&b.name, None, Some(b));
                j += 1;
            }
            (None, None) => break,
        }
    }
}

/// Forgets the folder `rel` and every folder below it.
fn drop_subtree(folders: &mut BTreeMap<PathBuf, Folder>, rel: &Path) {
    let below: Vec<PathBuf> = folders.range(rel.to_path_buf()..).map(|(k, _)| k).take_while(|k| k.starts_with(rel)).cloned().collect();
    for k in below {
        folders.remove(&k);
    }
}

/// Makes the parent's listing say what `rel` is now (`stamp`): gone, or no longer a folder.
fn forget_in_parent(folders: &mut BTreeMap<PathBuf, Folder>, rel: &Path, stamp: Stamp) {
    let (Some(parent), Some(name)) = (rel.parent(), rel.file_name()) else { return };
    let Some(folder) = folders.get_mut(parent) else { return };
    let Ok(i) = folder.entries.binary_search_by(|e| e.name.as_os_str().cmp(name)) else { return };
    if stamp.exists {
        folder.entries[i] = Entry { name: name.to_os_string(), dir: false, len: stamp.len, mtime: stamp.mtime };
    } else {
        folder.entries.remove(i);
    }
}

fn entries(folders: &BTreeMap<PathBuf, Folder>) -> usize {
    folders.values().map(|f| f.entries.len()).sum()
}

#[cfg(test)]
#[path = "watch_sweep_tests.rs"]
mod tests;
