//! When a repository last fetched: the time of `FETCH_HEAD`, which every fetch and pull
//! rewrites. A linked worktree has its own `FETCH_HEAD` in its git dir, but a fetch from any
//! worktree also moves the shared remote-tracking refs, so the newer of the worktree's file and
//! the common dir's counts.

use std::path::Path;
use std::time::UNIX_EPOCH;

/// The newer `FETCH_HEAD` time of `git_dir` and `common` in Unix seconds; None when neither
/// file exists or its time is before the epoch.
pub(super) fn fetched_at(git_dir: &Path, common: &Path) -> Option<i64> {
    [git_dir, common].into_iter().filter_map(|dir| modified(&dir.join("FETCH_HEAD"))).max()
}

fn modified(file: &Path) -> Option<i64> {
    let time = std::fs::metadata(file).ok()?.modified().ok()?;
    let secs = time.duration_since(UNIX_EPOCH).ok()?.as_secs();
    i64::try_from(secs).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::{LoadOptions, load};
    use crate::testutil::{git, scratch_repo, with_origin, write};
    use std::time::{Duration, SystemTime};

    fn now() -> i64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
    }

    /// Writes `dir`/FETCH_HEAD with its modification time `secs` after the epoch.
    fn fetch_head_at(dir: &Path, secs: u64) {
        let file = dir.join("FETCH_HEAD");
        std::fs::write(&file, "").unwrap();
        let f = std::fs::OpenOptions::new().write(true).open(&file).unwrap();
        f.set_modified(UNIX_EPOCH + Duration::from_secs(secs)).unwrap();
    }

    #[test]
    fn missing_fetch_head_gives_none() {
        let dir = scratch_repo("fetch-time-none");
        write(&dir, "a.txt", "a\n");
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "-q", "-m", "one"]);
        assert_eq!(load(&dir, &LoadOptions::all(10)).unwrap().fetched_at, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_fetch_sets_the_time() {
        let (work, bare) = with_origin("fetch-time-fetch");
        git(&work, &["fetch", "-q", "origin"]);
        let at = load(&work, &LoadOptions::all(10)).unwrap().fetched_at.expect("FETCH_HEAD was written");
        assert!((now() - at).abs() < 60, "{at} is not about now");
        let _ = std::fs::remove_dir_all(&work);
        let _ = std::fs::remove_dir_all(&bare);
    }

    #[test]
    fn a_linked_worktree_takes_the_newer_file() {
        let main = scratch_repo("fetch-time-linked");
        write(&main, "a.txt", "a\n");
        git(&main, &["add", "."]);
        git(&main, &["commit", "-q", "-m", "one"]);
        let linked = main.with_file_name(format!("{}-wt", main.file_name().unwrap().to_string_lossy()));
        let _ = std::fs::remove_dir_all(&linked);
        git(&main, &["worktree", "add", "-q", "-b", "side", &linked.display().to_string()]);
        let snap = load(&linked, &LoadOptions::all(10)).unwrap();
        let (own, common) = (snap.git_dir.clone(), snap.common.dir.clone());
        assert_ne!(own, common);
        fetch_head_at(&common, 2_000_000_000);
        fetch_head_at(&own, 1_000_000_000);
        assert_eq!(fetched_at(&own, &common), Some(2_000_000_000), "the common dir's is newer");
        fetch_head_at(&own, 2_100_000_000);
        assert_eq!(load(&linked, &LoadOptions::all(10)).unwrap().fetched_at, Some(2_100_000_000), "the worktree's is newer");
        let _ = std::fs::remove_dir_all(&linked);
        let _ = std::fs::remove_dir_all(&main);
    }
}
