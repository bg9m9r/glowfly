//! Resolving a conflict: the side(s) picked per chunk, the file they produce, and writing and
//! staging it (which clears the conflict).

use super::conflict::{ConflictContent, ConflictFile, ConflictIds, Segment, Side};
use crate::error::{Error, Result};
use git2::Repository;
use std::path::Path;

/// What a conflict chunk resolves to.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Choice {
    #[default]
    Unresolved,
    Ours,
    Theirs,
    OursThenTheirs,
    TheirsThenOurs,
}

impl Choice {
    /// The sides taken, in output order.
    pub fn sides(self) -> &'static [Side] {
        match self {
            Choice::Unresolved => &[],
            Choice::Ours => &[Side::Ours],
            Choice::Theirs => &[Side::Theirs],
            Choice::OursThenTheirs => &[Side::Ours, Side::Theirs],
            Choice::TheirsThenOurs => &[Side::Theirs, Side::Ours],
        }
    }

    pub fn takes(self, s: Side) -> bool {
        self.sides().contains(&s)
    }

    /// Ticks or unticks side `s`; a side ticked second goes second.
    pub fn toggle(self, s: Side) -> Choice {
        use Choice::*;
        match (self, s) {
            (Unresolved, _) => Choice::only(s),
            (Ours, Side::Ours) | (Theirs, Side::Theirs) => Unresolved,
            (Ours, Side::Theirs) => OursThenTheirs,
            (Theirs, Side::Ours) => TheirsThenOurs,
            (OursThenTheirs | TheirsThenOurs, Side::Ours) => Theirs,
            (OursThenTheirs | TheirsThenOurs, Side::Theirs) => Ours,
        }
    }

    pub fn only(s: Side) -> Choice {
        match s {
            Side::Ours => Choice::Ours,
            Side::Theirs => Choice::Theirs,
        }
    }
}

/// The file as it will be saved, or `None` until every chunk has a choice.
pub fn render_output(file: &ConflictFile, choices: &[Choice]) -> Option<String> {
    let ConflictContent::Text(segs) = &file.content else { return None };
    if choices.len() != file.chunk_count() || choices.contains(&Choice::Unresolved) {
        return None;
    }
    let mut out = String::new();
    let mut push = |line: &str| {
        // A side without a final newline followed by more text still needs a line break.
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(line);
    };
    let mut k = 0;
    for seg in segs {
        match seg {
            Segment::Clean(lines) => lines.iter().for_each(|l| push(l)),
            Segment::Conflict(c) => {
                for &side in choices[k].sides() {
                    c.side(side).iter().for_each(|l| push(l));
                }
                k += 1;
            }
        }
    }
    Some(if file.crlf { to_crlf(&out) } else { out })
}

/// Turns every LF not already preceded by CR into CR LF.
fn to_crlf(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + text.len() / 32);
    let mut prev = '\0';
    for c in text.chars() {
        if c == '\n' && prev != '\r' {
            out.push('\r');
        }
        out.push(c);
        prev = c;
    }
    out
}

/// Opens the index and checks the conflict at `path` still has the stages it was loaded with.
fn checked_index(repo: &Repository, path: &str, expected: &ConflictIds) -> Result<git2::Index> {
    let index = repo.index()?;
    let current = index.conflict_get(Path::new(path)).ok().map(|c| ConflictIds {
        ancestor: c.ancestor.map(|e| e.id),
        ours: c.our.map(|e| e.id),
        theirs: c.their.map(|e| e.id),
    });
    if current.as_ref() != Some(expected) {
        return Err(Error::Other(format!("The conflict in {path} changed since it was opened. Close and reopen it.")));
    }
    Ok(index)
}

/// Writes `contents` to the working file and stages it, which marks the conflict resolved.
/// Refused if the conflict changed since `expected` was read.
pub fn write_resolution(workdir: &Path, path: &str, contents: &str, expected: &ConflictIds) -> Result<()> {
    let repo = Repository::open(workdir)?;
    let mut index = checked_index(&repo, path, expected)?;
    write_file(workdir, path, contents.as_bytes())?;
    index.add_path(Path::new(path))?;
    index.write()?;
    Ok(())
}

/// Resolves with one side's whole version: its blob, or deleting the file if that side deleted
/// it. Refused if the conflict changed since `expected` was read.
pub fn resolve_with_side(workdir: &Path, path: &str, side: Side, expected: &ConflictIds) -> Result<()> {
    let repo = Repository::open(workdir)?;
    let mut index = checked_index(&repo, path, expected)?;
    let full = workdir.join(path);
    match expected.side(side) {
        Some(id) => {
            let blob = repo.find_blob(id)?;
            let crlf = std::fs::read(&full).is_ok_and(|b| b.windows(2).any(|w| w == b"\r\n"));
            match std::str::from_utf8(blob.content()) {
                Ok(text) if crlf && !blob.is_binary() => write_file(workdir, path, to_crlf(text).as_bytes())?,
                _ => write_file(workdir, path, blob.content())?,
            }
            index.add_path(Path::new(path))?;
        }
        None => {
            if full.exists() {
                std::fs::remove_file(&full)?;
            }
            index.remove_path(Path::new(path))?;
        }
    }
    index.write()?;
    Ok(())
}

fn write_file(workdir: &Path, path: &str, bytes: &[u8]) -> Result<()> {
    let full = workdir.join(path);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(full, bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choice_toggle_table() {
        use Choice::*;
        use Side::{Ours as O, Theirs as T};
        let table = [
            (Unresolved, O, Ours),
            (Unresolved, T, Theirs),
            (Ours, O, Unresolved),
            (Ours, T, OursThenTheirs),
            (Theirs, T, Unresolved),
            (Theirs, O, TheirsThenOurs),
            (OursThenTheirs, O, Theirs),
            (OursThenTheirs, T, Ours),
            (TheirsThenOurs, O, Theirs),
            (TheirsThenOurs, T, Ours),
        ];
        for (from, side, to) in table {
            assert_eq!(from.toggle(side), to, "{from:?} toggling {side:?}");
        }
        assert!(!Unresolved.takes(O) && !Unresolved.takes(T));
        assert!(Ours.takes(O) && !Ours.takes(T));
        assert!(Theirs.takes(T) && !Theirs.takes(O));
        assert!(OursThenTheirs.takes(O) && OursThenTheirs.takes(T));
        assert!(TheirsThenOurs.takes(O) && TheirsThenOurs.takes(T));
        assert_eq!((Choice::only(O), Choice::only(T)), (Ours, Theirs));
        assert_eq!(Choice::default(), Unresolved);
    }

    #[test]
    fn crlf_conversion_keeps_existing_pairs() {
        assert_eq!(to_crlf("a\nb\r\nc"), "a\r\nb\r\nc");
    }
}
