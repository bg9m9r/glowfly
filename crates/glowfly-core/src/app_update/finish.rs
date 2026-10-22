//! The update helper, `glowfly --finish-update <plan>`: a copy of the program that waits for
//! Glowfly to quit, runs the MSI if there is one, deletes the swapped program's backup and
//! starts Glowfly again.
//!
//! Glowfly holds the helper's stdin open: it writes "go" once everything is prepared, and the
//! pipe closes when Glowfly exits (no process ids, which get reused). End of input without
//! "go" means Glowfly gave up or crashed, so the helper does nothing. It prints nothing and
//! logs one line per step to `update.log` in the logs folder.

use super::plan::{MsiStep, Plan};
use crate::native_path::shown;
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// What Glowfly writes once the update is ready.
pub const GO: &[u8] = b"go\n";
/// How long the helper waits for [`GO`].
pub const GO_TIMEOUT: Duration = Duration::from_secs(30);
/// How long after [`GO`] the helper waits for Glowfly to exit.
pub const EXIT_TIMEOUT: Duration = Duration::from_secs(120);
/// The helper's log, in the logs folder.
pub const LOG_FILE: &str = "update.log";
const LOG_BYTES: u64 = 256 << 10;

/// How the wait for Glowfly ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wait {
    /// "go", then the end of input: Glowfly has exited.
    Exited,
    /// The input ended (or stopped coming) without "go".
    NoGo,
    /// "go", but Glowfly didn't exit in time.
    TimedOut,
}

/// Whether `bytes` (all the input so far) starts with the whole go signal.
pub fn go_received(bytes: &[u8]) -> bool {
    bytes.starts_with(GO)
}

enum Chunk {
    Got(Vec<u8>),
    Eof,
}

/// Reads `input` until it ends: "go" within `go`, then the end within `exit`.
pub fn wait_for_exit(input: impl Read + Send + 'static, go: Duration, exit: Duration) -> Wait {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut input = input;
        let mut buf = [0u8; 64];
        loop {
            match input.read(&mut buf) {
                Ok(0) | Err(_) => {
                    let _ = tx.send(Chunk::Eof);
                    return;
                }
                Ok(n) => {
                    if tx.send(Chunk::Got(buf[..n].to_vec())).is_err() {
                        return;
                    }
                }
            }
        }
    });
    let mut got = Vec::new();
    let deadline = Instant::now() + go;
    while !go_received(&got) {
        // Anything that can't become "go" is no signal.
        if !GO.starts_with(&got) {
            return Wait::NoGo;
        }
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Chunk::Got(b)) => got.extend(b),
            Ok(Chunk::Eof) | Err(_) => return Wait::NoGo,
        }
    }
    let deadline = Instant::now() + exit;
    loop {
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Chunk::Got(_)) => {}
            Ok(Chunk::Eof) | Err(mpsc::RecvTimeoutError::Disconnected) => return Wait::Exited,
            Err(mpsc::RecvTimeoutError::Timeout) => return Wait::TimedOut,
        }
    }
}

/// msiexec, from the system folder (never searched for), and its arguments: a passive install
/// (a progress bar, no questions) that never restarts the computer, with a verbose log.
pub fn msiexec_command(system_root: &Path, step: &MsiStep) -> (PathBuf, Vec<OsString>) {
    let program = system_root.join("System32").join("msiexec.exe");
    let args = vec!["/i".into(), step.file.clone().into(), "/passive".into(), "/norestart".into(), "/l*v".into(), step.log.clone().into()];
    (program, args)
}

/// How msiexec ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MsiOutcome {
    Installed,
    Cancelled,
    /// Another installation was running.
    Busy,
    Failed(Option<i32>),
}

/// msiexec's exit code read: success (also when a restart is needed or was started), the
/// user's cancel, another installation running, or a failure.
pub fn msi_outcome(code: Option<i32>) -> MsiOutcome {
    match code {
        Some(0 | 3010 | 1641) => MsiOutcome::Installed,
        Some(1602) => MsiOutcome::Cancelled,
        Some(1618) => MsiOutcome::Busy,
        other => MsiOutcome::Failed(other),
    }
}

/// Appends `line` to the update log (nothing without a logs folder).
fn log(line: &str) {
    use std::io::Write;
    let Some(dir) = crate::settings::log_dir() else { return };
    let mut file = crate::logs::RotatingFile::new(dir.join(LOG_FILE), LOG_BYTES, 1);
    let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let _ = file.write_all(format!("{stamp} {line}\n").as_bytes());
}

/// Runs the helper on the plan at `plan_path`, with Glowfly's pipe as `stdin`; the exit status.
pub fn run(plan_path: &Path, stdin: impl Read + Send + 'static) -> i32 {
    run_with(plan_path, stdin, (GO_TIMEOUT, EXIT_TIMEOUT), &mut |l| log(l))
}

fn run_with(plan_path: &Path, stdin: impl Read + Send + 'static, (go, exit): (Duration, Duration), log: &mut dyn FnMut(&str)) -> i32 {
    let plan = match Plan::read(plan_path) {
        Ok(p) => p,
        Err(e) => {
            log(&format!("not updating: {e}"));
            return 1;
        }
    };
    if plan.msi.is_some() && !cfg!(windows) {
        log("not updating: an MSI only installs on Windows");
        return 1;
    }
    match wait_for_exit(stdin, go, exit) {
        Wait::Exited => log(&format!("Glowfly has quit; installing {}", plan.version)),
        Wait::NoGo => {
            log("not updating: Glowfly didn't hand over the update");
            return 1;
        }
        Wait::TimedOut => {
            log(&format!("not updating: Glowfly didn't quit within {} s", exit.as_secs()));
            return 1;
        }
    }
    if let Some(step) = &plan.msi {
        run_msi(step, log);
    }
    for file in &plan.remove {
        match crate::persist::remove_file(file) {
            Ok(()) => log(&format!("removed {}", shown(file))),
            Err(e) => log(&format!("couldn't remove {}: {e}", shown(file))),
        }
    }
    match super::launch::start_detached(&plan.relaunch, &[], plan.relaunch.parent()) {
        Ok(()) => log(&format!("started {}", shown(&plan.relaunch))),
        Err(e) => log(&format!("couldn't start {}: {e}", shown(&plan.relaunch))),
    }
    // On Windows this program is still running from the folder, which Tidy removes later.
    if let Err(e) = crate::persist::remove_tree(&plan.staging) {
        log(&format!("left {} for later: {e}", shown(&plan.staging)));
    }
    0
}

fn run_msi(step: &MsiStep, log: &mut dyn FnMut(&str)) {
    let Some(root) = std::env::var_os("SystemRoot").filter(|r| !r.is_empty()).map(PathBuf::from) else {
        log("couldn't run the installer: SystemRoot isn't set");
        return;
    };
    let (program, args) = msiexec_command(&root, step);
    log(&format!("running {} on {}", shown(&program), shown(&step.file)));
    let outcome = match std::process::Command::new(&program).args(&args).status() {
        Ok(s) => msi_outcome(s.code()),
        Err(e) => {
            log(&format!("couldn't run the installer: {e}"));
            return;
        }
    };
    log(&match outcome {
        MsiOutcome::Installed => "the installer finished".to_owned(),
        MsiOutcome::Cancelled => "the installer was cancelled".to_owned(),
        MsiOutcome::Busy => "another installation is running; try the update again later".to_owned(),
        MsiOutcome::Failed(code) => format!("the installer failed ({code:?}); its log is {}", shown(&step.log)),
    });
}

#[cfg(test)]
#[path = "finish_tests.rs"]
mod tests;
