//! recent.txt: the recently opened repositories, newest first, one path per line. A save merges
//! in the entries a late read found (see the parent module) and Clear recent empties the file
//! for good, without them.

use super::{Begun, Carried, SaveRefused, begin, byte_lines, finish, note_skipped, read_state_file, write};
use glowfly_core::persist::{self, KEPT_BACKUPS};
use glowfly_model::msg::SaveNotes;
use glowfly_model::state::repo_lists::RECENT_LIMIT;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn recent_file() -> Option<PathBuf> {
    glowfly_core::settings::config_dir().map(|d| d.join("recent.txt"))
}

/// Recently opened repositories, newest first, including folders that no longer exist. Reads
/// only Glowfly's own file: nothing in the list is looked at (a network share may not answer).
/// Called once, at start: only the newest `KEPT_BACKUPS` copies of the file are kept.
pub fn load_recent() -> Vec<PathBuf> {
    let Some(f) = recent_file() else { return Vec::new() };
    let list = load_recent_with(&f, &glowfly_core::settings::READ_WAITS, |p| std::fs::read(p));
    persist::prune_backups(&f, KEPT_BACKUPS);
    list
}

/// `load_recent` from `file`, reading with `read` (see `read_state_file`).
pub(super) fn load_recent_with(file: &Path, waits: &[Duration], read: impl FnMut(&Path) -> std::io::Result<Vec<u8>>) -> Vec<PathBuf> {
    let Some(bytes) = read_state_file(file, waits, read) else { return Vec::new() };
    let (list, skipped) = parse_recent_bytes(&bytes);
    note_skipped(file, skipped, bytes);
    list
}

/// The recent list in recent.txt: one path per line (`paths::encode`, which leaves plain paths
/// as earlier versions wrote them).
pub fn parse_recent(text: &str) -> Vec<PathBuf> {
    parse_recent_bytes(text.as_bytes()).0
}

/// `parse_recent` of the file's bytes, and how many lines it skipped: a line that isn't valid
/// UTF-8 can't name the folder it was written for, so it is skipped and the rest are kept.
pub(super) fn parse_recent_bytes(bytes: &[u8]) -> (Vec<PathBuf>, usize) {
    let mut skipped = 0;
    let list = byte_lines(bytes)
        .filter_map(|l| {
            let line = std::str::from_utf8(l).ok();
            skipped += usize::from(line.is_none());
            line
        })
        .map(glowfly_core::paths::decode)
        .collect();
    (list, skipped)
}

pub(super) fn format_recent(list: &[PathBuf]) -> String {
    list.iter().map(|p| glowfly_core::paths::encode(p)).collect::<Vec<_>>().join("\n")
}

/// Saves the recent-repositories list, newest first.
pub fn save_recent(list: &[PathBuf]) -> Result<SaveNotes, SaveRefused> {
    let Some(f) = recent_file() else { return Ok(SaveNotes::default()) };
    save_recent_to(&f, list)
}

/// Writes the recent-repositories list to `file`.
pub fn save_recent_to(file: &Path, list: &[PathBuf]) -> Result<SaveNotes, SaveRefused> {
    save_recent_with(file, list, false, |p| std::fs::read(p))
}

/// Empties the recent-repositories file (Clear recent).
pub fn clear_recent() -> Result<SaveNotes, SaveRefused> {
    let Some(f) = recent_file() else { return Ok(SaveNotes::default()) };
    clear_recent_to(&f)
}

/// Empties the recent-repositories file `file`, for good: entries a late read found are dropped.
pub fn clear_recent_to(file: &Path) -> Result<SaveNotes, SaveRefused> {
    save_recent_with(file, &[], true, |p| std::fs::read(p))
}

/// Writes `list` to `file`, after the entries a late read found (`clearing`: none, the file is
/// emptied). `read` is the one read of a file whose start read failed.
pub(super) fn save_recent_with(
    file: &Path,
    list: &[PathBuf],
    clearing: bool,
    read: impl FnOnce(&Path) -> std::io::Result<Vec<u8>>,
) -> Result<SaveNotes, SaveRefused> {
    let Begun { mut notes, late, carried } = begin(file, clearing, read, parse_recent_bytes)?;
    if clearing {
        write(file, "").map_err(SaveRefused::Io)?;
        return Ok(notes);
    }
    let mut carry = match carried {
        Some(Carried::Recent(c)) => c,
        _ => Vec::new(),
    };
    if let Some(disk) = late {
        notes.read_late = Some(disk.clone());
        carry = disk;
    }
    // An entry this save holds is the session's own from now on.
    carry.retain(|p| !list.iter().any(|l| glowfly_core::paths::same_spelling(l, p)));
    let mut merged = list.to_vec();
    merged.extend(carry.iter().cloned());
    merged.truncate(RECENT_LIMIT);
    carry.retain(|p| merged.contains(p));
    let carried = (!carry.is_empty()).then_some(Carried::Recent(carry));
    finish(file, &format_recent(&merged), carried).map_err(SaveRefused::Io)?;
    Ok(notes)
}
