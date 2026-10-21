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

pub(super) fn collect(rx: &Drained, grace: Option<Duration>) -> Vec<u8> {
    match grace {
        Some(d) => rx.recv_timeout(d).unwrap_or_default(),
        None => rx.recv().unwrap_or_default(),
    }
}

/// Waits for `child`, killing it once `timeout` passes, and stopping its whole process tree
/// once `stop` is requested.
pub(super) fn wait(child: &mut Child, timeout: Option<Duration>, stop: Option<(&Stop, &Tree)>) -> std::io::Result<Waited> {
    let ended = |status| Waited { status, timed_out: false, stopped: false, forced: false };
    if timeout.is_none() && stop.is_none() {
        return child.wait().map(|s| ended(Some(s)));
    }
    let deadline = timeout.map(|t| Instant::now() + t);
    let poll = Duration::from_millis(if timeout.is_some() { 5 } else { 10 });
    loop {
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
