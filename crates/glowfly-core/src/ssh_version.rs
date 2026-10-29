//! Which ssh git's children run, and how they start, for the askpass set-up in [`crate::gitcli`].
//!
//! On Unix every git (or ssh-add) child starts in a new session ([`prepare`]): ssh then has no
//! controlling terminal, so it can't ask on the terminal Glowfly was started from and asks the
//! askpass helper instead, and a background job can't stop on that terminal either.
//!
//! OpenSSH 8.4 and newer use the askpass helper whenever `SSH_ASKPASS_REQUIRE=force` is set.
//! Older versions ask it only when `DISPLAY` is set and ssh has no terminal, so with such a
//! version an unset `DISPLAY` gets a placeholder ([`crate::gitcli::display_placeholder`]). The
//! version is that of the ssh git runs in the command's repository ([`ssh_command_for`]: the
//! environment, then the repository's `core.sshCommand`), read once per distinct command with
//! `<ssh> -V`, run the way git runs it (a command through `/bin/sh`). Only the version is read
//! from its output, and the command itself is never logged: it may hold a password. Windows
//! never needs this.

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::Command;

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

/// What git runs to reach an SSH remote.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SshCommand {
    /// A command line (`GIT_SSH_COMMAND` or `core.sshCommand`) git hands to the shell as
    /// `sh -c '<command> "$@"'`, so quoting, variables and wrappers work as in sh.
    Shell(OsString),
    /// A program (`GIT_SSH`, or plain `ssh` from PATH) git runs directly.
    Program(OsString),
}

/// The ssh git runs, in git's order: `GIT_SSH_COMMAND`, then `core.sshCommand`, then
/// `GIT_SSH`, then `ssh`. A value that is empty or only blanks counts as unset. Pure.
pub fn ssh_command(git_ssh_command: Option<&OsStr>, core_ssh_command: Option<&OsStr>, git_ssh: Option<&OsStr>) -> SshCommand {
    let set = |v: Option<&OsStr>| v.filter(|v| is_set(v)).map(OsStr::to_owned);
    match set(git_ssh_command).or_else(|| set(core_ssh_command)) {
        Some(command) => SshCommand::Shell(command),
        None => SshCommand::Program(set(git_ssh).unwrap_or_else(|| "ssh".into())),
    }
}

fn is_set(v: &OsStr) -> bool {
    !v.to_string_lossy().trim().is_empty()
}

/// The ssh a git child started in `cwd` runs: `env` (the variables the caller gives it) and
/// the process environment first, then `core.sshCommand` as git reads it there
/// ([`configured_ssh_command`]). The config is read only when `GIT_SSH_COMMAND` doesn't decide.
pub fn ssh_command_for(cwd: Option<&Path>, env: &[(&str, &str)]) -> SshCommand {
    let var = |name: &str| match env.iter().rev().find(|(k, _)| *k == name) {
        Some((_, v)) => Some(OsString::from(v)),
        None => std::env::var_os(name),
    };
    let git_ssh_command = var("GIT_SSH_COMMAND");
    let configured = if git_ssh_command.as_deref().is_some_and(is_set) { None } else { configured_ssh_command(cwd) };
    ssh_command(git_ssh_command.as_deref(), configured.as_deref(), var("GIT_SSH").as_deref())
}

/// `core.sshCommand` as git sees it in `cwd`: from the repository there with its global and
/// system config, else (outside a repository, or with no `cwd`) from those alone.
pub fn configured_ssh_command(cwd: Option<&Path>) -> Option<OsString> {
    let repo = cwd.and_then(|d| git2::Repository::open_ext(d, git2::RepositoryOpenFlags::empty(), std::iter::empty::<&OsStr>()).ok());
    let config = match repo {
        Some(r) => r.config().ok(),
        None => git2::Config::open_default().ok(),
    }?;
    config.get_string("core.sshCommand").ok().map(OsString::from)
}

/// The command that prints `ssh`'s version, started as git starts it: a shell command as
/// `/bin/sh -c '<command> "$@"' <command> -V`, a program directly with `-V`.
#[cfg(unix)]
pub fn version_command(ssh: &SshCommand) -> Command {
    match ssh {
        SshCommand::Shell(command) => {
            let mut script = command.clone();
            script.push(" \"$@\"");
            let mut cmd = Command::new("/bin/sh");
            cmd.arg("-c").arg(script).arg(command).arg("-V");
            cmd
        }
        SshCommand::Program(program) => {
            let mut cmd = Command::new(program);
            cmd.arg("-V");
            cmd
        }
    }
}

/// Runs `ssh -V` (its whole process tree stopped after 5 s) and reads its version.
#[cfg(unix)]
pub fn detect(ssh: &SshCommand) -> Option<(u32, u32)> {
    use std::io::Read;
    use std::process::Stdio;
    use std::time::{Duration, Instant};
    let mut cmd = version_command(ssh);
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    crate::env_path::apply(&mut cmd);
    crate::process_tree::prepare(&mut cmd);
    let mut child = cmd.spawn().ok()?;
    let tree = crate::process_tree::attach(&child);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            _ => {
                tree.terminate(&mut child);
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

/// Whether `ssh` is older than OpenSSH 8.4, checked once per process for each command; always
/// false on Windows, whose OpenSSH doesn't need the legacy set-up.
pub fn legacy_for(ssh: &SshCommand) -> bool {
    #[cfg(unix)]
    {
        use std::collections::HashMap;
        use std::sync::{Arc, LazyLock, Mutex, OnceLock, PoisonError};
        type Known = Mutex<HashMap<SshCommand, Arc<OnceLock<bool>>>>;
        static KNOWN: LazyLock<Known> = LazyLock::new(Known::default);
        // The map is held only to find the entry; the check runs outside it, so a slow ssh
        // holds up only the children that need that same ssh.
        let entry = KNOWN.lock().unwrap_or_else(PoisonError::into_inner).entry(ssh.clone()).or_default().clone();
        *entry.get_or_init(|| needs_legacy(detect(ssh)))
    }
    #[cfg(not(unix))]
    {
        let _ = ssh;
        false
    }
}

/// [`legacy_for`] the ssh a git child started in `cwd` with `env` runs ([`ssh_command_for`]);
/// always false on Windows, without reading any config.
pub fn legacy_askpass_in(cwd: Option<&Path>, env: &[(&str, &str)]) -> bool {
    cfg!(unix) && legacy_for(&ssh_command_for(cwd, env))
}

/// [`legacy_askpass_in`] outside any repository, with the process environment.
pub fn legacy_askpass() -> bool {
    legacy_askpass_in(None, &[])
}

/// Prepares a git (or ssh-add) child for the askpass helper: on Unix it starts in a new
/// session, so ssh has no terminal and asks the helper. Nothing to do on Windows.
pub fn prepare(cmd: &mut Command) {
    #[cfg(unix)]
    detach_from_terminal(cmd);
    #[cfg(not(unix))]
    let _ = cmd;
}

/// Starts `cmd`'s child in a new session (`setsid`), without a controlling terminal: ssh can't
/// open `/dev/tty`, and a background job can't block on the terminal Glowfly was started from.
/// The child also leads its own process group, which [`crate::process_tree`] stops as a whole.
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
