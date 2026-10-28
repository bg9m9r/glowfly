//! Undoing an abort that kept its work (`aborted.rs`): the operation's paths get their index
//! entries (conflicts too) and working copies back, then the files that say the merge,
//! cherry-pick or revert is in progress, and the kept ref goes. Only while HEAD is where the
//! abort left it and every one of those paths is still as the abort left it (as HEAD has it);
//! other files may have changed since, and stay as they are. Redoing aborts again, keeping the
//! work anew.

use super::aborted::{ABORT_REF_PREFIX, Entry, GITLINK, STATE_FILES, entry, op_of, path_commands, tree_entries};
use super::planning::{at, busy_name, dirty, stale};
use super::record::head;
use super::{Action, Plan, Refusal, Saved, saved};
use crate::error::{Error, Result};
use crate::gitcli;
use crate::repo::status_at;
use git2::{Commit, Oid, Repository};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// What a kept abort holds (see `aborted.rs`).
struct Kept {
    paths: Vec<String>,
    absent: BTreeSet<String>,
    index: BTreeMap<String, Entry>,
    /// `[base, ours, theirs]` of each conflicted path.
    conflicts: BTreeMap<String, [Option<Entry>; 3]>,
    work: BTreeMap<String, Entry>,
    state: Vec<(String, Oid)>,
}

fn names(repo: &Repository, blob: Oid) -> Result<Vec<String>> {
    let b = repo.find_blob(blob)?;
    let text = std::str::from_utf8(b.content()).map_err(|_| Error::Other("The kept work's path list is damaged".into()))?;
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
    let state = repo.find_tree(part("state")?)?;
    let state = state.iter().filter_map(|e| Some((e.name()?.to_owned(), e.id()))).filter(|(n, _)| STATE_FILES.contains(&n.as_str()));
    Ok(Kept {
        paths: names(repo, part("paths")?)?,
        absent: names(repo, part("absent")?)?.into_iter().collect(),
        index: sub("index")?,
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

/// Undoes (or, with `redo`, does again) the kept abort `action`.
pub(super) fn plan(workdir: &Path, action: &Action, redo: bool) -> Result<Plan> {
    let Action::AbortKept { op, saved, head: at_head, branch } = action else { unreachable!("a kept abort") };
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
    let (unstaged, staged, _) = status_at(workdir, true)?;
    let changed: BTreeSet<String> = unstaged.into_iter().chain(staged).flat_map(|f| [Some(f.path), f.renamed_from]).flatten().collect();
    for p in &kept.paths {
        let now = index.get_path(Path::new(p), 0).map(|e| (e.id, e.mode));
        let conflicted = (1..=3).any(|s| index.get_path(Path::new(p), s).is_some());
        let moved = now.as_ref() != in_head.get(p) || conflicted;
        // A submodule's own checkout is never touched, so only its entry counts.
        let edited = !is_gitlink(&kept, &in_head, p)
            && (changed.contains(p) || (!in_head.contains_key(p) && workdir.join(p).symlink_metadata().is_ok()));
        if moved || edited {
            return Ok(dirty(action, redo));
        }
    }
    Ok(Plan::RestoreAbort)
}

/// Puts the kept abort `action` back (planned by `plan` as `Plan::RestoreAbort`): the index
/// entries, the working copies, then the operation's own files; then drops the kept ref.
/// Returns the job's output.
pub fn restore_abort(workdir: &Path, action: &Action) -> Result<String> {
    let Action::AbortKept { op, saved, head: at_head, .. } = action else { unreachable!("a kept abort") };
    let repo = Repository::open(workdir)?;
    let kept = read_kept(&repo, &repo.find_commit(*saved)?)?;
    let in_head = tree_entries(&repo.find_commit(*at_head)?.tree()?)?;
    let failed = |e: Error| Error::Other(format!("{e}. The kept work stays in {ABORT_REF_PREFIX}{saved}"));

    let mut index = repo.index()?;
    index.read(true)?;
    for p in &kept.paths {
        let _ = index.remove_path(Path::new(p));
    }
    for (p, e) in &kept.index {
        index.add(&entry(p, *e))?;
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
    index.write().map_err(|e| failed(e.into()))?;

    let mut from_index = Vec::new();
    for p in &kept.paths {
        if is_gitlink(&kept, &in_head, p) {
            continue;
        }
        let write = |content: Option<Saved>| saved::write(workdir, p, content).map_err(failed);
        if let Some((blob, mode)) = kept.work.get(p) {
            write(Some(Saved { blob: *blob, executable: *mode == 0o100755 }))?;
        } else if kept.absent.contains(p) || !kept.index.contains_key(p) {
            write(None)?;
        } else {
            from_index.push(p.clone());
        }
    }
    let mut output = Vec::new();
    for cmd in path_commands(&["--literal-pathspecs", "checkout-index", "-f", "-u", "--"], &from_index) {
        let (ok, out) = gitcli::run(workdir, &cmd);
        if !ok {
            return Err(failed(Error::Other(out.trim().to_owned())));
        }
        output.push(out);
    }
    for (name, blob) in &kept.state {
        let bytes = repo.find_blob(*blob)?;
        crate::persist::atomic_write(&repo.path().join(name), bytes.content()).map_err(|e| failed(e.into()))?;
    }
    if let Ok(mut r) = repo.find_reference(&format!("{ABORT_REF_PREFIX}{saved}")) {
        let _ = r.delete();
    }
    output.push(format!("The {} is in progress again, with the work it had", op.noun()));
    Ok(output.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n"))
}
