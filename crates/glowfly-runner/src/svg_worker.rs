//! The one thread that draws SVG previews. The render budget (`preview_svg_budget`) refuses
//! the costly drawings it knows of; this bounds the rest. Only one SVG draws at a time, so a
//! drawing that slips past the budget can pin one core at most, never one per click. A request
//! made while the worker is busy waits in a single slot, and a newer request takes that slot
//! (the older one gives up: its diff is no longer the one asked for). Each caller waits at most
//! `DEADLINE` and then shows the side as "Can't preview this SVG"; the drawing carries on, and
//! its result is dropped when it arrives. No thread is ever stopped.

use glowfly_core::preview_svg::{self, SvgError, SvgRender};
use std::sync::mpsc::{self, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

/// How long a caller waits for its drawing.
pub(crate) const DEADLINE: Duration = Duration::from_secs(3);

/// What a worker draws with.
type Draw<R> = dyn Fn(&[u8]) -> R + Send + Sync;

/// A drawing asked for: the bytes and where its result goes.
struct Job<R> {
    bytes: Vec<u8>,
    reply: SyncSender<R>,
}

/// One drawing thread and the single slot that feeds it.
pub(crate) struct Worker<R> {
    slot: Arc<(Mutex<Option<Job<R>>>, Condvar)>,
    deadline: Duration,
}

impl<R: Send + 'static> Worker<R> {
    /// A worker drawing with `draw`, its callers waiting at most `deadline`.
    pub(crate) fn new(draw: Arc<Draw<R>>, deadline: Duration) -> Self {
        let slot: Arc<(Mutex<Option<Job<R>>>, Condvar)> = Arc::new((Mutex::new(None), Condvar::new()));
        let feed = slot.clone();
        let spawned = std::thread::Builder::new().name("svg-preview".into()).spawn(move || {
            loop {
                let job = {
                    let (lock, ready) = &*feed;
                    let mut pending = lock.lock().unwrap_or_else(|e| e.into_inner());
                    loop {
                        match pending.take() {
                            Some(job) => break job,
                            None => pending = ready.wait(pending).unwrap_or_else(|e| e.into_inner()),
                        }
                    }
                };
                // A caller that stopped waiting has hung up: its result is dropped.
                let _ = job.reply.send(draw(&job.bytes));
            }
        });
        if let Err(e) = spawned {
            tracing::warn!("svg preview worker: {e}");
        }
        Self { slot, deadline }
    }

    /// `bytes` drawn, or `None` when the drawing outlived the deadline or a newer request took
    /// its place before it started.
    pub(crate) fn draw(&self, bytes: &[u8]) -> Option<R> {
        let (reply, result) = mpsc::sync_channel(1);
        {
            let (lock, ready) = &*self.slot;
            // The request it replaces (if any) is dropped with its sender: that caller gives up.
            *lock.lock().unwrap_or_else(|e| e.into_inner()) = Some(Job { bytes: bytes.to_vec(), reply });
            ready.notify_one();
        }
        match result.recv_timeout(self.deadline) {
            Ok(r) => Some(r),
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => None,
        }
    }
}

/// `bytes` drawn on the app's one SVG worker, within `DEADLINE`.
pub(crate) fn render(bytes: &[u8]) -> Result<SvgRender, SvgError> {
    static WORKER: OnceLock<Worker<Result<SvgRender, SvgError>>> = OnceLock::new();
    let worker = WORKER.get_or_init(|| Worker::new(Arc::new(preview_svg::render), DEADLINE));
    worker.draw(bytes).unwrap_or(Err(SvgError::Abandoned))
}

#[cfg(test)]
#[path = "svg_worker_tests.rs"]
mod tests;
