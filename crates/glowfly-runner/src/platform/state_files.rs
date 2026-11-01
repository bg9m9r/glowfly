//! The recent-repositories (`recent.txt`) and open-tabs (`tabs.txt`) files in the config folder.
//!
//! Neither file may be lost to a bad read. Each line is decoded on its own, so a line that isn't
//! valid UTF-8 (a hand edit, a damaged disk) costs only that line. A read that keeps failing
//! (another program holding the file, a permissions change) leaves the file where it is and
//! stops this session's saves of it, so the next start can still restore the list instead of
//! finding it overwritten with this session's few entries. Both files hold repository paths, so
//! on Unix they are written for this user only.

use glowfly_core::persist::{self, WriteOptions};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

/// Files whose read failed this session: saving over them would lose what they hold.
static UNREAD: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

fn is_unread(file: &Path) -> bool {
    UNREAD.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|f| f == file)
}

/// The bytes of `file`, or `None` when there is none. Each failed try is followed by one of
/// `waits` (a virus scanner or sync client lets go within moments); when none are left the file
/// is marked unread, so this session doesn't save over it.
fn read_state_file(file: &Path, waits: &[Duration], mut read: impl FnMut(&Path) -> std::io::Result<Vec<u8>>) -> Option<Vec<u8>> {
    let mut waits = waits.iter();
    loop {
        match read(file) {
            Ok(bytes) => return Some(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
            Err(e) => match waits.next() {
                Some(wait) => std::thread::sleep(*wait),
                None => {
                    tracing::warn!("could not read {}: {e}; it is kept and not saved over until it can be read", file.display());
                    UNREAD.lock().unwrap_or_else(|e| e.into_inner()).push(file.to_owned());
                    return None;
                }
            },
        }
    }
}

/// Replaces `file` with `text`, private to this user on Unix, unless its read failed this
/// session (then nothing is written, and the file keeps the list it had).
fn save_state_file(file: &Path, text: &str) -> std::io::Result<()> {
    if is_unread(file) {
        return Ok(());
    }
    persist::write_file(file, text.as_bytes(), WriteOptions { private: true, ..WriteOptions::default() })
}

/// The lines of `bytes`, without a trailing `\r` or empty lines, as bytes.
fn byte_lines(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    bytes.split(|b| *b == b'\n').map(|l| l.strip_suffix(b"\r").unwrap_or(l)).filter(|l| !l.is_empty())
}

fn recent_file() -> Option<PathBuf> {
    glowfly_core::settings::config_dir().map(|d| d.join("recent.txt"))
}

/// Recently opened repositories, newest first, including folders that no longer exist. Reads
/// only Glowfly's own file: nothing in the list is looked at (a network share may not answer).
pub fn load_recent() -> Vec<PathBuf> {
    recent_file().map(|f| load_recent_with(&f, &glowfly_core::settings::READ_WAITS, |p| std::fs::read(p))).unwrap_or_default()
}

/// `load_recent` from `file`, reading with `read` (see [`read_state_file`]).
fn load_recent_with(file: &Path, waits: &[Duration], read: impl FnMut(&Path) -> std::io::Result<Vec<u8>>) -> Vec<PathBuf> {
    read_state_file(file, waits, read).map(|b| parse_recent_bytes(&b)).unwrap_or_default()
}

/// The recent list in recent.txt: one path per line (`paths::encode`, which leaves plain paths
/// as earlier versions wrote them).
pub fn parse_recent(text: &str) -> Vec<PathBuf> {
    parse_recent_bytes(text.as_bytes())
}

/// `parse_recent` of the file's bytes: a line that isn't valid UTF-8 can't name the folder it
/// was written for, so it is skipped and the rest are kept.
fn parse_recent_bytes(bytes: &[u8]) -> Vec<PathBuf> {
    byte_lines(bytes).filter_map(|l| std::str::from_utf8(l).ok()).map(glowfly_core::paths::decode).collect()
}

pub(crate) fn format_recent(list: &[PathBuf]) -> String {
    list.iter().map(|p| glowfly_core::paths::encode(p)).collect::<Vec<_>>().join("\n")
}

/// Saves the recent-repositories list, newest first.
pub fn save_recent(list: &[PathBuf]) -> std::io::Result<()> {
    let Some(f) = recent_file() else { return Ok(()) };
    save_recent_to(&f, list)
}

/// Writes the recent-repositories list to `file`.
pub fn save_recent_to(file: &Path, list: &[PathBuf]) -> std::io::Result<()> {
    save_state_file(file, &format_recent(list))
}

fn tabs_file() -> Option<PathBuf> {
    glowfly_core::settings::config_dir().map(|d| d.join("tabs.txt"))
}

/// The open tabs of a saved session: their folders, their labels (one per folder) and the
/// index of the active one.
type SavedTabs = (Vec<PathBuf>, Vec<Option<String>>, usize);

/// The tabs open when Glowfly last closed, their labels and the index of the active one.
/// Nothing is looked at here (this runs before the window opens, and a network share may not
/// answer): a tab whose folder is gone fails to load when selected, like any other.
pub fn load_tabs() -> SavedTabs {
    tabs_file().map(|f| load_tabs_with(&f, &glowfly_core::settings::READ_WAITS, |p| std::fs::read(p))).unwrap_or_default()
}

/// `load_tabs` from `file`, reading with `read` (see [`read_state_file`]).
fn load_tabs_with(file: &Path, waits: &[Duration], read: impl FnMut(&Path) -> std::io::Result<Vec<u8>>) -> SavedTabs {
    read_state_file(file, waits, read).map(|b| parse_tabs_bytes(&b)).unwrap_or_default()
}

/// tabs.txt: the active index on the first line, then one repository per line
/// (`paths::encode`, which escapes every control character), followed by a tab character and
/// the tab's label when it has one. Files from earlier versions have no labels.
pub fn parse_tabs(text: &str) -> SavedTabs {
    parse_tabs_bytes(text.as_bytes())
}

/// `parse_tabs` of the file's bytes. A line whose path isn't valid UTF-8 is skipped (it can't
/// name its folder); a label is decoded lossily, since a damaged character there names nothing.
fn parse_tabs_bytes(bytes: &[u8]) -> SavedTabs {
    let mut lines = bytes.split(|b| *b == b'\n').map(|l| l.strip_suffix(b"\r").unwrap_or(l));
    let active: usize = lines.next().and_then(|l| String::from_utf8_lossy(l).trim().parse().ok()).unwrap_or(0);
    let (paths, labels): (Vec<PathBuf>, Vec<Option<String>>) = lines
        .filter(|l| !l.is_empty())
        .filter_map(|l| {
            let (path, label) = match l.iter().position(|b| *b == b'\t') {
                Some(at) => (&l[..at], &l[at + 1..]),
                None => (l, &[][..]),
            };
            let path = std::str::from_utf8(path).ok()?;
            let label: String = String::from_utf8_lossy(label).chars().filter(|c| !c.is_control()).collect();
            let label = label.trim();
            Some((glowfly_core::paths::decode(path), (!label.is_empty()).then(|| label.to_owned())))
        })
        .unzip();
    let active = active.min(paths.len().saturating_sub(1));
    (paths, labels, active)
}

pub(crate) fn format_tabs(paths: &[PathBuf], labels: &[Option<String>], active: usize) -> String {
    let mut text = active.to_string();
    for (k, p) in paths.iter().enumerate() {
        text.push('\n');
        text.push_str(&glowfly_core::paths::encode(p));
        if let Some(label) = labels.get(k).and_then(Option::as_deref) {
            let label: String = label.chars().filter(|c| !c.is_control()).collect();
            text.push('\t');
            text.push_str(label.trim());
        }
    }
    text
}

pub fn save_tabs(paths: &[PathBuf], labels: &[Option<String>], active: usize) -> std::io::Result<()> {
    let Some(f) = tabs_file() else { return Ok(()) };
    save_tabs_to(&f, paths, labels, active)
}

/// Writes the open tabs to `file`.
pub fn save_tabs_to(file: &Path, paths: &[PathBuf], labels: &[Option<String>], active: usize) -> std::io::Result<()> {
    save_state_file(file, &format_tabs(paths, labels, active))
}

#[cfg(test)]
#[path = "state_files_tests.rs"]
mod tests;
