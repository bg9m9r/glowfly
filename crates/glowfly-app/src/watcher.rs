//! The file watcher for the active tab's repository: notify reports raw events, a thread of its
//! own drops the ones that don't matter (`glowfly_core::watch`), coalesces the rest into bursts
//! and sends one `WatchMsg::RepoChanged` per burst, with the work-tree paths it saw when nothing
//! else changed. Owned by the runner (`Effect::Watch`).

use crate::effect::WatchTarget;
use crate::msg::Msg;
use crate::msg::WatchMsg;
use crate::runner::Waker;
use glowfly_core::watch::{Change, Ignores, MAX_WATCH_PATHS, Touch, WatchPaths};
use notify::event::ModifyKind;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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
            // A network or virtual file system may never report changes made elsewhere.
            if let Some(kind) = glowfly_core::volume::remote_kind(&target.workdir) {
                let msg = Msg::Watch(WatchMsg::RemoteVolume { workdir: target.workdir.clone(), kind });
                if tx.send(msg).is_ok() {
                    waker.wake();
                }
            }
            let started = start(&target, tx.clone(), waker.clone());
            let mut s = slot.lock().unwrap_or_else(|e| e.into_inner());
            if s.0 != generation {
                return; // superseded; a started watcher is dropped here
            }
            match started {
                Ok(w) => s.1 = Some(w),
                Err(error) => {
                    drop(s);
                    if tx.send(Msg::Watch(WatchMsg::WatchFailed { workdir: target.workdir, error })).is_ok() {
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

/// Watches the work tree and git dir of `target` and, for a linked worktree, the refs, packed
/// refs, HEAD and config it shares through the common git dir. Symbolic links are not followed: one
/// pointing at a home folder would use up Linux's inotify watches, and what it points to isn't
/// part of the repository.
fn start<W: Waker + Clone>(target: &WatchTarget, tx: Sender<Msg>, waker: W) -> Result<RepoWatcher, String> {
    let (raw_tx, raw_rx) = std::sync::mpsc::channel::<notify::Result<Event>>();
    let describe = |p: &Path, e: notify::Error| format!("could not watch {}: {e}", glowfly_core::native_path::shown(p));
    let config = notify::Config::default().with_follow_symlinks(false);
    let mut inner = notify::RecommendedWatcher::new(raw_tx, config).map_err(|e| format!("could not start a file watcher: {e}"))?;
    inner.watch(&target.workdir, RecursiveMode::Recursive).map_err(|e| describe(&target.workdir, e))?;
    if !target.git_dir.starts_with(&target.workdir) {
        inner.watch(&target.git_dir, RecursiveMode::Recursive).map_err(|e| describe(&target.git_dir, e))?;
    }
    let common = glowfly_core::watch::common_dir(&target.git_dir);
    if let Some(common) = &common {
        let refs = common.join("refs");
        if refs.is_dir() {
            inner.watch(&refs, RecursiveMode::Recursive).map_err(|e| describe(&refs, e))?;
        }
        // HEAD and packed-refs sit in the folder itself.
        inner.watch(common, RecursiveMode::NonRecursive).map_err(|e| describe(common, e))?;
    }
    let target = target.clone();
    std::thread::Builder::new()
        .name("trellis-watch".into())
        .spawn(move || event_loop(raw_rx, target, common, tx, waker))
        .map_err(|e| format!("could not start the file watcher thread: {e}"))?;
    Ok(RepoWatcher { _inner: inner })
}

fn event_loop<W: Waker>(raw: Receiver<notify::Result<Event>>, target: WatchTarget, common: Option<PathBuf>, tx: Sender<Msg>, waker: W) {
    let mut paths = WatchPaths::new(&target.workdir, &target.git_dir);
    if let Some(c) = &common {
        paths = paths.with_common(c);
    }
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
                let (change, changed) = classify(&ev, &paths, &mut ignores);
                burst.add(change, changed, Instant::now());
            }
            Ok(Err(e)) => {
                if !reported {
                    reported = true;
                    let msg = Msg::Watch(WatchMsg::WatchFailed { workdir: target.workdir.clone(), error: e.to_string() });
                    if tx.send(msg).is_err() {
                        return;
                    }
                    waker.wake();
                }
                // Events may have been lost.
                burst.add(Change::Full, Vec::new(), Instant::now());
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        if let Some((change, changed, last_event)) = burst.take_due(Instant::now()) {
            let full = change == Change::Full;
            // Paths with none named (only folders' own modifications): read everything.
            let paths = (change == Change::Paths && !changed.is_empty()).then_some(changed);
            let count = paths.as_ref().map_or(0, Vec::len);
            tracing::debug!(workdir = %target.workdir.display(), full, paths = count, "files changed outside Trellis");
            let msg = Msg::Watch(WatchMsg::RepoChanged { full, paths, last_event });
            if tx.send(Msg::Scoped { workdir: target.workdir.clone(), msg: Box::new(msg) }).is_err() {
                return;
            }
            waker.wake();
        }
    }
}

/// What one raw event calls for, with the work-tree paths it names while that is only
/// `Change::Paths`.
fn classify(ev: &Event, paths: &WatchPaths, ignores: &mut Ignores) -> (Change, Vec<String>) {
    if matches!(ev.kind, EventKind::Access(_)) {
        return (Change::None, Vec::new());
    }
    if ev.need_rescan() {
        return (Change::Full, Vec::new()); // the OS dropped events
    }
    let mut change = Change::None;
    let mut changed = Vec::new();
    let what = touch(&ev.kind);
    for p in &ev.paths {
        let name = p.file_name().and_then(|n| n.to_str());
        let parent = p.parent().and_then(|d| d.file_name()).and_then(|n| n.to_str());
        if name == Some(".gitignore") || (name == Some("exclude") && parent == Some("info")) {
            ignores.clear();
        }
        let (c, rel) = paths.classify_path(p, what, &mut |rel| ignores.is_ignored(rel));
        change = change.max(c);
        // A folder's own modification (Windows reports one whenever an entry inside is written)
        // names no change of its own: the entry's event does.
        if let Some(rel) = rel.filter(|_| change == Change::Paths && !(what == Touch::Modified && p.is_dir())) {
            changed.push(rel);
        }
        if change == Change::Full {
            break;
        }
    }
    if change != Change::Paths {
        changed.clear();
    }
    (change, changed)
}

/// A content or metadata change, as opposed to a path coming, going or being renamed.
fn touch(kind: &EventKind) -> Touch {
    match kind {
        EventKind::Modify(ModifyKind::Name(_)) => Touch::Other,
        EventKind::Modify(_) => Touch::Modified,
        _ => Touch::Other,
    }
}

/// Relevant events waiting to be reported as one change.
#[derive(Default)]
struct Burst {
    first: Option<Instant>,
    last: Option<Instant>,
    change: Change,
    /// The work-tree paths seen while the burst is only `Change::Paths`.
    paths: BTreeSet<String>,
}

impl Burst {
    /// Adds an event naming `paths` at `at`; `Change::None` neither starts nor extends a burst.
    /// Past `MAX_WATCH_PATHS` paths, or once anything but paths changed, the burst re-reads the
    /// whole status and keeps no paths.
    fn add(&mut self, change: Change, paths: Vec<String>, at: Instant) {
        if change == Change::None {
            return;
        }
        self.first.get_or_insert(at);
        self.last = Some(at);
        self.change = self.change.max(change);
        if self.change != Change::Paths {
            self.paths.clear();
            return;
        }
        self.paths.extend(paths);
        if self.paths.len() > MAX_WATCH_PATHS {
            self.change = Change::Status;
            self.paths.clear();
        }
    }

    fn due_at(&self) -> Option<Instant> {
        Some((self.last? + QUIET).min(self.first? + MAX_WAIT))
    }

    /// How long until the burst is due; `None` when there is no burst.
    fn wait(&self, now: Instant) -> Option<Duration> {
        self.due_at().map(|d| d.saturating_duration_since(now))
    }

    /// The burst's change, its paths and its last event's time once it is due, starting over.
    fn take_due(&mut self, now: Instant) -> Option<(Change, Vec<String>, Instant)> {
        if now < self.due_at()? {
            return None;
        }
        let last = self.last?;
        let Burst { change, paths, .. } = std::mem::take(self);
        Some((change, paths.into_iter().collect(), last))
    }
}

#[cfg(test)]
mod tests;
