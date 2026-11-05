//! The SVG worker with a fake drawing the test can hold until it lets go: one drawing at a
//! time, a newer request replacing the queued one, and callers giving up at the deadline while
//! the drawing runs on. Every step waits for an event (a drawing starting or ending, a request
//! reaching the slot), never for the clock, so the tests hold under any load.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::time::Instant;

/// A guard against a hang, long enough that only a broken worker reaches it; never an ordering.
const PATIENCE: Duration = Duration::from_secs(60);

/// First byte of a request: the drawing waits for `release` before it returns.
const HOLD: u8 = 1;
/// First byte of a request: the drawing returns at once.
const GO: u8 = 0;

/// A worker whose drawing of `[hold, tag]` reports `tag` on `started`, waits for `release` when
/// `hold` is `HOLD`, reports `tag` on `finished` and returns it. `most` is the most drawings
/// ever running at once.
struct Fake {
    worker: Arc<Worker<u8>>,
    most: Arc<AtomicUsize>,
    started: Receiver<u8>,
    finished: Receiver<u8>,
    release: Sender<()>,
}

fn fake(deadline: Duration) -> Fake {
    let (running, most) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    let (started_tx, started) = mpsc::channel();
    let (finished_tx, finished) = mpsc::channel();
    let (release, release_rx) = mpsc::channel::<()>();
    let release_rx = Mutex::new(release_rx);
    let peak = most.clone();
    let draw = move |b: &[u8]| {
        let now = running.fetch_add(1, Ordering::SeqCst) + 1;
        peak.fetch_max(now, Ordering::SeqCst);
        let _ = started_tx.send(b[1]);
        if b[0] == HOLD {
            // A dropped `release` lets it go too, so a failed test never leaves it held.
            let _ = release_rx.lock().unwrap_or_else(|e| e.into_inner()).recv();
        }
        running.fetch_sub(1, Ordering::SeqCst);
        let _ = finished_tx.send(b[1]);
        b[1]
    };
    Fake { worker: Arc::new(Worker::new("test-preview", Arc::new(draw), deadline)), most, started, finished, release }
}

/// `bytes` drawn on another thread, as a second caller would.
fn draw_aside(w: &Arc<Worker<u8>>, bytes: [u8; 2]) -> std::thread::JoinHandle<Option<u8>> {
    let w = w.clone();
    std::thread::spawn(move || w.draw(&bytes))
}

/// Returns once the request tagged `tag` waits in the worker's slot.
fn wait_until_queued(w: &Worker<u8>, tag: u8) {
    let start = Instant::now();
    loop {
        let queued = w.slot.0.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|j| j.bytes[1]);
        if queued == Some(tag) {
            return;
        }
        assert!(start.elapsed() < PATIENCE, "request {tag} never reached the slot");
        std::thread::yield_now();
    }
}

#[test]
fn a_drawing_comes_back() {
    let f = fake(PATIENCE);
    assert_eq!(f.worker.draw(&[GO, 7]), Some(7));
    assert_eq!(f.worker.draw(&[GO, 8]), Some(8));
}

#[test]
fn a_drawing_past_the_deadline_is_given_up_and_the_worker_goes_on() {
    let deadline = Duration::from_millis(200);
    let f = fake(deadline);
    let start = Instant::now();
    assert_eq!(f.worker.draw(&[HOLD, 1]), None, "a held drawing against a 200 ms deadline");
    let waited = start.elapsed();
    assert!(waited >= deadline, "gave up early: {waited:?}");
    // The caller gave up while the drawing still runs: it has started and not finished.
    assert_eq!(f.started.recv_timeout(PATIENCE), Ok(1));
    assert!(f.finished.try_recv().is_err(), "the held drawing finished");
    // The worker finishes the abandoned drawing, then takes the next. A loaded machine may
    // miss the short deadline on a try, so ask again until one comes back.
    f.release.send(()).unwrap();
    assert_eq!(f.finished.recv_timeout(PATIENCE), Ok(1));
    let next = (0..50).find_map(|_| f.worker.draw(&[GO, 2]));
    assert_eq!(next, Some(2));
}

#[test]
fn one_drawing_at_a_time_and_a_newer_request_replaces_the_queued_one() {
    let f = fake(PATIENCE);
    // The first occupies the worker until it is let go.
    let busy = draw_aside(&f.worker, [HOLD, 1]);
    assert_eq!(f.started.recv_timeout(PATIENCE), Ok(1));
    // The second waits in the slot, and the third takes its place.
    let queued = draw_aside(&f.worker, [GO, 2]);
    wait_until_queued(&f.worker, 2);
    let newest = draw_aside(&f.worker, [GO, 3]);
    wait_until_queued(&f.worker, 3);
    // The replaced caller gives up at once, while the first still draws.
    assert_eq!(queued.join().unwrap(), None, "replaced before it started");
    f.release.send(()).unwrap();
    assert_eq!(busy.join().unwrap(), Some(1));
    assert_eq!(newest.join().unwrap(), Some(3));
    assert_eq!(f.started.try_iter().collect::<Vec<_>>(), [3], "the replaced request never drew");
    assert_eq!(f.most.load(Ordering::SeqCst), 1, "never two drawings at once");
}
