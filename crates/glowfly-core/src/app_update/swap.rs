//! Swapping a portable copy's program in place: the running program is moved aside to
//! `<name>.bak` (Windows lets a running program be renamed on its volume; Unix keeps running
//! the old file), and the new one is written where it was. Doc files are replaced only where
//! they already are. Every target is the program's own path or a fixed name beside it, never
//! a name read from the archive. A failure puts back everything done so far.

use super::archive::{DOC_FILES, Entry, program_name};
use super::{TargetOs, UpdateError};
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
pub fn plan(entries: &[Entry], exe: &Path, os: TargetOs, exists: impl Fn(&Path) -> bool) -> Result<SwapPlan, UpdateError> {
    let program = program_name(os);
    let Some(p) = entries.iter().position(|e| e.name == program) else {
        return Err(UpdateError::NoProgram { program: program.to_owned() });
    };
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
    pub fn rollback(self) -> Result<(), UpdateError> {
        undo(self.done)
    }
}

fn undo(done: Vec<(PathBuf, Option<PathBuf>)>) -> Result<(), UpdateError> {
    let mut failed = Vec::new();
    for (target, backup) in done.into_iter().rev() {
        let back = persist::remove_file(&target).and_then(|()| match &backup {
            Some(b) => std::fs::rename(b, &target),
            None => Ok(()),
        });
        if let Err(e) = back {
            let at = backup.as_deref().map_or_else(|| shown(&target), shown);
            failed.push((at, e.to_string()));
        }
    }
    if failed.is_empty() { Ok(()) } else { Err(UpdateError::RollbackFailed { failed }) }
}

/// Carries out `plan` with `entries`' bytes; on the first failure, puts back what was done
/// and says why.
pub fn apply(plan: &SwapPlan, entries: &[Entry]) -> Result<Applied, UpdateError> {
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
            let backups: Vec<PathBuf> = done.iter().filter_map(|(_, b)| b.clone()).collect();
            let left = match undo(done) {
                Ok(()) => Vec::new(),
                Err(_) => backups.iter().filter(|b| b.symlink_metadata().is_ok()).map(|b| shown(b)).collect(),
            };
            return Err(UpdateError::ReplaceFailed { folder, said: e.to_string(), left });
        }
    }
    let backups = done.iter().filter_map(|(_, b)| b.clone()).collect();
    Ok(Applied { backups, done })
}

#[cfg(test)]
#[path = "swap_tests.rs"]
mod tests;
