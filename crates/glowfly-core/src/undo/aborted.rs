//! Aborting a merge, single cherry-pick or single revert while keeping its work for Undo, also
//! with conflicts left and with edits unrelated to the operation in the working tree.
//!
//! `abort_saving` first writes everything the abort throws away to the object database, as one
//! commit kept reachable by `refs/glowfly/aborted/<id>`: the index entries of the operation's
//! paths (`index`, the conflict stages `base`, `ours` and `theirs`: git can't stash unmerged
//! entries, and `flags`, the entries' intent-to-add, skip-worktree and assume-unchanged flags),
//! the exact bytes of whatever is on disk at each of those paths (`work`, ignored files too, with
//! `absent` naming the paths where nothing is), every path it touches (`paths`) and the files
//! that say an operation is in progress (`state`: MERGE_HEAD, MERGE_MSG and the like). Its
//! parents are HEAD and the commits being merged or picked, so they stay reachable too. Then it
//! returns the commands that put only those paths back as HEAD has them and end the operation:
//! an edit to any other file stays where it is. The operation's paths are every path where the
//! index differs from HEAD, so a staged rename's old path is one of them. A folder at one of
//! those paths is refused unless every file in it is one of the operation's paths too, and so is
//! a symbolic link: nothing there could come back.
//!
//! `aborted_run.rs` runs the commands (putting everything back when one fails), `aborted_restore.rs`
//! undoes the abort. A kept abort's ref that is never undone is dropped `KEEP_DAYS` later, at the
//! next one, unless an undo entry still names it.

use super::record::{branch_name, head};
use super::{Action, Changes, MoveOp, saved};
use crate::error::{Error, Result};
use crate::gitcli::args;
use crate::repo::CLI_PATH_BYTES;
use git2::{Index, IndexEntry, IndexTime, ObjectType, Oid, Repository, RepositoryState, Signature, Tree, TreeWalkMode, TreeWalkResult};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Where the kept work of each abort is referenced, by the kept commit's id.
pub const ABORT_REF_PREFIX: &str = "refs/glowfly/aborted/";

/// How long the kept work of an abort that is never undone stays referenced.
const KEEP_DAYS: i64 = 30;

/// The files in the git folder that say an operation is in progress, and ORIG_HEAD, which the
/// abort's reset moves (those present are kept).
pub(super) const STATE_FILES: [&str; 6] = ["MERGE_HEAD", "MERGE_MSG", "MERGE_MODE", "CHERRY_PICK_HEAD", "REVERT_HEAD", "ORIG_HEAD"];

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

/// The operation `state` is, when an abort can keep its work.
pub(super) fn op_of(state: RepositoryState) -> Option<MoveOp> {
    match state {
        RepositoryState::Merge => Some(MoveOp::Merge),
        RepositoryState::CherryPick => Some(MoveOp::CherryPick),
        RepositoryState::Revert => Some(MoveOp::Revert),
        _ => None,
    }
}

fn not_utf8() -> Error {
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
struct Split {
    stage0: BTreeMap<String, Entry>,
    flags: BTreeMap<String, (u16, u16)>,
    conflicts: BTreeMap<String, [Option<Entry>; 3]>,
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

/// What is on disk at a path.
pub(super) enum OnDisk {
    Nothing,
    Folder,
    /// A file, a symbolic link or anything else that isn't a folder.
    Item,
}

/// What is at `path`; nothing also when a folder on the way is a file.
pub(super) fn on_disk(workdir: &Path, path: &str) -> Result<OnDisk> {
    match workdir.join(crate::repo::contained(path)?).symlink_metadata() {
        Ok(m) if m.is_dir() => Ok(OnDisk::Folder),
        Ok(_) => Ok(OnDisk::Item),
        Err(e) if matches!(e.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory) => Ok(OnDisk::Nothing),
        Err(e) => Err(e.into()),
    }
}

/// Refuses a folder at the operation's path `dir` that holds anything but the operation's own
/// `paths` (ignored files too): the abort would remove it with nothing kept.
fn check_folder(workdir: &Path, dir: &str, paths: &BTreeSet<String>) -> Result<()> {
    for e in std::fs::read_dir(workdir.join(crate::repo::contained(dir)?))? {
        let e = e?;
        let name = e.file_name().into_string().map_err(|_| not_utf8())?;
        let rel = format!("{dir}/{name}");
        if e.file_type()?.is_dir() {
            check_folder(workdir, &rel, paths)?;
        } else if !paths.contains(&rel) {
            return Err(Error::Unsupported(format!("{dir} is a folder holding {rel}, which the abort would remove; move it away first")));
        }
    }
    Ok(())
}

/// Refuses a file (or link) standing where a folder of the operation's path `path` should be,
/// unless it is one of the operation's `paths` itself (kept in its own turn): the abort would
/// remove it to put the folder back, with nothing kept.
fn check_ancestors(workdir: &Path, path: &str, paths: &BTreeSet<String>) -> Result<()> {
    for (i, _) in path.match_indices('/') {
        let dir = &path[..i];
        match on_disk(workdir, dir)? {
            OnDisk::Folder => {}
            OnDisk::Nothing => return Ok(()),
            OnDisk::Item if paths.contains(dir) => return Ok(()),
            OnDisk::Item => {
                return Err(Error::Unsupported(format!("{dir} is a file where the abort needs a folder; move it away first")));
            }
        }
    }
    Ok(())
}

/// Keeps the work of the merge, single cherry-pick or single revert in progress (see the module
/// doc) and returns the commands that then abort it. Refused, with nothing changed, when it
/// can't all be kept: something at one of its paths that isn't a regular file (or a folder of
/// its own files), or a name that isn't UTF-8. Refs of kept aborts older than `KEEP_DAYS` are
/// dropped unless `prune` (the kept commits undo entries still name) lists them; `None` drops
/// none.
pub(super) fn abort_saving(workdir: &Path, prune: Option<&[Oid]>) -> Result<AbortSaving> {
    let repo = Repository::open(workdir)?;
    let Some(op) = op_of(repo.state()) else {
        return Err(Error::Unsupported("Only a merge, a single cherry-pick or a single revert can be aborted keeping its work".into()));
    };
    let (pos, Some(head_oid)) = head(&repo) else { return Err(Error::Unsupported("HEAD has no commit yet".into())) };
    let head_commit = repo.find_commit(head_oid)?;
    let in_head = tree_entries(&head_commit.tree()?)?;
    let mut index = repo.index()?;
    index.read(true)?;
    let Split { stage0, flags, conflicts } = split_index(&index)?;
    let mut paths: BTreeSet<String> = conflicts.keys().cloned().collect();
    paths.extend(stage0.iter().filter(|(p, e)| in_head.get(*p) != Some(e)).map(|(p, _)| p.clone()));
    paths.extend(in_head.keys().filter(|p| !stage0.contains_key(*p) && !conflicts.contains_key(*p)).cloned());
    let gitlink = |p: &String| {
        let modes = conflicts.get(p).into_iter().flatten().flatten().chain(stage0.get(p)).chain(in_head.get(p));
        modes.into_iter().any(|e| e.1 == GITLINK)
    };

    // Whatever is on disk at each path, whether status shows it or not (ignored, or hidden by a
    // clean filter).
    let (mut work, mut absent) = (BTreeMap::new(), Vec::new());
    for p in paths.iter().filter(|p| !gitlink(p)) {
        match on_disk(workdir, p)? {
            OnDisk::Nothing => {
                check_ancestors(workdir, p, &paths)?;
                absent.push(p.as_str());
            }
            // Its files are the operation's paths, each kept in its own turn.
            OnDisk::Folder => {
                check_folder(workdir, p, &paths)?;
                absent.push(p.as_str());
            }
            OnDisk::Item => match saved::save(workdir, p) {
                Ok(Some(s)) => {
                    work.insert(p, (s.blob, if s.executable { 0o100755 } else { 0o100644 }));
                }
                Ok(None) => absent.push(p.as_str()),
                Err(Error::Unsupported(_)) => {
                    return Err(Error::Unsupported(format!("{p} isn't a regular file, so it can't be kept for Undo")));
                }
                Err(e) => return Err(e),
            },
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
        if name != "ORIG_HEAD" && name.ends_with("_HEAD") {
            let text = String::from_utf8_lossy(&bytes);
            heads.extend(text.lines().filter_map(|l| Oid::from_str(l.trim()).ok()).filter_map(|o| repo.find_commit(o).ok()));
        }
        state.insert(name, repo.blob(&bytes)?, 0o100644)?;
    }
    top.insert("state", state.write()?, 0o040000)?;
    let joined = |v: Vec<&str>| v.join("\0");
    top.insert("paths", repo.blob(joined(paths.iter().map(String::as_str).collect()).as_bytes())?, 0o100644)?;
    top.insert("absent", repo.blob(joined(absent).as_bytes())?, 0o100644)?;
    let flags: String = flags.iter().filter(|(p, _)| paths.contains(*p)).map(|(p, (f, x))| format!("{p}\0{f}\0{x}\0")).collect();
    top.insert("flags", repo.blob(flags.as_bytes())?, 0o100644)?;
    let tree = repo.find_tree(top.write()?)?;
    heads.dedup_by_key(|c| c.id());
    let sig = Signature::now("Glowfly", "glowfly@localhost")?;
    let message = format!("Glowfly: work kept from aborting a {}\n\nUndo in Glowfly puts it back.\n", op.noun());
    let parents: Vec<&git2::Commit> = heads.iter().collect();
    let kept = repo.commit(None, &sig, &sig, &message, &tree, &parents)?;
    if let Some(keep) = prune {
        prune_old(&repo, keep);
    }
    let name = format!("{ABORT_REF_PREFIX}{kept}");
    repo.reference(&name, kept, true, &format!("glowfly: kept the work of an aborted {}", op.noun()))?;

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
    let action = Action::AbortKept { op, saved: kept, head: head_oid, branch: branch_name(&pos), left: Changes(Vec::new()) };
    Ok(AbortSaving { cmds, action, kept: name })
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
