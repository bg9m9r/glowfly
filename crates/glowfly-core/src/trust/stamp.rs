//! A cheap stamp of what a trust scan reads (`scan_now`): the size and modification time of
//! the repository's own config files and the files they include, of each file in the hooks
//! folder, of the commands file, of the todo and strategy files of a rebase or pick in progress
//! (`steps::FILES`), and the commit HEAD is on (the version of the hooks and config
//! programs that follow the working tree); the config files and hooks of its submodules' git
//! dirs (`repo::submodule_stamps`), and the commit each submodule that follows its own working
//! tree is on. Stats and small reads only: while the stamp is unchanged, the scan it was taken
//! with still stands, so frequent reads (a status read by git) can reuse it.

use super::TrustScan;
use git2::{ConfigLevel, Repository};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// The stamp of `repo` for its scan `scan` (whose `hooks_dir` says where hooks are listed).
pub fn stamp(repo: &Repository, scan: &TrustScan) -> u64 {
    let mut h = DefaultHasher::new();
    let mut files: Vec<PathBuf> = vec![repo.path().join("config"), repo.path().join("config.worktree"), repo.commondir().join("config")];
    let included = include_entries(repo);
    // An included file that isn't listed can't be stamped: no two stamps are then alike, so
    // every read scans again.
    if included.iter().any(|i| i.unsure) {
        static NEVER_ALIKE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        NEVER_ALIKE.fetch_add(1, std::sync::atomic::Ordering::Relaxed).hash(&mut h);
    }
    files.extend(included.into_iter().flat_map(|i| i.paths));
    if let Some(w) = repo.workdir() {
        files.push(w.join(".glowfly").join("commands.toml"));
    }
    files.extend(super::steps::FILES.iter().map(|f| repo.path().join(f)));
    for f in &files {
        meta(f, &mut h);
    }
    if let Some(dir) = &scan.hooks_dir {
        meta(dir, &mut h);
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir).map(|r| r.flatten().map(|e| e.path()).collect()).unwrap_or_default();
        entries.sort();
        for e in &entries {
            e.hash(&mut h);
            meta(e, &mut h);
        }
    }
    repo.head().ok().and_then(|r| r.target()).hash(&mut h);
    crate::repo::submodule_stamps(repo, std::time::SystemTime::now(), &mut h);
    if let Some(w) = repo.workdir() {
        for sub in scan.submodules.iter().filter(|s| !s.scan.follows.is_empty()) {
            let head = Repository::open(w.join(&sub.path)).ok().and_then(|r| r.head().ok().and_then(|r| r.target()));
            (&sub.path, head).hash(&mut h);
        }
    }
    h.finish()
}

/// One include setting of the repository's own config, with the files it may name.
pub(super) struct Include {
    /// The key as libgit2 spells it (`include.path`, `includeif.gitdir:x/.path`).
    pub name: String,
    pub value: String,
    pub paths: Vec<PathBuf>,
    /// Where the file is isn't listed (`paths` is empty): a relative include after
    /// `MAX_INCLUDES` others or after the folders to look in reached `MAX_FOLDERS`, or a value
    /// or folder longer than `MAX_PATH_BYTES`. The scan then follows the whole working tree
    /// (`programs.rs`) and no two stamps are alike.
    pub unsure: bool,
}

/// The most folders a relative include is looked up in.
const MAX_FOLDERS: usize = 32;
/// The most relative include settings whose files are listed.
const MAX_INCLUDES: usize = 256;
/// The longest include value, and folder to look in, that is followed.
const MAX_PATH_BYTES: usize = 4096;

/// Each include setting of the repository's own config (`include.path`, `includeIf.*.path`,
/// nested ones too, which libgit2 lists at the same level), in file order, with the files it
/// may name: `~` from the home folder, and a relative one from the git dir, from the common dir
/// (a linked worktree's shared config) and from beside each file already found (git resolves
/// it from the folder of the file that includes it), each folder once. Bounded by
/// `MAX_FOLDERS`, `MAX_INCLUDES` and `MAX_PATH_BYTES`; an include past one is `unsure`.
pub(super) fn include_entries(repo: &Repository) -> Vec<Include> {
    let mut out: Vec<Include> = Vec::new();
    let Ok(config) = repo.config() else { return out };
    // Where an included file may be: the git dir, the common dir and beside each file found.
    let mut folders = vec![repo.path().to_path_buf()];
    if repo.commondir() != repo.path() {
        folders.push(repo.commondir().to_path_buf());
    }
    let (mut full, mut listed) = (false, 0);
    for level in [ConfigLevel::Local, ConfigLevel::Worktree] {
        let Ok(c) = config.open_level(level) else { continue };
        let Ok(mut entries) = c.entries(None) else { continue };
        while let Some(Ok(e)) = entries.next() {
            let name = e.name().unwrap_or_default().to_owned();
            let lower = name.to_ascii_lowercase();
            let included = lower == "include.path" || (lower.starts_with("includeif.") && lower.ends_with(".path"));
            if !included {
                continue;
            }
            // A bare key names no file (git refuses one).
            let Some(v) = crate::config_entry::utf8(e) else { continue };
            let p = crate::paths::expand_home(v, crate::settings::home_dir().as_deref());
            let long = |p: &Path| p.as_os_str().len() > MAX_PATH_BYTES;
            let paths: Option<Vec<PathBuf>> = if long(&p) {
                None
            } else if p.is_absolute() {
                Some(vec![p])
            } else if full || listed >= MAX_INCLUDES {
                None
            } else {
                listed += 1;
                Some(folders.iter().map(|d| d.join(&p)).collect())
            };
            let unsure = paths.is_none();
            let paths = paths.unwrap_or_default();
            for folder in paths.iter().filter_map(|f| f.parent()) {
                if folders.iter().any(|d| d == folder) {
                    continue;
                }
                if folders.len() >= MAX_FOLDERS || long(folder) {
                    full = true;
                    break;
                }
                folders.push(folder.to_path_buf());
            }
            out.push(Include { name, value: v.to_owned(), paths, unsure });
        }
    }
    out
}

/// The size and times of `path`, and of what it links to (a hook linked elsewhere): its
/// creation time too, and on Unix its inode and change time, so a file replaced by another
/// (an atomic write) changes the stamp even with the same size and modification time.
fn meta(path: &Path, h: &mut DefaultHasher) {
    for m in [std::fs::symlink_metadata(path), std::fs::metadata(path)] {
        match m {
            Ok(m) => {
                m.len().hash(h);
                m.modified().ok().hash(h);
                m.created().ok().hash(h);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    (m.ino(), m.ctime(), m.ctime_nsec()).hash(h);
                }
                m.file_type().is_symlink().hash(h);
            }
            Err(_) => 0u8.hash(h),
        }
    }
}

#[cfg(test)]
#[path = "stamp_tests.rs"]
mod tests;
