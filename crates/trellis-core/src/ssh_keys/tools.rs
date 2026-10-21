//! Finding ssh's own programs (`ssh-keygen`, `ssh-add`): on PATH, in Git for Windows' bundled
//! copy, or in Windows' OpenSSH.

use crate::gitcli::{self, args};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Where to look for the program `name`, in order: each PATH entry, Git for Windows' bundled
/// copy (three folders above git's exec path, in usr/bin), then Windows' own OpenSSH.
pub fn tool_candidates(
    name: &str,
    path_var: Option<&OsStr>,
    exe_suffix: &str,
    git_exec_path: Option<&Path>,
    system_root: Option<&Path>,
) -> Vec<PathBuf> {
    let file = format!("{name}{exe_suffix}");
    let mut out: Vec<PathBuf> = path_var
        .map(|p| std::env::split_paths(p).filter(|d| !d.as_os_str().is_empty()).map(|d| d.join(&file)).collect())
        .unwrap_or_default();
    if let Some(git_root) = git_exec_path.and_then(|p| p.ancestors().nth(3)) {
        out.push(git_root.join("usr").join("bin").join(format!("{name}.exe")));
    }
    if let Some(root) = system_root {
        out.push(root.join("System32").join("OpenSSH").join(format!("{name}.exe")));
    }
    out
}

/// [`tool_candidates`] for ssh-keygen.
pub fn keygen_candidates(
    path_var: Option<&OsStr>,
    exe_suffix: &str,
    git_exec_path: Option<&Path>,
    system_root: Option<&Path>,
) -> Vec<PathBuf> {
    tool_candidates("ssh-keygen", path_var, exe_suffix, git_exec_path, system_root)
}

/// Git's exec path and the Windows folder, which say where Git for Windows' and Windows' own
/// copies are. Both None elsewhere.
pub(super) fn windows_roots(cwd: &Path) -> (Option<PathBuf>, Option<PathBuf>) {
    if !cfg!(windows) {
        return (None, None);
    }
    let out = gitcli::read(cwd, &args(&["--exec-path"]));
    let exec = (out.ok && !out.stdout.trim().is_empty()).then(|| PathBuf::from(out.stdout.trim()));
    (exec, std::env::var_os("SystemRoot").map(PathBuf::from))
}

/// The first of `candidates` that exists.
fn first_present(candidates: Vec<PathBuf>) -> Option<PathBuf> {
    candidates.into_iter().find(|p| crate::external_tools::locate_here(p).is_some())
}

/// The program `name` to run, if one is installed.
fn find_tool(name: &str, cwd: &Path) -> Option<PathBuf> {
    let path_var = crate::env_path::path();
    let (exec_path, system_root) = windows_roots(cwd);
    first_present(tool_candidates(name, path_var.as_deref(), std::env::consts::EXE_SUFFIX, exec_path.as_deref(), system_root.as_deref()))
}

/// The ssh-keygen program to run, if one is installed.
pub fn find_keygen(cwd: &Path) -> Option<PathBuf> {
    find_tool("ssh-keygen", cwd)
}

/// The ssh-add program to run, if one is installed. On Windows, Windows' own copy comes first:
/// it is the one that talks to the OpenSSH Authentication Agent service.
pub fn find_ssh_add(cwd: &Path) -> Option<PathBuf> {
    let path_var = crate::env_path::path();
    let (exec_path, system_root) = windows_roots(cwd);
    first_present(super::ssh_add_candidates_for(
        cfg!(windows),
        path_var.as_deref(),
        std::env::consts::EXE_SUFFIX,
        exec_path.as_deref(),
        system_root.as_deref(),
    ))
}
