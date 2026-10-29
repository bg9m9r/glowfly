//! Files a `reset --keep`, `reset --hard` or checkout to another commit would write over without
//! a word: git keeps no copy of an ignored file, and `reset --hard` replaces untracked ones too.
//! Every undo or redo that moves HEAD that way first lists the paths the target commit tracks and
//! HEAD doesn't; when any of them (or a folder on the way to one) is on disk, it is refused
//! naming them, and the entry stays for when they have been moved.

use super::{Action, Plan, Refusal};
use crate::error::Result;
use git2::{Delta, Oid, Repository, Tree};
use std::path::Path;

/// How many paths a refusal names before "and N more".
const NAMED: usize = 3;

/// The paths `target` tracks that HEAD doesn't, which something on disk is in the way of
/// (an untracked or ignored file or folder there, or a file where one of its folders goes).
pub(super) fn in_the_way(workdir: &Path, repo: &Repository, target: Oid) -> Result<Vec<String>> {
    let to = repo.find_commit(target)?.tree()?;
    let head = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
    let diff = repo.diff_tree_to_tree(head.as_ref(), Some(&to), None)?;
    let tracked = |p: &str| head.as_ref().is_some_and(|t: &Tree| t.get_path(Path::new(p)).is_ok());
    let mut out = Vec::new();
    for d in diff.deltas() {
        if d.status() != Delta::Added {
            continue;
        }
        let Some(path) = d.new_file().path_bytes().map(|p| String::from_utf8_lossy(p).into_owned()) else { continue };
        let on_disk = |p: &str| workdir.join(p).symlink_metadata().ok();
        let blocked = on_disk(&path).is_some()
            || path.match_indices('/').any(|(i, _)| {
                let folder = &path[..i];
                on_disk(folder).is_some_and(|m| !m.is_dir()) && !tracked(folder)
            });
        if blocked {
            out.push(path);
        }
    }
    Ok(out)
}

/// The refusal for `paths` in the way of undoing (or redoing) `action`: `Unsafe`, since stashing
/// can't move ignored files out of the way, so no stash is offered.
pub(super) fn refusal(paths: &[String], action: &Action, redo: bool) -> Plan {
    let verb = if redo { "redoing" } else { "undoing" };
    let mut names = paths.iter().take(NAMED).cloned().collect::<Vec<_>>().join(", ");
    if paths.len() > NAMED {
        names.push_str(&format!(" and {} more", paths.len() - NAMED));
    }
    Plan::Refuse(Refusal::Unsafe(format!(
        "Move {names} away first: {verb} {} would write over {}, which git doesn't keep a copy of.",
        action.describe(),
        if paths.len() == 1 { "it" } else { "them" }
    )))
}

/// `refusal` when something is in the way of moving to `target`.
pub(super) fn check(workdir: &Path, repo: &Repository, target: Oid, action: &Action, redo: bool) -> Result<Option<Plan>> {
    let paths = in_the_way(workdir, repo, target)?;
    Ok((!paths.is_empty()).then(|| refusal(&paths, action, redo)))
}
