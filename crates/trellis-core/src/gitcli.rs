//! Thin wrapper around the `git` executable, used for operations that should honour the
//! user's git configuration, hooks, signing and credential helpers (commit, fetch, push, ...).
//!
//! Once the app has started its credential-prompt listener ([`enable_askpass`]), every git
//! child also gets Trellis as its `GIT_ASKPASS` / `SSH_ASKPASS` program (see
//! [`crate::askpass`]), so prompts no credential helper answers open a dialog instead of
//! failing. A command that must never prompt passes [`NO_PROMPTS`] as its environment.

use std::ffi::OsString;
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

/// The environment that points git and ssh at the askpass helper. `SSH_ASKPASS_REQUIRE=force`
/// makes ssh use it even without a display.
pub fn askpass_vars(a: &AskpassEnv) -> Vec<(&'static str, OsString)> {
    vec![
        ("GIT_ASKPASS", a.exe.clone().into_os_string()),
        ("SSH_ASKPASS", a.exe.clone().into_os_string()),
        ("SSH_ASKPASS_REQUIRE", "force".into()),
        (crate::askpass::ENV_ADDR, a.addr.clone().into()),
        (crate::askpass::ENV_TOKEN, a.token.clone().into()),
    ]
}

/// Caller environment for a command that must fail rather than prompt (background jobs,
/// hosting-service sign-in). An empty `GIT_ASKPASS` also stops git from falling back to
/// `core.askpass` or `SSH_ASKPASS`.
pub const NO_PROMPTS: &[(&str, &str)] = &[("GIT_ASKPASS", ""), ("SSH_ASKPASS", ""), ("SSH_ASKPASS_REQUIRE", "never")];

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

/// Runs git as [`output`] describes, returning stdout undecoded next to the result, whose
/// `stdout` is left empty (stderr is decoded).
fn raw(cwd: &Path, args: &[String], input: &str, env: &[(&str, &str)], timeout: Option<Duration>) -> (GitOutput, Vec<u8>) {
    let mut cmd = Command::new("git");
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
        // Untranslated messages, so the text Trellis matches is the same in every locale.
        .env("LC_ALL", "C")
        .env("LANGUAGE", "C")
        .envs(ASKPASS.get().map(askpass_vars).unwrap_or_default())
        .envs(env.iter().copied());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return (GitOutput::failed(format!("failed to run git: {e}")), Vec::new()),
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
        Err(e) => return (GitOutput::failed(format!("failed to run git: {e}")), Vec::new()),
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
mod tests {
    use super::*;

    #[test]
    fn caller_env_overrides_the_defaults() {
        let dir = crate::paths::temp_dir();
        let (ok, out) = run_with_input(&dir, &args(&["var", "GIT_EDITOR"]), "", &[("GIT_EDITOR", "foo")]);
        assert!(ok, "{out}");
        assert_eq!(out.trim(), "foo");
        let (ok, out) = run(&dir, &args(&["var", "GIT_EDITOR"]));
        assert!(ok, "{out}");
        assert_eq!(out.trim(), "true", "other calls keep the default");
    }

    #[test]
    fn askpass_variables() {
        let a = AskpassEnv { exe: "C:/x/trellis.exe".into(), addr: "127.0.0.1:1".into(), token: "ab".into() };
        let vars = askpass_vars(&a);
        let get = |k: &str| vars.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string_lossy().into_owned());
        assert_eq!(get("GIT_ASKPASS").as_deref(), Some("C:/x/trellis.exe"));
        assert_eq!(get("SSH_ASKPASS").as_deref(), Some("C:/x/trellis.exe"));
        assert_eq!(get("SSH_ASKPASS_REQUIRE").as_deref(), Some("force"));
        assert_eq!(get("TRELLIS_ASKPASS").as_deref(), Some("127.0.0.1:1"));
        assert_eq!(get("TRELLIS_ASKPASS_TOKEN").as_deref(), Some("ab"));
    }

    #[test]
    fn no_prompts_fails_instead_of_asking() {
        let dir = crate::paths::temp_dir();
        let start = std::time::Instant::now();
        let a = args(&["-c", "credential.helper=", "credential", "fill"]);
        let (ok, out) = run_with_input(&dir, &a, "protocol=https\nhost=example.invalid\n\n", NO_PROMPTS);
        assert!(!ok, "{out}");
        assert!(start.elapsed() < std::time::Duration::from_secs(30));
    }

    #[test]
    fn stdout_and_stderr_stay_apart() {
        let dir = crate::paths::temp_dir();
        let out = output(&dir, &args(&["version"]), "", &[], None);
        assert!(out.ok && out.code == Some(0) && out.stdout.starts_with("git version") && out.stderr.is_empty(), "{out:?}");
        let out = output(&dir, &args(&["config", "--no-such-option"]), "", &[], None);
        assert!(!out.ok && out.stdout.is_empty() && !out.stderr.is_empty(), "{out:?}");
        assert_eq!(out.combined(), out.stderr.trim_end());
    }

    #[test]
    fn output_bytes_keeps_raw_stdout() {
        let dir = crate::paths::temp_dir();
        let a = args(&["-c", r"alias.b=!printf '\377\376'", "b"]);
        let (out, bytes) = output_bytes(&dir, &a, &[], None);
        assert!(out.ok, "{out:?}");
        assert_eq!(bytes, [0xFF, 0xFE]);
        assert!(out.stdout.is_empty(), "{out:?}");
    }

    #[test]
    fn messages_are_untranslated() {
        let dir = crate::paths::temp_dir();
        let env = |k: &str| {
            let a = args(&["-c", &format!("alias.e=!echo ${k}"), "e"]);
            output(&dir, &a, "", &[("LANG", "de_DE.UTF-8")], None).stdout
        };
        assert_eq!(env("LC_ALL").trim(), "C");
        assert_eq!(env("LANGUAGE").trim(), "C");
        assert_eq!(env("LANG").trim(), "de_DE.UTF-8", "the rest of the environment is kept");
    }

    #[test]
    fn large_input_does_not_deadlock() {
        // Several MB in and out: writing stdin before draining stdout would block both sides.
        let dir = crate::paths::temp_dir();
        let input = ("x".repeat(64) + "\n").repeat(100_000);
        let a = args(&["-c", "alias.cat=!cat", "cat"]);
        let out = output(&dir, &a, &input, &[], Some(Duration::from_secs(60)));
        assert!(out.ok, "{}", out.stderr);
        assert_eq!(out.stdout.len(), input.len());
    }

    #[test]
    fn reads_git_versions() {
        assert_eq!(
            parse_version(
                "git version 2.54.0.windows.1
"
            ),
            Some((2, 54, 0))
        );
        assert_eq!(parse_version("git version 2.39.3 (Apple Git-146)"), Some((2, 39, 3)));
        assert_eq!(parse_version("git version 2.38.0.rc1"), Some((2, 38, 0)));
        assert_eq!(parse_version("git version 2.40"), Some((2, 40, 0)));
        for bad in ["", "hello", "git version", "git version x.y.z", "git version 2"] {
            assert_eq!(parse_version(bad), None, "{bad}");
        }
        let v = version(&crate::paths::temp_dir()).expect("git runs");
        assert!((v.0, v.1) >= crate::conflict_check::MIN_GIT, "{v:?}");
    }

    #[test]
    fn timeout_kills_the_command() {
        let dir = crate::paths::temp_dir();
        let start = Instant::now();
        let a = args(&["-c", "alias.wait=!sleep 30", "wait"]);
        let out = output(&dir, &a, "", &[], Some(Duration::from_millis(300)));
        assert!(!out.ok && out.timed_out && out.code.is_none(), "{out:?}");
        assert!(out.stderr.contains("was stopped"), "{out:?}");
        assert!(start.elapsed() < Duration::from_secs(10), "{:?}", start.elapsed());
    }
}
