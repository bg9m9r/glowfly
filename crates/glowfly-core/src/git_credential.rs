//! Saving and dropping HTTPS sign-ins typed in the credential prompt with git's own credential
//! helpers (`git credential approve` / `reject`), so the next fetch or push needn't ask again.
//!
//! When git has no `credential.helper` at all, [`remember`] sets up the platform's credential
//! store (Git Credential Manager or wincred, the macOS keychain, libsecret) in the global git
//! config first; on Linux without a keyring helper that is git's in-memory `cache`. A helper set
//! to the empty value means the user turned helpers off; nothing is saved then. The password
//! only ever reaches git on stdin. Every git call here runs with [`gitcli::BACKGROUND`], so none
//! of them prompts.

use crate::askpass::{self, Secret};
use crate::gitcli::{self, GitOutput, args};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How long `git credential approve` may take: a keychain may ask the user to allow it.
const APPROVE_TIMEOUT: Duration = Duration::from_secs(120);

/// Whether `url` is an `http://` or `https://` URL (any case).
pub fn is_http_url(url: &str) -> bool {
    let lower = url.get(..8).unwrap_or(url).to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// A username and password or token typed for an HTTP(S) remote, in the fields git's credential
/// protocol uses.
#[derive(Clone, Debug)]
pub struct HttpsCredential {
    /// `http` or `https`.
    pub protocol: String,
    /// Lowercase, with any port.
    pub host: String,
    /// Without its leading `/`; None when git's prompt named no path.
    pub path: Option<String>,
    pub username: String,
    pub password: Secret,
}

impl HttpsCredential {
    /// The credential for git's prompt URL (`scheme://[user[:x]@]host[:port][/path]`), or None
    /// unless it is http(s) with a host, the username isn't empty, and no field holds a newline
    /// or NUL (which git's credential protocol refuses).
    pub fn from_prompt(url: &str, username: &str, password: Secret) -> Option<Self> {
        let (scheme, rest) = url.split_once("://")?;
        let protocol = scheme.to_ascii_lowercase();
        if protocol != "http" && protocol != "https" {
            return None;
        }
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h).to_ascii_lowercase();
        let path = Some(path.to_owned()).filter(|p| !p.is_empty());
        let bad = |s: &str| s.contains(['\n', '\0']);
        if host.is_empty() || username.is_empty() || [host.as_str(), username, password.expose()].into_iter().any(bad) {
            return None;
        }
        if path.as_deref().is_some_and(bad) {
            return None;
        }
        Some(HttpsCredential { protocol, host, path, username: username.to_owned(), password })
    }

    /// `protocol://host[/path]`, for `git config --get-urlmatch` and messages.
    pub fn url(&self) -> String {
        let path = self.path.as_deref().map(|p| format!("/{p}")).unwrap_or_default();
        format!("{}://{}{path}", self.protocol, self.host)
    }

    /// The credential as git's credential protocol reads it on stdin. The caller wipes the text
    /// ([`askpass::wipe`]) once it is sent.
    pub fn request(&self, with_password: bool) -> String {
        let mut s = format!("protocol={}\nhost={}\n", self.protocol, self.host);
        if let Some(p) = &self.path {
            s.push_str(&format!("path={p}\n"));
        }
        s.push_str(&format!("username={}\n", self.username));
        if with_password {
            s.push_str("password=");
            s.push_str(self.password.expose());
            s.push('\n');
        }
        s.push('\n');
        s
    }
}

/// What git's `credential.helper` is for a URL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelperConfig {
    /// A helper is configured: git already ran it when the credential worked.
    Set,
    /// Set to the empty value: the user turned helpers off.
    TurnedOff,
    /// Not set anywhere.
    None,
}

/// Reads `git config --get-urlmatch credential.helper`'s result.
fn helper_config_from(out: &GitOutput) -> HelperConfig {
    match out.code {
        Some(0) if out.stdout.trim().is_empty() => HelperConfig::TurnedOff,
        Some(0) => HelperConfig::Set,
        _ => HelperConfig::None,
    }
}

/// [`gitcli::BACKGROUND`] followed by `env`.
fn quiet_env<'a>(env: &[(&'a str, &'a str)]) -> Vec<(&'a str, &'a str)> {
    let mut all = gitcli::BACKGROUND.to_vec();
    all.extend_from_slice(env);
    all
}

/// The credential helper git uses for `url`, run in `cwd` with `env` added.
pub fn helper_config(cwd: &Path, url: &str, env: &[(&str, &str)]) -> HelperConfig {
    let out = gitcli::output(
        cwd,
        &args(&["config", "--get-urlmatch", "credential.helper", url]),
        "",
        &quiet_env(env),
        Some(gitcli::READ_TIMEOUT),
    );
    helper_config_from(&out)
}

/// The credential stores git ships or recommends on `os` (`std::env::consts::OS`), best first.
pub fn platform_helpers(os: &str) -> &'static [&'static str] {
    match os {
        "windows" => &["manager", "wincred"],
        "macos" => &["osxkeychain"],
        "linux" => &["libsecret"],
        _ => &[],
    }
}

/// How to get a credential store on `os`, for the error when none is installed.
pub fn setup_hint(os: &str) -> &'static str {
    match os {
        "windows" => "install Git Credential Manager or set credential.helper",
        "macos" => "install git-credential-osxkeychain or set credential.helper",
        "linux" => "install git-credential-libsecret or set credential.helper",
        _ => "set credential.helper",
    }
}

/// The helper to fall back to on `os` when none of its [`platform_helpers`] is installed: git's
/// in-memory `cache` on Linux, where a keyring helper is often missing. It keeps a sign-in for 15
/// minutes after git last used it, and nothing reaches the disk.
pub fn fallback_helper(os: &str) -> Option<&'static str> {
    (os == "linux").then_some("cache")
}

/// What [`remember`] does for a URL.
#[derive(Debug, PartialEq)]
pub(crate) enum Plan {
    /// Hand the sign-in to the helper the user configured.
    Approve,
    /// The user turned helpers off: save nothing.
    TurnedOff,
    /// Add this persistent store to the global git config first.
    SetUp(String),
    /// No persistent store is installed: add this memory-only helper first.
    Cache(String),
    /// Nothing to save it with.
    Missing,
}

/// Chooses the [`Plan`] from git's helper setting, the persistent store `installed` finds (only
/// asked when git has no helper) and the memory-only `fallback` this git can run (only asked when
/// no persistent store is installed).
pub(crate) fn plan(config: HelperConfig, installed: impl FnOnce() -> Option<String>, fallback: impl FnOnce() -> Option<String>) -> Plan {
    match config {
        HelperConfig::Set => Plan::Approve,
        HelperConfig::TurnedOff => Plan::TurnedOff,
        HelperConfig::None => match installed() {
            Some(name) => Plan::SetUp(name),
            None => fallback().map_or(Plan::Missing, Plan::Cache),
        },
    }
}

/// This OS's [`fallback_helper`], when this git has it: a git built without Unix sockets, or a
/// package that leaves it out, has no `credential-cache`, and naming a missing helper in the
/// config would make git warn on every HTTPS operation.
pub fn usable_fallback(cwd: &Path, env: &[(&str, &str)]) -> Option<String> {
    let name = fallback_helper(std::env::consts::OS)?;
    cache_runs(cwd, env).then(|| name.to_owned())
}

/// Whether `git credential-cache` runs here: `exit` on a socket nobody uses checks the command
/// and its `--socket` option without touching a running daemon (nor making the folder).
pub(crate) fn cache_runs(cwd: &Path, env: &[(&str, &str)]) -> bool {
    let unused = crate::paths::temp_dir().join(format!("glowfly-cc-probe-{}", std::process::id())).join("s");
    let socket = format!("--socket={}", unused.display());
    gitcli::output(cwd, &args(&["credential-cache", &socket, "exit"]), "", &quiet_env(env), Some(gitcli::READ_TIMEOUT)).ok
}

/// `value` as a POSIX extended regex that matches only itself, for `git config --unset`'s value
/// pattern (`--fixed-value` needs git 2.30).
fn exact_value(value: &str) -> String {
    let mut re = String::from("^");
    for ch in value.chars() {
        if "\\.^$*+?()[]{}|".contains(ch) {
            re.push('\\');
        }
        re.push(ch);
    }
    re.push('$');
    re
}

/// The first of this OS's [`platform_helpers`] that is installed: `git-credential-<name>` in
/// git's exec path (or the `bin` folder next to it, where Git for Windows keeps Git Credential
/// Manager) or on PATH.
pub fn installed_helper(cwd: &Path) -> Option<String> {
    let exec = gitcli::read(cwd, &args(&["--exec-path"]));
    let exec_dir = PathBuf::from(exec.stdout.trim());
    let mut dirs = Vec::new();
    if exec.ok && !exec_dir.as_os_str().is_empty() {
        dirs.extend(exec_dir.parent().and_then(Path::parent).map(|p| p.join("bin")));
        dirs.insert(0, exec_dir);
    }
    let in_dir = |d: &Path, p: &str| d.join(p).is_file() || (cfg!(windows) && d.join(format!("{p}.exe")).is_file());
    let found =
        |program: &str| dirs.iter().any(|d| in_dir(d, program)) || crate::external_tools::find_on_path(Path::new(program)).is_some();
    platform_helpers(std::env::consts::OS).iter().find(|name| found(&format!("git-credential-{name}"))).map(|n| n.to_string())
}

/// What [`remember`] did.
#[derive(Debug, Clone, PartialEq)]
pub enum Remembered {
    /// Handed to the helper the user already has.
    Helper,
    /// No helper was set, so this one was added to the global git config first.
    SetUp(String),
    /// No helper was set and no persistent store is installed, so git's in-memory `cache` was
    /// added to the global git config first: the sign-in lasts while git keeps using it.
    Cached,
    /// `credential.helper` is set to the empty value: the user turned saving off, so nothing was
    /// saved.
    TurnedOff,
}

/// Saves `c` with git's credential helper, setting up the platform's store when git has none
/// (or, on Linux without a keyring helper, git's in-memory cache).
pub fn remember(cwd: &Path, c: &HttpsCredential) -> Result<Remembered, String> {
    remember_in(cwd, c, &[], installed_helper, |cwd| usable_fallback(cwd, &[]))
}

/// [`remember`] with `env` added to every git call, `find` naming the store to set up and
/// `fallback` the memory-only helper for when it finds none. A helper added to the global config
/// is removed again (that value only) when the save itself fails, so no broken set-up is left.
pub(crate) fn remember_in(
    cwd: &Path,
    c: &HttpsCredential,
    env: &[(&str, &str)],
    find: impl Fn(&Path) -> Option<String>,
    fallback: impl Fn(&Path) -> Option<String>,
) -> Result<Remembered, String> {
    let (helper, done) = match plan(helper_config(cwd, &c.url(), env), || find(cwd), || fallback(cwd)) {
        Plan::Approve => return send(cwd, c, "approve", env).map(|()| Remembered::Helper),
        Plan::TurnedOff => return Ok(Remembered::TurnedOff),
        Plan::Missing => return Err(format!("no credential store is set up ({})", setup_hint(std::env::consts::OS))),
        Plan::SetUp(name) => (name.clone(), Remembered::SetUp(name)),
        Plan::Cache(helper) => (helper, Remembered::Cached),
    };
    let add = args(&["config", "--global", "--add", "credential.helper", &helper]);
    let out = gitcli::output(cwd, &add, "", &quiet_env(env), Some(gitcli::READ_TIMEOUT));
    if !out.ok {
        return Err(failure(&out, "could not set credential.helper"));
    }
    send(cwd, c, "approve", env).map(|()| done).inspect_err(|_| take_back(cwd, &helper, env))
}

/// Removes `helper` from the global config's `credential.helper`, that exact value only.
pub(crate) fn take_back(cwd: &Path, helper: &str, env: &[(&str, &str)]) {
    let unset = args(&["config", "--global", "--unset", "credential.helper", &exact_value(helper)]);
    let _ = gitcli::output(cwd, &unset, "", &quiet_env(env), Some(gitcli::READ_TIMEOUT));
}

/// Tells git's credential helpers that `c` was rejected, so they drop it.
pub fn forget(cwd: &Path, c: &HttpsCredential) -> Result<(), String> {
    forget_in(cwd, c, &[])
}

/// [`forget`] with `env` added to the git call.
pub(crate) fn forget_in(cwd: &Path, c: &HttpsCredential, env: &[(&str, &str)]) -> Result<(), String> {
    send(cwd, c, "reject", env)
}

/// Runs `git credential <action>` with `c` on stdin, wiping the text afterwards.
fn send(cwd: &Path, c: &HttpsCredential, action: &str, env: &[(&str, &str)]) -> Result<(), String> {
    let mut input = c.request(true);
    let out = gitcli::output(cwd, &args(&["credential", action]), &input, &quiet_env(env), Some(APPROVE_TIMEOUT));
    askpass::wipe(&mut input);
    if out.ok { Ok(()) } else { Err(failure(&out, &format!("git credential {action} failed"))) }
}

/// The last line git printed, or `fallback` when it printed nothing.
fn failure(out: &GitOutput, fallback: &str) -> String {
    let text = out.combined();
    text.lines().last().map(str::trim).filter(|l| !l.is_empty()).unwrap_or(fallback).to_owned()
}

#[cfg(test)]
#[path = "git_credential_tests.rs"]
mod tests;
