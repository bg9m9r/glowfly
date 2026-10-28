//! Carrying out an undo or redo of a conflict change (`marked.rs`), once `plan` has checked it
//! still applies: the bytes about to be written over are kept for the journal, then each
//! file's index entries are set exactly (conflict stages with their modes and blobs, or the
//! stage-0 entry, or none) in one index write, then each working path gets its recorded bytes
//! or link. git's resolve-undo record follows: a conflict brought back takes its record out
//! (`git update-index --unresolve`, when the record holds those very stages), and a resolution
//! put back records the conflict's stages again (libgit2 does, staging over the conflict).

use super::aborted::{GITLINK, KEPT_FLAGS, entry};
use super::kept::{self, KeptWhat};
use super::marked::{ConflictFile, EntryFlags, IndexAt, WorkAt, index_at, mixed, settle, too_large, work_at, wrote};
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
/// bytes written over stay kept for the journal. The index is written only when nothing else
/// wrote it since it was read, and a file only while it still holds what `plan` checked.
pub fn put_conflicts(workdir: &Path, action: &Action, redo: bool) -> Result<String> {
    let Action::Conflicts { change, files, .. } = action else { unreachable!("a conflict change") };
    let target = |f: &ConflictFile| if redo { f.after.clone() } else { f.before.clone() };
    let expect = |f: &ConflictFile| if redo { f.before.clone() } else { f.after.clone() };
    let moved = |path: &str| Error::Other(format!("{path} changed in the index while it was being put back"));
    // Checked again here, before `unresolve` can touch the index.
    {
        let repo = Repository::open(workdir)?;
        let index = repo.index()?;
        if let Some(f) = files.iter().find(|f| mixed(&index, &f.path) || index_at(&index, &f.path) != expect(f).index) {
            return Err(moved(&f.path));
        }
    }
    let mut records = Vec::new();
    for f in files {
        let now = work_at(workdir, &f.path, false)?;
        if let WorkAt::File(s) = now
            && now != target(f).work
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
            let stamp = index_stamp(&repo);
            for f in files {
                let now = index_at(&index, &f.path);
                // `unresolve` may have put the conflict's stages back already.
                if mixed(&index, &f.path) || (now != expect(f).index && now != target(f).index) {
                    return Err(moved(&f.path));
                }
            }
            for f in files {
                set_index(&repo, &mut index, &f.path, &target(f).index)?;
            }
            if index_stamp(&repo) != stamp {
                return Err(Error::Other("Something else wrote the index meanwhile".into()));
            }
            index.write()?;
        }
        done = "the index was put back, but not every file";
        for f in files {
            let now = work_at(workdir, &f.path, false)?;
            let want = target(f);
            if now == want.work {
                continue;
            }
            if now != expect(f).work {
                return Err(Error::Other(format!("{} changed while it was being put back, so it was left as it is", f.path)));
            }
            write_work(workdir, &f.path, now, want.work, &want.index)?;
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

/// The index file's size, modification time and content hash, to tell whether something wrote
/// it since it was read (`None` when it can't be read).
fn index_stamp(repo: &Repository) -> Option<(u64, Option<std::time::SystemTime>, Oid)> {
    let file = repo.path().join("index");
    let meta = std::fs::metadata(&file).ok()?;
    let hash = Oid::hash_file(git2::ObjectType::Blob, &file).ok()?;
    Some((meta.len(), meta.modified().ok(), hash))
}

/// Takes git's resolve-undo record out for each conflict about to come back whose record holds
/// exactly its stages, as `git checkout -m` would. Only tidies: the index entries are set
/// exactly afterwards either way, so a failure (an older git) changes nothing that matters.
/// Skipped with a split index, whose shared part git would rewrite. Returns whether it ran.
fn unresolve(workdir: &Path, files: &[ConflictFile], redo: bool) -> bool {
    let split = Repository::open(workdir).ok().and_then(|r| r.config().ok()).and_then(|c| c.get_bool("core.splitIndex").ok());
    if split == Some(true) {
        return false;
    }
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

/// Sets `path`'s entries in `index` to `want` exactly (a stage-0 entry with its flags). Any
/// failure stops before a file is written.
fn set_index(repo: &Repository, index: &mut Index, path: &str, want: &IndexAt) -> Result<()> {
    let p = Path::new(path);
    let conflicted = (1..=3).any(|s| index.get_path(p, s).is_some());
    match want {
        // Staged over the conflict so libgit2 moves its stages into the resolve-undo record,
        // as `git add` does. A submodule can't be staged from a buffer.
        IndexAt::Merged(Some(s), flags) if conflicted && s.mode != GITLINK => {
            let blob = repo.find_blob(s.blob)?;
            index.add_frombuffer(&entry(path, (s.blob, s.mode)), blob.content())?;
            set_flags(index, path, *flags)?;
            if index.get_path(p, 0).map(|e| (e.id, e.mode)) != Some((s.blob, s.mode)) {
                return Err(Error::Other(format!("{path} could not be staged as it was")));
            }
        }
        // Removed with the conflict moving into the resolve-undo record, as `git rm` does.
        IndexAt::Merged(None, _) => index.remove_path(p)?,
        IndexAt::Merged(Some(Staged { mode, blob }), flags) => {
            remove_stages(index, p)?;
            index.add(&entry(path, (*blob, *mode)))?;
            set_flags(index, path, *flags)?;
        }
        IndexAt::Conflict(stages) => {
            remove_stages(index, p)?;
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

/// Gives `path`'s stage-0 entry the kept `flags` (assume-unchanged, skip-worktree,
/// intent-to-add), when it had any.
fn set_flags(index: &mut Index, path: &str, flags: EntryFlags) -> Result<()> {
    if flags == EntryFlags::default() {
        return Ok(());
    }
    let mut e = index.get_path(Path::new(path), 0).ok_or_else(|| Error::Other(format!("{path} is missing from the index")))?;
    e.flags |= flags.flags & KEPT_FLAGS;
    e.flags_extended = flags.extended;
    index.add(&e)?;
    Ok(())
}

/// Removes every entry of `p`, leaving the resolve-undo record alone (`remove_path` would move
/// a conflict's stages into it). A stage that isn't there is fine; any other failure isn't.
fn remove_stages(index: &mut Index, p: &Path) -> Result<()> {
    for stage in 0..=3 {
        if index.get_path(p, stage).is_some() {
            index.remove(p, stage)?;
        }
    }
    Ok(())
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
        IndexAt::Merged(s, _) => link(s).then_some(0),
        IndexAt::Conflict(stages) => stages.iter().position(link).map(|i| i + 1),
    }
}
