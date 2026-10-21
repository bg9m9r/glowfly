//! How much fetching a shallow clone's full history may download, for the confirmation before a
//! large one. No git protocol call reports a remote's size before fetching, so the estimate is
//! the hosting service's reported repository size (`forge::repo_size`) or, for a source on this
//! computer, the size of its object store on disk; anything else is unknown.

use std::path::{Path, PathBuf};

/// The size from which Fetch full history asks first: 500 MiB.
pub const LARGE_HISTORY_BYTES: u64 = 500 * 1024 * 1024;

/// The most directory entries [`local_store_bytes`] reads.
const MAX_ENTRIES: usize = 200_000;

/// Where a known size comes from.
#[derive(Clone, Debug, PartialEq)]
pub enum SizeSource {
    /// The hosting service reported it (its name, e.g. "GitHub").
    Service(String),
    /// A repository on this computer (the remote's URL as configured).
    Folder(String),
}

/// What fetching the full history may download.
#[derive(Clone, Debug, PartialEq)]
pub enum HistorySize {
    Known { bytes: u64, source: SizeSource },
    Unknown,
}

impl HistorySize {
    /// Large enough to ask before fetching ([`LARGE_HISTORY_BYTES`] or more).
    pub fn is_large(&self) -> bool {
        matches!(self, HistorySize::Known { bytes, .. } if *bytes >= LARGE_HISTORY_BYTES)
    }
}

/// The folder `url` names on this computer: a plain path (relative ones from `workdir`) or a
/// `file://` URL. None for anything else.
fn local_path(workdir: &Path, url: &str) -> Option<PathBuf> {
    let url = url.trim();
    if let Some(rest) = url.strip_prefix("file://") {
        let decoded = percent_encoding::percent_decode_str(rest).decode_utf8().ok()?;
        let b = decoded.as_bytes();
        // file:///C:/x names C:/x; file:///srv/x names /srv/x.
        let path = if b.len() >= 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':' { &decoded[1..] } else { &decoded[..] };
        return Some(PathBuf::from(path)).filter(|p| p.is_absolute());
    }
    if !crate::clone_ops::is_local_source(url) {
        return None;
    }
    let p = crate::paths::expand_home(url, crate::settings::home_dir().as_deref());
    Some(if p.is_relative() { workdir.join(p) } else { p })
}

/// The size on disk of the object store of the repository `url` names on this computer (its
/// pack files and loose objects), read without git. None when `url` isn't a local folder or
/// the folder isn't a repository. A huge store is counted up to `MAX_ENTRIES` entries.
pub fn local_store_bytes(workdir: &Path, url: &str) -> Option<u64> {
    let path = local_path(workdir, url)?;
    let repo = git2::Repository::open_ext(&path, git2::RepositoryOpenFlags::NO_SEARCH, std::iter::empty::<&std::ffi::OsStr>()).ok()?;
    let objects = repo.commondir().join("objects");
    let mut budget = MAX_ENTRIES;
    let mut total = 0u64;
    let mut sum = |dir: &Path, keep: &dyn Fn(&str) -> bool, budget: &mut usize| {
        let Ok(read) = std::fs::read_dir(dir) else { return };
        for e in read.flatten() {
            if *budget == 0 {
                return;
            }
            *budget -= 1;
            let name = e.file_name().to_string_lossy().into_owned();
            if keep(&name)
                && let Ok(m) = e.metadata()
                && m.is_file()
            {
                total += m.len();
            }
        }
    };
    sum(&objects.join("pack"), &|n| n.ends_with(".pack"), &mut budget);
    let loose: Vec<PathBuf> = std::fs::read_dir(&objects)
        .map(|r| {
            r.flatten()
                .filter(|e| {
                    let n = e.file_name().to_string_lossy().into_owned();
                    n.len() == 2 && n.bytes().all(|b| b.is_ascii_hexdigit()) && e.file_type().is_ok_and(|t| t.is_dir())
                })
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default();
    for dir in loose {
        sum(&dir, &|_| true, &mut budget);
    }
    Some(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{file_url, git, scratch_repo, write};

    fn packs(dir: &Path) -> u64 {
        std::fs::read_dir(dir.join(".git/objects/pack"))
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".pack"))
            .map(|e| e.metadata().unwrap().len())
            .sum()
    }

    #[test]
    fn a_local_source_is_its_object_store() {
        let src = scratch_repo("history-size-src");
        for i in 0..3 {
            write(&src, "a.txt", &format!("{i}\n"));
            git(&src, &["add", "."]);
            git(&src, &["commit", "-q", "-m", &format!("c{i}")]);
        }
        git(&src, &["repack", "-adq"]);
        let want = packs(&src);
        assert!(want > 0);
        let here = scratch_repo("history-size-here");
        assert_eq!(local_store_bytes(&here, &src.display().to_string()), Some(want), "a plain path");
        assert_eq!(local_store_bytes(&here, &file_url(&src)), Some(want), "a file:// URL");
        let relative = format!("../{}", src.file_name().unwrap().to_string_lossy());
        assert_eq!(local_store_bytes(&here, &relative), Some(want), "relative to the work tree");
        assert_eq!(local_store_bytes(&here, "https://github.com/o/r.git"), None);
        assert_eq!(local_store_bytes(&here, "git@github.com:o/r.git"), None);
        let plain = crate::paths::temp_dir().join(format!("glowfly-test-history-size-plain-{}", std::process::id()));
        std::fs::create_dir_all(&plain).unwrap();
        assert_eq!(local_store_bytes(&here, &plain.display().to_string()), None, "not a repository");
        for d in [&src, &here, &plain] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    #[test]
    fn large_from_500_mib() {
        let at = |bytes| HistorySize::Known { bytes, source: SizeSource::Folder("x".into()) };
        assert!(at(LARGE_HISTORY_BYTES).is_large());
        assert!(!at(LARGE_HISTORY_BYTES - 1).is_large());
        assert!(!HistorySize::Unknown.is_large());
    }
}
