//! Aborting a merge, cherry-pick, revert, rebase or patch series while keeping its work for
//! Undo, also with conflicts left and with edits unrelated to the operation in the working tree.
//!
//! `abort_saving` first writes everything the abort throws away to the object database, as one
//! commit kept reachable by `refs/glowfly/aborted/<id>`: the index entries of the operation's
//! paths (`index`, the conflict stages `base`, `ours` and `theirs`: git can't stash unmerged
//! entries, and `flags`, the entries' intent-to-add, skip-worktree and assume-unchanged flags),
//! the exact bytes of whatever is on disk at each of those paths (`work`, ignored files too, with
//! `absent` naming the paths where nothing is), every path it touches (`paths`) and the files
//! that say an operation is in progress (`state`: MERGE_HEAD, REBASE_HEAD, MERGE_MSG and the
//! like). Its parents are HEAD and the commits being merged, picked or rebased, so they stay
//! reachable too. Which paths those are, and the refusals for what is on disk at them, are in
//! `aborted_paths.rs`.
//!
//! A merge, single cherry-pick or single revert then gets the commands that put only those paths
//! back as HEAD has them and end the operation: an edit to any other file stays where it is. A
//! run of steps (a rebase, a patch series, a multi-commit cherry-pick or revert) also keeps its
//! state folder, where HEAD was and the refs its abort moves (`aborted_state.rs`), then runs git's
//! own `<verb> --abort`, which takes the branch back where it started.
//!
//! `aborted_run.rs` runs the commands (putting everything back when one fails), `aborted_restore.rs`
//! undoes the abort. A kept abort's ref that is never undone is dropped `KEEP_DAYS` later, at the
//! next one, unless an undo entry still names it.

use super::record::{branch_name, head};
use super::{Action, Changes, HeadAt, MoveOp, aborted_paths, aborted_state};
use crate::error::{Error, Result};
use crate::gitcli::args;
use crate::repo::CLI_PATH_BYTES;
use git2::{Index, IndexEntry, IndexTime, ObjectType, Oid, Repository, RepositoryState, Signature, Tree, TreeWalkMode, TreeWalkResult};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;

/// Where the kept work of each abort is referenced, by the kept commit's id.
pub const ABORT_REF_PREFIX: &str = "refs/glowfly/aborted/";

/// How long the kept work of an abort that is never undone stays referenced.
const KEEP_DAYS: i64 = 30;

/// The files in the git folder that say an operation is in progress, and ORIG_HEAD, which the
/// abort's reset moves (those present are kept).
pub(super) const STATE_FILES: [&str; 9] =
    ["MERGE_HEAD", "MERGE_MSG", "MERGE_MODE", "CHERRY_PICK_HEAD", "REVERT_HEAD", "REBASE_HEAD", "SQUASH_MSG", "AUTO_MERGE", "ORIG_HEAD"];

/// A gitlink's mode: a submodule, whose own checkout no abort or undo touches.
pub(super) const GITLINK: u32 = 0o160000;

/// The index entry flag bits kept besides the stage and name length: assume-unchanged.
pub(super) const KEPT_FLAGS: u16 = 0x8000;

/// An index or tree entry: its object and mode.
pub(super) type Entry = (Oid, u32);

/// What `abort_saving` kept and the commands that then abort.
#[derive(Debug)]
pub(super) struct AbortSaving {
    /// Run in order through `gitcli`.
    pub cmds: Vec<Vec<String>>,
    /// What Undo reverses, its `left` still to be read once the commands have run.
    pub action: Action,
    /// The ref that keeps the work.
    pub kept: String,
}

/// How an operation in progress is aborted keeping its work.
pub(super) enum Aborting {
    /// A merge, single cherry-pick or single revert: only its own paths go back, path-wise.
    Single(MoveOp),
    /// A run of steps, ended by git's own `<verb> --abort` once its state folder `dir` (in the
    /// git dir) and everything else that abort throws away is kept.
    Steps { op: MoveOp, dir: &'static str, verb: &'static str },
}

impl Aborting {
    fn op(&self) -> MoveOp {
        match self {
            Aborting::Single(op) | Aborting::Steps { op, .. } => *op,
        }
    }
}

/// How the operation `state` is aborted keeping its work, when it can be.
pub(super) fn aborting(state: RepositoryState) -> Option<Aborting> {
    use RepositoryState::*;
    let steps = |op, dir, verb| Some(Aborting::Steps { op, dir, verb });
    match state {
        Merge => Some(Aborting::Single(MoveOp::Merge)),
        CherryPick => Some(Aborting::Single(MoveOp::CherryPick)),
        Revert => Some(Aborting::Single(MoveOp::Revert)),
        CherryPickSequence => steps(MoveOp::CherryPick, "sequencer", "cherry-pick"),
        RevertSequence => steps(MoveOp::Revert, "sequencer", "revert"),
        RebaseMerge | RebaseInteractive => steps(MoveOp::Rebase, "rebase-merge", "rebase"),
        Rebase => steps(MoveOp::Rebase, "rebase-apply", "rebase"),
        ApplyMailbox => steps(MoveOp::Am, "rebase-apply", "am"),
        _ => None,
    }
}

/// The operation `state` is, when an abort can keep its work.
pub(super) fn op_of(state: RepositoryState) -> Option<MoveOp> {
    aborting(state).map(|a| a.op())
}

pub(super) fn not_utf8() -> Error {
    Error::Unsupported("A file name isn't UTF-8, so the work can't be kept for Undo".into())
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
        return Err(not_utf8());
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

/// The index as stage-0 entries, their kept flags (`KEPT_FLAGS`, extended flags) where any is
/// set, and conflicts (`[base, ours, theirs]`), by path.
pub(super) struct Split {
    pub stage0: BTreeMap<String, Entry>,
    pub flags: BTreeMap<String, (u16, u16)>,
    pub conflicts: BTreeMap<String, [Option<Entry>; 3]>,
}

fn split_index(index: &Index) -> Result<Split> {
    let mut s = Split { stage0: BTreeMap::new(), flags: BTreeMap::new(), conflicts: BTreeMap::new() };
    for e in index.iter() {
        let path = String::from_utf8(e.path.clone()).map_err(|_| not_utf8())?;
        match (e.flags >> 12) & 3 {
            0 => {
                let kept = (e.flags & KEPT_FLAGS, e.flags_extended);
                if kept != (0, 0) {
                    s.flags.insert(path.clone(), kept);
                }
                s.stage0.insert(path, (e.id, e.mode));
            }
            n => s.conflicts.entry(path).or_default()[n as usize - 1] = Some((e.id, e.mode)),
        }
    }
    Ok(s)
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

/// Keeps the work of the operation in progress (see the module doc) and returns the commands
/// that then abort it. Refused, with nothing changed, when it can't all be kept: something at
/// one of its paths that isn't a regular file (or a folder of its own files), a name that isn't
/// UTF-8, or a run of steps git's abort wouldn't take back as Undo expects (`aborted_state`).
/// Refs of kept aborts older than `KEEP_DAYS` are dropped unless `prune` (the kept commits undo
/// entries still name) lists them; `None` drops none.
pub(super) fn abort_saving(workdir: &Path, prune: Option<&[Oid]>) -> Result<AbortSaving> {
    let repo = Repository::open(workdir)?;
    let Some(how) = aborting(repo.state()) else {
        return Err(Error::Unsupported("Only a merge, cherry-pick, revert, rebase or patch series can be aborted keeping its work".into()));
    };
    let op = how.op();
    let (pos, Some(head_oid)) = head(&repo) else { return Err(Error::Unsupported("HEAD has no commit yet".into())) };
    let steps = match how {
        Aborting::Steps { op, dir, verb } => Some(aborted_state::read(&repo, op, dir, verb, &pos, head_oid)?),
        Aborting::Single(_) => None,
    };
    let in_head = tree_entries(&repo.find_commit(head_oid)?.tree()?)?;
    let target = match &steps {
        Some(s) => Some(tree_entries(&repo.find_commit(s.target)?.tree()?)?),
        None => None,
    };
    let mut index = repo.index()?;
    index.read(true)?;
    let split = split_index(&index)?;
    let paths = aborted_paths::op_paths(workdir, &split, &in_head, target.as_ref(), steps.as_ref().is_some_and(|s| s.resets_all))?;
    let Split { stage0, flags, conflicts } = split;
    let gitlink = |p: &String| {
        let modes = conflicts.get(p).into_iter().flatten().flatten().chain(stage0.get(p)).chain(in_head.get(p));
        modes.chain(target.as_ref().and_then(|t| t.get(p))).any(|e| e.1 == GITLINK)
    };
    let (work, absent) = aborted_paths::work(workdir, &paths, gitlink)?;

    let mut top = repo.treebuilder(None)?;
    let index_tree = tree_of(&repo, paths.iter().filter_map(|p| stage0.get(p).map(|e| (p, *e))))?;
    top.insert("index", index_tree, 0o040000)?;
    for (i, name) in ["base", "ours", "theirs"].into_iter().enumerate() {
        let t = tree_of(&repo, conflicts.iter().filter_map(|(p, s)| s[i].map(|e| (p, e))))?;
        top.insert(name, t, 0o040000)?;
    }
    top.insert("work", tree_of(&repo, work.into_iter())?, 0o040000)?;
    let mut state = repo.treebuilder(None)?;
    let mut heads = vec![head_oid];
    heads.extend(steps.as_ref().map(|s| s.target));
    for name in STATE_FILES {
        let bytes = match std::fs::read(repo.path().join(name)) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        };
        let ids = || String::from_utf8_lossy(&bytes).lines().filter_map(|l| Oid::from_str(l.trim()).ok()).collect::<Vec<_>>();
        if name == "AUTO_MERGE" {
            // A tree, kept reachable beside the parts.
            if let Some(t) = ids().into_iter().find(|o| repo.find_tree(*o).is_ok()) {
                top.insert("auto-merge", t, 0o040000)?;
            }
        } else if name != "ORIG_HEAD" && name.ends_with("_HEAD") {
            heads.extend(ids());
        }
        state.insert(name, repo.blob(&bytes)?, 0o100644)?;
    }
    top.insert("state", state.write()?, 0o040000)?;
    if let Some(s) = &steps {
        aborted_state::keep(&repo, s, &mut top)?;
        heads.extend(&s.commits);
    }
    let joined = |v: Vec<&str>| v.join("\0");
    top.insert("paths", repo.blob(joined(paths.iter().map(String::as_str).collect()).as_bytes())?, 0o100644)?;
    top.insert("absent", repo.blob(joined(absent).as_bytes())?, 0o100644)?;
    let flags: String = flags.iter().filter(|(p, _)| paths.contains(*p)).map(|(p, (f, x))| format!("{p}\0{f}\0{x}\0")).collect();
    top.insert("flags", repo.blob(flags.as_bytes())?, 0o100644)?;
    let tree = repo.find_tree(top.write()?)?;
    let mut seen = HashSet::new();
    let parents: Vec<git2::Commit> = heads.into_iter().filter(|o| seen.insert(*o)).filter_map(|o| repo.find_commit(o).ok()).collect();
    let sig = Signature::now("Glowfly", "glowfly@localhost")?;
    let message = format!("Glowfly: work kept from aborting a {}\n\nUndo in Glowfly puts it back.\n", op.noun());
    let kept = repo.commit(None, &sig, &sig, &message, &tree, &parents.iter().collect::<Vec<_>>())?;
    if let Some(keep) = prune {
        prune_old(&repo, keep);
    }
    let name = format!("{ABORT_REF_PREFIX}{kept}");
    repo.reference(&name, kept, true, &format!("glowfly: kept the work of an aborted {}", op.noun()))?;

    let left = Changes(Vec::new());
    let Some(steps) = steps else {
        let cmds = path_commands_of(&paths, &in_head, gitlink);
        let action = Action::AbortKept { op, saved: kept, head: head_oid, branch: branch_name(&pos), left, during: None };
        return Ok(AbortSaving { cmds, action, kept: name });
    };
    let during = Some(HeadAt { branch: branch_name(&pos), oid: head_oid });
    let action = Action::AbortKept { op, saved: kept, head: steps.target, branch: steps.after_branch, left, during };
    Ok(AbortSaving { cmds: vec![args(&[steps.verb, "--abort"])], action, kept: name })
}

/// The commands that put only the operation's `paths` back as HEAD (`in_head`) has them and end
/// a merge, single pick or revert: an edit to any other file stays where it is.
fn path_commands_of(paths: &BTreeSet<String>, in_head: &BTreeMap<String, Entry>, gitlink: impl Fn(&String) -> bool) -> Vec<Vec<String>> {
    let all: Vec<String> = paths.iter().cloned().collect();
    let (old, new): (Vec<String>, Vec<String>) = all.iter().cloned().partition(|p| in_head.contains_key(p));
    let new: Vec<String> = new.into_iter().filter(|p| !gitlink(p)).collect();
    let mut cmds = path_commands(&["--literal-pathspecs", "reset", "-q", "--"], &all);
    // A file the operation added is untracked once reset, and is kept: it goes (ignored or not),
    // before HEAD's files come back, which may need its folder. A submodule it added keeps its
    // checkout, as with git's own abort.
    cmds.extend(path_commands(&["--literal-pathspecs", "clean", "-f", "-x", "-q", "--"], &new));
    cmds.extend(path_commands(&["--literal-pathspecs", "checkout", "-q", "--"], &old));
    // Ends the operation; the edits to other files stay.
    cmds.push(args(&["reset", "-q", "--merge"]));
    cmds
}

/// Drops the refs of kept aborts older than `KEEP_DAYS`, except those of the commits in `keep`.
/// A failure only leaves them for later.
fn prune_old(repo: &Repository, keep: &[Oid]) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    let cutoff = now - KEEP_DAYS * 86_400;
    let Ok(refs) = repo.references_glob(&format!("{ABORT_REF_PREFIX}*")) else { return };
    for mut r in refs.flatten() {
        let Ok(c) = r.peel_to_commit() else { continue };
        if c.time().seconds() < cutoff && !keep.contains(&c.id()) {
            let _ = r.delete();
        }
    }
}
