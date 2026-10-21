//! Running a diff or merge tool command from git config (`difftool.<name>.cmd`,
//! `mergetool.<name>.cmd`) as git does: evaluated by POSIX sh (`custom_commands::find_sh`) in
//! the repository's top folder, with the files it works on in `LOCAL`, `REMOTE`, `MERGED` and
//! `BASE`. The command travels in an environment variable, never in argv (see
//! custom_commands/shell.rs).

use super::{ExitReport, Watched, early_exit_message, wait_report, watch};
use crate::custom_commands::{SCRIPT_VAR, ShellMissing, find_sh};
use crate::error::{Error, Result};
use crate::native_path::native;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

/// A tool command to run: the script, its variables and the folder it runs in.
#[derive(Clone, Debug, PartialEq)]
pub struct ShellLaunch {
    pub script: String,
    pub env: Vec<(&'static str, OsString)>,
    pub cwd: PathBuf,
}

/// sh's arguments: evaluate the command (quoted, so its own quoting is kept as written). Its
/// errors stay on stderr, to say why it failed.
pub const TOOL_SH_ARGS: [&str; 2] = ["-c", "eval \"$GLOWFLY_COMMAND\""];

/// The sh process for `l`, with no input or output but stderr.
fn command(l: &ShellLaunch) -> Result<Command> {
    let sh = find_sh().map_err(|e| Error::Other(e.to_string()))?;
    let mut cmd = Command::new(sh);
    cmd.args(TOOL_SH_ARGS)
        .env(SCRIPT_VAR, &l.script)
        .envs(l.env.iter().map(|(k, v)| (*k, v)))
        .current_dir(native(&l.cwd))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    crate::env_path::apply(&mut cmd);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    Ok(cmd)
}

fn start(l: &ShellLaunch) -> Result<Child> {
    command(l)?.spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => Error::Other(ShellMissing.to_string()),
        _ => Error::Io(e),
    })
}

/// Starts `l` and watches it for `EARLY_EXIT`, as `external_tools::spawn` does: a quick
/// failure is the error; after that it is left running.
pub fn spawn_shell(l: &ShellLaunch) -> Result<()> {
    match watch(start(l)?) {
        Watched::Running | Watched::Exited { code: Some(0), .. } => Ok(()),
        Watched::Exited { code, stderr } => Err(Error::Other(early_exit_message(code, &stderr))),
    }
}

/// Runs `l` until it exits, however long that is.
pub fn run_shell_to_exit(l: &ShellLaunch) -> Result<ExitReport> {
    wait_report(start(l)?)
}

/// A diff command's variables: the older (`local`) and newer (`remote`) files, and the
/// repo-relative path compared (`merged`), which git also gives as `BASE`.
pub fn diff_env(local: &Path, remote: &Path, merged: &str) -> Vec<(&'static str, OsString)> {
    vec![
        ("LOCAL", native(local).into_os_string()),
        ("REMOTE", native(remote).into_os_string()),
        ("MERGED", merged.into()),
        ("BASE", merged.into()),
    ]
}

/// A merge command's variables: the ancestor, ours and theirs copies, and the repo-relative
/// working file to save the result to (`merged`).
pub fn merge_env(base: &Path, local: &Path, remote: &Path, merged: &str) -> Vec<(&'static str, OsString)> {
    vec![
        ("BASE", native(base).into_os_string()),
        ("LOCAL", native(local).into_os_string()),
        ("REMOTE", native(remote).into_os_string()),
        ("MERGED", merged.into()),
    ]
}

#[cfg(test)]
#[path = "shell_tool_tests.rs"]
mod tests;
