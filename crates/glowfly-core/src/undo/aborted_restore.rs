//! Undoing an abort that kept its work (`aborted.rs`): the operation's paths get their index
//! entries (conflicts and flags too) and exact bytes on disk back, then the files that say the
//! merge, cherry-pick or revert is in progress, and the kept ref goes. Only while HEAD is where
//! the abort left it and every one of those paths is still as the abort left it: the same index
//! entry as HEAD, and the same bytes on disk as right after the abort (`left`, read byte for
//! byte, so an edit a clean filter hides from status counts). Other files may have changed
//! since, and stay as they are. Redoing aborts again, keeping the work anew. `put_back` is also
//! how a failed abort is rolled back.

use super::aborted::{ABORT_REF_PREFIX, Entry, GITLINK, KEPT_FLAGS, OnDisk, STATE_FILES, entry, on_disk, op_of, tree_entries};
use super::planning::{at, busy_name, dirty, stale};
use super::record::head;
use super::stash::hash;
use super::{Action, Changes, Plan, Refusal, Saved, saved};
use crate::error::{Error, Result};
use git2::{Commit, Oid, Repository};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// What a kept abort holds (see `aborted.rs`).
struct Kept {
    paths: Vec<String>,
    absent: BTreeSet<String>,
    index: BTreeMap<String, Entry>,
    /// Each stage-0 entry's kept flags (`KEPT_FLAGS`, extended flags), where any is set.
    flags: BTreeMap<String, (u16, u16)>,
    /// `[base, ours, theirs]` of each conflicted path.
    conflicts: BTreeMap<String, [Option<Entry>; 3]>,
    work: BTreeMap<String, Entry>,
    state: Vec<(String, Oid)>,
}

fn damaged() -> Error {
    Error::Other("The kept work is damaged".into())
}

/// The NUL-separated fields of blob `blob`.
fn fields(repo: &Repository, blob: Oid) -> Result<Vec<String>> {
    let b = repo.find_blob(blob)?;
    let text = std::str::from_utf8(b.content()).map_err(|_| damaged())?;
    Ok(text.split('\0').filter(|s| !s.is_empty()).map(str::to_owned).collect())
}

fn read_kept(repo: &Repository, commit: &Commit) -> Result<Kept> {
    let tree = commit.tree()?;
    let part = |name: &str| -> Result<Oid> {
        tree.get_name(name).map(|e| e.id()).ok_or_else(|| Error::Other(format!("The kept work has no {name}")))
    };
    let sub = |name: &str| -> Result<BTreeMap<String, Entry>> { tree_entries(&repo.find_tree(part(name)?)?) };
    let mut conflicts = BTreeMap::<String, [Option<Entry>; 3]>::new();
    for (i, name) in ["base", "ours", "theirs"].into_iter().enumerate() {
        for (p, e) in sub(name)? {
            conflicts.entry(p).or_default()[i] = Some(e);
        }
    }
    let mut flags = BTreeMap::new();
    for f in fields(repo, part("flags")?)?.chunks(3) {
        let [p, f, x] = f else { return Err(damaged()) };
        flags.insert(p.clone(), (f.parse().map_err(|_| damaged())?, x.parse().map_err(|_| damaged())?));
    }
    let state = repo.find_tree(part("state")?)?;
    let state = state.iter().filter_map(|e| Some((e.name()?.to_owned(), e.id()))).filter(|(n, _)| STATE_FILES.contains(&n.as_str()));
    Ok(Kept {
        paths: fields(repo, part("paths")?)?,
        absent: fields(repo, part("absent")?)?.into_iter().collect(),
        index: sub("index")?,
        flags,
        conflicts,
        work: sub("work")?,
        state: state.collect(),
    })
}

/// Whether `path` is a submodule in any of the entries the abort saw.
fn is_gitlink(kept: &Kept, head: &BTreeMap<String, Entry>, path: &str) -> bool {
    let conflict = kept.conflicts.get(path).into_iter().flatten().flatten();
    conflict.chain(kept.index.get(path)).chain(head.get(path)).any(|e| e.1 == GITLINK)
}

/// The bytes on disk at each of the kept abort `saved`'s paths, read right after the abort.
pub(super) fn left(workdir: &Path, saved: Oid) -> Result<Changes> {
    let repo = Repository::open(workdir)?;
    let kept = read_kept(&repo, &repo.find_commit(saved)?)?;
    let mut out = Vec::with_capacity(kept.paths.len());
    for p in kept.paths {
        let h = hash(workdir, &p)?;
        out.push((p, h));
    }
    Ok(Changes(out))
}

/// Undoes (or, with `redo`, does again) the kept abort `action`.
pub(super) fn plan(workdir: &Path, action: &Action, redo: bool) -> Result<Plan> {
    let Action::AbortKept { op, saved, head: at_head, branch, left } = action else { unreachable!("a kept abort") };
    let repo = Repository::open(workdir)?;
    let (pos, head_oid) = head(&repo);
    let what = action.describe();
    if redo {
        if op_of(repo.state()) != Some(*op) {
            return Ok(stale(format!("The {} is no longer in progress", op.noun())));
        }
        return Ok(if at(&pos, head_oid, branch, *at_head) { Plan::AbortAgain } else { stale(format!("HEAD has moved since the {what}")) });
    }
    if let Some(busy) = busy_name(repo.state()) {
        return Ok(Plan::Refuse(Refusal::Busy(format!("Finish or abort the {busy} in progress first."))));
    }
    let kept_in = format!("; the kept work stays in {ABORT_REF_PREFIX}{saved}");
    if !at(&pos, head_oid, branch, *at_head) {
        return Ok(stale(format!("HEAD has moved since the {what}{kept_in}")));
    }
    let Ok(commit) = repo.find_commit(*saved) else { return Ok(stale(format!("The work kept by the {what} is gone"))) };
    let kept = read_kept(&repo, &commit)?;
    let in_head = tree_entries(&repo.find_commit(*at_head)?.tree()?)?;
    let index = repo.index()?;
    let after: BTreeMap<&str, Option<Oid>> = left.0.iter().map(|(p, h)| (p.as_str(), *h)).collect();
    for p in &kept.paths {
        let now = index.get_path(Path::new(p), 0).map(|e| (e.id, e.mode));
        let conflicted = (1..=3).any(|s| index.get_path(Path::new(p), s).is_some());
        // A submodule's own checkout is never touched, so only its entry counts.
        let edited = !is_gitlink(&kept, &in_head, p) && after.get(p.as_str()) != Some(&hash(workdir, p)?);
        if now.as_ref() != in_head.get(p) || conflicted || edited {
            return Ok(dirty(action, redo));
        }
    }
    Ok(Plan::RestoreAbort)
}

/// Puts the kept abort `action` back (planned by `plan` as `Plan::RestoreAbort`) and returns
/// the job's output.
pub fn restore_abort(workdir: &Path, action: &Action) -> Result<String> {
    let Action::AbortKept { op, saved, head, .. } = action else { unreachable!("a kept abort") };
    put_back(workdir, *saved, *head)?;
    Ok(format!("The {} is in progress again, with the work it had", op.noun()))
}

/// Removes the file or link at `path`, if there is one (a folder stays: its files are paths of
/// their own).
fn remove(workdir: &Path, path: &str) -> Result<()> {
    match on_disk(workdir, path)? {
        OnDisk::Item => saved::write(workdir, path, None),
        OnDisk::Nothing | OnDisk::Folder => Ok(()),
    }
}

/// Puts the work kept in `saved` (HEAD at `at_head`) back: the index entries, then what was on
/// disk (removals first, so a file can become a folder and back), then the operation's own
/// files; then drops the kept ref. Doesn't check the repository first: `plan` does for Undo, and
/// a failed abort is rolled back with it. A failure partway says how far it got; the ref stays.
pub(super) fn put_back(workdir: &Path, saved: Oid, at_head: Oid) -> Result<()> {
    let name = format!("{ABORT_REF_PREFIX}{saved}");
    let mut done = "nothing was changed";
    let mut step = || -> Result<()> {
        let repo = Repository::open(workdir)?;
        let kept = read_kept(&repo, &repo.find_commit(saved)?)?;
        let in_head = tree_entries(&repo.find_commit(at_head)?.tree()?)?;
        let mut index = repo.index()?;
        index.read(true)?;
        for p in &kept.paths {
            let _ = index.remove_path(Path::new(p));
        }
        for (p, e) in &kept.index {
            let mut e = entry(p, *e);
            if let Some((f, x)) = kept.flags.get(p) {
                e.flags |= f & KEPT_FLAGS;
                e.flags_extended = *x;
            }
            index.add(&e)?;
        }
        for (p, sides) in &kept.conflicts {
            for (stage, e) in (1u16..).zip(sides) {
                if let Some(e) = e {
                    let mut e = entry(p, *e);
                    e.flags |= stage << 12;
                    index.add(&e)?;
                }
            }
        }
        index.write()?;
        done = "the index was put back, but not every file";
        let files: Vec<&String> = kept.paths.iter().filter(|p| !is_gitlink(&kept, &in_head, p)).collect();
        for p in files.iter().filter(|p| !kept.work.contains_key(**p)) {
            remove(workdir, p)?;
        }
        for (p, (blob, mode)) in kept.work.iter().filter(|(p, _)| !kept.absent.contains(*p)) {
            saved::write(workdir, p, Some(Saved { blob: *blob, executable: *mode == 0o100755 }))?;
        }
        done = "the index and the files were put back, but the operation isn't in progress yet";
        for (file, blob) in &kept.state {
            crate::persist::atomic_write(&repo.path().join(file), repo.find_blob(*blob)?.content())?;
        }
        if let Ok(mut r) = repo.find_reference(&name) {
            let _ = r.delete();
        }
        Ok(())
    };
    let result = step();
    result.map_err(|e| Error::Other(format!("{e}. Putting the work back stopped: {done}. All of it is still kept in {name}")))
}
