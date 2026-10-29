//! The file watcher for the active tab's repository: notify reports raw events, a thread of its
//! own drops the ones that don't matter (`glowfly_core::watch`), coalesces the rest into bursts
//! and sends one message per burst: `WatchMsg::RepoChanged`, with the work-tree paths it saw when
//! nothing else changed, or `WatchMsg::IndexChanged` when the index changed too (with those
//! paths). Owned by the runner (`Effect::Watch`). On Windows the safety poll asks the same
//! thread to sweep the tree (`Effect::SweepWatch`, `glowfly_core::watch_sweep`) for changes the
//! watcher may have dropped; what it finds goes through the same classification and bursts.
//! Config files the repository's config includes are watched where they are (`includes.rs`).

use crate::effect::WatchTarget;
use crate::msg::Msg;
use crate::msg::WatchMsg;
use crate::runner::Waker;
use glowfly_core::watch::{Change, Ignores, MAX_WATCH_PATHS, Touch, WatchPaths};
use glowfly_core::watch_sweep::Sweep;
use notify::event::ModifyKind;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

mod includes;
use includes::IncludeWatches;

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
        let spawned = std::thread::Builder::new().name("glowfly-watch-start".into()).spawn(move || {
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

    /// Asks the live watcher of `workdir` to sweep its tree; false when no watcher of that
    /// repository is running (it failed or is still starting).
    pub fn sweep(&self, workdir: &Path) -> bool {
        let slot = self.0.lock().unwrap_or_else(|e| e.into_inner());
        slot.1.as_ref().is_some_and(|w| glowfly_core::paths::same_spelling(&w.workdir, workdir) && w.input.send(Input::Sweep).is_ok())
    }
}

/// What the event thread receives: notify's events, and sweep requests.
enum Input {
    Event(notify::Result<Event>),
    Sweep,
}

/// A running watcher. Dropping it stops notify and the sender, which ends the event thread
/// (which holds notify only weakly, to watch included config files' folders).
pub struct RepoWatcher {
    _inner: Arc<Mutex<notify::RecommendedWatcher>>,
    input: Sender<Input>,
    workdir: PathBuf,
}

/// Watches the work tree and git dir of `target` and, for a linked worktree, the refs, packed
/// refs, HEAD and config it shares through the common git dir. Symbolic links are not followed: one
/// pointing at a home folder would use up Linux's inotify watches, and what it points to isn't
/// part of the repository.
fn start<W: Waker + Clone>(target: &WatchTarget, tx: Sender<Msg>, waker: W) -> Result<RepoWatcher, String> {
    let (raw_tx, raw_rx) = std::sync::mpsc::channel::<Input>();
    let describe = |p: &Path, e: notify::Error| format!("could not watch {}: {e}", glowfly_core::native_path::shown(p));
    let config = notify::Config::default().with_follow_symlinks(false);
    let events = raw_tx.clone();
    let handler = move |r| {
        let _ = events.send(Input::Event(r));
    };
    let mut inner = notify::RecommendedWatcher::new(handler, config).map_err(|e| format!("could not start a file watcher: {e}"))?;
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
    let mut paths = WatchPaths::new(&target.workdir, &target.git_dir);
    if let Some(c) = &common {
        paths = paths.with_common(c);
    }
    let mut includes = IncludeWatches::default();
    includes.refresh(&mut inner, target, common.as_deref(), &mut paths);
    let inner = Arc::new(Mutex::new(inner));
    let notify = Arc::downgrade(&inner);
    let workdir = target.workdir.clone();
    let target = target.clone();
    let looped = Looped { target, common, paths, includes, notify };
    std::thread::Builder::new()
        .name("glowfly-watch".into())
        .spawn(move || event_loop(raw_rx, looped, tx, waker))
        .map_err(|e| format!("could not start the file watcher thread: {e}"))?;
    Ok(RepoWatcher { _inner: inner, input: raw_tx, workdir })
}

/// What the event thread works with.
struct Looped {
    target: WatchTarget,
    common: Option<PathBuf>,
    paths: WatchPaths,
    includes: IncludeWatches,
    notify: Weak<Mutex<notify::RecommendedWatcher>>,
}

impl Looped {
    /// After a reload of everything (the config may include other files now): follows the
    /// includes again.
    fn refresh_includes(&mut self) {
        let Some(inner) = self.notify.upgrade() else { return };
        let mut w = inner.lock().unwrap_or_else(|e| e.into_inner());
        self.includes.refresh(&mut *w, &self.target, self.common.as_deref(), &mut self.paths);
    }
}

fn event_loop<W: Waker>(raw: Receiver<Input>, mut looped: Looped, tx: Sender<Msg>, waker: W) {
    let (target, common) = (looped.target.clone(), looped.common.clone());
    let mut ignores = Ignores::open(&target.workdir);
    let mut sweep = Sweep::new(&target.workdir, &target.git_dir, common.as_deref());
    let mut burst = Burst::default();
    let mut reported = false;
    loop {
        let next = match burst.wait(Instant::now()) {
            None => raw.recv().map_err(|_| RecvTimeoutError::Disconnected),
            Some(d) => raw.recv_timeout(d),
        };
        match next {
            Ok(Input::Sweep) => swept(&mut sweep, &looped.paths, &mut ignores, &mut burst),
            Ok(Input::Event(Ok(ev))) => {
                let (change, changed) = classify(&ev, &looped.paths, &mut ignores);
                burst.add(change, changed, Instant::now());
            }
            Ok(Input::Event(Err(e))) => {
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
            let count = changed.len();
            tracing::debug!(workdir = %target.workdir.display(), ?change, paths = count, "files changed outside Glowfly");
            if change == Change::Full {
                looped.refresh_includes();
            }
            let msg = Msg::Watch(match change {
                Change::Index => WatchMsg::IndexChanged { paths: changed, last_event },
                // Paths with none named (only folders' own modifications): read everything.
                Change::Paths if !changed.is_empty() => WatchMsg::RepoChanged { full: false, paths: Some(changed), last_event },
                _ => WatchMsg::RepoChanged { full: change == Change::Full, paths: None, last_event },
            });
            if tx.send(Msg::Scoped { workdir: target.workdir.clone(), msg: Box::new(msg) }).is_err() {
                return;
            }
            waker.wake();
        }
    }
}

/// Runs one sweep of the tree into `burst`: the paths it found as events, or a whole read when
/// it has nothing to compare with (its first run, or a tree too large to sweep).
fn swept(sweep: &mut Sweep, paths: &WatchPaths, ignores: &mut Ignores, burst: &mut Burst) {
    let started = Instant::now();
    let found = sweep.sweep(ignores, std::time::SystemTime::now());
    tracing::debug!(took = ?started.elapsed(), found = found.as_ref().map(Vec::len), "swept the work tree");
    match found {
        None => burst.add(Change::Status, Vec::new(), Instant::now()),
        Some(found) => {
            for p in found {
                let (change, changed) = classify(&Event::new(EventKind::Any).add_path(p), paths, ignores);
                burst.add(change, changed, Instant::now());
            }
        }
    }
}

/// What one raw event calls for, with the work-tree paths it names while that is at most
/// `Change::Index`.
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
        let (c, rel) = paths.classify_path(p, what, &mut |rel| ignores.is_ignored(rel, what));
        // The index may have changed (a force-add makes an ignored file tracked): read it again
        // before the next ignored path is checked.
        if c >= Change::Index {
            ignores.index_changed();
        }
        change = change.max(c);
        // A folder's own modification (Windows reports one whenever an entry inside is written)
        // names no change of its own: the entry's event does.
        if let Some(rel) = rel.filter(|_| change <= Change::Index && !(what == Touch::Modified && p.is_dir())) {
            changed.push(rel);
        }
        if change == Change::Full {
            break;
        }
    }
    if change > Change::Index {
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
    /// The work-tree paths seen while the burst is at most `Change::Index`.
    paths: BTreeSet<String>,
}

impl Burst {
    /// Adds an event naming `paths` at `at`; `Change::None` neither starts nor extends a burst.
    /// Past `MAX_WATCH_PATHS` paths, or once anything but paths or the index changed, the burst
    /// re-reads the whole status and keeps no paths.
    fn add(&mut self, change: Change, paths: Vec<String>, at: Instant) {
        if change == Change::None {
            return;
        }
        self.first.get_or_insert(at);
        self.last = Some(at);
        self.change = self.change.max(change);
        if self.change > Change::Index {
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
#[path = "watcher/include_tests.rs"]
mod include_tests;
#[cfg(test)]
#[path = "watcher/sweep_tests.rs"]
mod sweep_tests;
#[cfg(test)]
mod tests;
