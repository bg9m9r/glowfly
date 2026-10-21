//! Stashing a chosen staged rename whole. `git stash push -- <new>` takes only the new path: the
//! old path's deletion stays staged, and the old path can't be named to the stash either, since
//! it is in neither the index nor the working tree (git refuses it as unknown). The stash's
//! recorded index is the whole index, so it already holds the rename; what's left is to put the
//! old path back from HEAD after the stash, which makes the tree whole and lets
//! `stash pop --index` bring the rename back.
//!
//! Nothing is ever forced: the put-back uses `checkout-index` without `--force`, and the job
//! checks `restore_blocked` before it stashes, so a file that sits at an old path stays untouched.

use super::push::PATHS_ON_COMMAND_LINE;
use crate::gitcli::args;
use crate::repo::FileEntry;
use std::collections::HashSet;
use std::path::Path;

/// The start of the job details when `restore_blocked` stops a stash (`restore_blocked_output`),
/// matched to pick the translated headline.
pub const RESTORE_BLOCKED: &str = "Nothing was stashed: something is already at ";

/// The HEAD paths of the staged renames among `paths` (looked up in the `staged` rows), in the
/// order of `paths`, each once, leaving out those already in `paths`. A stash of the new path
/// alone leaves the old path's deletion staged; `restore_commands` then takes the rename whole.
pub fn rename_sources(paths: &[String], staged: &[FileEntry]) -> Vec<String> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut out = Vec::new();
    for p in paths {
        let old = staged.iter().find(|f| &f.path == p && f.status == 'R').and_then(|f| f.renamed_from.as_deref());
        if let Some(old) = old
            && !paths.iter().any(|q| q == old)
            && seen.insert(old)
        {
            out.push(old.to_owned());
        }
    }
    out
}

/// How `restore_commands` writes the old files back: `checkout-index` without `--force`, which
/// never replaces something already on disk at a path (or in the way of its folder).
const PUT_BACK: [&str; 3] = ["checkout-index", "-u", "--"];

/// The commands that run after a stash of chosen files to put the old paths `old` of its staged
/// renames (`rename_sources`) back as HEAD has them: `git reset -q HEAD -- <old>` (the index
/// only, literal pathspecs), then `git checkout-index -u -- <old>` (`PUT_BACK`), in pairs that
/// each name at most `PATHS_ON_COMMAND_LINE` bytes of paths.
pub fn restore_commands(old: &[String]) -> Vec<Vec<String>> {
    let mut chunks: Vec<Vec<String>> = Vec::new();
    let mut bytes = 0;
    for p in old {
        match chunks.last_mut() {
            Some(c) if bytes + p.len() < PATHS_ON_COMMAND_LINE => c.push(p.clone()),
            _ => {
                chunks.push(vec![p.clone()]);
                bytes = 0;
            }
        }
        bytes += p.len() + 1;
    }
    let with = |start: &[&str], paths: &[String]| args(start).into_iter().chain(paths.iter().cloned()).collect::<Vec<_>>();
    chunks.iter().flat_map(|c| [with(&["--literal-pathspecs", "reset", "-q", "HEAD", "--"], c), with(&PUT_BACK, c)]).collect()
}

/// The first path that the put-back commands among `cmds` (`restore_commands`) would write
/// where something is on disk now, so the job (or its redo) can refuse before it stashes.
/// Other commands don't count: only `restore_commands` builds a `checkout-index -u --` line.
pub fn restore_blocked(workdir: &Path, cmds: &[Vec<String>]) -> Option<String> {
    cmds.iter()
        .filter(|c| c.len() > PUT_BACK.len() && c[..PUT_BACK.len()] == PUT_BACK)
        .flat_map(|c| &c[PUT_BACK.len()..])
        .find(|p| occupied(workdir, p))
        .cloned()
}

/// The job details (English, like the other runner texts) when `restore_blocked` found `path`.
pub fn restore_blocked_output(path: &str) -> String {
    format!(
        "{RESTORE_BLOCKED}{path}, where the stash would put back the old file of a staged rename. \
         Move it away, or keep the staged changes, and try again."
    )
}

/// Whether something is on disk at `path`, or a file or link stands where one of its folders
/// would go. The system looks the names up, so a case twin on a volume that ignores case, the
/// other Unicode form where the volume folds those, and an ignored file (status lists none of
/// them under this name) all count. A path that can't be reached by its name counts too.
fn occupied(workdir: &Path, path: &str) -> bool {
    let Ok(rel) = crate::repo::contained(path) else { return true };
    let parts: Vec<&str> = rel.split('/').collect();
    let mut at = workdir.to_path_buf();
    for (i, part) in parts.iter().enumerate() {
        at.push(part);
        match at.symlink_metadata() {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return false,
            Ok(m) if i + 1 < parts.len() && m.is_dir() => {}
            _ => return true,
        }
    }
    false
}
