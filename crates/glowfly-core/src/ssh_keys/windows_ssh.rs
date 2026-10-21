//! Which ssh git runs on Windows, and pointing it at Windows' own OpenSSH, the one that reads
//! keys from the OpenSSH Authentication Agent service (Git for Windows' bundled ssh doesn't).
//!
//! git runs `GIT_SSH_COMMAND`, else `core.sshCommand`, else `GIT_SSH`, else `ssh` from its PATH,
//! where Git for Windows puts its bundled `usr/bin` first. Glowfly writes one form for Windows'
//! ssh, the program alone, sh-quoted with forward slashes
//! (`'C:/Windows/System32/OpenSSH/ssh.exe'`), and recognises it by that exact spelling. Every
//! decision here is pure or reads only a folder tree, so it is tested on every OS; only
//! [`facts`] looks at the running system, and only on Windows.

use super::command::{KeyChoice, SSH_COMMAND_KEY, parse_ssh_command, split_sh_words};
use super::tools::tool_candidates;
use crate::paths::{resolve, same_spelling};
use crate::rebase::shell_quote_for;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// What decides which ssh git runs on Windows, read from the system.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct WinSshFacts {
    /// `%SystemRoot%\System32\OpenSSH\ssh.exe`, in its on-disk spelling, when it exists.
    pub windows: Option<PathBuf>,
    /// What `ssh -V` says Windows' ssh is (major, minor).
    pub windows_version: Option<(u32, u32)>,
    /// Git for Windows' own `<git root>/usr/bin/ssh.exe`, when it exists.
    pub bundled: Option<PathBuf>,
    /// The first `ssh.exe` in Glowfly's PATH.
    pub on_path: Option<PathBuf>,
    /// GIT_SSH from the environment (non-empty).
    pub git_ssh: Option<String>,
    /// The system config's core.sshCommand.
    pub system_command: Option<String>,
}

/// The ssh git runs for SSH remotes.
#[derive(Clone, Debug, PartialEq)]
pub enum GitSsh {
    /// GIT_SSH_COMMAND is set: it beats every setting.
    EnvCommand,
    /// Git for Windows' bundled ssh.
    Bundled,
    /// Windows' own OpenSSH.
    WindowsOwn,
    /// Another program (plink, say).
    Other(String),
    /// It can't be told: a command sh would expand, or no ssh found.
    Unknown,
}

/// Whether `word` is the spelling Glowfly writes for Windows' ssh: a drive (`X:` and a
/// separator) and the last three parts exactly `System32`, `OpenSSH`, `ssh.exe`.
pub fn is_windows_ssh_word(word: &str) -> bool {
    let b = word.as_bytes();
    if b.len() < 3 || !b[0].is_ascii_alphabetic() || b[1] != b':' || !matches!(b[2], b'/' | b'\\') {
        return false;
    }
    let parts: Vec<&str> = word[2..].split(['/', '\\']).filter(|p| !p.is_empty()).collect();
    parts.len() >= 3 && parts[parts.len() - 3..] == ["System32", "OpenSSH", "ssh.exe"]
}

/// The core.sshCommand value that makes git run `program` (Windows' ssh): sh-quoted, with
/// forward slashes. None when the path isn't UTF-8.
pub fn windows_ssh_command_for(program: &Path) -> Option<String> {
    Some(shell_quote_for(crate::native_path::native(program).to_str()?, true))
}

/// The program a command runs: its first word as sh splits it.
pub fn program_word(command: &str) -> Option<String> {
    split_sh_words(command)?.into_iter().next()
}

/// What a setting makes git run.
enum Program {
    /// Plain `ssh`, found on git's PATH.
    Bare,
    Named(String),
    Unknown,
}

fn program_named(word: String) -> Program {
    if word == "ssh" || word == "ssh.exe" { Program::Bare } else { Program::Named(word) }
}

/// The program a core.sshCommand value runs; None when it is unset.
fn program_of(choice: &KeyChoice) -> Option<Program> {
    Some(match choice {
        KeyChoice::Unset => return None,
        KeyChoice::Key(_) => Program::Bare,
        KeyChoice::WindowsSsh(p) => Program::Named(p.to_string_lossy().into_owned()),
        KeyChoice::Other { value, .. } => program_word(value).map_or(Program::Unknown, program_named),
    })
}

/// Which ssh git runs, from GIT_SSH_COMMAND (`env_command_set`), the global core.sshCommand,
/// then the system one, GIT_SSH, and plain `ssh`.
pub fn git_runs(env_command_set: bool, global: &KeyChoice, facts: &WinSshFacts) -> GitSsh {
    if env_command_set {
        return GitSsh::EnvCommand;
    }
    let system = facts.system_command.as_deref().map(parse_ssh_command);
    let program = program_of(global)
        .or_else(|| system.as_ref().and_then(program_of))
        .or_else(|| facts.git_ssh.clone().filter(|s| !s.is_empty()).map(program_named))
        .unwrap_or(Program::Bare);
    let is = |p: &Path, known: &Option<PathBuf>| known.as_deref().is_some_and(|k| same_spelling(p, k));
    match program {
        Program::Bare => match &facts.on_path {
            _ if facts.bundled.is_some() => GitSsh::Bundled,
            Some(p) if is(p, &facts.windows) => GitSsh::WindowsOwn,
            Some(p) => GitSsh::Other(p.display().to_string()),
            None => GitSsh::Unknown,
        },
        Program::Named(word) => {
            if is_windows_ssh_word(&word) || is(Path::new(&word), &facts.windows) {
                GitSsh::WindowsOwn
            } else if is(Path::new(&word), &facts.bundled) {
                GitSsh::Bundled
            } else {
                GitSsh::Other(word)
            }
        }
        Program::Unknown => GitSsh::Unknown,
    }
}

/// `path`, resolved, when it is a file.
fn file_at(path: PathBuf) -> Option<PathBuf> {
    path.is_file().then(|| resolve(&path))
}

/// The facts for a system whose Windows folder is `system_root`, git's exec path is
/// `git_exec_path` (Git for Windows' root is three folders above it) and PATH is `path_var`.
/// `version` reads the version of Windows' ssh when it exists.
pub fn facts_from(
    system_root: &Path,
    git_exec_path: Option<&Path>,
    path_var: Option<&OsStr>,
    git_ssh: Option<String>,
    system_command: Option<String>,
    version: impl FnOnce(&Path) -> Option<(u32, u32)>,
) -> WinSshFacts {
    let windows = file_at(system_root.join("System32").join("OpenSSH").join("ssh.exe"));
    let bundled = git_exec_path.and_then(|p| p.ancestors().nth(3)).and_then(|root| file_at(root.join("usr").join("bin").join("ssh.exe")));
    let on_path = tool_candidates("ssh", path_var, ".exe", None, None).into_iter().find_map(file_at);
    let windows_version = windows.as_deref().and_then(version);
    WinSshFacts { windows, windows_version, bundled, on_path, git_ssh, system_command }
}

/// The facts of the running system; None anywhere but Windows, or without SystemRoot. Slow (it
/// runs git and ssh): call it off the UI thread.
#[cfg(windows)]
pub fn facts(cwd: &Path) -> Option<WinSshFacts> {
    let root = std::env::var_os("SystemRoot").filter(|r| !r.is_empty()).map(PathBuf::from)?;
    let (exec, _) = super::tools::windows_roots(cwd);
    let path_var = crate::env_path::path();
    let git_ssh = std::env::var("GIT_SSH").ok().filter(|s| !s.is_empty());
    let version = |ssh: &Path| {
        let mut cmd = crate::signing::command(ssh);
        cmd.arg("-V");
        let (_, out, err) = super::agent::run_with_timeout(cmd, std::time::Duration::from_secs(5)).ok()?;
        crate::ssh_version::parse(&err).or_else(|| crate::ssh_version::parse(&out))
    };
    Some(facts_from(&root, exec.as_deref(), path_var.as_deref(), git_ssh, read_system_command(cwd), version))
}

/// The facts of the running system; None anywhere but Windows.
#[cfg(not(windows))]
pub fn facts(_cwd: &Path) -> Option<WinSshFacts> {
    None
}

/// `git config --system --get core.sshCommand`; None when it is unset or can't be read.
#[cfg(windows)]
fn read_system_command(cwd: &Path) -> Option<String> {
    use crate::gitcli::{self, NO_PROMPTS, READ_TIMEOUT, args};
    let out = gitcli::output(cwd, &args(&["config", "--system", "--get", SSH_COMMAND_KEY]), "", NO_PROMPTS, Some(READ_TIMEOUT));
    let value = out.stdout.trim_end_matches(['\r', '\n']);
    (out.ok && !value.trim().is_empty()).then(|| value.to_owned())
}

/// Makes git run `program` (Windows' ssh) for SSH remotes in every repository: the global
/// core.sshCommand. `env` is added to git's environment (tests point `GIT_CONFIG_GLOBAL` at a
/// file of their own).
pub fn write_windows_ssh(cwd: &Path, program: &Path, env: &[(&str, &str)]) -> Result<(), String> {
    let value = windows_ssh_command_for(program).ok_or("This program's path can't be written into git config")?;
    crate::identity::set_config(cwd, true, SSH_COMMAND_KEY, Some(&value), env)
}
