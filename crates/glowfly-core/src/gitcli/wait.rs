//! Waiting for a git child: draining its pipes on their own threads, killing it once its timeout
//! passes, and stopping it (and everything it started) when the caller asks ([`Stop`]).

use crate::process_tree::Tree;
use std::process::{Child, ExitStatus};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// A request to stop a running git command, shared between the thread that runs it
/// ([`super::output_until`]) and the one that asks. Cloning shares the same request.
#[derive(Clone, Debug, Default)]
pub struct Stop(Arc<AtomicBool>);

impl Stop {
    pub fn new() -> Self {
        Self::default()
    }

    /// Asks the command to stop; it is stopped within about 10 ms, then given a grace period.
    pub fn stop(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn requested(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    /// Whether `a` and `b` are the same request (clones of one `Stop`).
    pub fn same(a: &Stop, b: &Stop) -> bool {
        Arc::ptr_eq(&a.0, &b.0)
    }
}

/// How a wait ended.
pub(super) struct Waited {
    /// The exit status; None when it was killed for its timeout or stopped.
    pub status: Option<ExitStatus>,
    pub timed_out: bool,
    pub stopped: bool,
    /// The stop needed a hard kill (git didn't exit within the grace period, or Windows).
    pub forced: bool,
}

pub(super) type Drained = std::sync::mpsc::Receiver<Vec<u8>>;

/// Reads `pipe` to the end on a new thread.
pub(super) fn drain(pipe: Option<impl std::io::Read + Send + 'static>) -> Drained {
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

/// A pipe read on a new thread in chunks as they arrive, so the caller can watch it (git's
/// progress on stderr) while it waits.
pub(super) struct Streamed {
    rx: std::sync::mpsc::Receiver<Vec<u8>>,
    got: Vec<u8>,
}

impl Streamed {
    pub fn start(pipe: Option<impl std::io::Read + Send + 'static>) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        if let Some(mut pipe) = pipe {
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                loop {
                    match pipe.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if tx.send(buf[..n].to_vec()).is_err() {
                                break;
                            }
                        }
                    }
                }
            });
        }
        Self { rx, got: Vec::new() }
    }

    /// Hands what arrived since the last call to `watch`.
    pub fn poll(&mut self, watch: &mut dyn FnMut(&[u8])) {
        while let Ok(chunk) = self.rx.try_recv() {
            watch(&chunk);
            self.got.extend_from_slice(&chunk);
        }
    }

    /// Everything read, once the pipe is closed (or, with `grace`, once that much time passed).
    pub fn finish(mut self, grace: Option<Duration>) -> Vec<u8> {
        let deadline = grace.map(|g| Instant::now() + g);
        loop {
            let next = match deadline {
                Some(d) => self.rx.recv_timeout(d.saturating_duration_since(Instant::now())).ok(),
                None => self.rx.recv().ok(),
            };
            match next {
                Some(chunk) => self.got.extend_from_slice(&chunk),
                None => return self.got,
            }
        }
    }
}

pub(super) fn collect(rx: &Drained, grace: Option<Duration>) -> Vec<u8> {
    match grace {
        Some(d) => rx.recv_timeout(d).unwrap_or_default(),
        None => rx.recv().unwrap_or_default(),
    }
}

/// Waits for `child`, killing it once `timeout` passes, and stopping its whole process tree
/// once `stop` is requested; `tick` runs at every poll.
pub(super) fn wait(
    child: &mut Child,
    timeout: Option<Duration>,
    stop: Option<(&Stop, &Tree)>,
    tick: &mut dyn FnMut(),
) -> std::io::Result<Waited> {
    let ended = |status| Waited { status, timed_out: false, stopped: false, forced: false };
    if timeout.is_none() && stop.is_none() {
        return child.wait().map(|s| ended(Some(s)));
    }
    let deadline = timeout.map(|t| Instant::now() + t);
    let poll = Duration::from_millis(if timeout.is_some() { 5 } else { 10 });
    loop {
        tick();
        if let Some(s) = child.try_wait()? {
            return Ok(ended(Some(s)));
        }
        if let Some((stop, tree)) = stop
            && stop.requested()
        {
            let forced = tree.terminate(child);
            return Ok(Waited { status: None, timed_out: false, stopped: true, forced });
        }
        if deadline.is_some_and(|d| Instant::now() >= d) {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(Waited { status: None, timed_out: true, stopped: false, forced: false });
        }
        std::thread::sleep(poll);
    }
}
