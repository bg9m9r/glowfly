//! What a stopped clone leaves behind. git removes the folder it was cloning into when it gets
//! SIGTERM, but a hard kill (SIGKILL, or terminating its job on Windows) skips that and leaves a
//! half-made repository. Before the clone runs, [`note`] records whether its folder existed and
//! what was in it; after the stop, once git and everything it started have exited, [`clean`]
//! removes the folder only when the clone made it, and otherwise (an empty folder the user
//! picked) only the entries that appeared since, keeping the folder itself. A path that isn't a
//! plain folder by then (a link, a file) is never touched.

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The folder a `git clone` runs into, as it was before the clone started.
#[derive(Debug)]
pub struct Target {
    dest: PathBuf,
    /// None: nothing was there. Else the names in the folder that was.
    before: Option<HashSet<OsString>>,
}

/// What [`clean`] did.
#[derive(Debug, PartialEq)]
pub enum Cleaned {
    /// Nothing was left (git removed it, or it never started).
    Nothing,
    /// The folder the clone made was removed.
    Folder(PathBuf),
    /// The folder existed before and is kept; this many entries the clone made were removed.
    Entries(PathBuf, usize),
    /// Something couldn't be removed: what, and why.
    Failed(PathBuf, String),
}

/// The folder the clone `cmd` (its arguments, without `git`) run in `cwd` writes to, noted as it
/// is now. Only a clone that names its folder after `--` (`clone [options] -- <url> <folder>`, as
/// Glowfly runs it) has one; any other command has none.
pub fn note(cwd: &Path, cmd: &[String]) -> Option<Target> {
    if crate::gitcli::subcommand(cmd) != Some("clone") {
        return None;
    }
    let after = cmd.iter().position(|a| a == "--")?;
    let [_, folder] = &cmd[after + 1..] else { return None };
    let dest = cwd.join(folder);
    let meta = std::fs::symlink_metadata(&dest);
    let before = match meta {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Ok(m) if m.is_dir() => Some(std::fs::read_dir(&dest).ok()?.flatten().map(|e| e.file_name()).collect()),
        // A link, a file or an unreadable path: git refuses it, and nothing here is removed.
        _ => return None,
    };
    Some(Target { dest, before })
}

/// Removes what the stopped clone left in `t`'s folder. Call it only after git and the processes
/// it started have exited.
pub fn clean(t: &Target) -> Cleaned {
    let now_dir = std::fs::symlink_metadata(&t.dest).is_ok_and(|m| m.is_dir());
    if !now_dir {
        return Cleaned::Nothing;
    }
    match &t.before {
        None => match retried(|| crate::persist::remove_tree(&t.dest)) {
            Ok(()) => Cleaned::Folder(t.dest.clone()),
            Err(e) => Cleaned::Failed(t.dest.clone(), e.to_string()),
        },
        Some(before) => {
            let Ok(read) = std::fs::read_dir(&t.dest) else { return Cleaned::Nothing };
            let new: Vec<PathBuf> = read.flatten().filter(|e| !before.contains(&e.file_name())).map(|e| e.path()).collect();
            let mut removed = 0;
            for p in &new {
                if let Err(e) = retried(|| crate::persist::remove_tree(p)) {
                    return Cleaned::Failed(p.clone(), e.to_string());
                }
                removed += 1;
            }
            if removed == 0 { Cleaned::Nothing } else { Cleaned::Entries(t.dest.clone(), removed) }
        }
    }
}

/// `f` again a few times while it fails: a virus scanner or indexer may still hold a file the
/// clone just wrote (Windows refuses to delete an open file).
fn retried(f: impl Fn() -> std::io::Result<()>) -> std::io::Result<()> {
    let mut last = f();
    for _ in 0..4 {
        if last.is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
        last = f();
    }
    last
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gitcli::args;

    fn parent(name: &str) -> PathBuf {
        let dir = crate::paths::temp_dir().join(format!("glowfly-clone-stop-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// What a hard-killed clone leaves: a git dir and part of a checkout.
    fn half_clone(dest: &Path) {
        std::fs::create_dir_all(dest.join(".git/objects/pack")).unwrap();
        std::fs::write(dest.join(".git/objects/pack/tmp_pack_1"), "x").unwrap();
        std::fs::write(dest.join("README"), "x").unwrap();
    }

    #[test]
    fn a_folder_the_clone_made_is_removed() {
        let p = parent("made");
        let t = note(&p, &args(&["clone", "--depth=1", "--", "https://example.com/r.git", "r"])).unwrap();
        half_clone(&p.join("r"));
        assert_eq!(clean(&t), Cleaned::Folder(p.join("r")));
        assert!(!p.join("r").exists() && p.exists());
        let _ = std::fs::remove_dir_all(&p);
    }

    #[test]
    fn an_empty_folder_that_was_there_is_kept_and_emptied() {
        let p = parent("empty");
        std::fs::create_dir(p.join("r")).unwrap();
        let t = note(&p, &args(&["clone", "--", "https://example.com/r.git", "r"])).unwrap();
        half_clone(&p.join("r"));
        assert_eq!(clean(&t), Cleaned::Entries(p.join("r"), 2));
        assert!(p.join("r").is_dir(), "the folder the user picked stays");
        assert_eq!(std::fs::read_dir(p.join("r")).unwrap().count(), 0);
        // Nothing left to remove the second time.
        assert_eq!(clean(&t), Cleaned::Nothing);
        let _ = std::fs::remove_dir_all(&p);
    }

    #[test]
    fn entries_that_were_there_before_are_kept() {
        let p = parent("kept");
        std::fs::create_dir(p.join("r")).unwrap();
        std::fs::write(p.join("r/mine.txt"), "mine").unwrap();
        let t = note(&p, &args(&["clone", "--", "u", "r"])).unwrap();
        half_clone(&p.join("r"));
        assert_eq!(clean(&t), Cleaned::Entries(p.join("r"), 2));
        assert!(p.join("r/mine.txt").is_file());
        let _ = std::fs::remove_dir_all(&p);
    }

    #[test]
    fn nothing_is_removed_when_git_already_did_or_it_isnt_a_folder() {
        let p = parent("gone");
        let t = note(&p, &args(&["clone", "--", "u", "r"])).unwrap();
        assert_eq!(clean(&t), Cleaned::Nothing);
        std::fs::write(p.join("r"), "a file now").unwrap();
        assert_eq!(clean(&t), Cleaned::Nothing);
        assert!(p.join("r").is_file());
        // A file there before: git refuses it, and there is nothing to note.
        assert!(note(&p, &args(&["clone", "--", "u", "r"])).is_none());
        let _ = std::fs::remove_dir_all(&p);
    }

    #[test]
    fn only_a_clone_naming_its_folder_after_the_separator_is_noted() {
        let p = parent("args");
        assert!(note(&p, &args(&["fetch", "--", "u", "r"])).is_none());
        assert!(note(&p, &args(&["clone", "u", "r"])).is_none(), "no separator");
        assert!(note(&p, &args(&["clone", "--", "u"])).is_none(), "folder named by git");
        assert!(note(&p, &args(&["-c", "a=b", "clone", "--", "u", "r"])).is_some());
        let _ = std::fs::remove_dir_all(&p);
    }
}
