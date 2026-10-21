//! The real gpg runs and the real app: [`Real`] runs the user's program, [`EnvAsker`] asks
//! through the askpass listener gitcli pointed the wrapper at.

use super::ENV_WRAPPER;
use super::args;
use super::flow::{AskResult, Asker, Gpg, GpgRun};
use crate::askpass::{self, GpgAsk, Outcome};
use std::ffi::OsString;
use std::io::{self, Write};
use std::process::{Command, Stdio};

/// No console window for a child on Windows (the wrapper may itself have none).
pub(super) fn no_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

/// A command for the user's program, marked so a loop back into Glowfly stops. PATH and the
/// rest of the environment are git's, as gpg would have had them without Glowfly.
fn command(program: &str, args: &[OsString]) -> Command {
    let mut cmd = Command::new(program);
    cmd.args(args).env(ENV_WRAPPER, "nested");
    no_window(&mut cmd);
    cmd
}

/// The user's OpenPGP program.
pub struct Real {
    program: String,
}

impl Real {
    pub fn new(program: String) -> Self {
        Real { program }
    }
}

impl Gpg for Real {
    fn run(&mut self, args: &[OsString], input: &[u8]) -> io::Result<GpgRun> {
        let mut cmd = command(&self.program, args);
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn()?;
        let stdin = child.stdin.take();
        // Feed stdin on its own thread while both pipes drain, so a large payload can't leave
        // gpg and the wrapper each waiting for the other.
        let out = std::thread::scope(|s| {
            s.spawn(move || {
                if let Some(mut stdin) = stdin {
                    // gpg may exit without reading everything (a failure); that's fine.
                    let _ = stdin.write_all(input);
                }
            });
            child.wait_with_output()
        })?;
        Ok(GpgRun { ok: out.status.success(), code: out.status.code(), stdout: out.stdout, stderr: out.stderr })
    }

    fn describe(&mut self, key: Option<&str>) -> (String, Option<String>) {
        let Some(spec) = key.filter(|k| !k.is_empty()) else { return ("your signing key".into(), None) };
        let mut cmd = command(&self.program, &args::list_key(spec));
        cmd.stdin(Stdio::null()).stderr(Stdio::null());
        let listed = cmd.output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
        match listed.as_deref().map(|t| crate::signing::parse_gpg_colons(t, now)).and_then(|keys| keys.into_iter().next()) {
            Some(k) => (k.id, Some(k.label).filter(|l| !l.is_empty())),
            None => (spec.to_owned(), None),
        }
    }

    fn program(&self) -> &str {
        &self.program
    }
}

/// Asks the app whose listener the environment names.
pub struct EnvAsker;

impl Asker for EnvAsker {
    fn ask(&mut self, ask: &GpgAsk) -> AskResult {
        match askpass::ask_gpg_from_env(ask) {
            Ok(Outcome::Answer(s)) => AskResult::Answer(s),
            Ok(Outcome::Cancelled) => AskResult::Cancelled,
            Ok(Outcome::TimedOut) => AskResult::TimedOut,
            Ok(Outcome::UseSystem) => AskResult::UseSystem,
            Err(_) => AskResult::Unavailable,
        }
    }
}

/// Runs `program` with `args` and the wrapper's own stdin, stdout and stderr (a verification,
/// or signing without the app). Returns its exit code, or 2 when it couldn't be run.
pub fn passthrough(program: &str, args: &[OsString]) -> i32 {
    match command(program, args).status() {
        Ok(status) => status.code().unwrap_or(2),
        Err(e) => {
            eprintln!("Glowfly: could not run {program}: {e}");
            2
        }
    }
}
