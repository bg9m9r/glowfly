//! The update helper's instructions (`plan.json` in the staging folder), written by the
//! running program and read by its copy, so the format never meets another build. Paths are
//! saved through `paths::encode`, so no name is lost.

use crate::paths;
use crate::persist::{self, WriteOptions};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The format this build writes and reads.
pub const FORMAT: u32 = 1;

/// What the helper does once Glowfly has quit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub format: u32,
    /// The version being installed, for the log.
    pub version: String,
    /// The MSI to run, if any.
    pub msi: Option<MsiStep>,
    /// Files to delete (a swapped program's `.bak`); each one's record (`backups::record_path`)
    /// goes once the file is gone.
    pub remove: Vec<PathBuf>,
    /// The program to start afterwards.
    pub relaunch: PathBuf,
    /// The staging folder, removed at the end.
    pub staging: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MsiStep {
    pub file: PathBuf,
    /// msiexec's verbose log.
    pub log: PathBuf,
}

/// The plan as saved: every path encoded.
#[derive(Serialize, Deserialize)]
struct Raw {
    format: u32,
    version: String,
    msi: Option<RawMsi>,
    remove: Vec<String>,
    relaunch: String,
    staging: String,
}

#[derive(Serialize, Deserialize)]
struct RawMsi {
    file: String,
    log: String,
}

impl Plan {
    /// Writes the plan to `path`, readable by this user only.
    pub fn write(&self, path: &Path) -> Result<(), String> {
        let enc = |p: &PathBuf| paths::encode(p);
        let raw = Raw {
            format: self.format,
            version: self.version.clone(),
            msi: self.msi.as_ref().map(|m| RawMsi { file: enc(&m.file), log: enc(&m.log) }),
            remove: self.remove.iter().map(enc).collect(),
            relaunch: enc(&self.relaunch),
            staging: enc(&self.staging),
        };
        let json = serde_json::to_vec_pretty(&raw).map_err(|e| e.to_string())?;
        persist::write_file(path, &json, WriteOptions { private: true, ..WriteOptions::default() })
            .map_err(|e| format!("Couldn't write {}: {e}", crate::native_path::shown(path)))
    }

    /// Reads the plan at `path`: only this format, and only absolute paths.
    pub fn read(path: &Path) -> Result<Plan, String> {
        let shown = crate::native_path::shown(path);
        let bytes = std::fs::read(path).map_err(|e| format!("Couldn't read {shown}: {e}"))?;
        let raw: Raw = serde_json::from_slice(&bytes).map_err(|e| format!("Couldn't read {shown}: {e}"))?;
        if raw.format != FORMAT {
            return Err(format!("{shown} is in format {}, not {FORMAT}.", raw.format));
        }
        let dec = |s: &str| -> Result<PathBuf, String> {
            let p = paths::decode(s);
            if p.is_absolute() { Ok(p) } else { Err(format!("{shown} names a relative path: {s}")) }
        };
        let msi = match raw.msi {
            Some(m) => Some(MsiStep { file: dec(&m.file)?, log: dec(&m.log)? }),
            None => None,
        };
        Ok(Plan {
            format: raw.format,
            version: raw.version,
            msi,
            remove: raw.remove.iter().map(|s| dec(s)).collect::<Result<_, _>>()?,
            relaunch: dec(&raw.relaunch)?,
            staging: dec(&raw.staging)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = crate::paths::temp_dir().join(format!("glowfly-plan-{tag}-{}", std::process::id()));
        let _ = persist::remove_tree(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample(dir: &Path) -> Plan {
        Plan {
            format: FORMAT,
            version: "0.3.0".into(),
            msi: Some(MsiStep { file: dir.join("glowfly v0.3.0.msi"), log: dir.join("install.log") }),
            remove: vec![dir.join("Glöwfly folder").join("glowfly.exe.bak")],
            relaunch: dir.join("Glöwfly folder").join("glowfly.exe"),
            staging: dir.to_owned(),
        }
    }

    #[test]
    fn a_plan_round_trips() {
        let dir = scratch("round");
        let plan = sample(&dir);
        let file = dir.join("plan.json");
        plan.write(&file).unwrap();
        assert_eq!(Plan::read(&file).unwrap(), plan);
        let no_msi = Plan { msi: None, remove: Vec::new(), ..plan };
        no_msi.write(&file).unwrap();
        assert_eq!(Plan::read(&file).unwrap(), no_msi);
        let _ = persist::remove_tree(&dir);
    }

    #[test]
    fn another_format_or_a_relative_path_is_refused() {
        let dir = scratch("refused");
        let file = dir.join("plan.json");
        Plan { format: 2, ..sample(&dir) }.write(&file).unwrap();
        assert!(Plan::read(&file).unwrap_err().contains("format 2"));
        Plan { relaunch: PathBuf::from("glowfly.exe"), ..sample(&dir) }.write(&file).unwrap();
        assert!(Plan::read(&file).unwrap_err().contains("relative path"));
        assert!(Plan::read(&dir.join("missing.json")).unwrap_err().starts_with("Couldn't read "));
        let _ = persist::remove_tree(&dir);
    }
}
