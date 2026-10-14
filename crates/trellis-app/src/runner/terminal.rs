//! The built-in terminal's shells. Each session has a thread that owns its PTY and carries out
//! the writes, resizes and kills update asks for, in order, so a full PTY input pipe can never
//! block the UI thread. Requests sent while the shell is still starting wait in its channel.

use super::{Runner, Waker};
use crate::msg::Msg;
use crate::terminal::{TerminalEffect, TerminalMsg};
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, MutexGuard};
use trellis_core::terminal::pty;

/// A request for a session's thread.
pub(super) enum Control {
    Write(Vec<u8>),
    Resize(u16, u16),
    Kill,
    /// The shell has ended: close the PTY and stop.
    Closed,
}

/// The running sessions' channels by id. Dropping it (the app quitting) stops every shell.
#[derive(Default)]
pub(super) struct Terminals(Mutex<HashMap<u64, Sender<Control>>>);

impl Terminals {
    fn map(&self) -> MutexGuard<'_, HashMap<u64, Sender<Control>>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Sends `c` to session `id`; a session that is gone is ignored.
    fn send(&self, id: u64, c: Control) {
        if let Some(ctl) = self.map().get(&id) {
            let _ = ctl.send(c);
        }
    }
}

impl Drop for Terminals {
    fn drop(&mut self) {
        // The OS closes the pseudoconsoles at exit anyway; this asks the shells to stop first.
        for ctl in self.map().values() {
            let _ = ctl.send(Control::Kill);
        }
    }
}

/// Sends a session's results to update and wakes the UI.
#[derive(Clone)]
struct Reporter<W> {
    tx: Sender<Msg>,
    waker: W,
}

impl<W: Waker> Reporter<W> {
    fn send(&self, m: TerminalMsg) {
        if self.tx.send(Msg::Terminal(m)).is_ok() {
            self.waker.wake();
        }
    }
}

impl<W: Waker + Clone> Runner<W> {
    pub(super) fn terminal(&self, e: TerminalEffect) {
        match e {
            TerminalEffect::Spawn { id, cwd, rows, cols } => {
                let (ctl, rx) = mpsc::channel();
                self.terminals.map().insert(id, ctl.clone());
                let report = Reporter { tx: self.tx.clone(), waker: self.waker.clone() };
                let failed = report.clone();
                let spawned = std::thread::Builder::new()
                    .name("trellis-terminal".into())
                    .spawn(move || session(id, &cwd, (rows, cols), rx, ctl, report));
                if let Err(e) = spawned {
                    self.terminals.map().remove(&id);
                    failed.send(TerminalMsg::SpawnFailed { id, error: format!("could not start a thread: {e}") });
                }
            }
            TerminalEffect::Write { id, bytes } => self.terminals.send(id, Control::Write(bytes)),
            TerminalEffect::Resize { id, rows, cols } => self.terminals.send(id, Control::Resize(rows, cols)),
            TerminalEffect::Kill(id) => self.terminals.send(id, Control::Kill),
            TerminalEffect::Close(id) => {
                self.terminals.map().remove(&id);
            }
        }
    }
}

/// Runs session `id`: starts the shell, reports it, then carries out requests until the shell
/// has ended. Returning drops the PTY, which ends its output reader.
fn session<W: Waker + Clone>(
    id: u64,
    cwd: &Path,
    (rows, cols): (u16, u16),
    rx: Receiver<Control>,
    ctl: Sender<Control>,
    report: Reporter<W>,
) {
    let shell = pty::default_shell();
    let (out, exit) = (report.clone(), report.clone());
    let spawned = pty::spawn(
        cwd,
        rows,
        cols,
        &shell,
        move |bytes| out.send(TerminalMsg::Output { id, bytes }),
        move |code| {
            exit.send(TerminalMsg::Exited { id, code });
            let _ = ctl.send(Control::Closed);
        },
    );
    let mut pty = match spawned {
        Ok(p) => p,
        Err(e) => return report.send(TerminalMsg::SpawnFailed { id, error: e.to_string() }),
    };
    report.send(TerminalMsg::Started { id, program: short_name(&shell.program) });
    for c in rx {
        let result = match c {
            Control::Write(bytes) => pty.write(&bytes).map_err(|e| e.to_string()),
            Control::Resize(rows, cols) => pty.resize(rows, cols).map_err(|e| e.to_string()),
            Control::Kill => pty.kill().map_err(|e| e.to_string()),
            Control::Closed => break,
        };
        if let Err(e) = result {
            tracing::debug!("terminal session {id}: {e}");
        }
    }
}

/// The program's file name without its extension: "pwsh" for "pwsh.exe", "zsh" for "/bin/zsh".
fn short_name(program: &str) -> String {
    Path::new(program).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| program.to_owned())
}

#[cfg(test)]
mod tests {
    use super::short_name;

    #[test]
    fn short_names() {
        assert_eq!(short_name("pwsh.exe"), "pwsh");
        assert_eq!(short_name("/bin/zsh"), "zsh");
        assert_eq!(short_name(r"C:\Windows\system32\cmd.exe"), if cfg!(windows) { "cmd" } else { r"C:\Windows\system32\cmd" });
    }
}
