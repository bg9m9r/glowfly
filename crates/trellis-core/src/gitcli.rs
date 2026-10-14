//! Thin wrapper around the `git` executable, used for operations that should honour the
//! user's git configuration, hooks, signing and credential helpers (commit, fetch, push, ...).

use std::path::Path;
use std::process::{Command, Stdio};

/// Runs `git <args>` in `cwd`. Returns success and the combined stdout/stderr.
pub fn run(cwd: &Path, args: &[String]) -> (bool, String) {
    run_with_input(cwd, args, "", &[])
}

/// Like `run`, feeding `input` on stdin and setting extra environment variables. `env` is
/// applied after the fixed defaults (no terminal prompts, `true` as the editor and sequence
/// editor, no merge message editing), so a caller can override one of them for one command.
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
}
