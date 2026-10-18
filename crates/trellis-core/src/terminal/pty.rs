//! Shell sessions on a pseudo-terminal: a running session with its output and exit reported
//! through callbacks on their own threads (terminal/shell.rs picks the shell).

use crate::error::{Error, Result};
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::io::{Read, Write};
use std::path::Path;

/// A program and its arguments.
#[derive(Clone, Debug, PartialEq)]
pub struct ShellCommand {
    pub program: String,
    pub args: Vec<String>,
}

/// A running shell. Dropping it closes the terminal. On Windows the output reader only ends
/// once this is dropped, so the owner drops it after the exit callback has run.
pub struct PtySession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
}

impl PtySession {
    /// Sends input to the program.
    pub fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.writer.write_all(bytes)?;
        self.writer.flush()
    }

    pub fn resize(&self, rows: u16, cols: u16) -> Result<()> {
        self.master.resize(size(rows, cols)).map_err(other)
    }

    /// A handle that stops the shell from any thread, so a stop never waits for a write that
    /// is blocked on a full input pipe.
    pub fn killer(&self) -> PtyKiller {
        PtyKiller(self.killer.clone_killer())
    }
}

/// Stops a session's shell (and, on Windows, everything attached to its console).
pub struct PtyKiller(Box<dyn ChildKiller + Send + Sync>);

impl PtyKiller {
    pub fn kill(&mut self) -> std::io::Result<()> {
        self.0.kill()
    }
}

fn size(rows: u16, cols: u16) -> PtySize {
    PtySize { rows: rows.max(1), cols: cols.max(1), pixel_width: 0, pixel_height: 0 }
}

/// portable-pty reports errors as `anyhow::Error`; `{:#}` keeps its whole cause chain.
fn other(e: impl std::fmt::Display) -> Error {
    Error::Other(format!("{e:#}"))
}

/// Starts `shell` in `cwd` on a new pseudo-terminal of `rows` × `cols`. `on_output` gets each
/// chunk the program writes (on a reader thread) and `on_exit` its exit code once it ends
/// (on a waiter thread; None if waiting failed).
pub fn spawn(
    cwd: &Path,
    rows: u16,
    cols: u16,
    shell: &ShellCommand,
    mut on_output: impl FnMut(Vec<u8>) + Send + 'static,
    on_exit: impl FnOnce(Option<u32>) + Send + 'static,
) -> Result<PtySession> {
    let pair = native_pty_system().openpty(size(rows, cols)).map_err(other)?;
    let mut cmd = CommandBuilder::new(&shell.program);
    cmd.args(&shell.args);
    cmd.cwd(cwd);
    // Only when it differs from the inherited PATH (macOS), so the builder's own copy of the
    // environment never has to match Windows' `Path` against `PATH`.
    if let Some(p) = crate::env_path::override_path() {
        cmd.env("PATH", p);
    }
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    let mut child = pair.slave.spawn_command(cmd).map_err(other)?;
    // The pseudoconsole only reports the end of output once nothing else holds the slave.
    drop(pair.slave);
    let killer = child.clone_killer();
    let mut reader = pair.master.try_clone_reader().map_err(other)?;
    let writer = pair.master.take_writer().map_err(other)?;
    std::thread::Builder::new().name("trellis-pty-read".into()).spawn(move || {
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => on_output(buf[..n].to_vec()),
            }
        }
    })?;
    std::thread::Builder::new().name("trellis-pty-wait".into()).spawn(move || {
        on_exit(child.wait().ok().map(|s| s.exit_code()));
    })?;
    Ok(PtySession { master: pair.master, writer, killer })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    #[test]
    fn spawns_a_command_and_reports_output_and_exit() {
        let dir = crate::paths::temp_dir().join(format!("trellis-pty-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let shell = if cfg!(windows) {
            ShellCommand { program: "cmd.exe".into(), args: vec!["/C".into(), "echo trellis-pty".into()] }
        } else {
            ShellCommand { program: "/bin/sh".into(), args: vec!["-c".into(), "printf trellis-pty".into()] }
        };
        let (out_tx, out_rx) = mpsc::channel();
        let (exit_tx, exit_rx) = mpsc::channel();
        let mut session = spawn(
            &dir,
            24,
            80,
            &shell,
            move |b| {
                let _ = out_tx.send(b);
            },
            move |code| {
                let _ = exit_tx.send(code);
            },
        )
        .expect("spawn");
        let deadline = Instant::now() + Duration::from_secs(20);
        // The screen answers the pseudoconsole's cursor position query, as the app does.
        let mut screen = crate::terminal::TermScreen::new(24, 80);
        let mut text = Vec::new();
        while !String::from_utf8_lossy(&text).contains("trellis-pty") {
            let left = deadline.saturating_duration_since(Instant::now());
            match out_rx.recv_timeout(left) {
                Ok(b) => {
                    let reply = screen.feed(&b);
                    if !reply.is_empty() {
                        session.write(&reply).expect("reply");
                    }
                    text.extend(b);
                }
                Err(e) => panic!("no output ({e}): {:?}", String::from_utf8_lossy(&text)),
            }
        }
        let code = exit_rx.recv_timeout(Duration::from_secs(20)).expect("exit");
        assert_eq!(code, Some(0));
        drop(session);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
