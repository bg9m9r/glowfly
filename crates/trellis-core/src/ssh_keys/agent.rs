//! The local SSH agent: whether one is running and which keys it holds (`ssh-add -l`, with
//! their public lines from `ssh-add -L`), which ssh git runs on Windows, the
//! Windows OpenSSH Authentication Agent service and Pageant, and adding a key with `ssh-add`.
//! A passphrase only ever reaches ssh-add through the askpass helper, never its arguments.

use super::SshKeyPair;
use super::tools::tool_candidates;
use super::windows_ssh::WinSshFacts;
use crate::gitcli;
use crate::signing::command;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// How long `ssh-add -l` may take.
const LIST_TIMEOUT: Duration = Duration::from_secs(10);
/// How long the Windows service queries may take.
const SC_TIMEOUT: Duration = Duration::from_secs(5);
/// How long adding a key may take, passphrase prompt included.
const ADD_TIMEOUT: Duration = Duration::from_secs(300);

/// A key the agent holds, as `ssh-add -l` lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentKey {
    pub bits: Option<u32>,
    pub fingerprint: String,
    pub comment: String,
    pub kind: String,
    /// Its public key line from `ssh-add -L`, matched by fingerprint; None when the agent
    /// didn't give one that matches.
    pub public_key: Option<String>,
}

/// What `ssh-add -l` found.
#[derive(Clone, Debug, PartialEq)]
pub enum AgentReach {
    /// An agent answered, holding these keys (maybe none).
    Keys(Vec<AgentKey>),
    NoAgent,
    Failed(String),
}

/// The Windows OpenSSH Authentication Agent service.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WinService {
    Running,
    Stopped,
    Disabled,
    Missing,
}

/// Everything Preferences > SSH shows about the agent.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct AgentStatus {
    pub ssh_add: Option<PathBuf>,
    /// None without ssh-add.
    pub reach: Option<AgentReach>,
    /// Windows only.
    pub service: Option<WinService>,
    /// A Pageant named pipe exists (Windows).
    pub pageant: bool,
    pub auth_sock_set: bool,
    /// "GIT_SSH_COMMAND" or "GIT_SSH", whichever is set in Trellis's environment.
    pub git_ssh_env: Option<String>,
    /// GIT_SSH_COMMAND is set: it overrides core.sshCommand for every git command.
    pub git_ssh_command_set: bool,
    /// Which ssh git would run (Windows only).
    pub win_ssh: Option<WinSshFacts>,
}

impl AgentStatus {
    /// An agent answered.
    pub fn running(&self) -> bool {
        matches!(self.reach, Some(AgentReach::Keys(_)))
    }

    /// The agent holds the key with this fingerprint.
    pub fn holds(&self, fingerprint: &str) -> bool {
        match &self.reach {
            Some(AgentReach::Keys(keys)) => keys.iter().any(|k| k.fingerprint == fingerprint),
            _ => false,
        }
    }

    /// The agent holds the key of `pair` (see [`pair_matches`]).
    pub fn holds_pair(&self, pair: &SshKeyPair) -> bool {
        match &self.reach {
            Some(AgentReach::Keys(keys)) => keys.iter().any(|k| pair_matches(pair, k)),
            _ => false,
        }
    }
}

/// Whether the agent's `key` is the key of `pair`: the same fingerprint, or the same public key
/// body (when the pair has no fingerprint because ssh-keygen is missing).
pub fn pair_matches(pair: &SshKeyPair, key: &AgentKey) -> bool {
    let body = super::key_body(&pair.public_key);
    pair.fingerprint.as_deref() == Some(key.fingerprint.as_str())
        || (body.is_some() && key.public_key.as_deref().and_then(super::key_body) == body)
}

/// Fills each key's public line from `ssh-add -L` output, pairing them by the fingerprint
/// computed from each line (never by position). Keys with no matching line stay None.
pub fn attach_public_keys(keys: &mut [AgentKey], list_l_stdout: &str) {
    let lines: Vec<(String, &str)> =
        list_l_stdout.lines().map(str::trim).filter_map(|l| super::public_fingerprint(l).map(|fp| (fp, l))).collect();
    for key in keys {
        key.public_key = lines.iter().find(|(fp, _)| *fp == key.fingerprint).map(|(_, l)| (*l).to_owned());
    }
}

/// One line of `ssh-add -l`, e.g. "256 SHA256:abc ada@x laptop (ED25519)".
pub fn parse_agent_line(line: &str) -> Option<AgentKey> {
    let (bits, fingerprint, kind) = super::parse_fingerprint(line)?;
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let comment = tokens[2..tokens.len() - 1].join(" ");
    Some(AgentKey { bits, fingerprint, comment, kind, public_key: None })
}

/// `ssh-add -l`'s answer by its exit code: 0 lists keys, 1 means none are loaded, 2 means no
/// agent could be reached.
pub fn parse_agent_list(code: Option<i32>, stdout: &str, stderr: &str) -> AgentReach {
    match code {
        Some(0) => AgentReach::Keys(stdout.lines().filter_map(parse_agent_line).collect()),
        Some(1) => AgentReach::Keys(Vec::new()),
        Some(2) => AgentReach::NoAgent,
        _ => AgentReach::Failed(first_line(stderr).unwrap_or("ssh-add failed").to_owned()),
    }
}

fn first_line(text: &str) -> Option<&str> {
    text.lines().map(str::trim).find(|l| !l.is_empty())
}

/// `sc.exe query ssh-agent`: missing (error 1060), running, or stopped.
pub fn parse_sc_query(code: Option<i32>, stdout: &str) -> WinService {
    if code == Some(1060) || stdout.contains("FAILED 1060") || stdout.to_ascii_lowercase().contains("does not exist") {
        return WinService::Missing;
    }
    let running = stdout.lines().any(|l| l.contains("STATE") && l.contains("RUNNING"));
    if running { WinService::Running } else { WinService::Stopped }
}

/// `sc.exe qc ssh-agent` says the service can't start.
pub fn sc_start_disabled(stdout: &str) -> bool {
    stdout.lines().any(|l| l.contains("START_TYPE") && l.contains("DISABLED"))
}

/// A named pipe Pageant listens on ("pageant.<user>.<hash>").
pub fn is_pageant_pipe(name: &str) -> bool {
    name.to_ascii_lowercase().starts_with("pageant.")
}

/// Where to look for ssh-add. On Windows, Windows' own copy comes first, since that one talks
/// to the agent service; otherwise the usual order (see `tool_candidates`).
pub fn ssh_add_candidates_for(
    windows: bool,
    path_var: Option<&OsStr>,
    exe_suffix: &str,
    git_exec_path: Option<&Path>,
    system_root: Option<&Path>,
) -> Vec<PathBuf> {
    let mut all = tool_candidates("ssh-add", path_var, exe_suffix, git_exec_path, system_root);
    if windows && let Some(root) = system_root {
        let windows_own = root.join("System32").join("OpenSSH").join("ssh-add.exe");
        all.retain(|p| *p != windows_own);
        all.insert(0, windows_own);
    }
    all
}

/// Runs `cmd` (stdout and stderr piped), killing it after `timeout`: the exit code (None if
/// killed), stdout and stderr.
pub(super) fn run_with_timeout(mut cmd: Command, timeout: Duration) -> std::io::Result<(Option<i32>, String, String)> {
    let mut child = cmd.spawn()?;
    let read = |pipe: Option<Box<dyn std::io::Read + Send>>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_end(&mut buf);
            }
            String::from_utf8_lossy(&buf).into_owned()
        })
    };
    let out = read(child.stdout.take().map(|p| Box::new(p) as Box<dyn std::io::Read + Send>));
    let err = read(child.stderr.take().map(|p| Box::new(p) as Box<dyn std::io::Read + Send>));
    let deadline = Instant::now() + timeout;
    let code = loop {
        if let Some(status) = child.try_wait()? {
            break status.code();
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Ok((None, String::new(), format!("stopped after {} seconds", timeout.as_secs())));
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    Ok((code, out.join().unwrap_or_default(), err.join().unwrap_or_default()))
}

/// Everything about the agent, read from the system. Slow (it runs programs): call it off the
/// UI thread.
pub fn agent_status(cwd: &Path) -> AgentStatus {
    let ssh_add = super::find_ssh_add(cwd);
    let list = |p: &Path, flag: &str| {
        let mut cmd = command(p);
        cmd.arg(flag).env("SSH_ASKPASS_REQUIRE", "never");
        run_with_timeout(cmd, LIST_TIMEOUT)
    };
    let reach = ssh_add.as_ref().map(|p| {
        let mut reach = match list(p, "-l") {
            Ok((code, stdout, stderr)) => parse_agent_list(code, &stdout, &stderr),
            Err(e) => AgentReach::Failed(format!("could not run ssh-add: {e}")),
        };
        if let AgentReach::Keys(keys) = &mut reach
            && !keys.is_empty()
            && let Ok((Some(0), stdout, _)) = list(p, "-L")
        {
            attach_public_keys(keys, &stdout);
        }
        reach
    });
    let set = |name: &str| std::env::var_os(name).is_some_and(|v| !v.is_empty());
    let git_ssh_env = ["GIT_SSH_COMMAND", "GIT_SSH"].into_iter().find(|n| set(n)).map(str::to_owned);
    AgentStatus {
        ssh_add,
        reach,
        service: windows_service(),
        pageant: pageant_running(),
        auth_sock_set: set("SSH_AUTH_SOCK"),
        git_ssh_command_set: set("GIT_SSH_COMMAND"),
        git_ssh_env,
        win_ssh: super::windows_ssh::facts(cwd),
    }
}

#[cfg(windows)]
fn windows_service() -> Option<WinService> {
    let sc = |verb: &str| {
        let mut cmd = command("sc.exe");
        cmd.args([verb, "ssh-agent"]);
        run_with_timeout(cmd, SC_TIMEOUT).ok()
    };
    let (code, stdout, _) = sc("query")?;
    match parse_sc_query(code, &stdout) {
        WinService::Stopped => {
            let disabled = sc("qc").is_some_and(|(_, out, _)| sc_start_disabled(&out));
            Some(if disabled { WinService::Disabled } else { WinService::Stopped })
        }
        s => Some(s),
    }
}

#[cfg(not(windows))]
fn windows_service() -> Option<WinService> {
    let _ = SC_TIMEOUT;
    None
}

#[cfg(windows)]
fn pageant_running() -> bool {
    std::fs::read_dir(r"\\.\pipe\").is_ok_and(|d| d.flatten().any(|e| is_pageant_pipe(&e.file_name().to_string_lossy())))
}

#[cfg(not(windows))]
fn pageant_running() -> bool {
    false
}

/// ssh-add's arguments for adding `private`: only the key's path.
pub fn add_args(private: &Path) -> Vec<OsString> {
    vec![crate::native_path::native(private).into_os_string()]
}

/// Adds `private` to the agent. A passphrase-protected key asks through the askpass helper
/// (once the app has enabled it; until then, and in tests, ssh-add can't prompt at all).
pub fn add_to_agent(ssh_add: &Path, private: &Path) -> Result<(), String> {
    let mut cmd = command(ssh_add);
    cmd.args(add_args(private));
    let askpass = gitcli::askpass_env();
    if askpass.is_empty() {
        cmd.envs(gitcli::NO_PROMPTS.iter().copied());
    } else {
        cmd.envs(askpass);
    }
    crate::ssh_version::prepare(&mut cmd);
    match run_with_timeout(cmd, ADD_TIMEOUT) {
        Ok((Some(0), _, _)) => Ok(()),
        Ok((_, _, stderr)) => Err(first_line(&stderr).unwrap_or("ssh-add failed").to_owned()),
        Err(e) => Err(format!("could not run ssh-add: {e}")),
    }
}
