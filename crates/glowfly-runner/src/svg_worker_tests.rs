//! The SVG worker with a fake drawing that sleeps as long as its bytes say: one drawing at a
//! time, a newer request replacing the queued one, and callers giving up at the deadline
//! while the drawing runs on.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

/// A worker whose drawing sleeps `bytes[0]` × 10 ms and returns `bytes[1]`, recording the most
/// drawings ever running at once.
fn worker(deadline: Duration) -> (Arc<Worker<u8>>, Arc<AtomicUsize>) {
    let (running, most) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    let peak = most.clone();
    let draw = move |b: &[u8]| {
        let now = running.fetch_add(1, Ordering::SeqCst) + 1;
        peak.fetch_max(now, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(u64::from(b[0]) * 10));
        running.fetch_sub(1, Ordering::SeqCst);
        b[1]
    };
    (Arc::new(Worker::new(Arc::new(draw), deadline)), most)
}

#[test]
fn a_drawing_comes_back() {
    let (w, _) = worker(Duration::from_secs(5));
    assert_eq!(w.draw(&[0, 7]), Some(7));
    assert_eq!(w.draw(&[1, 8]), Some(8));
}

#[test]
fn a_drawing_past_the_deadline_is_given_up_and_the_worker_goes_on() {
    let (w, _) = worker(Duration::from_millis(200));
    let start = Instant::now();
    assert_eq!(w.draw(&[100, 1]), None, "a second-long drawing against a 200 ms deadline");
    let waited = start.elapsed();
    assert!(waited >= Duration::from_millis(200) && waited < Duration::from_millis(900), "{waited:?}");
    // The worker finishes the abandoned drawing, then takes the next.
    std::thread::sleep(Duration::from_millis(900));
    assert_eq!(w.draw(&[0, 2]), Some(2));
}

#[test]
fn one_drawing_at_a_time_and_a_newer_request_replaces_the_queued_one() {
    let (w, most) = worker(Duration::from_secs(5));
    // The first occupies the worker for 300 ms.
    let busy = std::thread::spawn({
        let w = w.clone();
        move || w.draw(&[30, 1])
    });
    std::thread::sleep(Duration::from_millis(50));
    // The second waits in the slot, and the third takes its place.
    let queued = std::thread::spawn({
        let w = w.clone();
        move || w.draw(&[0, 2])
    });
    std::thread::sleep(Duration::from_millis(50));
    let newest = std::thread::spawn({
        let w = w.clone();
        move || w.draw(&[0, 3])
    });
    assert_eq!(busy.join().unwrap(), Some(1));
    assert_eq!(queued.join().unwrap(), None, "replaced before it started");
    assert_eq!(newest.join().unwrap(), Some(3));
    assert_eq!(most.load(Ordering::SeqCst), 1, "never two drawings at once");
}
