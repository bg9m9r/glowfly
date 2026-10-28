//! A stand-in Recycle Bin / Trash for tests: a folder that files are moved into under a unique
//! name (`<nanos>-<name>`), each with a `.from` note of where it was, so a test's `Trasher` and
//! `Untrasher` can use it and tests never touch the real one. Each crate's tests keep their own
//! folder and wrap these in plain functions.

use crate::repo::{TrashProblem, TrashRefusal, Untrashed};
use std::path::Path;

/// Moves `full` into the folder `bin` (made when missing), noting where it was. The temp dir
/// and scratch repositories share a volume, so a rename works.
pub fn move_in(bin: &Path, full: &Path) -> Result<(), TrashRefusal> {
    let other = |e: std::io::Error| TrashRefusal::new(TrashProblem::Other, e.to_string());
    std::fs::create_dir_all(bin).map_err(other)?;
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let name = full.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let item = bin.join(format!("{nanos}-{name}"));
    std::fs::rename(full, &item).map_err(other)?;
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
    // The nanosecond prefixes have the same length, so the names sort by time.
    items.sort_by(|a, b| b.0.cmp(&a.0));
    let Some((item, note)) = items.into_iter().find(|(item, _)| ours(Path::new(item))) else { return Untrashed::NotFound };
    if full.symlink_metadata().is_ok() {
        return Untrashed::Failed(format!("{} is in the way", full.display()));
    }
    match std::fs::rename(&item, full) {
        Ok(()) => {
            let _ = std::fs::remove_file(note);
            Untrashed::Restored
        }
        Err(e) => Untrashed::Failed(e.to_string()),
    }
}

/// How many items in `bin` were named `name`.
pub fn count(bin: &Path, name: &str) -> usize {
    std::fs::read_dir(bin)
        .map(|d| d.flatten().filter(|e| e.file_name().to_string_lossy().ends_with(&format!("-{name}"))).count())
        .unwrap_or(0)
}
