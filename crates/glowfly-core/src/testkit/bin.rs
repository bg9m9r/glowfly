//! A stand-in Recycle Bin / Trash for tests: a folder that files are moved into under a unique
//! name (`<nanos>-<seq>-<name>`), each with a `.from` note of where it was, so a test's `Trasher` and
//! `Untrasher` can use it and tests never touch the real one. Each crate's tests keep their own
//! folder and wrap these in plain functions.

use crate::repo::{TrashProblem, TrashRefusal, Untrashed};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// This process's stand-in bin `<temp>/<prefix>-<pid>`, emptied on its first use. Windows reuses
/// process ids, so a folder of that name left by an earlier test run (with items a failed or
/// stopped test never took back out) would otherwise hold copies that look like this run's,
/// down to their contents and `.from` notes, since the scratch repositories carry the id too.
pub fn own(prefix: &str) -> PathBuf {
    static CLEARED: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let dir = crate::paths::temp_dir().join(format!("{prefix}-{}", std::process::id()));
    // Held while clearing, so no other test gets the path (and moves an item in) before then.
    let mut cleared = CLEARED.lock().unwrap_or_else(|e| e.into_inner());
    if !cleared.iter().any(|p| p == prefix) {
        let _ = std::fs::remove_dir_all(&dir);
        cleared.push(prefix.to_owned());
    }
    dir
}

/// Numbers the items this process moves in, so two moved in within the clock's resolution never
/// get the same name (a rename would silently replace the first).
static SEQ: AtomicU64 = AtomicU64::new(0);

/// Renames `from` to `to`, trying again for a moment when Windows reports the file busy (another
/// process, such as a virus scanner, briefly holding a file just written in the temp dir).
fn rename(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut tries = 0;
    loop {
        match std::fs::rename(from, to) {
            Err(e) if tries < 40 && busy(&e) => tries += 1,
            done => return done,
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

/// Whether `e` is the transient "in use" or "access denied" Windows gives for a held file.
fn busy(e: &std::io::Error) -> bool {
    cfg!(windows) && (e.kind() == std::io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(32))
}

/// Moves `full` into the folder `bin` (made when missing), noting where it was. The temp dir
/// and scratch repositories share a volume, so a rename works.
pub fn move_in(bin: &Path, full: &Path) -> Result<(), TrashRefusal> {
    let other = |e: std::io::Error| TrashRefusal::new(TrashProblem::Other, e.to_string());
    std::fs::create_dir_all(bin).map_err(other)?;
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let name = full.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let item = bin.join(format!("{nanos}-{seq:08}-{name}"));
    rename(full, &item).map_err(other)?;
    let mut note = item.into_os_string();
    note.push(".from");
    std::fs::write(note, full.to_string_lossy().as_bytes()).map_err(other)
}

/// Takes the newest item in `bin` that was at `full` and whose bytes `ours` accepts back there,
/// never over anything.
pub fn take_out(bin: &Path, full: &Path, ours: &dyn Fn(&Path) -> bool) -> Untrashed {
    let Ok(dir) = std::fs::read_dir(bin) else { return Untrashed::NotFound };
    let want = full.to_string_lossy();
    let mut items: Vec<_> = dir
        .flatten()
        .filter_map(|e| {
            let note = e.path();
            let item = note.to_string_lossy().strip_suffix(".from")?.to_owned();
            (std::fs::read_to_string(&note).ok()? == want).then_some((item, note))
        })
        .collect();
    // The nanosecond and sequence prefixes have fixed lengths, so the names sort by time.
    items.sort_by(|a, b| b.0.cmp(&a.0));
    let Some((item, note)) = items.into_iter().find(|(item, _)| ours(Path::new(item))) else { return Untrashed::NotFound };
    if full.symlink_metadata().is_ok() {
        return Untrashed::Failed(format!("{} is in the way", full.display()));
    }
    match rename(Path::new(&item), full) {
        Ok(()) => {
            let _ = std::fs::remove_file(note);
            Untrashed::Restored
        }
        Err(e) => Untrashed::Failed(e.to_string()),
    }
}

/// How many items in `bin` came from `name` in the folder `dir`, by their `.from` notes. Tests
/// share a bin and use the same file names, so a count by name alone sees the other tests' items
/// come and go; `dir` is a test's own scratch repository. The notes are compared by their last
/// components, so the form `dir` is written in doesn't matter.
pub fn count(bin: &Path, dir: &Path, name: &str) -> usize {
    let Some(folder) = dir.file_name() else { return 0 };
    let want = Path::new(folder).join(name);
    let Ok(items) = std::fs::read_dir(bin) else { return 0 };
    items
        .flatten()
        .filter(|e| {
            let note = e.path();
            note.extension().is_some_and(|x| x == "from")
                && note.with_extension("").exists()
                && std::fs::read_to_string(&note).is_ok_and(|from| Path::new(&from).ends_with(&want))
        })
        .count()
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_bin_left_by_an_earlier_process_with_this_id_is_emptied_first() {
        let left = crate::paths::temp_dir().join(format!("glowfly-own-probe-{}", std::process::id()));
        std::fs::create_dir_all(&left).unwrap();
        std::fs::write(left.join("1-u.txt"), "from an earlier run\n").unwrap();
        let bin = super::own("glowfly-own-probe");
        assert_eq!(bin, left);
        assert!(!left.join("1-u.txt").exists(), "the earlier run's item is gone");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("2-u.txt"), "this run\n").unwrap();
        assert_eq!(super::own("glowfly-own-probe"), bin);
        assert!(bin.join("2-u.txt").exists(), "only the first use empties it");
        let _ = std::fs::remove_dir_all(&bin);
    }
}
