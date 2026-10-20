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
    /// The repository of the tab it was logged in; None for a tab without one and for lines
    /// older builds wrote.
    pub repo: Option<PathBuf>,
    /// The operation this line records, when it is one (a "run"); None for a plain note and for
    /// lines older builds wrote.
    pub run: Option<RunInfo>,
}

/// What an activity line records about a finished operation (a job).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunInfo {
    /// The job's label ("Push", "Fetch all").
    pub title: String,
    /// One short line: the plain cause of a failure, or what a success did. May be empty.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub summary: String,
    /// How long the job ran, in milliseconds, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub took_ms: Option<u64>,
    /// A background auto-fetch, which the panel folds with its neighbours.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub routine: bool,
}

/// Whether two entries' repositories are the same: both none, or the same spelling.
pub fn same_repo(a: Option<&Path>, b: Option<&Path>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => crate::paths::same_spelling(a, b),
        _ => false,
    }
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
    /// Written with `paths::encode`, so any path survives the JSON text.
    #[serde(skip_serializing_if = "Option::is_none")]
    repo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    run: Option<RunInfo>,
}

#[derive(Deserialize)]
struct LineIn {
    v: u32,
    session: i64,
    at: i64,
    ok: bool,
    text: String,
    #[serde(default)]
    repo: Option<String>,
    #[serde(default)]
    run: Option<RunInfo>,
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
    let line = LineOut {
        v: VERSION,
        session: e.session,
        at: e.at,
        ok: e.ok,
        text: &capped(&e.text),
        repo: e.repo.as_deref().map(crate::paths::encode),
        run: e.run.as_ref().map(|r| RunInfo { summary: capped(&r.summary).into_owned(), ..r.clone() }),
    };
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
            Ok(l) if l.v == VERSION => entries.push(ActivityEntry {
                session: l.session,
                at: l.at,
                ok: l.ok,
                text: l.text,
                repo: l.repo.as_deref().map(crate::paths::decode),
                run: l.run,
            }),
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

    /// Removes the entries logged for `repo` (None: those logged without a repository),
    /// keeping every other repository's. A missing file is fine; nothing is rewritten when
    /// nothing was removed.
    pub fn remove_repo(&mut self, repo: Option<&Path>) -> io::Result<()> {
        let (mut entries, _) = read(&self.path)?;
        let before = entries.len();
        entries.retain(|e| !same_repo(e.repo.as_deref(), repo));
        if entries.len() != before {
            write_all(&self.path, &entries)?;
        }
        self.lines = entries.len();
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
