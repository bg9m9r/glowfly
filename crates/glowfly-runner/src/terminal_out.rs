//! A shell's output on its way to the UI thread, which parses it. A shell printing fast would
//! otherwise flood the message channel with small chunks faster than they are parsed, with no
//! bound on what waits. Instead each session keeps one `Output` message in flight: what arrives
//! meanwhile is merged into the next one, sent once update has fed the last (its `Ack` is
//! dropped). Past `MAX_WAITING` bytes the PTY reader waits, which holds the shell back.

use glowfly_model::terminal::{Ack, TerminalMsg};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

/// Output kept waiting for the UI before the reader stops reading.
const MAX_WAITING: usize = 1 << 20;

#[derive(Default)]
struct Waiting {
    bytes: Vec<u8>,
    /// An `Output` message hasn't been fed to the screen yet.
    in_flight: bool,
    /// The reader has ended: nothing more will arrive.
    closed: bool,
}

#[derive(Default)]
struct Shared {
    waiting: Mutex<Waiting>,
    changed: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Waiting> {
        self.waiting.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn wait<'a>(&self, g: MutexGuard<'a, Waiting>) -> MutexGuard<'a, Waiting> {
        self.changed.wait(g).unwrap_or_else(|e| e.into_inner())
    }
}

/// The PTY reader's end. Dropping it (the reader has ended) lets the forwarder finish.
pub(super) struct Input(Arc<Shared>);

impl Input {
    /// Adds output from the shell, first waiting while too much is waiting for the UI.
    pub(super) fn push(&self, bytes: Vec<u8>) {
        let mut w = self.0.lock();
        while w.bytes.len() >= MAX_WAITING {
            w = self.0.wait(w);
        }
        w.bytes.extend_from_slice(&bytes);
        self.0.changed.notify_all();
    }
}

impl Drop for Input {
    fn drop(&mut self) {
        self.0.lock().closed = true;
        self.0.changed.notify_all();
    }
}

/// Starts forwarding session `id`'s output through `send` on a thread of its own, and returns
/// the end the PTY reader pushes into. Without the thread, output goes straight to `send`.
pub(super) fn forward(id: u64, send: impl Fn(TerminalMsg) + Send + Clone + 'static) -> Box<dyn FnMut(Vec<u8>) + Send> {
    let shared = Arc::new(Shared::default());
    let (s, out) = (Arc::clone(&shared), send.clone());
    match std::thread::Builder::new().name("glowfly-terminal-out".into()).spawn(move || run(id, &s, &out)) {
        Ok(_) => {
            let input = Input(shared);
            Box::new(move |bytes| input.push(bytes))
        }
        Err(e) => {
            tracing::error!("could not start the terminal output thread: {e}");
            Box::new(move |bytes| send(TerminalMsg::Output { id, bytes, ack: Ack::default() }))
        }
    }
}

/// Sends what is waiting, one message at a time, until the reader has ended and all is sent.
fn run(id: u64, shared: &Arc<Shared>, send: &impl Fn(TerminalMsg)) {
    loop {
        let bytes = {
            let mut w = shared.lock();
            while (w.bytes.is_empty() || w.in_flight) && !(w.closed && w.bytes.is_empty()) {
                w = shared.wait(w);
            }
            if w.bytes.is_empty() {
                return;
            }
            w.in_flight = true;
            shared.changed.notify_all();
            std::mem::take(&mut w.bytes)
        };
        let s = Arc::clone(shared);
        let ack = Ack::new(move || {
            s.lock().in_flight = false;
            s.changed.notify_all();
        });
        send(TerminalMsg::Output { id, bytes, ack });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    fn output(m: TerminalMsg) -> (Vec<u8>, Ack) {
        match m {
            TerminalMsg::Output { bytes, ack, .. } => (bytes, ack),
            _ => panic!("not output"),
        }
    }

    #[test]
    fn output_waits_for_the_last_message_and_merges() {
        let (tx, rx) = mpsc::channel();
        let send = move |m| {
            let _ = tx.send(m);
        };
        let mut push = forward(7, send);
        push(b"one ".to_vec());
        let (first, ack) = output(rx.recv_timeout(Duration::from_secs(5)).unwrap());
        assert_eq!(first, b"one ");
        push(b"two ".to_vec());
        push(b"three".to_vec());
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err(), "nothing more while the first is unread");
        drop(ack);
        let (next, _ack) = output(rx.recv_timeout(Duration::from_secs(5)).unwrap());
        assert_eq!(next, b"two three", "what came meanwhile arrives as one message");
        drop(_ack);
        // The reader ends: what is left is still sent, then the thread stops.
        push(b"bye".to_vec());
        drop(push);
        let (last, _) = output(rx.recv_timeout(Duration::from_secs(5)).unwrap());
        assert_eq!(last, b"bye");
        assert!(matches!(rx.recv_timeout(Duration::from_secs(5)), Err(mpsc::RecvTimeoutError::Disconnected)));
    }
}
