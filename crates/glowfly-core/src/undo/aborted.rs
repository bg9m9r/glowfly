//! Aborting a merge, single cherry-pick or single revert while keeping its work for Undo, also
//! with conflicts left and with edits unrelated to the operation in the working tree.
//!
//! `abort_saving` first writes everything the abort throws away to the object database, as one
//! commit kept reachable by `refs/glowfly/aborted/<id>`: the index entries of the operation's
//! paths (`index`, and the conflict stages `base`, `ours` and `theirs`: git can't stash
//! unmerged entries), the working copies of those paths that differ from the index (`work`, the
//! exact bytes, with `absent` naming the ones deleted), every path it touches (`paths`) and the
//! files that say an operation is in progress (`state`: MERGE_HEAD, MERGE_MSG and the like). Its
//! parents are HEAD and the commits being merged or picked, so they stay reachable too. Then it
//! returns the commands that put only those paths back as HEAD has them and end the operation:
//! an edit to any other file stays where it is. The operation's paths are every path where the
//! index differs from HEAD, so a staged rename's old path is one of them.
//!
//! Undoing (`aborted_restore.rs`) puts the index, the files and the operation back and drops the
//! ref; a kept abort's ref that is never undone is dropped `KEEP_DAYS` later, at the next one.

use super::record::{branch_name, head};
use super::{Action, MoveOp, saved};
use crate::error::{Error, Result};
use crate::gitcli::args;
use crate::repo::{CLI_PATH_BYTES, Hazard, status_at};
use git2::{Index, IndexEntry, IndexTime, ObjectType, Oid, Repository, RepositoryState, Signature, Tree, TreeWalkMode, TreeWalkResult};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Where the kept work of each abort is referenced, by the kept commit's id.
pub const ABORT_REF_PREFIX: &str = "refs/glowfly/aborted/";

/// How long the kept work of an abort that is never undone stays referenced.
const KEEP_DAYS: i64 = 30;

/// The files in the git folder that say an operation is in progress (those present are kept).
pub(super) const STATE_FILES: [&str; 5] = ["MERGE_HEAD", "MERGE_MSG", "MERGE_MODE", "CHERRY_PICK_HEAD", "REVERT_HEAD"];

/// A gitlink's mode: a submodule, whose own checkout no abort or undo touches.
pub(super) const GITLINK: u32 = 0o160000;

/// An index or tree entry: its object and mode.
pub(super) type Entry = (Oid, u32);

/// What `abort_saving` kept and the commands that then abort.
#[derive(Debug)]
pub struct AbortSaving {
    /// Run in order through `gitcli`.
    pub cmds: Vec<Vec<String>>,
    /// What Undo reverses once the commands have run.
    pub action: Action,
    /// The ref that keeps the work, named in the job's output if the commands fail.
    pub kept: String,
}

/// The operation `state` is, when an abort can keep its work.
pub(super) fn op_of(state: RepositoryState) -> Option<MoveOp> {
    match state {
        RepositoryState::Merge => Some(MoveOp::Merge),
        RepositoryState::CherryPick => Some(MoveOp::CherryPick),
        RepositoryState::Revert => Some(MoveOp::Revert),
        _ => None,
    }
}

/// Every file (blob or gitlink) under `tree`, by path.
pub(super) fn tree_entries(tree: &Tree) -> Result<BTreeMap<String, Entry>> {
    let mut out = BTreeMap::new();
    let mut bad = false;
    tree.walk(TreeWalkMode::PreOrder, |root, e| {
        if e.kind() != Some(ObjectType::Tree) {
            match std::str::from_utf8(e.name_bytes()) {
                Ok(name) => {
                    out.insert(format!("{root}{name}"), (e.id(), e.filemode() as u32));
                }
                Err(_) => bad = true,
            }
        }
        TreeWalkResult::Ok
    })?;
    if bad {
        return Err(Error::Unsupported("A file name isn't UTF-8, so the work can't be kept for Undo".into()));
    }
    Ok(out)
}

/// A stage-0 index entry for `path`, without stat data (git compares the content instead).
pub(super) fn entry(path: &str, (id, mode): Entry) -> IndexEntry {
    IndexEntry {
        ctime: IndexTime::new(0, 0),
        mtime: IndexTime::new(0, 0),
        dev: 0,
        ino: 0,
        mode,
        uid: 0,
        gid: 0,
        file_size: 0,
        id,
        flags: path.len().min(0xfff) as u16,
        flags_extended: 0,
        path: path.as_bytes().to_vec(),
    }
}

/// A tree holding `entries` (path, object, mode).
fn tree_of<'a>(repo: &Repository, entries: impl Iterator<Item = (&'a String, Entry)>) -> Result<Oid> {
    let mut ix = Index::new()?;
    for (p, e) in entries {
        ix.add(&entry(p, e))?;
    }
    Ok(ix.write_tree_to(repo)?)
}

/// The index as stage-0 entries and conflicts (`[base, ours, theirs]`), by path.
type Split = (BTreeMap<String, Entry>, BTreeMap<String, [Option<Entry>; 3]>);

fn split_index(index: &Index) -> Result<Split> {
    let (mut stage0, mut conflicts) = (BTreeMap::new(), BTreeMap::<String, [Option<Entry>; 3]>::new());
    for e in index.iter() {
        let path = String::from_utf8(e.path.clone())
            .map_err(|_| Error::Unsupported("A file name isn't UTF-8, so the work can't be kept for Undo".into()))?;
        match (e.flags >> 12) & 3 {
            0 => {
                stage0.insert(path, (e.id, e.mode));
            }
            s => conflicts.entry(path).or_default()[s as usize - 1] = Some((e.id, e.mode)),
        }
    }
    Ok((stage0, conflicts))
}

/// Commands `start <paths…>`, as many as keep each command line's paths under `CLI_PATH_BYTES`.
pub(super) fn path_commands(start: &[&str], paths: &[String]) -> Vec<Vec<String>> {
    let (mut out, mut cmd, mut bytes) = (Vec::new(), args(start), 0);
    for p in paths {
        if bytes > 0 && bytes + p.len() + 1 > CLI_PATH_BYTES {
            out.push(std::mem::replace(&mut cmd, args(start)));
            bytes = 0;
        }
        bytes += p.len() + 1;
        cmd.push(p.clone());
    }
    if bytes > 0 {
        out.push(cmd);
    }
    out
}

/// Keeps the work of the merge, single cherry-pick or single revert in progress (see the module
/// doc) and returns the commands that then abort it. Refused, with nothing changed, when it
/// can't all be kept: a changed working copy that isn't a regular file, or a name that isn't
/// UTF-8.
pub fn abort_saving(workdir: &Path) -> Result<AbortSaving> {
    let repo = Repository::open(workdir)?;
    let Some(op) = op_of(repo.state()) else {
        return Err(Error::Unsupported("Only a merge, a single cherry-pick or a single revert can be aborted keeping its work".into()));
    };
    let (pos, Some(head_oid)) = head(&repo) else { return Err(Error::Unsupported("HEAD has no commit yet".into())) };
    let head_commit = repo.find_commit(head_oid)?;
    let in_head = tree_entries(&head_commit.tree()?)?;
    let mut index = repo.index()?;
    index.read(true)?;
    let (stage0, conflicts) = split_index(&index)?;
    let mut paths: BTreeSet<String> = conflicts.keys().cloned().collect();
    paths.extend(stage0.iter().filter(|(p, e)| in_head.get(*p) != Some(e)).map(|(p, _)| p.clone()));
    paths.extend(in_head.keys().filter(|p| !stage0.contains_key(*p) && !conflicts.contains_key(*p)).cloned());

    // The working copies that differ from the index.
    let (unstaged, _, _) = status_at(workdir, true)?;
    let mut touched = BTreeSet::new();
    for f in unstaged {
        if matches!(f.hazard, Some(Hazard::NotUtf8(_))) {
            return Err(Error::Unsupported("A changed file's name isn't UTF-8, so the work can't be kept for Undo".into()));
        }
        touched.extend(f.renamed_from);
        touched.insert(f.path);
    }
    let gitlink = |p: &String| {
        let modes = conflicts.get(p).into_iter().flatten().flatten().chain(stage0.get(p)).chain(in_head.get(p));
        modes.into_iter().any(|e| e.1 == GITLINK)
    };
    let (mut work, mut absent) = (BTreeMap::new(), Vec::new());
    for p in paths.iter().filter(|p| (conflicts.contains_key(*p) || touched.contains(*p)) && !gitlink(p)) {
        match saved::save(workdir, p) {
            Ok(Some(s)) => {
                work.insert(p, (s.blob, if s.executable { 0o100755 } else { 0o100644 }));
            }
            Ok(None) => absent.push(p.as_str()),
            Err(Error::Unsupported(_)) => {
                return Err(Error::Unsupported(format!("{p} isn't a regular file, so its changes can't be kept for Undo")));
            }
            Err(e) => return Err(e),
        }
    }

    let mut top = repo.treebuilder(None)?;
    let index_tree = tree_of(&repo, paths.iter().filter_map(|p| stage0.get(p).map(|e| (p, *e))))?;
    top.insert("index", index_tree, 0o040000)?;
    for (i, name) in ["base", "ours", "theirs"].into_iter().enumerate() {
        let t = tree_of(&repo, conflicts.iter().filter_map(|(p, s)| s[i].map(|e| (p, e))))?;
        top.insert(name, t, 0o040000)?;
    }
    top.insert("work", tree_of(&repo, work.into_iter())?, 0o040000)?;
    let mut state = repo.treebuilder(None)?;
    let mut heads = vec![head_commit.clone()];
    for name in STATE_FILES {
        let bytes = match std::fs::read(repo.path().join(name)) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        };
        if name.ends_with("_HEAD") {
            let text = String::from_utf8_lossy(&bytes);
            heads.extend(text.lines().filter_map(|l| Oid::from_str(l.trim()).ok()).filter_map(|o| repo.find_commit(o).ok()));
        }
        state.insert(name, repo.blob(&bytes)?, 0o100644)?;
    }
    top.insert("state", state.write()?, 0o040000)?;
    let joined = |v: Vec<&str>| v.join("\0");
    top.insert("paths", repo.blob(joined(paths.iter().map(String::as_str).collect()).as_bytes())?, 0o100644)?;
    top.insert("absent", repo.blob(joined(absent).as_bytes())?, 0o100644)?;
    let tree = repo.find_tree(top.write()?)?;
    heads.dedup_by_key(|c| c.id());
    let sig = Signature::now("Glowfly", "glowfly@localhost")?;
    let message = format!("Glowfly: work kept from aborting a {}\n\nUndo in Glowfly puts it back.\n", op.noun());
    let parents: Vec<&git2::Commit> = heads.iter().collect();
    let kept = repo.commit(None, &sig, &sig, &message, &tree, &parents)?;
    prune(&repo);
    let name = format!("{ABORT_REF_PREFIX}{kept}");
    repo.reference(&name, kept, true, &format!("glowfly: kept the work of an aborted {}", op.noun()))?;

    let all: Vec<String> = paths.iter().cloned().collect();
    let (old, new): (Vec<String>, Vec<String>) = all.iter().cloned().partition(|p| in_head.contains_key(p));
    let new: Vec<String> = new.into_iter().filter(|p| !gitlink(p)).collect();
    let mut cmds = path_commands(&["--literal-pathspecs", "reset", "-q", "--"], &all);
    cmds.extend(path_commands(&["--literal-pathspecs", "checkout", "-q", "--"], &old));
    // A file the operation added is untracked once reset, and is kept: it goes (ignored or not).
    // A submodule it added keeps its checkout, as with git's own abort.
    cmds.extend(path_commands(&["--literal-pathspecs", "clean", "-f", "-x", "-q", "--"], &new));
    // Ends the operation; the edits to other files stay.
    cmds.push(args(&["reset", "-q", "--merge"]));
    let action = Action::AbortKept { op, saved: kept, head: head_oid, branch: branch_name(&pos) };
    Ok(AbortSaving { cmds, action, kept: name })
}

/// Drops the refs of kept aborts older than `KEEP_DAYS`. A failure only leaves them for later.
fn prune(repo: &Repository) {
    let cutoff = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64) - KEEP_DAYS * 86_400;
    let Ok(refs) = repo.references_glob(&format!("{ABORT_REF_PREFIX}*")) else { return };
    for mut r in refs.flatten() {
        let old = r.peel_to_commit().map_or(true, |c| c.time().seconds() < cutoff);
        if old {
            let _ = r.delete();
        }
    }
}
