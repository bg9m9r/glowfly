//! The built-in terminal's shells. Each session has a thread that owns its PTY and carries out
//! the writes and resizes update asks for, in order, so a full PTY input pipe can never block
//! the UI thread. Requests sent while the shell is still starting wait in its channel. Kills
//! bypass that queue: a write stuck on a shell that isn't reading must not hold up a stop.
//! Whether a shell is running a program is read on a thread of its own, since a process
//! snapshot takes milliseconds.

use super::execute::{Cx, Execute};
use super::{Runner, Waker};
use crate::msg::Msg;
use crate::terminal::{AfterCheck, BusyCheck, TerminalEffect, TerminalMsg};
use glowfly_core::settings::ShellSetting;
use glowfly_core::terminal::{ShellActivity, ShellProbe, pty, shell};
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};

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

/// The sessions are run by `Runner::run` itself (`Runner::terminal`), never on a worker.
impl Execute for TerminalEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        tracing::warn!("terminal effect reached a worker; it is handled by Runner::run");
        None
    }
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

/// Reads a session's shell activity once the shell has started.
#[derive(Default)]
struct ProbeSlot(Mutex<Option<ShellProbe>>);

impl ProbeSlot {
    fn set(&self, p: ShellProbe) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(p);
    }

    fn get(&self) -> Option<ShellProbe> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

/// A running session: its request channel, its kill switch and its activity probe.
struct Handle {
    ctl: Sender<Control>,
    kill: Arc<KillSwitch>,
    probe: Arc<ProbeSlot>,
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

impl Terminals {
    /// Session `id`'s probe, once its shell has started.
    fn probe(&self, id: u64) -> Option<ShellProbe> {
        self.map().get(&id).and_then(|h| h.probe.get())
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
            TerminalEffect::Spawn { id, cwd, rows, cols, shell } => {
                let (ctl, rx) = mpsc::channel();
                let kill = Arc::new(KillSwitch::default());
                let probe = Arc::new(ProbeSlot::default());
                self.terminals.map().insert(id, Handle { ctl: ctl.clone(), kill: kill.clone(), probe: probe.clone() });
                let report = Reporter { tx: self.tx.clone(), waker: self.waker.clone() };
                let failed = report.clone();
                let spawned = std::thread::Builder::new()
                    .name("glowfly-terminal".into())
                    .spawn(move || session(id, &cwd, (rows, cols), &shell, Channels { rx, ctl, kill, probe }, report));
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
            TerminalEffect::CheckActivity { id, then } => {
                let report = Reporter { tx: self.tx.clone(), waker: self.waker.clone() };
                check_activity(id, then, self.terminals.probe(id), report);
            }
            TerminalEffect::CheckBusy { ids, then } => {
                let report = Reporter { tx: self.tx.clone(), waker: self.waker.clone() };
                let probes = ids.into_iter().map(|id| (id, self.terminals.probe(id))).collect();
                check_busy(probes, then, report);
            }
        }
    }
}

/// Reports what session `id`'s shell is doing (with `then`, what waits on the answer), reading
/// `probe` on its own thread so the UI thread never waits for it. No probe (the session is gone
/// or still starting) is Unknown.
fn check_activity<W: Waker + Clone>(id: u64, then: AfterCheck, probe: Option<ShellProbe>, report: Reporter<W>) {
    let Some(probe) = probe else {
        return report.send(TerminalMsg::Activity { id, activity: ShellActivity::Unknown, then });
    };
    let thread_report = report.clone();
    let spawned = std::thread::Builder::new()
        .name("glowfly-terminal-probe".into())
        .spawn(move || thread_report.send(TerminalMsg::Activity { id, activity: probe.activity(), then }));
    if spawned.is_err() {
        report.send(TerminalMsg::Activity { id, activity: ShellActivity::Unknown, then });
    }
}

/// Reports what each of `probes`' shells is doing in one answer (with `then`), reading them on
/// one thread of its own. A missing probe, or no thread, is Unknown.
fn check_busy<W: Waker + Clone>(probes: Vec<(u64, Option<ShellProbe>)>, then: BusyCheck, report: Reporter<W>) {
    let unknown: Vec<(u64, ShellActivity)> = probes.iter().map(|(id, _)| (*id, ShellActivity::Unknown)).collect();
    let thread_report = report.clone();
    let spawned = std::thread::Builder::new().name("glowfly-terminal-probe".into()).spawn(move || {
        let found = probes.into_iter().map(|(id, p)| (id, p.map_or(ShellActivity::Unknown, |p| p.activity()))).collect();
        thread_report.send(TerminalMsg::BusyChecked { found, then });
    });
    if spawned.is_err() {
        report.send(TerminalMsg::BusyChecked { found: unknown, then });
    }
}

/// A session thread's ends of its channel, its kill switch and its probe slot.
struct Channels {
    rx: Receiver<Control>,
    ctl: Sender<Control>,
    kill: Arc<KillSwitch>,
    probe: Arc<ProbeSlot>,
}

/// Runs session `id`: starts the shell `choice` names, reports it, then carries out requests
/// until the shell has ended. Returning drops the PTY, which ends its output reader. A chosen
/// shell that can't be found is reported, never replaced by another.
fn session<W: Waker + Clone>(id: u64, cwd: &Path, (rows, cols): (u16, u16), choice: &ShellSetting, ch: Channels, report: Reporter<W>) {
    let Channels { rx, ctl, kill, probe } = ch;
    let shell = match shell::resolve(choice) {
        Ok(s) => s,
        Err(error) => return report.send(TerminalMsg::SpawnFailed { id, error }),
    };
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
    probe.set(pty.probe());
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
    use super::{Control, Handle, KillSwitch, ProbeSlot, Reporter, Terminals, check_activity, short_name};
    use crate::msg::Msg;
    use crate::terminal::{AfterCheck, BusyCheck, TerminalMsg};
    use glowfly_core::terminal::{ShellActivity, ShellProbe};
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
        terminals.map().insert(7, Handle { ctl, kill, probe: Arc::default() });
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
        terminals.map().insert(1, Handle { ctl: mpsc::channel().0, kill, probe: Arc::default() });
        drop(terminals);
        assert_eq!(n.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_probe_is_there_once_set() {
        let slot = ProbeSlot::default();
        assert!(slot.get().is_none());
        slot.set(ShellProbe::default());
        assert_eq!(slot.get().map(|p| p.activity()), Some(ShellActivity::Unknown), "a probe of no process");
        let terminals = Terminals::default();
        terminals.map().insert(3, Handle { ctl: mpsc::channel().0, kill: Arc::default(), probe: Arc::default() });
        assert!(terminals.probe(3).is_none(), "not started yet");
        assert!(terminals.probe(4).is_none(), "no such session");
    }

    #[test]
    fn checking_a_session_without_a_probe_reports_unknown() {
        let (tx, rx) = mpsc::channel();
        #[derive(Clone)]
        struct NoWake;
        impl super::Waker for NoWake {
            fn wake(&self) {}
        }
        check_activity(9, AfterCheck::Kill, None, Reporter { tx, waker: NoWake });
        match rx.try_recv() {
            Ok(Msg::Terminal(TerminalMsg::Activity { id: 9, activity, then })) => {
                assert_eq!(activity, ShellActivity::Unknown);
                assert_eq!(then, AfterCheck::Kill, "the answer carries what waits on it");
            }
            _ => panic!("expected an Activity report"),
        }
    }

    #[test]
    fn checking_several_sessions_reports_them_in_one_answer() {
        #[derive(Clone)]
        struct NoWake;
        impl super::Waker for NoWake {
            fn wake(&self) {}
        }
        let (tx, rx) = mpsc::channel();
        let probes = vec![(4, None), (5, Some(ShellProbe::default()))];
        super::check_busy(probes, BusyCheck::Quit, Reporter { tx, waker: NoWake });
        match rx.recv_timeout(std::time::Duration::from_secs(10)) {
            Ok(Msg::Terminal(TerminalMsg::BusyChecked { found, then })) => {
                assert_eq!(found, vec![(4, ShellActivity::Unknown), (5, ShellActivity::Unknown)]);
                assert_eq!(then, BusyCheck::Quit);
            }
            _ => panic!("expected a BusyChecked report"),
        }
    }
}
