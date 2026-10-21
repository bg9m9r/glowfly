//! Which OpenSSH git's children run, for the askpass set-up in [`crate::gitcli`].
//!
//! OpenSSH 8.4 and newer use the askpass helper whenever `SSH_ASKPASS_REQUIRE=force` is set.
//! Older versions ask it only when `DISPLAY` is set and ssh has no terminal to read from, so on
//! Unix, with such a version, every git child gets a placeholder `DISPLAY` and starts in a new
//! session ([`prepare`]), which leaves ssh without a controlling terminal. The version is read
//! once per process (`ssh -V`), the first time a git child starts. Windows never needs this.

use std::ffi::{OsStr, OsString};
use std::process::Command;
#[cfg(unix)]
use std::sync::OnceLock;

/// Reads `major.minor` from `ssh -V` output such as "OpenSSH_9.6p1, LibreSSL 3.3.6" or
/// "OpenSSH_for_Windows_9.5p1, LibreSSL 3.8.2".
pub fn parse(s: &str) -> Option<(u32, u32)> {
    let rest = &s[s.find("OpenSSH_")? + "OpenSSH_".len()..];
    let rest = rest.strip_prefix("for_Windows_").unwrap_or(rest);
    let end = rest.find(['p', ',', ' ', '\n', '\r']).unwrap_or(rest.len());
    let (major, minor) = rest[..end].split_once('.')?;
    Some((major.parse().ok()?, minor.parse().ok()?))
}

/// Whether ssh needs the legacy askpass set-up: a version before 8.4, or one that couldn't be
/// read (the safe choice, as it was before the version was checked).
pub fn needs_legacy(v: Option<(u32, u32)>) -> bool {
    v.is_none_or(|v| v < (8, 4))
}

/// The ssh git runs: the first word of `GIT_SSH_COMMAND` (unless it is empty or quoted), else
/// `GIT_SSH`, else `ssh` from PATH.
pub fn ssh_program(git_ssh_command: Option<&OsStr>, git_ssh: Option<&OsStr>) -> OsString {
    let first = git_ssh_command.map(|c| c.to_string_lossy().split_whitespace().next().unwrap_or("").to_owned());
    if let Some(word) = first.filter(|w| !w.is_empty() && !w.starts_with(['"', '\''])) {
        return word.into();
    }
    match git_ssh.filter(|s| !s.is_empty()) {
        Some(s) => s.to_owned(),
        None => "ssh".into(),
    }
}

/// Runs `<ssh> -V` (killed after 5 s) and reads its version.
#[cfg(unix)]
fn detect() -> Option<(u32, u32)> {
    use std::io::Read;
    use std::process::Stdio;
    use std::time::{Duration, Instant};
    let program = ssh_program(std::env::var_os("GIT_SSH_COMMAND").as_deref(), std::env::var_os("GIT_SSH").as_deref());
    let mut cmd = Command::new(program);
    cmd.arg("-V").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    crate::env_path::apply(&mut cmd);
    let mut child = cmd.spawn().ok()?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    fn read(pipe: Option<impl Read>) -> String {
        let mut text = String::new();
        if let Some(mut p) = pipe {
            let _ = p.read_to_string(&mut text);
        }
        text
    }
    let err = read(child.stderr.take());
    let out = read(child.stdout.take());
    parse(&err).or_else(|| parse(&out))
}

#[cfg(unix)]
static LEGACY: OnceLock<bool> = OnceLock::new();

/// Whether git's ssh is older than OpenSSH 8.4 (checked once per process); always false on
/// Windows, whose OpenSSH doesn't need the legacy set-up.
pub fn legacy_askpass() -> bool {
    #[cfg(unix)]
    {
        *LEGACY.get_or_init(|| needs_legacy(detect()))
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// Prepares a git (or ssh-add) child for the askpass helper: with OpenSSH before 8.4 on Unix,
/// it starts in a new session, so ssh has no terminal and asks the helper.
#[cfg(unix)]
pub fn prepare(cmd: &mut Command) {
    if legacy_askpass() {
        detach_from_terminal(cmd);
    }
}

/// Prepares a git (or ssh-add) child for the askpass helper; nothing to do on Windows.
#[cfg(not(unix))]
pub fn prepare(_cmd: &mut Command) {}

/// Starts `cmd`'s child in a new session (`setsid`), without a controlling terminal: ssh can't
/// open `/dev/tty`, and a background job can't block on the terminal Glowfly was started from.
#[cfg(unix)]
pub fn detach_from_terminal(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;
    let detach = || -> std::io::Result<()> {
        // SAFETY: setsid is async-signal-safe and only changes the child's session.
        let _ = unsafe { libc::setsid() };
        Ok(())
    };
    // SAFETY: the hook only calls setsid, which is safe to run between fork and exec.
    unsafe { cmd.pre_exec(detach) };
}

#[cfg(test)]
#[path = "ssh_version_tests.rs"]
mod tests;
