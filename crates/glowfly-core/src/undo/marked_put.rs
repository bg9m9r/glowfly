//! Carrying out an undo or redo of a conflict change (`marked.rs`), once `plan` has checked it
//! still applies: the bytes about to be written over are kept for the journal, then each
//! file's index entries are set exactly (conflict stages with their modes and blobs, or the
//! stage-0 entry, or none) in one index write, then each working path gets its recorded bytes
//! or link. git's resolve-undo record follows: a conflict brought back takes its record out
//! (`git update-index --unresolve`, when the record holds those very stages), and a resolution
//! put back records the conflict's stages again (libgit2 does, staging over the conflict).

use super::aborted::{GITLINK, entry};
use super::kept::{self, KeptWhat};
use super::marked::{ConflictFile, IndexAt, WorkAt, settle, too_large, work_at, wrote};
use super::{Action, MAX_KEPT_BYTES, Staged, saved};
use crate::error::{Error, Result};
use crate::gitcli::{self, NO_PROMPTS, READ_TIMEOUT, args, output_bytes};
use crate::repo::{self, CLI_PATH_BYTES, ConflictIds};
use git2::{Index, Oid, Repository};
use std::path::Path;

/// A symbolic link's mode.
const LINK: u32 = 0o120000;

/// Puts the files of the conflict change `action` back as they were before it (or, with
/// `redo`, after it) and returns the job's output. A failure partway says how far it got; the
/// bytes written over stay kept for the journal.
pub fn put_conflicts(workdir: &Path, action: &Action, redo: bool) -> Result<String> {
    let Action::Conflicts { change, files, .. } = action else { unreachable!("a conflict change") };
    let target = |f: &ConflictFile| if redo { f.after.clone() } else { f.before.clone() };
    let mut records = Vec::new();
    for f in files {
        let now = work_at(workdir, &f.path, false)?;
        if let WorkAt::File(s) = now
            && !now.same(&target(f).work)
        {
            let len = std::fs::metadata(workdir.join(repo::contained(&f.path)?))?.len();
            if len > MAX_KEPT_BYTES {
                settle(workdir, &records);
                return Err(too_large(&f.path));
            }
            let kept = saved::save(workdir, &f.path)?.unwrap_or(s);
            records.push((f.path.as_str(), kept::keep_file(workdir, &f.path, kept, KeptWhat::Conflict)?, kept.blob));
        }
    }
    let mut done = if unresolve(workdir, files, redo) {
        "git put some of the conflicts' stages back from its resolve-undo record, nothing else changed"
    } else {
        "nothing was changed"
    };
    let mut step = || -> Result<()> {
        {
            let repo = Repository::open(workdir)?;
            let mut index = repo.index()?;
            index.read(true)?;
            for f in files {
                set_index(&repo, &mut index, &f.path, &target(f).index)?;
            }
            index.write()?;
        }
        done = "the index was put back, but not every file";
        for f in files {
            let now = work_at(workdir, &f.path, false)?;
            let want = target(f);
            if !now.same(&want.work) {
                write_work(workdir, &f.path, now, want.work, &want.index)?;
            }
        }
        Ok(())
    };
    let result = step();
    settle(workdir, &records);
    match result {
        Ok(()) => Ok(wrote(*change, files, redo)),
        Err(e) if records.is_empty() => Err(Error::Other(format!("{e}. Putting the files back stopped: {done}"))),
        Err(e) => {
            Err(Error::Other(format!("{e}. Putting the files back stopped: {done}. What they held is kept in the journal's Lost tab")))
        }
    }
}

/// Takes git's resolve-undo record out for each conflict about to come back whose record holds
/// exactly its stages, as `git checkout -m` would. Only tidies: the index entries are set
/// exactly afterwards either way, so a failure (an older git) changes nothing that matters.
/// Returns whether it ran.
fn unresolve(workdir: &Path, files: &[ConflictFile], redo: bool) -> bool {
    let mut paths = Vec::new();
    for f in files {
        let IndexAt::Conflict([base, ours, theirs]) = (if redo { &f.after } else { &f.before }).index else { continue };
        let ids = ConflictIds { ancestor: base.map(|s| s.blob), ours: ours.map(|s| s.blob), theirs: theirs.map(|s| s.blob) };
        if repo::resolve_undo_ids(workdir, &f.path).ok().flatten() == Some(ids) {
            paths.push(f.path.clone());
        }
    }
    for batch in repo::path_batches(&paths, CLI_PATH_BYTES) {
        let mut cmd = args(&["-c", "core.fsmonitor=false", "update-index", "--unresolve", "--"]);
        cmd.extend(batch.iter().cloned());
        let _ = output_bytes(workdir, &cmd, NO_PROMPTS, Some(READ_TIMEOUT));
    }
    !paths.is_empty()
}

/// Sets `path`'s entries in `index` to `want` exactly.
fn set_index(repo: &Repository, index: &mut Index, path: &str, want: &IndexAt) -> Result<()> {
    let p = Path::new(path);
    let conflicted = (1..=3).any(|s| index.get_path(p, s).is_some());
    match want {
        // Staged over the conflict so libgit2 moves its stages into the resolve-undo record,
        // as `git add` does. A submodule can't be staged from a buffer.
        IndexAt::Merged(Some(s)) if conflicted && s.mode != GITLINK => {
            let blob = repo.find_blob(s.blob)?;
            index.add_frombuffer(&entry(path, (s.blob, s.mode)), blob.content())?;
            if index.get_path(p, 0).map(|e| (e.id, e.mode)) != Some((s.blob, s.mode)) {
                return Err(Error::Other(format!("{path} could not be staged as it was")));
            }
        }
        // Removed with the conflict moving into the resolve-undo record, as `git rm` does.
        IndexAt::Merged(None) => {
            let _ = index.remove_path(p);
        }
        IndexAt::Merged(Some(Staged { mode, blob })) => {
            remove_stages(index, p);
            index.add(&entry(path, (*blob, *mode)))?;
        }
        IndexAt::Conflict(stages) => {
            remove_stages(index, p);
            for (stage, s) in (1u16..).zip(stages) {
                if let Some(s) = s {
                    let mut e = entry(path, (s.blob, s.mode));
                    e.flags |= stage << 12;
                    index.add(&e)?;
                }
            }
        }
    }
    Ok(())
}

/// Removes every entry of `p`, leaving the resolve-undo record alone (`remove_path` would move
/// a conflict's stages into it).
fn remove_stages(index: &mut Index, p: &Path) {
    for stage in 0..=3 {
        let _ = index.remove(p, stage);
    }
}

/// Makes `path` hold `want` in place of `now` (`plan` refused a folder either way). A link is
/// checked out by git from the entry `index` holds for it (`plan` checked there is one).
fn write_work(workdir: &Path, path: &str, now: WorkAt, want: WorkAt, index: &IndexAt) -> Result<()> {
    if now != WorkAt::Absent && (matches!(now, WorkAt::Link(_)) || !matches!(want, WorkAt::File(_))) {
        saved::write(workdir, path, None)?;
    }
    match want {
        WorkAt::Absent => Ok(()),
        WorkAt::File(s) => saved::write(workdir, path, Some(s)),
        WorkAt::Link(target) => {
            let stage = link_stage(index, target).ok_or_else(|| Error::Other(format!("{path} has no link entry to check out")))?;
            let (ok, out) = gitcli::run(workdir, &args(&["checkout-index", "--force", &format!("--stage={stage}"), "--", path]));
            if ok { Ok(()) } else { Err(Error::Other(out.trim().to_owned())) }
        }
        WorkAt::Folder => Err(Error::Unsupported(format!("{path} would need to be a folder again"))),
    }
}

/// The stage of `index` holding the symbolic link to `target` (0 for a resolved entry).
pub(super) fn link_stage(index: &IndexAt, target: Oid) -> Option<usize> {
    let link = |s: &Option<Staged>| s.is_some_and(|s| s.mode == LINK && s.blob == target);
    match index {
        IndexAt::Merged(s) => link(s).then_some(0),
        IndexAt::Conflict(stages) => stages.iter().position(link).map(|i| i + 1),
    }
}
