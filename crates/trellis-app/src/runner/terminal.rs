//! The built-in terminal's shells. Each session has a thread that owns its PTY and carries out
//! the writes and resizes update asks for, in order, so a full PTY input pipe can never block
//! the UI thread. Requests sent while the shell is still starting wait in its channel. Kills
//! bypass that queue: a write stuck on a shell that isn't reading must not hold up a stop.

use super::{Runner, Waker};
use crate::msg::Msg;
use crate::terminal::{TerminalEffect, TerminalMsg};
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use trellis_core::terminal::pty;

/// A request for a session's thread.
pub(super) enum Control {
    Write(Vec<u8>),
    Resize(u16, u16),
    /// The shell has ended: close the PTY and stop.
    Closed,
}

type KillFn = Box<dyn FnMut() -> std::io::Result<()> + Send>;

/// Stops a session's shell from whichever thread asks. A stop asked for before the shell has
/// started is carried out as soon as it starts.
#[derive(Default)]
struct KillSwitch(Mutex<KillState>);

#[derive(Default)]
struct KillState {
    kill: Option<KillFn>,
    requested: bool,
}

impl KillSwitch {
    fn state(&self) -> MutexGuard<'_, KillState> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The shell has started and `kill` stops it.
    fn arm(&self, kill: KillFn) {
        let mut s = self.state();
        s.kill = Some(kill);
        if s.requested {
            s.fire();
        }
    }

    fn kill(&self) {
        let mut s = self.state();
        s.requested = true;
        s.fire();
    }
}

impl KillState {
    fn fire(&mut self) {
        if let Some(Err(e)) = self.kill.as_mut().map(|k| k()) {
            tracing::debug!("terminal kill: {e}");
        }
    }
}

/// A running session: its request channel and its kill switch.
struct Handle {
    ctl: Sender<Control>,
    kill: Arc<KillSwitch>,
}

/// The running sessions by id. Dropping it (the app quitting) stops every shell.
#[derive(Default)]
pub(super) struct Terminals(Mutex<HashMap<u64, Handle>>);

impl Terminals {
    fn map(&self) -> MutexGuard<'_, HashMap<u64, Handle>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Sends `c` to session `id`; a session that is gone is ignored.
    fn send(&self, id: u64, c: Control) {
        if let Some(h) = self.map().get(&id) {
            let _ = h.ctl.send(c);
        }
    }

    /// Stops session `id`'s shell now, whatever its thread is busy with.
    fn kill(&self, id: u64) {
        let kill = self.map().get(&id).map(|h| h.kill.clone());
        if let Some(k) = kill {
            k.kill();
        }
    }
}

impl Drop for Terminals {
    fn drop(&mut self) {
        // The OS closes the pseudoconsoles at exit anyway; this stops the shells first.
        for h in self.map().values() {
            h.kill.kill();
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
                let kill = Arc::new(KillSwitch::default());
                self.terminals.map().insert(id, Handle { ctl: ctl.clone(), kill: kill.clone() });
                let report = Reporter { tx: self.tx.clone(), waker: self.waker.clone() };
                let failed = report.clone();
                let spawned = std::thread::Builder::new()
                    .name("trellis-terminal".into())
                    .spawn(move || session(id, &cwd, (rows, cols), Channels { rx, ctl, kill }, report));
                if let Err(e) = spawned {
                    self.terminals.map().remove(&id);
                    failed.send(TerminalMsg::SpawnFailed { id, error: format!("could not start a thread: {e}") });
                }
            }
            TerminalEffect::Write { id, bytes } => self.terminals.send(id, Control::Write(bytes)),
            TerminalEffect::Resize { id, rows, cols } => self.terminals.send(id, Control::Resize(rows, cols)),
            TerminalEffect::Kill(id) => self.terminals.kill(id),
            // Only forgets the handle: the session's thread ends by itself once its shell has.
            TerminalEffect::Close(id) => {
                self.terminals.map().remove(&id);
            }
        }
    }
}

/// A session thread's ends of its channel, and its kill switch.
struct Channels {
    rx: Receiver<Control>,
    ctl: Sender<Control>,
    kill: Arc<KillSwitch>,
}

/// Runs session `id`: starts the shell, reports it, then carries out requests until the shell
/// has ended. Returning drops the PTY, which ends its output reader.
fn session<W: Waker + Clone>(id: u64, cwd: &Path, (rows, cols): (u16, u16), ch: Channels, report: Reporter<W>) {
    let Channels { rx, ctl, kill } = ch;
    let shell = pty::default_shell();
    let (out, exit) = (report.clone(), report.clone());
    let spawned = pty::spawn(cwd, rows, cols, &shell, super::terminal_out::forward(id, move |m| out.send(m)), move |code| {
        exit.send(TerminalMsg::Exited { id, code });
        let _ = ctl.send(Control::Closed);
    });
    let mut pty = match spawned {
        Ok(p) => p,
        Err(e) => return report.send(TerminalMsg::SpawnFailed { id, error: e.to_string() }),
    };
    let mut killer = pty.killer();
    kill.arm(Box::new(move || killer.kill()));
    report.send(TerminalMsg::Started { id, program: short_name(&shell.program) });
    for c in rx {
        let result = match c {
            Control::Write(bytes) => pty.write(&bytes).map_err(|e| e.to_string()),
            Control::Resize(rows, cols) => pty.resize(rows, cols).map_err(|e| e.to_string()),
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
    use super::{Control, Handle, KillSwitch, Terminals, short_name};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, mpsc};

    #[test]
    fn short_names() {
        assert_eq!(short_name("pwsh.exe"), "pwsh");
        assert_eq!(short_name("/bin/zsh"), "zsh");
        assert_eq!(short_name(r"C:\Windows\system32\cmd.exe"), if cfg!(windows) { "cmd" } else { r"C:\Windows\system32\cmd" });
    }

    /// Arms `kill` with a fake that counts its calls.
    fn counted(kill: &KillSwitch) -> Arc<AtomicUsize> {
        let n = Arc::new(AtomicUsize::new(0));
        let m = n.clone();
        kill.arm(Box::new(move || {
            m.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }));
        n
    }

    #[test]
    fn a_kill_before_the_shell_starts_runs_when_it_does() {
        let kill = KillSwitch::default();
        kill.kill();
        assert_eq!(counted(&kill).load(Ordering::SeqCst), 1);
    }

    #[test]
    fn kill_does_not_wait_behind_queued_writes() {
        // Nothing reads this channel, as when the session's thread is stuck in a write.
        let (ctl, _rx) = mpsc::channel();
        let kill = Arc::new(KillSwitch::default());
        let n = counted(&kill);
        let terminals = Terminals::default();
        terminals.map().insert(7, Handle { ctl, kill });
        terminals.send(7, Control::Write(b"stuck".to_vec()));
        terminals.kill(7);
        assert_eq!(n.load(Ordering::SeqCst), 1, "the shell is stopped at once");
        terminals.kill(8); // gone: ignored
        terminals.map().remove(&7);
        drop(terminals);
        assert_eq!(n.load(Ordering::SeqCst), 1, "a closed session is no longer the runner's to stop");
    }

    #[test]
    fn dropping_the_terminals_stops_every_shell() {
        let terminals = Terminals::default();
        let kill = Arc::new(KillSwitch::default());
        let n = counted(&kill);
        terminals.map().insert(1, Handle { ctl: mpsc::channel().0, kill });
        drop(terminals);
        assert_eq!(n.load(Ordering::SeqCst), 1);
    }
}
