//! The recent-repositories (`recent.txt`, `state_files/recent.rs`) and open-tabs (`tabs.txt`,
//! `state_files/tabs.rs`) files in the config folder, and what this session holds about each.
//!
//! Neither file may be lost to a bad read. One leading UTF-8 BOM (an editor's) is dropped, then
//! each line is decoded on its own, so a line that isn't valid UTF-8 (a hand edit, a damaged
//! disk) costs only that line; the file's bytes are kept, and copied beside it
//! (`persist::keep_copy`) before the first save replaces it. A read that keeps failing at start
//! (another program holding the file, a permissions change) leaves the file where it is, and
//! each save of it then tries one read first: while that fails nothing is written (the first
//! refusal is reported), so the next start can still restore the list; once it works, the
//! entries on disk are merged after this session's into that save and every later one, until
//! the session saves them itself. Both files hold repository paths, so on Unix they are written
//! for this user only.

use glowfly_core::native_path::shown;
use glowfly_core::persist::{self, WriteOptions};
use glowfly_model::msg::SaveNotes;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

mod recent;
mod tabs;

pub use recent::{clear_recent, clear_recent_to, load_recent, parse_recent, save_recent, save_recent_to};
pub use tabs::{load_tabs, parse_tabs, save_tabs, save_tabs_to};

/// What this session holds about a state file.
enum Held {
    /// Every start read failed: nothing is saved over it until a save's one read works. `told`:
    /// a refusal was reported.
    Unread { told: bool },
    /// The start read skipped lines: these bytes are copied beside it before the first save
    /// replaces it.
    Skipped(Vec<u8>),
    /// Read only late: on-disk entries this session hasn't saved itself, merged into every save.
    Carried(Carried),
}

/// The entries a late read found that this session's saves don't hold yet.
enum Carried {
    Recent(Vec<PathBuf>),
    /// Paths and their labels (parallel).
    Tabs(Vec<PathBuf>, Vec<Option<String>>),
}

/// Per file. Saves of one file run one at a time (runner/saves.rs), so a save can take its
/// file's state out, do its I/O unlocked, and put it back.
static HELD: Mutex<Vec<(PathBuf, Held)>> = Mutex::new(Vec::new());

fn held() -> MutexGuard<'static, Vec<(PathBuf, Held)>> {
    HELD.lock().unwrap_or_else(PoisonError::into_inner)
}

fn take_held(file: &Path) -> Option<Held> {
    let mut all = held();
    let at = all.iter().position(|(f, _)| f == file)?;
    Some(all.swap_remove(at).1)
}

fn put_held(file: &Path, state: Held) {
    let mut all = held();
    all.retain(|(f, _)| f != file);
    all.push((file.to_owned(), state));
}

/// Why a save of recent.txt or tabs.txt wrote nothing.
#[derive(Debug)]
pub enum SaveRefused {
    /// The file still can't be read (`detail`: its path and the error), so it isn't saved over.
    /// `report`: say so to the user (the first refusal of this file this session, or any
    /// Clear recent).
    Unread {
        detail: String,
        report: bool,
    },
    Io(std::io::Error),
}

/// The bytes of `file`, or `None` when there is none. Each failed try is followed by one of
/// `waits` (a virus scanner or sync client lets go within moments); when none are left the file
/// is held unread, so this session doesn't save over it before a save's own read works.
fn read_state_file(file: &Path, waits: &[Duration], mut read: impl FnMut(&Path) -> std::io::Result<Vec<u8>>) -> Option<Vec<u8>> {
    // A fresh read: whatever an earlier one held no longer applies.
    take_held(file);
    let mut waits = waits.iter();
    loop {
        match read(file) {
            Ok(bytes) => return Some(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
            Err(e) => match waits.next() {
                Some(wait) => std::thread::sleep(*wait),
                None => {
                    tracing::warn!("could not read {}: {e}; it is kept and not saved over until it can be read", shown(file));
                    put_held(file, Held::Unread { told: false });
                    return None;
                }
            },
        }
    }
}

/// `bytes` without one leading UTF-8 byte order mark.
fn strip_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes)
}

/// The lines of `bytes` (a BOM dropped), without a trailing `\r` or empty lines, as bytes.
fn byte_lines(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    strip_bom(bytes).split(|b| *b == b'\n').map(|l| l.strip_suffix(b"\r").unwrap_or(l)).filter(|l| !l.is_empty())
}

/// The start read of `file` skipped `skipped` lines: its `bytes` are held, to be copied beside
/// it before the first save replaces it.
fn note_skipped(file: &Path, skipped: usize, bytes: Vec<u8>) {
    if skipped > 0 {
        tracing::warn!(
            "{}: {skipped} line(s) aren't valid UTF-8 and were skipped; the file is copied before it is next saved",
            shown(file)
        );
        put_held(file, Held::Skipped(bytes));
    }
}

/// Replaces `file` with `text`, private to this user on Unix.
fn write(file: &Path, text: &str) -> std::io::Result<()> {
    persist::write_file(file, text.as_bytes(), WriteOptions { private: true, ..WriteOptions::default() })
}

/// One read of `file`, with no waits: its bytes, `None` when it is gone, or the path and error.
fn retry_read(file: &Path, read: impl FnOnce(&Path) -> std::io::Result<Vec<u8>>) -> Result<Option<Vec<u8>>, String> {
    match read(file) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", shown(file))),
    }
}

/// A save's read still failed: `file` stays unread, and the refusal is reported the first time
/// (or always, when `clearing`).
fn refuse(file: &Path, told: bool, clearing: bool, detail: String) -> SaveRefused {
    put_held(file, Held::Unread { told: true });
    SaveRefused::Unread { detail, report: clearing || !told }
}

/// How a save starts from what the session holds about its file.
struct Begun<T> {
    notes: SaveNotes,
    /// What a late read (this save's) found on disk.
    late: Option<T>,
    /// The entries an earlier late read found that are still to be merged.
    carried: Option<Carried>,
}

/// Takes what the session holds about `file` and does what it asks before a save: reads a file
/// held unread once (`read`; `parse` gives its entries and how many lines it skipped), and
/// copies a file with skipped lines beside it. Refused: nothing is to be written, and what was
/// held stays held (a copy not yet made is tried again by the next save).
fn begin<T>(
    file: &Path,
    clearing: bool,
    read: impl FnOnce(&Path) -> std::io::Result<Vec<u8>>,
    parse: impl Fn(&[u8]) -> (T, usize),
) -> Result<Begun<T>, SaveRefused> {
    let mut begun = Begun { notes: SaveNotes::default(), late: None, carried: None };
    match take_held(file) {
        None => {}
        Some(Held::Unread { told }) => match retry_read(file, read) {
            Err(detail) => return Err(refuse(file, told, clearing, detail)),
            // Gone since: nothing is lost by writing it.
            Ok(None) => {}
            Ok(Some(bytes)) => {
                let (late, skipped) = parse(&bytes);
                if skipped > 0 {
                    match persist::keep_copy(file, &bytes) {
                        Ok(backup) => begun.notes.kept = Some((backup, skipped)),
                        Err(e) => {
                            put_held(file, Held::Unread { told });
                            return Err(SaveRefused::Io(e));
                        }
                    }
                }
                begun.late = Some(late);
            }
        },
        Some(Held::Skipped(bytes)) => match persist::keep_copy(file, &bytes) {
            Ok(backup) => begun.notes.kept = Some((backup, parse(&bytes).1)),
            Err(e) => {
                put_held(file, Held::Skipped(bytes));
                return Err(SaveRefused::Io(e));
            }
        },
        Some(Held::Carried(c)) => begun.carried = Some(c),
    }
    Ok(begun)
}

/// Writes `text` to `file`, then holds `carried` (when there is any) for the next save, also when
/// the write failed. A copy `begin` made isn't made again, and a late read isn't repeated.
fn finish(file: &Path, text: &str, carried: Option<Carried>) -> std::io::Result<()> {
    let written = write(file, text);
    if let Some(c) = carried {
        put_held(file, Held::Carried(c));
    }
    written
}

#[cfg(test)]
#[path = "state_files_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "state_files_held_tests.rs"]
mod held_tests;
