//! Thin wrapper around the `git` executable, used for operations that should honour the
//! user's git configuration, hooks, signing and credential helpers (commit, fetch, push, ...).
//! The program run is [`crate::git_program::program`]: the git chosen in Preferences > Git, else
//! `git` from the PATH children get.
//!
//! Once the app has started its credential-prompt listener ([`enable_askpass`]), every git
//! child also gets Trellis as its `GIT_ASKPASS` / `SSH_ASKPASS` program (see
//! [`crate::askpass`]), so prompts no credential helper answers open a dialog instead of
//! failing, and Trellis as its `gpg.program` (see [`crate::gpg_prompt`]), so signing
//! passphrases are asked the same way. A command that must never prompt passes [`NO_PROMPTS`]
//! (or [`BACKGROUND`]) as its environment, which also leaves `gpg.program` alone. With OpenSSH
//! before 8.4 on Unix, children also start in a new session ([`crate::ssh_version::prepare`]).
//!
//! Every command run here is reported, once it ends, to the observer the app shell sets at
//! startup ([`observe_runs`]), which writes slow ones to the performance log.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// What git children need to reach the app's credential prompts.
#[derive(Clone)]
pub struct AskpassEnv {
    /// The Trellis executable, run by git and ssh as the askpass program.
    pub exe: PathBuf,
    /// The listener's `127.0.0.1:<port>`.
    pub addr: String,
    pub token: String,
}

/// Set once by the app shell at startup and never changed (never set in tests).
static ASKPASS: OnceLock<AskpassEnv> = OnceLock::new();

/// Makes every later git child prompt through the app. Returns false if it was already set.
pub fn enable_askpass(env: AskpassEnv) -> bool {
    ASKPASS.set(env).is_ok()
}

/// One git command that ended, for the observer set with [`observe_runs`].
#[derive(Clone, Debug, PartialEq)]
pub struct GitRun {
    pub cwd: PathBuf,
    pub args: Vec<String>,
    pub took: Duration,
    /// The exit code; None when git couldn't start, was killed or timed out.
    pub code: Option<i32>,
    /// Git started and exited by itself, with any exit code: false only when it couldn't
    /// start, was killed or timed out.
    pub ok: bool,
    pub timed_out: bool,
}

type Observer = Box<dyn Fn(&GitRun) + Send + Sync>;

/// Set once by the app shell at startup and never changed (never set in tests), like
/// [`ASKPASS`].
static OBSERVER: OnceLock<Observer> = OnceLock::new();

/// Reports every later git command to `f` once it ends (on the thread that ran it, so `f` must
/// be quick). Returns false if an observer was already set.
pub fn observe_runs(f: Observer) -> bool {
    OBSERVER.set(f).is_ok()
}

/// The environment that points git and ssh at the askpass helper. `SSH_ASKPASS_REQUIRE=force`
/// (OpenSSH 8.4 or newer) makes ssh use it even without a display. Older OpenSSH ignores that
/// and asks the helper only when `DISPLAY` is set and there is no terminal, so with such a
/// version ([`crate::ssh_version::legacy_askpass`]) an unset `DISPLAY` gets a placeholder.
pub fn askpass_vars(a: &AskpassEnv) -> Vec<(&'static str, OsString)> {
    askpass_vars_for(a, crate::ssh_version::legacy_askpass(), std::env::var_os("DISPLAY").as_deref())
}

/// [`askpass_vars`] for an ssh that is `legacy` (before 8.4) and the inherited `display`.
pub fn askpass_vars_for(a: &AskpassEnv, legacy: bool, display: Option<&OsStr>) -> Vec<(&'static str, OsString)> {
    let mut vars = vec![
        ("GIT_ASKPASS", a.exe.clone().into_os_string()),
        ("SSH_ASKPASS", a.exe.clone().into_os_string()),
        ("SSH_ASKPASS_REQUIRE", "force".into()),
        (crate::askpass::ENV_ADDR, a.addr.clone().into()),
        (crate::askpass::ENV_TOKEN, a.token.clone().into()),
    ];
    if let Some(d) = display_placeholder(display, legacy) {
        vars.push(("DISPLAY", d.into()));
    }
    vars
}

/// [`askpass_vars`] once the app has enabled the helper; empty before that (and in tests), so a
/// child given only these can't prompt through the app.
pub fn askpass_env() -> Vec<(&'static str, OsString)> {
    ASKPASS.get().map(askpass_vars).unwrap_or_default()
}

/// Trellis as git's gpg.program ([`crate::gpg_prompt::env::vars`]) once the helper is enabled,
/// for a command that may prompt (`env` isn't [`NO_PROMPTS`]).
fn gpg_env(env: &[(&str, &str)]) -> Vec<(&'static str, OsString)> {
    use crate::gpg_prompt::env::{allows_prompts, vars};
    let inherited = std::env::var_os("GIT_CONFIG_COUNT");
    ASKPASS
        .get()
        .filter(|_| allows_prompts(env))
        .map(|a| vars(&a.exe, inherited.as_deref(), &crate::git_program::program()))
        .unwrap_or_default()
}

/// The `DISPLAY` to set for ssh children so OpenSSH before 8.4 (`legacy`, Unix only) uses the
/// askpass helper: `:0` when `display` is unset or empty, else None. ssh only checks that it is
/// set; nothing connects to it unless X11 forwarding is configured.
pub fn display_placeholder(display: Option<&OsStr>, legacy: bool) -> Option<&'static str> {
    (legacy && display.is_none_or(|d| d.is_empty())).then_some(":0")
}

/// Caller environment for a command that must fail rather than prompt (background jobs,
/// hosting-service sign-in). An empty `GIT_ASKPASS` also stops git from falling back to
/// `core.askpass` or `SSH_ASKPASS`.
pub const NO_PROMPTS: &[(&str, &str)] = &[("GIT_ASKPASS", ""), ("SSH_ASKPASS", ""), ("SSH_ASKPASS_REQUIRE", "never")];

/// [`NO_PROMPTS`] for background jobs, which also keeps Git Credential Manager from opening its
/// own sign-in window.
pub const BACKGROUND: &[(&str, &str)] =
    &[("GIT_ASKPASS", ""), ("SSH_ASKPASS", ""), ("SSH_ASKPASS_REQUIRE", "never"), ("GCM_INTERACTIVE", "never")];

/// What a finished git command printed. `stdout` and `stderr` are kept apart so a caller that
/// parses output reads stdout only; job logs show [`GitOutput::combined`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GitOutput {
    pub ok: bool,
    /// The exit code; None when git could not be started, was killed or timed out.
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

impl GitOutput {
    /// stdout then stderr, trailing whitespace trimmed (what the job log shows).
    pub fn combined(&self) -> String {
        let mut text = self.stdout.clone();
        text.push_str(&self.stderr);
        text.trim_end().to_owned()
    }

    fn failed(message: String) -> Self {
        Self { stderr: message, ..Self::default() }
    }
}

/// How long a quick local read ([`read`]) may take before it is killed.
pub const READ_TIMEOUT: Duration = Duration::from_secs(30);

/// Runs `git <args>` in `cwd`. Returns success and the combined stdout/stderr.
pub fn run(cwd: &Path, args: &[String]) -> (bool, String) {
    run_with_input(cwd, args, "", &[])
}

/// Like `run`, feeding `input` on stdin and setting extra environment variables (see
/// [`output`]). Returns success and the combined stdout/stderr.
pub fn run_with_input(cwd: &Path, args: &[String], input: &str, env: &[(&str, &str)]) -> (bool, String) {
    let out = output(cwd, args, input, env, None);
    (out.ok, out.combined())
}

/// A quick local read whose stdout the caller parses (config values, `rev-parse`, versions):
/// never prompts and is killed after [`READ_TIMEOUT`]. Not for network or long-running jobs.
pub fn read(cwd: &Path, args: &[String]) -> GitOutput {
    output(cwd, args, "", NO_PROMPTS, Some(READ_TIMEOUT))
}

/// Runs `git <args>` in `cwd`, feeding `input` on stdin, and keeps stdout and stderr apart.
/// `env` is applied after the fixed defaults (no terminal prompts, `true` as the editor and
/// sequence editor, no merge message editing, untranslated messages) and the askpass variables
/// (once enabled), so a caller can override one of them for one command, e.g. with
/// [`NO_PROMPTS`]. With a `timeout`, the child is killed once it runs that long.
pub fn output(cwd: &Path, args: &[String], input: &str, env: &[(&str, &str)], timeout: Option<Duration>) -> GitOutput {
    let (mut out, stdout) = raw(cwd, args, input, env, timeout);
    out.stdout = String::from_utf8_lossy(&stdout).into_owned();
    out
}

/// Like [`output`], with stdout kept as raw bytes (patches, blobs); [`GitOutput::stdout`] is
/// empty. Nothing is fed on stdin.
pub fn output_bytes(cwd: &Path, args: &[String], env: &[(&str, &str)], timeout: Option<Duration>) -> (GitOutput, Vec<u8>) {
    raw(cwd, args, "", env, timeout)
}

/// [`output`] with `program` run instead of the chosen git (checking a program before it is
/// chosen, [`crate::git_program::probe`]).
pub fn output_with(
    program: &OsStr,
    cwd: &Path,
    args: &[String],
    input: &str,
    env: &[(&str, &str)],
    timeout: Option<Duration>,
) -> GitOutput {
    let (mut out, stdout) = raw_with(program, cwd, args, input, env, timeout);
    out.stdout = String::from_utf8_lossy(&stdout).into_owned();
    out
}

/// What [`GitOutput::stderr`] starts with when the program couldn't be started or waited for.
pub(crate) const SPAWN_FAILED: &str = "failed to run git: ";

/// Runs the chosen git as [`output`] describes, returning stdout undecoded next to the result,
/// whose `stdout` is left empty (stderr is decoded).
fn raw(cwd: &Path, args: &[String], input: &str, env: &[(&str, &str)], timeout: Option<Duration>) -> (GitOutput, Vec<u8>) {
    raw_with(&crate::git_program::program(), cwd, args, input, env, timeout)
}

/// [`raw`] running `program`. Reports the run to the observer, if one is set, exactly once
/// whichever way it ends.
fn raw_with(
    program: &OsStr,
    cwd: &Path,
    args: &[String],
    input: &str,
    env: &[(&str, &str)],
    timeout: Option<Duration>,
) -> (GitOutput, Vec<u8>) {
    let start = Instant::now();
    let result = raw_inner(program, cwd, args, input, env, timeout);
    if let Some(observe) = OBSERVER.get() {
        let out = &result.0;
        observe(&GitRun {
            cwd: cwd.to_owned(),
            args: args.to_vec(),
            took: start.elapsed(),
            code: out.code,
            ok: out.code.is_some() && !out.timed_out,
            timed_out: out.timed_out,
        });
    }
    result
}

fn raw_inner(
    program: &OsStr,
    cwd: &Path,
    args: &[String],
    input: &str,
    env: &[(&str, &str)],
    timeout: Option<Duration>,
) -> (GitOutput, Vec<u8>) {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .current_dir(cwd)
        .stdin(if input.is_empty() { Stdio::null() } else { Stdio::piped() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Never block on an interactive prompt or editor: there is no terminal to answer it.
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_EDITOR", "true")
        .env("GIT_SEQUENCE_EDITOR", "true")
        .env("GIT_MERGE_AUTOEDIT", "no")
        // Untranslated messages, so the text Trellis matches is the same in every locale. Only
        // the messages: `LC_ALL=C` would also give hooks, gpg and pinentry an ASCII-only
        // character set. gettext reads `LANGUAGE` first whenever the message locale isn't C,
        // so this holds even when the user sets `LC_ALL`.
        .env("LANGUAGE", "C")
        .env("LC_MESSAGES", "C")
        .envs(askpass_env())
        .envs(gpg_env(env))
        .envs(env.iter().copied());
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
        Err(e) => return (GitOutput::failed(format!("{SPAWN_FAILED}{e}")), Vec::new()),
    };
    // Feed stdin and drain both pipes on their own threads, so a large input or output can't
    // leave git and Trellis each waiting for the other.
    if let Some(mut stdin) = child.stdin.take() {
        let input = input.as_bytes().to_vec();
        std::thread::spawn(move || {
            use std::io::Write;
            // A command that exits without reading all of stdin closes the pipe; that's fine.
            let _ = stdin.write_all(&input);
        });
    }
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let (status, timed_out) = match wait(&mut child, timeout) {
        Ok(s) => s,
        Err(e) => return (GitOutput::failed(format!("{SPAWN_FAILED}{e}")), Vec::new()),
    };
    // After a kill, a grandchild (a hook, ssh) may still hold the pipes; don't wait on it.
    let grace = timed_out.then_some(Duration::from_secs(1));
    let stdout = collect(&stdout, grace);
    let mut out = GitOutput {
        ok: status.is_some_and(|s| s.success()),
        code: status.and_then(|s| s.code()),
        stdout: String::new(),
        stderr: String::from_utf8_lossy(&collect(&stderr, grace)).into_owned(),
        timed_out,
    };
    if timed_out {
        let secs = timeout.unwrap_or_default().as_secs_f32();
        out.stderr.push_str(&format!("\ngit {} took more than {secs}s and was stopped", args.join(" ")));
    }
    (out, stdout)
}

type Drained = std::sync::mpsc::Receiver<Vec<u8>>;

/// Reads `pipe` to the end on a new thread.
fn drain(pipe: Option<impl std::io::Read + Send + 'static>) -> Drained {
    let (tx, rx) = std::sync::mpsc::channel();
    if let Some(mut pipe) = pipe {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = pipe.read_to_end(&mut buf);
            let _ = tx.send(buf);
        });
    }
    rx
}

fn collect(rx: &Drained, grace: Option<Duration>) -> Vec<u8> {
    match grace {
        Some(d) => rx.recv_timeout(d).unwrap_or_default(),
        None => rx.recv().unwrap_or_default(),
    }
}

/// Waits for `child`, killing it once `timeout` passes. Returns its status (None if killed)
/// and whether it timed out.
fn wait(child: &mut Child, timeout: Option<Duration>) -> std::io::Result<(Option<ExitStatus>, bool)> {
    let Some(timeout) = timeout else { return child.wait().map(|s| (Some(s), false)) };
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(s) = child.try_wait()? {
            return Ok((Some(s), false));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Ok((None, true));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// The installed git's version (`git version`), or None when git can't be run or prints
/// something unexpected.
pub fn version(cwd: &Path) -> Option<(u32, u32, u32)> {
    parse_version(&read(cwd, &args(&["version"])).stdout)
}

/// Reads "git version 2.54.0.windows.1", "git version 2.39.3 (Apple Git-146)" or
/// "git version 2.38.0.rc1" as (major, minor, patch); a missing patch number is 0.
pub fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let rest = s.trim().strip_prefix("git version ")?;
    let word = rest.split_whitespace().next()?;
    let mut parts = word.split('.');
    let mut number = |required: bool| match parts.next().map(|p| p.parse::<u32>()) {
        Some(Ok(n)) => Some(n),
        _ if required => None,
        _ => Some(0),
    };
    Some((number(true)?, number(true)?, number(false)?))
}

/// Converts a list of string slices into an owned argument vector.
pub fn args(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

#[cfg(test)]
#[path = "gitcli_tests.rs"]
mod tests;
