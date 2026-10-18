//! Undoing stashes. A stash push is undone by popping that stash back (with its index), as long
//! as none of the paths it holds has changed since (staged files a push of selected files left as
//! they were don't count). A pop or apply onto a clean working tree is
//! undone by putting the stash back in the list (when it was dropped) and clearing what it
//! applied, as long as the working tree is exactly as the pop left it: `changes` fingerprints it.
//! A drop is undone by storing the dropped commit back at the top of the list.

use super::planning::{dirty, gone, is_clean, stale};
use super::{Action, Changes, Plan, reset::stash_index};
use crate::error::{Error, Result};
use crate::gitcli::args;
use crate::repo::{CLI_PATH_BYTES, FileEntry, Hazard, status_at};
use git2::{Diff, Index, ObjectType, Oid, Repository, Tree, TreeWalkMode, TreeWalkResult};
use std::collections::BTreeSet;
use std::path::Path;

/// Every path stash `oid` holds: its working-tree and index changes and its untracked files.
fn stash_paths(repo: &Repository, oid: Oid) -> Result<BTreeSet<String>> {
    let stash = repo.find_commit(oid)?;
    let base = stash.parent(0)?.tree()?;
    let mut paths = BTreeSet::new();
    let mut add = |diff: Diff| {
        for d in diff.deltas() {
            for f in [d.old_file(), d.new_file()] {
                if let Some(p) = f.path_bytes() {
                    paths.insert(String::from_utf8_lossy(p).into_owned());
                }
            }
        }
    };
    add(repo.diff_tree_to_tree(Some(&base), Some(&stash.tree()?), None)?);
    if let Ok(index) = stash.parent(1) {
        add(repo.diff_tree_to_tree(Some(&base), Some(&index.tree()?), None)?);
    }
    if let Ok(untracked) = stash.parent(2) {
        untracked.tree()?.walk(TreeWalkMode::PreOrder, |root, entry| {
            if entry.kind() != Some(ObjectType::Tree) {
                paths.insert(format!("{root}{}", String::from_utf8_lossy(entry.name_bytes())));
            }
            TreeWalkResult::Ok
        })?;
    }
    Ok(paths)
}

/// The changed paths now (staged, unstaged and untracked), sorted.
fn status_entries(workdir: &Path) -> Result<Vec<FileEntry>> {
    let (unstaged, staged, _) = status_at(workdir, true)?;
    Ok(unstaged.into_iter().chain(staged).collect())
}

/// The hash of the working file `path` as it is: `None` when absent, the link text's hash for a
/// symbolic link, and zero for a folder (a submodule).
fn hash(workdir: &Path, path: &str) -> Result<Option<Oid>> {
    let f = workdir.join(crate::repo::contained(path)?);
    let meta = match f.symlink_metadata() {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    Ok(Some(if meta.is_file() {
        Oid::hash_file(ObjectType::Blob, &f)?
    } else if meta.file_type().is_symlink() {
        Oid::hash_object(ObjectType::Blob, std::fs::read_link(&f)?.to_string_lossy().as_bytes())?
    } else {
        Oid::zero()
    }))
}

/// The working tree's fingerprint: each changed path with its working file's hash. Refused for
/// a path that isn't UTF-8, which can't be read back reliably.
pub(super) fn changes(workdir: &Path) -> Result<Changes> {
    let mut paths = BTreeSet::new();
    for e in status_entries(workdir)? {
        if matches!(e.hazard, Some(Hazard::NotUtf8(_))) {
            return Err(Error::Unsupported("A changed file's name isn't UTF-8".into()));
        }
        paths.insert(e.path);
    }
    let mut out = Vec::with_capacity(paths.len());
    for p in paths {
        let h = hash(workdir, &p)?;
        out.push((p, h));
    }
    Ok(Changes(out))
}

/// Whether the push left `path` staged just as the stash holds it, so popping the stash with its
/// index changes nothing there. A push of selected files records the whole index but leaves the
/// staged files outside its paths as they were.
fn left_staged(index: &Index, staged: &Tree, work: &Tree, path: &str) -> bool {
    let p = Path::new(path);
    let now = index.get_path(p, 0).map(|e| (e.id, e.mode as i32));
    let held = |t: &Tree| t.get_path(p).ok().map(|e| (e.id(), e.filemode()));
    held(staged) == now && held(work) == now
}

/// `git clean` commands removing the untracked `paths`, as many as keep each command line's paths
/// under `CLI_PATH_BYTES`.
pub(super) fn clean_commands(paths: Vec<String>) -> Vec<Vec<String>> {
    let start = || args(&["--literal-pathspecs", "clean", "-f", "-q", "--"]);
    let (mut out, mut cmd, mut bytes) = (Vec::new(), start(), 0);
    for p in paths {
        if bytes > 0 && bytes + p.len() + 1 > CLI_PATH_BYTES {
            out.push(std::mem::replace(&mut cmd, start()));
            bytes = 0;
        }
        bytes += p.len() + 1;
        cmd.push(p);
    }
    if bytes > 0 {
        out.push(cmd);
    }
    out
}

fn stash_ref(n: usize) -> String {
    format!("stash@{{{n}}}")
}

/// Undoes (or redoes) a stash push.
pub(super) fn plan_push(workdir: &Path, repo: &Repository, action: &Action, head_oid: Option<Oid>, redo: bool) -> Result<Plan> {
    let Action::StashPush { stash, head, replay, .. } = action else { unreachable!("a stash push") };
    if redo {
        let Some(replay) = replay else { return Ok(stale("This stash listed its files in a file, so it can't be redone")) };
        if head_oid != Some(*head) {
            return Ok(stale("HEAD has moved since the stash"));
        }
        let now = status_entries(workdir)?;
        let any = match stash_paths(repo, *stash) {
            Ok(paths) => now.iter().any(|e| paths.contains(&e.path)),
            Err(_) => !now.is_empty(),
        };
        return Ok(if any { Plan::Run(vec![replay.clone()]) } else { stale("No changes to stash") });
    }
    if head_oid != Some(*head) {
        return Ok(stale("HEAD has moved since the stash"));
    }
    let Some(n) = stash_index(workdir, *stash)? else { return Ok(stale("The stash was dropped or applied since")) };
    let paths = stash_paths(repo, *stash)?;
    let commit = repo.find_commit(*stash)?;
    let (work, staged, index) = (commit.tree()?, commit.parent(1)?.tree()?, repo.index()?);
    let changed = |e: &FileEntry| e.status == '?' || !left_staged(&index, &staged, &work, &e.path);
    if status_entries(workdir)?.iter().any(|e| paths.contains(&e.path) && changed(e)) {
        return Ok(dirty(action, redo));
    }
    Ok(Plan::Run(vec![args(&["stash", "pop", "--index", &stash_ref(n)])]))
}

/// Undoes (or redoes) a pop or apply onto a clean working tree.
pub(super) fn plan_apply(workdir: &Path, repo: &Repository, action: &Action, head_oid: Option<Oid>, redo: bool) -> Result<Plan> {
    let Action::StashApply { pop, stash, head, message, left } = action else { unreachable!("a pop or apply") };
    if redo {
        if !is_clean(workdir)? {
            return Ok(dirty(action, redo));
        }
        if head_oid != Some(*head) {
            return Ok(stale(format!("HEAD has moved since the {}", action.describe())));
        }
        let Some(n) = stash_index(workdir, *stash)? else { return Ok(stale("The stash was dropped since")) };
        return Ok(Plan::Run(vec![args(&["stash", if *pop { "pop" } else { "apply" }, &stash_ref(n)])]));
    }
    if head_oid != Some(*head) {
        return Ok(stale(format!("HEAD has moved since the {}", action.describe())));
    }
    if changes(workdir).ok().as_ref() != Some(left) {
        return Ok(stale(format!("The working tree has changed since the {}", action.describe())));
    }
    let mut cmds = Vec::new();
    if stash_index(workdir, *stash)?.is_none() {
        if repo.find_commit(*stash).is_err() {
            return Ok(gone());
        }
        cmds.push(args(&["stash", "store", "-m", message, &stash.to_string()]));
    }
    cmds.push(args(&["reset", "--hard", "-q"]));
    let untracked: Vec<String> = status_entries(workdir)?.into_iter().filter(|e| e.status == '?').map(|e| e.path).collect();
    cmds.extend(clean_commands(untracked));
    Ok(Plan::Run(cmds))
}

/// Undoes a stash drop by storing the commit again under its list message (redo: drops it again,
/// wherever it is in the list now).
pub(super) fn plan_drop(workdir: &Path, repo: &Repository, action: &Action, redo: bool) -> Result<Plan> {
    let Action::StashDrop { stash, message } = action else { unreachable!("a stash drop") };
    let listed = stash_index(workdir, *stash)?;
    if redo {
        return Ok(match listed {
            Some(n) => Plan::Run(vec![args(&["stash", "drop", &stash_ref(n)])]),
            None => stale("The stash was dropped or applied since"),
        });
    }
    Ok(if listed.is_some() {
        stale("The stash is already in the list")
    } else if repo.find_commit(*stash).is_err() {
        gone()
    } else {
        Plan::Run(vec![args(&["stash", "store", "-m", message, &stash.to_string()])])
    })
}
