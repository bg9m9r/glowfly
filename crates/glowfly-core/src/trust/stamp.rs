//! A cheap stamp of what a trust scan reads (`scan_now`): the size and modification time of
//! the repository's own config files and the files they include, of each file in the hooks
//! folder, of the commands file, and the commit HEAD is on (the version of the hooks that follow
//! the working tree). Stats only, no file is read: while the stamp is unchanged, the scan it was
//! taken with still stands, so frequent reads (a status read by git) can reuse it.

use super::TrustScan;
use git2::{ConfigLevel, Repository};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// The stamp of `repo` for its scan `scan` (whose `hooks_dir` says where hooks are listed).
pub fn stamp(repo: &Repository, scan: &TrustScan) -> u64 {
    let mut h = DefaultHasher::new();
    let mut files: Vec<PathBuf> = vec![repo.path().join("config"), repo.path().join("config.worktree"), repo.commondir().join("config")];
    files.extend(includes(repo));
    if let Some(w) = repo.workdir() {
        files.push(w.join(".glowfly").join("commands.toml"));
    }
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
    h.finish()
}

/// The files the repository's own config includes (`include.path`, `includeIf.*.path`), as
/// named; relative ones from the git dir.
fn includes(repo: &Repository) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(config) = repo.config() else { return out };
    for level in [ConfigLevel::Local, ConfigLevel::Worktree] {
        let Ok(c) = config.open_level(level) else { continue };
        let Ok(mut entries) = c.entries(None) else { continue };
        while let Some(Ok(e)) = entries.next() {
            let name = e.name().unwrap_or_default().to_ascii_lowercase();
            let included = name == "include.path" || (name.starts_with("includeif.") && name.ends_with(".path"));
            if let Some(v) = e.value().filter(|_| included) {
                let p = Path::new(v);
                out.push(if p.is_absolute() { p.to_path_buf() } else { repo.path().join(p) });
            }
        }
    }
    out
}

/// The size and time of `path`, and of what it links to (a hook linked elsewhere).
fn meta(path: &Path, h: &mut DefaultHasher) {
    for m in [std::fs::symlink_metadata(path), std::fs::metadata(path)] {
        match m {
            Ok(m) => {
                m.len().hash(h);
                m.modified().ok().hash(h);
                m.file_type().is_symlink().hash(h);
            }
            Err(_) => 0u8.hash(h),
        }
    }
}
