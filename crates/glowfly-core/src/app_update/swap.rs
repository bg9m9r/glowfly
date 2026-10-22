//! Swapping a portable copy's program in place: the running program is moved aside to
//! `<name>.bak` (Windows lets a running program be renamed on its volume; Unix keeps running
//! the old file), and the new one is written where it was. Doc files are replaced only where
//! they already are. Every target is the program's own path or a fixed name beside it, never
//! a name read from the archive. A failure puts back everything done so far.

use super::TargetOs;
use super::archive::{DOC_FILES, Entry, program_name};
use crate::native_path::shown;
use crate::persist::{self, WriteOptions};
use std::path::{Path, PathBuf};

/// What to write where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwapPlan {
    /// The program first.
    pub items: Vec<SwapItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwapItem {
    /// Index into the archive's entries.
    pub entry: usize,
    pub target: PathBuf,
    pub program: bool,
}

/// The swap of `entries` into the copy whose program is `exe` (the running program's resolved
/// path, whatever it is called) on `os`. A doc file is planned only when `exists` finds it
/// beside the program, so a program kept in a shared folder doesn't fill it with READMEs.
pub fn plan(entries: &[Entry], exe: &Path, os: TargetOs, exists: impl Fn(&Path) -> bool) -> Result<SwapPlan, String> {
    let program = program_name(os);
    let Some(p) = entries.iter().position(|e| e.name == program) else { return Err(format!("The archive has no {program}.")) };
    let mut items = vec![SwapItem { entry: p, target: exe.to_owned(), program: true }];
    let dir = exe.parent().unwrap_or(Path::new(""));
    for doc in DOC_FILES {
        let Some(i) = entries.iter().position(|e| e.name == doc) else { continue };
        // From the fixed name, not the entry's.
        let target = dir.join(doc);
        if exists(&target) {
            items.push(SwapItem { entry: i, target, program: false });
        }
    }
    Ok(SwapPlan { items })
}

/// A swap that went through: the old files, kept until Glowfly has quit.
#[derive(Debug)]
pub struct Applied {
    /// Where the old files were moved.
    pub backups: Vec<PathBuf>,
    /// Each target written, with its backup (None when there was no old file), in order.
    done: Vec<(PathBuf, Option<PathBuf>)>,
}

impl Applied {
    /// Puts the old files back: each new file is removed and its backup renamed back, last
    /// first.
    pub fn rollback(self) -> Result<(), String> {
        undo(self.done)
    }
}

fn undo(done: Vec<(PathBuf, Option<PathBuf>)>) -> Result<(), String> {
    let mut failed = Vec::new();
    for (target, backup) in done.into_iter().rev() {
        let back = persist::remove_file(&target).and_then(|()| match &backup {
            Some(b) => std::fs::rename(b, &target),
            None => Ok(()),
        });
        if let Err(e) = back {
            let at = backup.as_deref().map_or_else(|| shown(&target), shown);
            failed.push(format!("{at}: {e}"));
        }
    }
    if failed.is_empty() { Ok(()) } else { Err(failed.join("; ")) }
}

/// Carries out `plan` with `entries`' bytes; on the first failure, puts back what was done
/// and says why.
pub fn apply(plan: &SwapPlan, entries: &[Entry]) -> Result<Applied, String> {
    let mut done: Vec<(PathBuf, Option<PathBuf>)> = Vec::new();
    for item in &plan.items {
        let Some(entry) = entries.get(item.entry) else { continue };
        let target = &item.target;
        let step = (|| {
            let backup = if target.symlink_metadata().is_ok() { Some(persist::move_aside(target)?) } else { None };
            // Recorded before the write, so a failed write puts the backup back too.
            done.push((target.clone(), backup));
            let opts = WriteOptions { executable: item.program.then_some(true), ..WriteOptions::default() };
            persist::write_file(target, &entry.bytes, opts)
        })();
        if let Err(e) = step {
            let folder = shown(target.parent().unwrap_or(target));
            let mut text = format!("Couldn't replace Glowfly's program in {folder}: {e}. Use Show file to install the update by hand.");
            let backups: Vec<PathBuf> = done.iter().filter_map(|(_, b)| b.clone()).collect();
            if undo(done).is_err() {
                for b in backups.iter().filter(|b| b.symlink_metadata().is_ok()) {
                    text.push_str(&format!(" The old file is at {}.", shown(b)));
                }
            }
            return Err(text);
        }
    }
    let backups = done.iter().filter_map(|(_, b)| b.clone()).collect();
    Ok(Applied { backups, done })
}

#[cfg(test)]
#[path = "swap_tests.rs"]
mod tests;
