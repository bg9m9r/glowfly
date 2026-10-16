//! The activity log file (`activity.jsonl`): one JSON object per entry, newest last.

use crate::persist;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

/// One line of the activity log as the file keeps it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActivityEntry {
    /// When the session that wrote it started, in Unix seconds.
    pub session: i64,
    /// When it was written, in Unix seconds.
    pub at: i64,
    pub ok: bool,
    pub text: String,
}

/// The longest text an entry keeps in the file; longer text is cut on a char boundary and ends
/// with "…".
pub const MAX_ENTRY_BYTES: usize = 16 * 1024;

/// The version of the line format this build reads and writes.
const VERSION: u32 = 1;

#[derive(Serialize)]
struct LineOut<'a> {
    v: u32,
    session: i64,
    at: i64,
    ok: bool,
    text: &'a str,
}

#[derive(Deserialize)]
struct LineIn {
    v: u32,
    session: i64,
    at: i64,
    ok: bool,
    text: String,
}

/// `text`, at most [`MAX_ENTRY_BYTES`] long.
fn capped(text: &str) -> std::borrow::Cow<'_, str> {
    if text.len() <= MAX_ENTRY_BYTES {
        return text.into();
    }
    let ellipsis = "…";
    let mut end = MAX_ENTRY_BYTES - ellipsis.len();
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{ellipsis}", &text[..end]).into()
}

/// The file's line for `e`, ending in '\n'. Newlines in the text are escaped by JSON, so each
/// entry stays on one line.
pub fn activity_line(e: &ActivityEntry) -> String {
    let line = LineOut { v: VERSION, session: e.session, at: e.at, ok: e.ok, text: &capped(&e.text) };
    let mut s = serde_json::to_string(&line).unwrap_or_default();
    s.push('\n');
    s
}

/// The entries in `text`, oldest first, and how many lines were skipped: blank ones, a line a
/// crash tore, lines that aren't JSON and lines in another version's format.
pub fn parse_activity(text: &str) -> (Vec<ActivityEntry>, usize) {
    let mut entries = Vec::new();
    let mut skipped = 0;
    for line in text.lines() {
        match serde_json::from_str::<LineIn>(line) {
            Ok(l) if l.v == VERSION => entries.push(ActivityEntry { session: l.session, at: l.at, ok: l.ok, text: l.text }),
            _ => skipped += 1,
        }
    }
    (entries, skipped)
}

/// The activity log file, kept to its newest `cap` entries (up to twice that between trims).
pub struct ActivityFile {
    path: PathBuf,
    cap: usize,
    /// About how many lines the file holds (another instance may append too).
    lines: usize,
}

impl ActivityFile {
    /// Reads the file at `path` (a missing file is empty) and returns its newest `cap` entries.
    /// The file is rewritten when entries or bad lines were dropped.
    pub fn open(path: PathBuf, cap: usize) -> io::Result<(Self, Vec<ActivityEntry>)> {
        let (mut entries, skipped) = read(&path)?;
        let dropped = keep_newest(&mut entries, cap);
        if dropped > 0 || skipped > 0 {
            write_all(&path, &entries)?;
        }
        Ok((Self { path, cap, lines: entries.len() }, entries))
    }

    /// Appends `entries` in one write; trims the file back to `cap` entries once it holds more
    /// than twice that.
    pub fn append(&mut self, entries: &[ActivityEntry]) -> io::Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let bytes: String = entries.iter().map(activity_line).collect();
        persist::append(&self.path, bytes.as_bytes())?;
        self.lines += entries.len();
        if self.lines > 2 * self.cap {
            let (mut all, _) = read(&self.path)?;
            keep_newest(&mut all, self.cap);
            write_all(&self.path, &all)?;
            self.lines = all.len();
        }
        Ok(())
    }

    /// Deletes the file; the next append starts a new one.
    pub fn clear(&mut self) -> io::Result<()> {
        persist::remove_file(&self.path)?;
        self.lines = 0;
        Ok(())
    }
}

/// The entries in the file at `path` (a missing file has none) and how many lines were skipped.
fn read(path: &Path) -> io::Result<(Vec<ActivityEntry>, usize)> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(parse_activity(&String::from_utf8_lossy(&bytes))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok((Vec::new(), 0)),
        Err(e) => Err(e),
    }
}

/// Drops all but the newest `cap` entries; returns how many were dropped.
fn keep_newest(entries: &mut Vec<ActivityEntry>, cap: usize) -> usize {
    let dropped = entries.len().saturating_sub(cap);
    entries.drain(..dropped);
    dropped
}

fn write_all(path: &Path, entries: &[ActivityEntry]) -> io::Result<()> {
    let bytes: String = entries.iter().map(activity_line).collect();
    persist::atomic_write(path, bytes.as_bytes())
}

#[cfg(test)]
#[path = "activity_tests.rs"]
mod tests;
