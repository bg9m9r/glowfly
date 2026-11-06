//! Which paths an abort that keeps its work (`aborted.rs`) touches, and what is on disk at each:
//! every path where the index differs from the stopped HEAD (conflicts and a staged rename's old
//! path too); for a run of steps also every path the abort's target has otherwise; and, for a
//! rebase, whose abort resets the whole tree, every tracked file with unstaged changes. A folder
//! at one of those paths is refused unless every file in it is one of the paths too, and so is a
//! file where the abort needs a folder, a symbolic link or anything else that isn't a regular
//! file: nothing there could come back.

use super::aborted::{Entry, Split, not_utf8};
use super::saved;
use crate::error::{Error, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

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

/// The paths the abort touches (see the module doc): `split` is the index, `in_head` the
/// stopped HEAD's files, `target` the files of the commit a run of steps' abort goes back to.
/// For a long rebase the stopped commit and the target can differ in thousands of paths; saving
/// them re-hashes files whose blobs mostly exist already, which is acceptable.
pub(super) fn op_paths(
    workdir: &Path,
    split: &Split,
    in_head: &BTreeMap<String, Entry>,
    target: Option<&BTreeMap<String, Entry>>,
    resets_all: bool,
) -> Result<BTreeSet<String>> {
    let Split { stage0, conflicts, .. } = split;
    let mut paths: BTreeSet<String> = conflicts.keys().cloned().collect();
    paths.extend(stage0.iter().filter(|(p, e)| in_head.get(*p) != Some(e)).map(|(p, _)| p.clone()));
    paths.extend(in_head.keys().filter(|p| !stage0.contains_key(*p) && !conflicts.contains_key(*p)).cloned());
    if let Some(target) = target {
        paths.extend(in_head.iter().filter(|(p, e)| target.get(*p) != Some(e)).map(|(p, _)| p.clone()));
        paths.extend(target.iter().filter(|(p, e)| in_head.get(*p) != Some(e)).map(|(p, _)| p.clone()));
    }
    if resets_all {
        // Conflicts ('!') are paths already; untracked files ('?') stay where they are.
        let (unstaged, _, _) = crate::repo::status_at(workdir, true)?;
        for f in unstaged.into_iter().filter(|f| !matches!(f.status, '?' | '!')) {
            paths.extend(f.renamed_from);
            paths.insert(f.path);
        }
    }
    Ok(paths)
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

/// Whatever is on disk at each of `paths` but submodules (`gitlink`), whether status shows it or
/// not (ignored, or hidden by a clean filter): the saved files' blobs and modes, and the paths
/// where nothing (or a folder of the operation's own files) is.
pub(super) fn work<'a>(
    workdir: &Path,
    paths: &'a BTreeSet<String>,
    gitlink: impl Fn(&String) -> bool,
) -> Result<(BTreeMap<&'a String, Entry>, Vec<&'a str>)> {
    let (mut work, mut absent) = (BTreeMap::new(), Vec::new());
    for p in paths.iter().filter(|p| !gitlink(p)) {
        match on_disk(workdir, p)? {
            OnDisk::Nothing => {
                check_ancestors(workdir, p, paths)?;
                absent.push(p.as_str());
            }
            // Its files are the operation's paths, each kept in its own turn.
            OnDisk::Folder => {
                check_folder(workdir, p, paths)?;
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
    Ok((work, absent))
}
