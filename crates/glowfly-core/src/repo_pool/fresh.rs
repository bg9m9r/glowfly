//! Whether an idle handle may be lent again (`repo_pool`): what libgit2 loads once per handle
//! and never checks again must still be as it was when the handle was opened.
//!
//! - The config files it read (the repository's own, the worktree's, and the global, XDG,
//!   system and ProgramData ones), and the files those include (`config_includes`, with HEAD
//!   when an `onbranch:` condition makes the choice depend on the branch): libgit2 keeps
//!   `core.*` values (line endings, file mode, case) from them for the life of the handle.
//! - The git dir and the work tree still there, the work tree's `.git` link (a moved or
//!   repaired worktree) and `objects/info/alternates`.
//!
//! A file counts as unchanged only by a size and time old enough to trust (`RACY_WINDOW`), so
//! a change made around the time a stamp is taken always shows up as a difference. Refs,
//! packed refs, attributes, ignore files and the shallow file libgit2 checks on every read; a
//! new pack it finds when an object is missing. The index libgit2 reads again only where a call
//! asks for it (a status, a diff from the index), not where it is read through
//! `Repository::index` (path status, attributes), so a `Use::Workdir` handle's index is
//! brought up to date here whenever the file's size, time or trailer changed.

use git2::Repository;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::SystemTime;

/// What one watched path looked like.
#[derive(Clone, Debug)]
enum Stamp {
    Missing,
    Dir,
    /// A file's size and modification time, old enough to trust.
    File(u64, SystemTime),
    /// A file modified too recently to trust: never equal to anything.
    Unsettled,
}

impl PartialEq for Stamp {
    fn eq(&self, other: &Stamp) -> bool {
        match (self, other) {
            (Stamp::Missing, Stamp::Missing) | (Stamp::Dir, Stamp::Dir) => true,
            (Stamp::File(a, x), Stamp::File(b, y)) => a == b && x == y,
            _ => false,
        }
    }
}

fn stamp(p: &Path, now: SystemTime) -> Stamp {
    match std::fs::metadata(p) {
        Err(_) => Stamp::Missing,
        Ok(m) if m.is_dir() => Stamp::Dir,
        Ok(m) => match m.modified() {
            Ok(t) if crate::repo::stamp_trustworthy(Some(t), now) => Stamp::File(m.len(), t),
            _ => Stamp::Unsettled,
        },
    }
}

/// The index file's size, time and last 20 bytes (its checksum, or zeros); None when it is
/// missing, unreadable, shorter than that or modified too recently to trust.
#[derive(Clone, Debug, PartialEq, Eq)]
struct IndexStamp(u64, SystemTime, [u8; 20]);

fn index_stamp(repo: &Repository, now: SystemTime) -> Option<IndexStamp> {
    let mut f = std::fs::File::open(repo.path().join("index")).ok()?;
    let m = f.metadata().ok()?;
    let t = m.modified().ok()?;
    if !crate::repo::stamp_trustworthy(Some(t), now) {
        return None;
    }
    Some(IndexStamp(m.len(), t, trailer(&mut f)?))
}

fn trailer(f: &mut std::fs::File) -> Option<[u8; 20]> {
    let mut trailer = [0; 20];
    f.seek(SeekFrom::End(-20)).ok()?;
    f.read_exact(&mut trailer).ok()?;
    Some(trailer)
}

/// Brings `repo`'s index up to date with the file. libgit2's own check, the checksum, is
/// enough (one written under `index.skipHash` has none, which never matches, so it is read),
/// except for a file that is gone: that one only empties the copy when forced.
fn reread_index(repo: &Repository) -> bool {
    let gone = !repo.path().join("index").exists();
    repo.index().and_then(|mut i| i.read(gone)).is_ok()
}

/// The config files outside the repository that every handle reads.
fn outside_config() -> &'static [PathBuf] {
    static FILES: OnceLock<Vec<PathBuf>> = OnceLock::new();
    FILES.get_or_init(|| {
        // libgit2 also reads `%PROGRAMDATA%\Git\config` on Windows; git2 has no finder for it.
        let programdata = std::env::var_os("PROGRAMDATA").filter(|_| cfg!(windows)).map(|d| PathBuf::from(d).join("Git").join("config"));
        [git2::Config::find_global().ok(), git2::Config::find_xdg().ok(), git2::Config::find_system().ok(), programdata]
            .into_iter()
            .flatten()
            .collect()
    })
}

fn watched(repo: &Repository) -> Vec<PathBuf> {
    let (git_dir, common) = (repo.path(), repo.commondir());
    let mut v = vec![git_dir.to_owned(), common.join("config"), git_dir.join("config.worktree"), common.join("objects/info/alternates")];
    if let Some(work) = repo.workdir() {
        v.extend([work.to_owned(), work.join(".git")]);
    }
    v.extend(outside_config().iter().cloned());
    // libgit2 reads an included file as part of the one that includes it.
    let mut roots = crate::config_includes::repo_roots(git_dir, Some(common));
    roots.extend(outside_config().iter().cloned());
    let included = crate::config_includes::included(&roots, &crate::config_includes::Context::of_repo(git_dir));
    if included.on_branch {
        v.push(git_dir.join("HEAD"));
    }
    v.extend(included.files);
    v
}

/// The stamps a handle is checked against before it is lent again.
#[derive(Debug)]
pub(super) struct Fresh {
    files: Vec<(PathBuf, Stamp)>,
    /// The index file as it was at the latest point the handle's copy (if read at all) is known
    /// to be at least as new as (`Use::Workdir` only).
    index: Option<IndexStamp>,
}

impl Fresh {
    /// Taken just after `repo` was opened, before anything read its index: whenever that is
    /// read, it is at least as new as the stamp.
    pub(super) fn taken(repo: &Repository, index: bool) -> Fresh {
        let now = SystemTime::now();
        let files = watched(repo)
            .into_iter()
            .map(|p| {
                let s = stamp(&p, now);
                (p, s)
            })
            .collect();
        let index = if index && !repo.is_bare() { index_stamp(repo, now) } else { None };
        Fresh { files, index }
    }

    /// Whether `repo` still sees what a new handle would; with `index`, its index is brought
    /// up to date first. False: drop it and open another.
    pub(super) fn still(&mut self, repo: &Repository, index: bool) -> bool {
        let now = SystemTime::now();
        if !self.files.iter().all(|(p, s)| *s == stamp(p, now)) {
            return false;
        }
        if index && !repo.is_bare() {
            let s = index_stamp(repo, now);
            if s.is_none() || s != self.index {
                if !reread_index(repo) {
                    return false;
                }
                self.index = s;
            }
        }
        true
    }
}
