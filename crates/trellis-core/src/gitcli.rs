//! Thin wrapper around the `git` executable, used for operations that should honour the
//! user's git configuration, hooks, signing and credential helpers (commit, fetch, push, ...).
//!
//! Once the app has started its credential-prompt listener ([`enable_askpass`]), every git
//! child also gets Trellis as its `GIT_ASKPASS` / `SSH_ASKPASS` program (see
//! [`crate::askpass`]), so prompts no credential helper answers open a dialog instead of
//! failing. A command that must never prompt passes [`NO_PROMPTS`] as its environment.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

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

/// Runs `git <args>` in `cwd`. Returns success and the combined stdout/stderr.
pub fn run(cwd: &Path, args: &[String]) -> (bool, String) {
    run_with_input(cwd, args, "", &[])
}

/// Like `run`, feeding `input` on stdin and setting extra environment variables. `env` is
/// applied after the fixed defaults (no terminal prompts, `true` as the editor and sequence
/// editor, no merge message editing) and the askpass variables (once enabled), so a caller can
/// override one of them for one command, e.g. with [`NO_PROMPTS`].
pub fn run_with_input(cwd: &Path, args: &[String], input: &str, env: &[(&str, &str)]) -> (bool, String) {
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
        .envs(ASKPASS.get().map(askpass_vars).unwrap_or_default())
        .envs(env.iter().copied());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let output = cmd.spawn().and_then(|mut child| {
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin.write_all(input.as_bytes())?;
        }
        child.wait_with_output()
    });
    match output {
        Ok(out) => {
            let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
            text.push_str(&String::from_utf8_lossy(&out.stderr));
            (out.status.success(), text.trim_end().to_owned())
        }
        Err(e) => (false, format!("failed to run git: {e}")),
    }
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
        let dir = std::env::temp_dir();
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
        let dir = std::env::temp_dir();
        let start = std::time::Instant::now();
        let a = args(&["-c", "credential.helper=", "credential", "fill"]);
        let (ok, out) = run_with_input(&dir, &a, "protocol=https\nhost=example.invalid\n\n", NO_PROMPTS);
        assert!(!ok, "{out}");
        assert!(start.elapsed() < std::time::Duration::from_secs(30));
    }
}
