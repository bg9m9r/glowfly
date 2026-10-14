//! Resolving a conflict: the side(s) picked per chunk, the file they produce, and writing and
//! staging it (which clears the conflict).

use super::conflict::{ConflictContent, ConflictFile, ConflictIds, Segment, Side};
use crate::error::{Error, Result};
use git2::build::CheckoutBuilder;
use git2::{IndexConflict, IndexEntry, IndexTime, Repository};
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
fn checked_index(repo: &Repository, path: &str, expected: &ConflictIds) -> Result<(git2::Index, IndexConflict)> {
    let index = repo.index()?;
    match index.conflict_get(Path::new(path)) {
        Ok(c) if ids_of(&c) == *expected => Ok((index, c)),
        _ => Err(Error::Other(format!("The conflict in {path} changed since it was opened. Close and reopen it."))),
    }
}

fn ids_of(c: &IndexConflict) -> ConflictIds {
    ConflictIds { ancestor: c.ancestor.as_ref().map(|e| e.id), ours: c.our.as_ref().map(|e| e.id), theirs: c.their.as_ref().map(|e| e.id) }
}

/// Writes `contents` to the working file and stages it, which marks the conflict resolved.
/// Refused if the conflict changed since `expected` was read.
pub fn write_resolution(workdir: &Path, path: &str, contents: &str, expected: &ConflictIds) -> Result<()> {
    let repo = Repository::open(workdir)?;
    let (mut index, _) = checked_index(&repo, path, expected)?;
    write_file(workdir, path, contents.as_bytes())?;
    index.add_path(Path::new(path))?;
    index.write()?;
    Ok(())
}

const TYPE_MASK: u32 = 0o170000;
const SYMLINK: u32 = 0o120000;
const GITLINK: u32 = 0o160000;
const STAGE_MASK: u16 = 0x3000;

/// Resolves with one side's whole version: its entry (content and type), or deleting the file
/// if that side deleted it. Refused if the conflict changed since `expected` was read.
pub fn resolve_with_side(workdir: &Path, path: &str, side: Side, expected: &ConflictIds) -> Result<()> {
    let repo = Repository::open(workdir)?;
    let (mut index, conflict) = checked_index(&repo, path, expected)?;
    let full = workdir.join(path);
    let chosen = match side {
        Side::Ours => conflict.our,
        Side::Theirs => conflict.their,
    };
    let Some(entry) = chosen else {
        remove_working(&full)?;
        index.remove_path(Path::new(path))?;
        index.write()?;
        return Ok(());
    };
    match entry.mode & TYPE_MASK {
        // The submodule's own checkout is left as it is.
        GITLINK => stage_entry(&mut index, path, entry)?,
        SYMLINK => {
            // Check the link out from the index (as a plain file where links are off), never
            // writing through whatever link the working tree holds now.
            stage_entry(&mut index, path, entry)?;
            remove_working(&full)?;
            let mut co = CheckoutBuilder::new();
            co.force().recreate_missing(true).skip_unmerged(true).disable_pathspec_match(true).path(path);
            repo.checkout_index(Some(&mut index), Some(&mut co))?;
        }
        _ => {
            let blob = repo.find_blob(entry.id)?;
            let crlf = !is_link(&full) && std::fs::read(&full).is_ok_and(|b| b.windows(2).any(|w| w == b"\r\n"));
            match std::str::from_utf8(blob.content()) {
                Ok(text) if crlf && !blob.is_binary() => write_file(workdir, path, to_crlf(text).as_bytes())?,
                _ => write_file(workdir, path, blob.content())?,
            }
            set_executable(&full, entry.mode)?;
            index.add_path(Path::new(path))?;
            // Where the file system can't say (no executable bit, links off), the mode comes from
            // the old entry: keep the chosen side's instead.
            if let Some(mut e) = index.get_path(Path::new(path), 0)
                && e.mode != entry.mode
            {
                e.mode = entry.mode;
                index.add(&e)?;
            }
        }
    }
    index.write()?;
    Ok(())
}

/// Stages `entry` (one side of the conflict at `path`) as the resolution.
fn stage_entry(index: &mut git2::Index, path: &str, mut entry: IndexEntry) -> Result<()> {
    index.conflict_remove(Path::new(path))?;
    entry.flags &= !STAGE_MASK;
    // No cached stat data, so the working file is always compared by content.
    let zero = IndexTime::new(0, 0);
    (entry.ctime, entry.mtime, entry.dev, entry.ino, entry.uid, entry.gid, entry.file_size) = (zero, zero, 0, 0, 0, 0, 0);
    index.add(&entry)?;
    Ok(())
}

/// Removes the file or link at `full` without following a link. A missing path or a real
/// directory is left alone.
fn remove_working(full: &Path) -> Result<()> {
    match std::fs::symlink_metadata(full) {
        Ok(m) if m.is_dir() => Ok(()),
        // A Windows link to a directory is removed as a directory.
        Ok(m) => Ok(std::fs::remove_file(full).or_else(|e| if m.file_type().is_symlink() { std::fs::remove_dir(full) } else { Err(e) })?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(unix)]
fn set_executable(full: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(full)?.permissions();
    let exec = mode & 0o111 != 0;
    let bits = perms.mode();
    perms.set_mode(if exec { bits | ((bits & 0o444) >> 2) } else { bits & !0o111 });
    std::fs::set_permissions(full, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_executable(_full: &Path, _mode: u32) -> Result<()> {
    Ok(())
}

fn is_link(full: &Path) -> bool {
    std::fs::symlink_metadata(full).is_ok_and(|m| m.file_type().is_symlink())
}

/// Writes a regular file at `path`, replacing a link there rather than writing through it.
fn write_file(workdir: &Path, path: &str, bytes: &[u8]) -> Result<()> {
    let full = workdir.join(path);
    if is_link(&full) {
        remove_working(&full)?;
    }
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
