//! The file watcher for the active tab's repository: notify reports raw events, a thread of its
//! own drops the ones that don't matter (`trellis_core::watch`), coalesces the rest into bursts
//! and sends one `Msg::RepoChanged` per burst. Owned by the runner (`Effect::Watch`).

use crate::effect::WatchTarget;
use crate::msg::Msg;
use crate::runner::Waker;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use trellis_core::watch::{Change, Ignores, WatchPaths};

/// A burst fires this long after its last relevant event…
pub const QUIET: Duration = Duration::from_millis(300);
/// …or this long after its first, if events keep coming.
pub const MAX_WAIT: Duration = Duration::from_secs(2);

/// The single live watcher; a generation guards against a slow start finishing after a newer
/// request.
#[derive(Default)]
pub struct WatchSlot(Arc<Mutex<(u64, Option<RepoWatcher>)>>);

impl WatchSlot {
    /// Stops the current watcher and, for `Some`, starts watching `target` on a background
    /// thread (a recursive registration can take a while on Linux).
    pub fn set<W: Waker + Clone>(&self, target: Option<WatchTarget>, tx: Sender<Msg>, waker: W) {
        let (generation, old) = {
            let mut slot = self.0.lock().unwrap_or_else(|e| e.into_inner());
            slot.0 += 1;
            (slot.0, slot.1.take())
        };
        drop(old);
        let Some(target) = target else { return };
        let slot = Arc::clone(&self.0);
        let spawned = std::thread::Builder::new().name("trellis-watch-start".into()).spawn(move || {
            let started = start(&target, tx.clone(), waker.clone());
            let mut s = slot.lock().unwrap_or_else(|e| e.into_inner());
            if s.0 != generation {
                return; // superseded; a started watcher is dropped here
            }
            match started {
                Ok(w) => s.1 = Some(w),
                Err(error) => {
                    drop(s);
                    if tx.send(Msg::WatchFailed { workdir: target.workdir, error }).is_ok() {
                        waker.wake();
                    }
                }
            }
        });
        if let Err(e) = spawned {
            tracing::error!("could not start the file watcher thread: {e}");
        }
    }
}

/// A running watcher. Dropping it stops notify, which ends the event thread.
pub struct RepoWatcher {
    _inner: notify::RecommendedWatcher,
}

fn start<W: Waker + Clone>(target: &WatchTarget, tx: Sender<Msg>, waker: W) -> Result<RepoWatcher, String> {
    let (raw_tx, raw_rx) = std::sync::mpsc::channel::<notify::Result<Event>>();
    let describe = |p: &Path, e: notify::Error| format!("could not watch {}: {e}", p.display());
    let mut inner = notify::recommended_watcher(raw_tx).map_err(|e| format!("could not start a file watcher: {e}"))?;
    inner.watch(&target.workdir, RecursiveMode::Recursive).map_err(|e| describe(&target.workdir, e))?;
    if !target.git_dir.starts_with(&target.workdir) {
        inner.watch(&target.git_dir, RecursiveMode::Recursive).map_err(|e| describe(&target.git_dir, e))?;
    }
    let target = target.clone();
    std::thread::Builder::new()
        .name("trellis-watch".into())
        .spawn(move || event_loop(raw_rx, target, tx, waker))
        .map_err(|e| format!("could not start the file watcher thread: {e}"))?;
    Ok(RepoWatcher { _inner: inner })
}

fn event_loop<W: Waker>(raw: Receiver<notify::Result<Event>>, target: WatchTarget, tx: Sender<Msg>, waker: W) {
    let paths = WatchPaths::new(&target.workdir, &target.git_dir);
    let mut ignores = Ignores::open(&target.workdir);
    let mut burst = Burst::default();
    let mut reported = false;
    loop {
        let next = match burst.wait(Instant::now()) {
            None => raw.recv().map_err(|_| RecvTimeoutError::Disconnected),
            Some(d) => raw.recv_timeout(d),
        };
        match next {
            Ok(Ok(ev)) => {
                let change = classify(&ev, &paths, &mut ignores);
                burst.add(change, Instant::now());
            }
            Ok(Err(e)) => {
                if !reported {
                    reported = true;
                    let msg = Msg::WatchFailed { workdir: target.workdir.clone(), error: e.to_string() };
                    if tx.send(msg).is_err() {
                        return;
                    }
                    waker.wake();
                }
                // Events may have been lost.
                burst.add(Change::Full, Instant::now());
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        if let Some((change, last_event)) = burst.take_due(Instant::now()) {
            let full = change == Change::Full;
            tracing::debug!(workdir = %target.workdir.display(), full, "files changed outside Trellis");
            let msg = Msg::RepoChanged { full, last_event };
            if tx.send(Msg::Scoped { workdir: target.workdir.clone(), msg: Box::new(msg) }).is_err() {
                return;
            }
            waker.wake();
        }
    }
}

/// What one raw event calls for.
fn classify(ev: &Event, paths: &WatchPaths, ignores: &mut Ignores) -> Change {
    if matches!(ev.kind, EventKind::Access(_)) {
        return Change::None;
    }
    if ev.need_rescan() {
        return Change::Full; // the OS dropped events
    }
    let mut change = Change::None;
    for p in &ev.paths {
        let name = p.file_name().and_then(|n| n.to_str());
        let parent = p.parent().and_then(|d| d.file_name()).and_then(|n| n.to_str());
        if name == Some(".gitignore") || (name == Some("exclude") && parent == Some("info")) {
            ignores.clear();
        }
        change = change.max(paths.classify(p, &mut |rel| ignores.is_ignored(rel)));
        if change == Change::Full {
            break;
        }
    }
    change
}

/// Relevant events waiting to be reported as one change.
#[derive(Default)]
struct Burst {
    first: Option<Instant>,
    last: Option<Instant>,
    change: Change,
}

impl Burst {
    /// Adds an event at `at`; `Change::None` neither starts nor extends a burst.
    fn add(&mut self, change: Change, at: Instant) {
        if change == Change::None {
            return;
        }
        self.first.get_or_insert(at);
        self.last = Some(at);
        self.change = self.change.max(change);
    }

    fn due_at(&self) -> Option<Instant> {
        Some((self.last? + QUIET).min(self.first? + MAX_WAIT))
    }

    /// How long until the burst is due; `None` when there is no burst.
    fn wait(&self, now: Instant) -> Option<Duration> {
        self.due_at().map(|d| d.saturating_duration_since(now))
    }

    /// The burst's change and its last event's time once it is due, starting over.
    fn take_due(&mut self, now: Instant) -> Option<(Change, Instant)> {
        if now < self.due_at()? {
            return None;
        }
        let last = self.last?;
        let change = std::mem::take(self).change;
        Some((change, last))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use trellis_core::gitcli::{args, run};

    const MS: Duration = Duration::from_millis(1);

    #[test]
    fn bursts_wait_for_quiet() {
        let t0 = Instant::now();
        let mut b = Burst::default();
        b.add(Change::Status, t0);
        b.add(Change::Status, t0 + 100 * MS);
        assert!(b.take_due(t0 + 350 * MS).is_none());
        assert_eq!(b.wait(t0 + 350 * MS), Some(50 * MS));
        assert_eq!(b.take_due(t0 + 400 * MS), Some((Change::Status, t0 + 100 * MS)));
        assert_eq!(b.wait(t0 + 400 * MS), None, "reset after firing");
        assert!(b.take_due(t0 + 5000 * MS).is_none());
    }

    #[test]
    fn a_continuous_burst_fires_within_max_wait() {
        let t0 = Instant::now();
        let mut b = Burst::default();
        let mut fired = None;
        for i in 0..30u32 {
            let t = t0 + i * 100 * MS;
            if let Some(f) = b.take_due(t) {
                fired = Some((t, f));
                break;
            }
            b.add(Change::Status, t);
        }
        let (at, _) = fired.expect("fired during the burst");
        assert!(at <= t0 + MAX_WAIT, "fired after {:?}", at - t0);
    }

    #[test]
    fn bursts_merge_and_ignore_nothing() {
        let t0 = Instant::now();
        let mut b = Burst::default();
        b.add(Change::Status, t0);
        b.add(Change::Full, t0 + 10 * MS);
        assert_eq!(b.take_due(t0 + 400 * MS).map(|(c, _)| c), Some(Change::Full));

        b.add(Change::None, t0);
        assert_eq!(b.wait(t0), None, "None doesn't start a burst");
        b.add(Change::Status, t0);
        b.add(Change::None, t0 + 200 * MS);
        assert_eq!(b.take_due(t0 + 300 * MS), Some((Change::Status, t0)), "None doesn't extend it");
    }

    #[derive(Clone)]
    struct NoWake;
    impl Waker for NoWake {
        fn wake(&self) {}
    }

    fn git(dir: &Path, a: &[&str]) {
        let (ok, out) = run(dir, &args(a));
        assert!(ok, "git {a:?} failed: {out}");
    }

    /// The next `RepoChanged` for `workdir` within `timeout`, as `full`.
    fn next_change(rx: &Receiver<Msg>, workdir: &Path, timeout: Duration) -> Option<bool> {
        match rx.recv_timeout(timeout).ok()? {
            Msg::Scoped { workdir: w, msg } if w == workdir => match *msg {
                Msg::RepoChanged { full, .. } => Some(full),
                _ => panic!("unexpected scoped message"),
            },
            Msg::WatchFailed { error, .. } => panic!("watcher failed: {error}"),
            _ => panic!("unexpected message"),
        }
    }

    #[test]
    fn watcher_reports_an_outside_edit() {
        let dir: PathBuf = std::env::temp_dir().join(format!("trellis-app-test-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]);
        git(&dir, &["config", "user.name", "Test"]);
        git(&dir, &["config", "user.email", "test@example.com"]);
        git(&dir, &["config", "commit.gpgsign", "false"]);
        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        std::fs::write(dir.join(".gitignore"), "target/\n").unwrap();
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "-q", "-m", "first"]);

        let (tx, rx) = std::sync::mpsc::channel();
        let target = WatchTarget { workdir: dir.clone(), git_dir: dir.join(".git") };
        let watcher = start(&target, tx, NoWake).expect("watcher starts");
        std::thread::sleep(Duration::from_millis(300));

        std::fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
        assert_eq!(next_change(&rx, &dir, Duration::from_secs(10)), Some(false), "an edit re-reads the status");

        git(&dir, &["checkout", "-q", "-b", "x"]);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !next_change(&rx, &dir, deadline.saturating_duration_since(Instant::now())).expect("a full refresh") {}
        // Let any trailing burst finish before checking that ignored files stay quiet.
        while next_change(&rx, &dir, Duration::from_secs(1)).is_some() {}

        std::fs::create_dir_all(dir.join("target/debug")).unwrap();
        std::fs::write(dir.join("target/debug/x.o"), "obj").unwrap();
        assert_eq!(next_change(&rx, &dir, Duration::from_secs(1)), None, "ignored files don't refresh");

        // Dropping the watcher ends its thread, which drops the message sender.
        drop(watcher);
        assert!(matches!(rx.recv_timeout(Duration::from_secs(10)), Err(RecvTimeoutError::Disconnected)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
