//! Thin wrapper around the `git` executable, used for operations that should honour the
//! user's git configuration, hooks, signing and credential helpers (commit, fetch, push, ...).
//! The program run is [`crate::git_program::program`]: the git chosen in Preferences > Git, else
//! `git` from the PATH children get.
//!
//! Once the app has started its credential-prompt listener ([`enable_askpass`]), every git
//! child also gets Glowfly as its `GIT_ASKPASS` / `SSH_ASKPASS` program (see
//! [`crate::askpass`]), so prompts no credential helper answers open a dialog instead of
//! failing, and Glowfly as its `gpg.program` (see [`crate::gpg_prompt`]), so signing
//! passphrases are asked the same way, and, for SSH-format signing, the key file ssh-keygen will
//! ask about (`GLOWFLY_SSH_SIGNING_KEY`). A command that must never prompt passes [`NO_PROMPTS`]
//! (or [`BACKGROUND`]) as its environment, which also leaves `gpg.program` alone. On Unix every
//! child starts in a new session ([`crate::ssh_version::prepare`]), so ssh asks the helper.
//!
//! Every command run here is reported, once it ends, to the observer the app shell sets at
//! startup ([`observe_runs`]), which writes slow ones to the performance log.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

mod askpass_vars;
mod wait;
pub use askpass_vars::{askpass_env, askpass_env_in, askpass_vars, askpass_vars_for, askpass_vars_in, display_placeholder};
pub use wait::Stop;

/// What git children need to reach the app's credential prompts.
#[derive(Clone)]
pub struct AskpassEnv {
    /// The Glowfly executable, run by git and ssh as the askpass program.
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

/// The listener [`enable_askpass`] set, for the app's own gpg runs (`gpg_prompt::extend`).
pub(crate) fn askpass_listener() -> Option<&'static AskpassEnv> {
    ASKPASS.get()
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
    /// It was stopped through a [`Stop`] ([`output_until`]).
    pub stopped: bool,
    /// The stop needed a hard kill: git didn't exit within the grace period (or Windows).
    pub forced: bool,
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
    let (mut out, stdout) = raw_with(program, cwd, args, input, env, timeout, None);
    out.stdout = String::from_utf8_lossy(&stdout).into_owned();
    out
}

/// [`output`] without a timeout that stops git, and every process it started, once `stop` is
/// requested ([`crate::process_tree`]): the result then has `stopped` set and is not `ok`. A
/// stop requested before the call returns at once without starting git. Only for commands
/// whose interruption leaves the repository consistent (fetch, push, clone; never a merge or
/// rebase); the caller removes the lock and temporary files a stopped one leaves
/// ([`crate::stop_cleanup`], [`crate::clone_stop`]).
pub fn output_until(cwd: &Path, args: &[String], input: &str, env: &[(&str, &str)], stop: &Stop) -> GitOutput {
    output_watching(cwd, args, input, env, stop, &mut |_| {})
}

/// [`output_until`], handing each piece of stderr to `watch` as git writes it (on the calling
/// thread, while it waits), for its `--progress` lines ([`crate::git_progress`]).
pub fn output_watching(
    cwd: &Path,
    args: &[String],
    input: &str,
    env: &[(&str, &str)],
    stop: &Stop,
    watch: &mut dyn FnMut(&[u8]),
) -> GitOutput {
    let program = crate::git_program::program();
    let (mut out, stdout) = raw_with(&program, cwd, args, input, env, None, Some((stop, watch)));
    out.stdout = String::from_utf8_lossy(&stdout).into_owned();
    out
}

/// The git subcommand `args` runs: the first argument that isn't a global option (`-c k=v` and
/// `-C dir` take the next argument; `--git-dir=…`, `--no-pager` and any other leading `-…`
/// are skipped).
pub fn subcommand(args: &[String]) -> Option<&str> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-c" | "-C" => {
                it.next();
            }
            s if s.starts_with('-') => {}
            s => return Some(s),
        }
    }
    None
}

/// A stoppable run's stop and the watcher of its stderr (`raw_with`).
type Until<'a> = Option<(&'a Stop, &'a mut dyn FnMut(&[u8]))>;

/// What [`GitOutput::stderr`] starts with when the program couldn't be started or waited for.
pub(crate) const SPAWN_FAILED: &str = "failed to run git: ";

/// Runs the chosen git as [`output`] describes, returning stdout undecoded next to the result,
/// whose `stdout` is left empty (stderr is decoded).
fn raw(cwd: &Path, args: &[String], input: &str, env: &[(&str, &str)], timeout: Option<Duration>) -> (GitOutput, Vec<u8>) {
    raw_with(&crate::git_program::program(), cwd, args, input, env, timeout, None)
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
    stop: Until<'_>,
) -> (GitOutput, Vec<u8>) {
    let start = Instant::now();
    let result = raw_inner(program, cwd, args, input, env, timeout, stop);
    if let Some(observe) = OBSERVER.get() {
        let out = &result.0;
        observe(&GitRun {
            cwd: cwd.to_owned(),
            args: args.to_vec(),
            took: start.elapsed(),
            code: out.code,
            ok: out.code.is_some() && !out.timed_out && !out.stopped,
            timed_out: out.timed_out,
        });
    }
    result
}

/// `git <args>` (the chosen git) in `cwd` with gitcli's environment (see [`output`]), for a git
/// process the caller starts and watches itself, such as a diff tool left running. Its run isn't
/// reported to the observer ([`observe_runs`]). Its input and output are left to the caller.
pub fn command(cwd: &Path, args: &[String], env: &[(&str, &str)]) -> Command {
    base_command(&crate::git_program::program(), cwd, args, env, false)
}

/// `program <args>` in `cwd` with the fixed environment, the askpass variables and `env`. On
/// Unix every command starts in its own session and process group, so ssh asks the helper; a
/// `stoppable` one is also set up so a stop reaches everything it starts ([`crate::process_tree`]).
fn base_command(program: &OsStr, cwd: &Path, args: &[String], env: &[(&str, &str)], stoppable: bool) -> Command {
    let mut cmd = Command::new(program);
    // merge's state restore and stash --index feed git apply on stdin; its whitespace warnings
    // are noise in a job log.
    cmd.args(crate::git_errors::with_quiet_apply(args).iter())
        .current_dir(cwd)
        // Never block on an interactive prompt or editor: there is no terminal to answer it.
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_EDITOR", "true")
        .env("GIT_SEQUENCE_EDITOR", "true")
        .env("GIT_MERGE_AUTOEDIT", "no")
        // Untranslated messages, so the text Glowfly matches is the same in every locale. Only
        // the messages: `LC_ALL=C` would also give hooks, gpg and pinentry an ASCII-only
        // character set. gettext reads `LANGUAGE` first whenever the message locale isn't C,
        // so this holds even when the user sets `LC_ALL`.
        .env("LANGUAGE", "C")
        .env("LC_MESSAGES", "C")
        .envs(askpass_env_in(cwd, env))
        .envs(crate::gpg_prompt::env::for_command(cwd, args, env, ASKPASS.get()))
        .envs(env.iter().copied());
    crate::env_path::apply(&mut cmd);
    if stoppable {
        crate::process_tree::prepare(&mut cmd);
    } else {
        crate::ssh_version::prepare(&mut cmd);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn raw_inner(
    program: &OsStr,
    cwd: &Path,
    args: &[String],
    input: &str,
    env: &[(&str, &str)],
    timeout: Option<Duration>,
    stop: Until<'_>,
) -> (GitOutput, Vec<u8>) {
    if stop.as_ref().is_some_and(|(s, _)| s.requested()) {
        let stderr = format!("git {} was cancelled before it started", args.join(" "));
        return (GitOutput { stderr, stopped: true, ..GitOutput::default() }, Vec::new());
    }
    let mut cmd = base_command(program, cwd, args, env, stop.is_some());
    cmd.stdin(if input.is_empty() { Stdio::null() } else { Stdio::piped() }).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return (GitOutput::failed(format!("{SPAWN_FAILED}{e}")), Vec::new()),
    };
    // Feed stdin and drain both pipes on their own threads, so a large input or output can't
    // leave git and Glowfly each waiting for the other.
    if let Some(mut stdin) = child.stdin.take() {
        let input = input.as_bytes().to_vec();
        std::thread::spawn(move || {
            use std::io::Write;
            // A command that exits without reading all of stdin closes the pipe; that's fine.
            let _ = stdin.write_all(&input);
        });
    }
    let tree = stop.as_ref().map(|_| crate::process_tree::attach(&child));
    let stdout = wait::drain(child.stdout.take());
    // A stoppable command's stderr is watched as it arrives.
    let mut stderr = wait::Streamed::start(child.stderr.take());
    let (stop, mut watch) = match stop {
        Some((s, w)) => (Some(s), Some(w)),
        None => (None, None),
    };
    let mut tick = || {
        if let Some(w) = watch.as_deref_mut() {
            stderr.poll(w);
        }
    };
    let waited = match wait::wait(&mut child, timeout, stop.zip(tree.as_ref()), &mut tick) {
        Ok(w) => w,
        Err(e) => return (GitOutput::failed(format!("{SPAWN_FAILED}{e}")), Vec::new()),
    };
    let (status, timed_out, stopped) = (waited.status, waited.timed_out, waited.stopped);
    // After a kill, a grandchild (a hook, ssh) may still hold the pipes; don't wait on it.
    let grace = (timed_out || stopped).then_some(Duration::from_secs(1));
    let stdout = wait::collect(&stdout, grace);
    let mut out = GitOutput {
        ok: status.is_some_and(|s| s.success()),
        code: status.and_then(|s| s.code()),
        stdout: String::new(),
        stderr: String::from_utf8_lossy(&stderr.finish(grace)).into_owned(),
        timed_out,
        stopped,
        forced: waited.forced,
    };
    if stopped {
        out.stderr.push_str(&format!("\ngit {} was cancelled", args.join(" ")));
    }
    if timed_out {
        let secs = timeout.unwrap_or_default().as_secs_f32();
        out.stderr.push_str(&format!("\ngit {} took more than {secs}s and was stopped", args.join(" ")));
    }
    (out, stdout)
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
