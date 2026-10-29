//! Install and restart, on Glowfly's side: the downloaded file is checked again, a private
//! staging folder gets a copy of this program as the helper (in the first place it runs from:
//! the temp folder, else beside the program or in the cache folder), and then either the MSI's
//! copy is staged or the portable program (or the AppImage file) is swapped in place. Only once
//! all of that worked is the helper started and told to go; any failure first undoes what was
//! done, so a helper that never hears "go" has nothing to undo.

use super::appimage;
use super::archive;
use super::finish::GO;
use super::plan::{FORMAT, MsiStep, Plan};
use super::staging;
use super::swap::{self, Applied};
use super::{InstallMethod, TargetOs, read_verified};
use crate::native_path::shown;
use crate::persist::{self, WriteOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// What to install and where this copy is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RestartRequest {
    /// The downloaded file.
    pub file: PathBuf,
    /// The SHA-256 it matched when it was downloaded.
    pub sha256: [u8; 32],
    pub method: InstallMethod,
    pub version: String,
    /// The running program, resolved: copied as the helper.
    pub exe: PathBuf,
    /// The file Install replaces and starts again: `exe` for an MSI or portable copy, the
    /// AppImage file for an AppImage copy.
    pub target: PathBuf,
    /// Where the staging folder may go, in order: `paths::temp_dir()`, then fallbacks for a
    /// temp folder the helper can't run from (`staging::create_runnable`).
    pub places: Vec<PathBuf>,
    pub os: TargetOs,
}

/// An update ready for the helper.
#[derive(Debug)]
pub struct Prepared {
    pub plan_path: PathBuf,
    pub helper: PathBuf,
    pub staging: PathBuf,
    applied: Option<Applied>,
    /// The backups' records (backups.rs), removed when the preparation is undone.
    records: Vec<PathBuf>,
}

impl Prepared {
    /// Undoes the preparation: the old program is put back and the staging folder removed.
    pub fn abandon(self) -> Result<(), String> {
        for r in &self.records {
            let _ = persist::remove_file(r);
        }
        let back = self.applied.map_or(Ok(()), Applied::rollback);
        let removed = persist::remove_tree(&self.staging).map_err(|e| format!("Couldn't remove {}: {e}", shown(&self.staging)));
        back.and(removed)
    }
}

/// The helper's name in the staging folder.
fn helper_name(os: TargetOs) -> &'static str {
    if os == TargetOs::Windows { "glowfly-update.exe" } else { "glowfly-update" }
}

/// Prepares the update without starting anything.
pub fn prepare(req: &RestartRequest) -> Result<Prepared, String> {
    prepare_with(req, &super::launch::try_run)
}

/// [`prepare`], with `runs` trying whether the copied helper can run where it was put.
pub fn prepare_with(req: &RestartRequest, runs: &dyn Fn(&Path) -> std::io::Result<()>) -> Result<Prepared, String> {
    let bytes = read_verified(&req.file, &req.sha256)?;
    if !req.method.restarts() {
        return Err("This update can't be installed in place.".to_owned());
    }
    let staged = staging::create_runnable(&req.places, &req.exe, helper_name(req.os), runs)?;
    let staging = staged.dir;
    let mut prepared =
        Prepared { plan_path: staging.join("plan.json"), helper: staged.helper, staging, applied: None, records: Vec::new() };
    match fill(req, &bytes, &mut prepared) {
        Ok(()) => Ok(prepared),
        // What couldn't be undone is said too.
        Err(e) => Err(match prepared.abandon() {
            Ok(()) => e,
            Err(undo) => format!("{e} {undo}"),
        }),
    }
}

fn fill(req: &RestartRequest, bytes: &[u8], prepared: &mut Prepared) -> Result<(), String> {
    // The helper is a copy (staging::create_runnable); for an MSI it has to be anyway, since
    // msiexec replaces the installed program.
    let staging = &prepared.staging;
    let mut msi = None;
    let mut remove = Vec::new();
    match req.method {
        InstallMethod::RunMsi => {
            let name = req.file.file_name().map_or_else(|| "glowfly.msi".into(), |n| n.to_owned());
            let file = staging.join(name);
            persist::write_file(&file, bytes, WriteOptions { private: true, ..WriteOptions::default() })
                .map_err(|e| format!("Couldn't write {}: {e}", shown(&file)))?;
            msi = Some(MsiStep { file, log: staging.join("install.log") });
        }
        InstallMethod::SwapPortable => {
            let file_name = req.file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let entries = archive::read(&file_name, bytes, archive::wanted(req.os))?;
            let plan = swap::plan(&entries, &req.target, req.os, Path::exists)?;
            let applied = swap::apply(&plan, &entries)?;
            remove = recorded(&applied.backups, &mut prepared.records);
            prepared.applied = Some(applied);
        }
        InstallMethod::SwapAppImage => {
            if !appimage::is_appimage(bytes) {
                return Err("The download isn't an AppImage.".to_owned());
            }
            // The image is the whole program; no doc files sit beside it.
            let entries = vec![archive::Entry { name: archive::program_name(TargetOs::Linux).to_owned(), bytes: bytes.to_vec() }];
            let plan = swap::plan(&entries, &req.target, req.os, |_| false)?;
            let applied = swap::apply(&plan, &entries)?;
            remove = recorded(&applied.backups, &mut prepared.records);
            prepared.applied = Some(applied);
        }
        InstallMethod::OpenAndQuit | InstallMethod::ShowOnly => return Err("This update can't be installed in place.".to_owned()),
    }
    let plan = Plan { format: FORMAT, version: req.version.clone(), msi, remove, relaunch: req.target.clone(), staging: staging.clone() };
    plan.write(&prepared.plan_path)
}

/// Records each backup (backups.rs), which lets the next start remove one the helper couldn't
/// delete, and returns the files the helper deletes: the backups, each with its record once it
/// is gone. A backup whose record couldn't be written is only left for the user.
fn recorded(backups: &[PathBuf], records: &mut Vec<PathBuf>) -> Vec<PathBuf> {
    for b in backups {
        if let Ok(r) = super::backups::record(b) {
            records.push(r);
        }
    }
    backups.to_vec()
}

/// Prepares the update, starts the helper and tells it to go. On success Glowfly should quit:
/// the helper waits for that.
pub fn start(req: &RestartRequest) -> Result<(), String> {
    let prepared = prepare(req)?;
    let cwd = req.target.parent().unwrap_or(&prepared.staging);
    let mut child = match super::launch::start_helper(&prepared.helper, &prepared.plan_path, cwd) {
        Ok(c) => c,
        Err(e) => {
            let _ = prepared.abandon();
            return Err(format!("Couldn't start the update helper: {e}"));
        }
    };
    let Some(mut stdin) = child.stdin.take() else {
        let _ = prepared.abandon();
        return Err("Couldn't start the update helper: it has no input".to_owned());
    };
    if let Err(e) = stdin.write_all(GO).and_then(|()| stdin.flush()) {
        // The helper sees the input end without "go" and does nothing.
        drop(stdin);
        let _ = prepared.abandon();
        return Err(format!("Couldn't start the update helper: {e}"));
    }
    // Never closed: the pipe stays open until Glowfly exits, and its end is the helper's signal
    // that Glowfly has quit.
    std::mem::forget(stdin);
    // The helper runs on by itself; it isn't waited for.
    drop(child);
    Ok(())
}

#[cfg(test)]
#[path = "restart_tests.rs"]
mod tests;
