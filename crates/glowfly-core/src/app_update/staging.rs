//! The private folder an update is prepared in: `<temp>/glowfly-update-<16 hex>`, made fresh
//! (never reused, so an existing name or a planted link fails) and, on Unix, readable by this
//! user only. It holds the helper (a copy of the running program), its plan and an MSI's copy.
//! When the helper can't run from the temporary folder (mounted noexec on Linux or macOS), the
//! next place is tried: beside the program, then the cache folder ([`create_runnable`]). On
//! Windows the helper can't delete its own program, so [`tidy`] removes old folders at the next
//! start.

use crate::native_path::shown;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub const PREFIX: &str = "glowfly-update-";
/// A staging folder older than this is left over.
pub const STALE_AFTER: Duration = Duration::from_secs(3600);

/// Creates a new staging folder inside `temp`.
pub fn create(temp: &Path) -> std::io::Result<PathBuf> {
    let mut last = None;
    for _ in 0..3 {
        let mut raw = [0u8; 8];
        getrandom::fill(&mut raw).map_err(|e| std::io::Error::other(e.to_string()))?;
        let name: String = raw.iter().map(|b| format!("{b:02x}")).collect();
        let dir = temp.join(format!("{PREFIX}{name}"));
        match make_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => last = Some(e),
            Err(e) => return Err(e),
        }
    }
    Err(last.unwrap_or_else(|| std::io::Error::other("no free staging folder name")))
}

/// Creates `dir` itself (not its parents): mode 0700 on Unix.
fn make_dir(dir: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().mode(0o700).create(dir)
    }
    #[cfg(not(unix))]
    {
        std::fs::DirBuilder::new().create(dir)
    }
}

/// Whether `name` is one [`create`] makes: the prefix and 16 lower-case hex digits.
pub fn is_staging_name(name: &str) -> bool {
    name.strip_prefix(PREFIX).is_some_and(|rest| rest.len() == 16 && rest.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
}

/// Whether a folder last changed at `modified` is left over at `now`; one from the future
/// isn't.
pub fn stale(modified: SystemTime, now: SystemTime) -> bool {
    now.duration_since(modified).is_ok_and(|age| age > STALE_AFTER)
}

/// Removes the left-over staging folders in `temp` (real folders, not links, named as
/// [`create`] names them and older than [`STALE_AFTER`]); errors are ignored. Returns how many
/// were removed.
pub fn tidy(temp: &Path, now: SystemTime) -> usize {
    let Ok(list) = std::fs::read_dir(temp) else { return 0 };
    let mut removed = 0;
    for entry in list.flatten() {
        if !is_staging_name(&entry.file_name().to_string_lossy()) {
            continue;
        }
        let path = entry.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
        let old = meta.modified().is_ok_and(|m| stale(m, now));
        if meta.is_dir() && !meta.file_type().is_symlink() && old && crate::persist::remove_tree(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// A staging folder with the helper in it.
#[derive(Debug, PartialEq, Eq)]
pub struct Staged {
    pub dir: PathBuf,
    pub helper: PathBuf,
}

/// Creates a staging folder in the first of `places` (folders that exist) where a copy of `exe`
/// named `helper_name` runs, as `runs` finds by trying it (`launch::try_run`). A place that
/// fails is left as it was; when none works, every place's reason is said.
pub fn create_runnable(
    places: &[PathBuf],
    exe: &Path,
    helper_name: &str,
    runs: &dyn Fn(&Path) -> std::io::Result<()>,
) -> Result<Staged, String> {
    let mut why = Vec::new();
    for (i, place) in places.iter().enumerate() {
        if places[..i].iter().any(|p| crate::paths::same_spelling(p, place)) {
            continue;
        }
        let dir = match create(place) {
            Ok(d) => d,
            Err(e) => {
                why.push(format!("{}: {e}", shown(place)));
                continue;
            }
        };
        let helper = dir.join(helper_name);
        // A copy keeps the executable bit.
        match std::fs::copy(exe, &helper).map(drop).and_then(|()| runs(&helper)) {
            Ok(()) => return Ok(Staged { dir, helper }),
            Err(e) => {
                why.push(format!("{}: {e}", shown(place)));
                let _ = crate::persist::remove_tree(&dir);
            }
        }
    }
    Err(format!("Couldn't find a folder the update helper can run from ({}).", why.join("; ")))
}

#[cfg(test)]
#[path = "staging_tests.rs"]
mod tests;
