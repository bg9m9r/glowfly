//! Running a shell command: POSIX sh on every OS (one syntax for a file a team shares, as git's
//! own `!` aliases and hooks): `/bin/sh` on Unix, Git for Windows' `sh.exe` on Windows. The
//! script travels in an environment variable, never in argv: the MSYS runtime behind `sh.exe`
//! re-parses its command line with its own quoting and globbing.

use crate::native_path::native;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Why a shell command can't start on Windows.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
#[error("shell commands need the sh that comes with Git for Windows, and it wasn't found next to git or on PATH")]
pub struct ShellMissing;

/// The variable holding the script.
pub const SCRIPT_VAR: &str = "GLOWFLY_COMMAND";
/// sh's arguments: run the script, its errors with its output.
pub const SH_ARGS: [&str; 2] = ["-c", "eval \"$GLOWFLY_COMMAND\" 2>&1"];
/// How much of a command's output is kept: its end.
pub const MAX_OUTPUT_BYTES: usize = 512 * 1024;

/// The sh to run: `/bin/sh` off Windows; on Windows the `sh.exe` beside the git in use
/// (`<Git>\bin\sh.exe` or `<Git>\usr\bin\sh.exe` from `<Git>\cmd\git.exe`, `<Git>\bin\git.exe`
/// or `<Git>\mingw64\bin\git.exe`), else `sh.exe` on PATH. `find` says where a path or bare
/// name is. Pure.
pub fn sh_for(windows: bool, git: Option<&Path>, find: &dyn Fn(&Path) -> Option<PathBuf>) -> Result<PathBuf, ShellMissing> {
    if !windows {
        return Ok(PathBuf::from("/bin/sh"));
    }
    let beside = git
        .into_iter()
        .flat_map(|g| g.ancestors().skip(1).take(3))
        .find_map(|dir| [dir.join("bin").join("sh.exe"), dir.join("usr").join("bin").join("sh.exe")].iter().find_map(|p| find(p)));
    beside.or_else(|| find(Path::new("sh.exe"))).ok_or(ShellMissing)
}

/// [`sh_for`] on this system: next to the git chosen in Preferences, else git on PATH. Runner
/// threads only.
pub fn find_sh() -> Result<PathBuf, ShellMissing> {
    use crate::external_tools::{find_on_path, locate_here};
    let git = crate::git_program::chosen().or_else(|| find_on_path(Path::new("git")));
    let find = |p: &Path| if p.components().count() == 1 { find_on_path(p) } else { locate_here(p) };
    sh_for(cfg!(windows), git.as_deref(), &find)
}

/// Runs `script` with sh in `cwd`, stdin closed, with git kept from waiting on an editor or a
/// terminal and credential prompts going to Glowfly's dialog. Returns whether it exited with 0
/// and its output (`$ <script>`, then what it printed, stderr merged, at most the last
/// `MAX_OUTPUT_BYTES`). Runner threads only.
pub fn run_shell(cwd: &Path, script: &str) -> (bool, String) {
    let sh = match find_sh() {
        Ok(sh) => sh,
        Err(e) => return (false, e.to_string()),
    };
    let mut cmd = Command::new(&sh);
    cmd.args(SH_ARGS)
        .env(SCRIPT_VAR, script)
        .env("GIT_EDITOR", "true")
        .env("GIT_SEQUENCE_EDITOR", "true")
        .env("GIT_TERMINAL_PROMPT", "0")
        .envs(crate::gitcli::askpass_env_in(cwd, &[]))
        .current_dir(native(cwd))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    crate::env_path::apply(&mut cmd);
    crate::ssh_version::prepare(&mut cmd);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (false, ShellMissing.to_string()),
        Err(e) => return (false, format!("{} couldn't start: {e}", sh.display())),
    };
    let (out, cut) = child.stdout.take().map(read_tail).unwrap_or_default();
    let ok = child.wait().is_ok_and(|s| s.success());
    let mut text = format!("$ {script}\n");
    if cut {
        text.push_str("(earlier output cut)\n");
    }
    text.push_str(&String::from_utf8_lossy(&out));
    (ok, text.trim_end().to_owned())
}

/// Everything `r` gives, keeping the last `MAX_OUTPUT_BYTES`; true when some was dropped.
fn read_tail(mut r: impl Read) -> (Vec<u8>, bool) {
    let mut kept = Vec::new();
    let mut cut = false;
    let mut buf = [0u8; 16 * 1024];
    loop {
        match r.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => kept.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
        if kept.len() > 2 * MAX_OUTPUT_BYTES {
            kept.drain(..kept.len() - MAX_OUTPUT_BYTES);
            cut = true;
        }
    }
    if kept.len() > MAX_OUTPUT_BYTES {
        kept.drain(..kept.len() - MAX_OUTPUT_BYTES);
        cut = true;
    }
    (kept, cut)
}

#[cfg(test)]
#[path = "shell_tests.rs"]
mod tests;
