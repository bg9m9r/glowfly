//! Resolving a conflict: the side(s) picked per chunk, the file they produce, and writing and
//! staging it (which clears the conflict).
//!
//! A resolution is always built in repository form, from the stages' own bytes, staged as a
//! blob, and then written to the working tree by git (`checkout-index`). So line endings,
//! `core.autocrlf`, `eol` and `filter` attributes, `working-tree-encoding` and the executable
//! bit (with `core.fileMode`) come out exactly as git itself would write them, and Trellis never
//! rewrites line endings or calls chmod on the working file.
//!
//! Resolutions keep git's resolve-undo record (the index's REUC extension), as `git add` does,
//! so a file resolved here can be marked conflicted again (`resolve_undo::mark_conflicted`).
//! Submodules are the exception: their resolution is staged without one.

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

/// The file as it will be staged, or `None` until every chunk has a choice. It is in
/// repository form: each line keeps the ending it has in the stage it came from, so a file that
/// mixes LF and CR LF lines keeps them as they are.
pub fn render_output(file: &ConflictFile, choices: &[Choice]) -> Option<String> {
    let ConflictContent::Text(segs) = &file.content else { return None };
    if choices.len() != file.chunk_count() || choices.contains(&Choice::Unresolved) {
        return None;
    }
    let mut out = String::new();
    let mut push = |line: &str| {
        // A side without a final newline followed by more text still needs a line break: the
        // one the text so far uses.
        if !out.is_empty() && !out.ends_with('\n') {
            out.push_str(if out.contains("\r\n") { "\r\n" } else { "\n" });
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
    Some(out)
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

/// Stages `contents` (in repository form, as [`render_output`] makes it) as the resolution of
/// `path`, which marks the conflict resolved, and has git write the working file from it.
/// Refused if the conflict changed since `expected` was read.
pub fn write_resolution(workdir: &Path, path: &str, contents: &str, expected: &ConflictIds) -> Result<()> {
    let repo = Repository::open(workdir)?;
    let (mut index, conflict) = checked_index(&repo, path, expected)?;
    let side = conflict.our.or(conflict.their);
    let mut entry = side.ok_or_else(|| Error::Other(format!("{path} has no side to resolve to")))?;
    if entry.mode & TYPE_MASK != REGULAR {
        entry.mode = 0o100644;
    }
    stage_recording(&mut index, entry, contents.as_bytes())?;
    index.write()?;
    check_out(workdir, path, index, repo)
}

const TYPE_MASK: u32 = 0o170000;
const SYMLINK: u32 = 0o120000;
const GITLINK: u32 = 0o160000;
const REGULAR: u32 = 0o100000;
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
            let target = repo.find_blob(entry.id)?.content().to_vec();
            stage_recording(&mut index, entry, &target)?;
            remove_working(&full)?;
            let mut co = CheckoutBuilder::new();
            co.force().recreate_missing(true).skip_unmerged(true).disable_pathspec_match(true).path(path);
            repo.checkout_index(Some(&mut index), Some(&mut co))?;
        }
        // A file: stage the side's blob as it is (the same blob id) and let git write it.
        _ => {
            let data = repo.find_blob(entry.id)?.content().to_vec();
            stage_recording(&mut index, entry, &data)?;
            index.write()?;
            return check_out(workdir, path, index, repo);
        }
    }
    index.write()?;
    Ok(())
}

/// Stages `entry` (one side of the conflict at `path`) as the resolution, without a
/// resolve-undo record: for submodules, which `stage_recording` can't stage.
fn stage_entry(index: &mut git2::Index, path: &str, mut entry: IndexEntry) -> Result<()> {
    index.conflict_remove(Path::new(path))?;
    clear_stage(&mut entry);
    index.add(&entry)?;
    Ok(())
}

/// Stages `data` as the resolution of the conflict `entry` belongs to (its path and mode), and
/// moves the conflict's stages into the resolve-undo record. libgit2 records them only when
/// adding from a buffer with the conflict still in the index, so it isn't removed first.
fn stage_recording(index: &mut git2::Index, mut entry: IndexEntry, data: &[u8]) -> Result<()> {
    clear_stage(&mut entry);
    index.add_frombuffer(&entry, data)?;
    Ok(())
}

/// Makes `entry` a stage-0 entry with no cached stat data, so the working file is always
/// compared by content.
fn clear_stage(entry: &mut IndexEntry) {
    entry.flags &= !STAGE_MASK;
    let zero = IndexTime::new(0, 0);
    (entry.ctime, entry.mtime, entry.dev, entry.ino, entry.uid, entry.gid, entry.file_size) = (zero, zero, 0, 0, 0, 0, 0);
}

/// Writes the staged `path` to the working tree with `git checkout-index`, so git's smudge
/// filter and encoding run, and refreshes its stat data in the index. The repository is
/// closed first: Windows locks files that are held open.
fn check_out(workdir: &Path, path: &str, index: git2::Index, repo: Repository) -> Result<()> {
    drop(index);
    drop(repo);
    let full = workdir.join(path);
    // checkout-index replaces a file but not a link; a link is never written through.
    if is_link(&full) {
        remove_working(&full)?;
    }
    super::index::git(workdir, &["checkout-index", "--force", "-u", "--", path])
}

/// Removes the file or link at `full` without following a link. A missing path or a real
/// directory is left alone.
fn remove_working(full: &Path) -> Result<()> {
    match std::fs::symlink_metadata(full) {
        Ok(m) if m.is_dir() => Ok(()),
        // A Windows link to a directory is removed as a directory.
        Ok(m) => {
            Ok(crate::persist::remove_file(full)
                .or_else(|e| if m.file_type().is_symlink() { std::fs::remove_dir(full) } else { Err(e) })?)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn is_link(full: &Path) -> bool {
    std::fs::symlink_metadata(full).is_ok_and(|m| m.file_type().is_symlink())
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
}
