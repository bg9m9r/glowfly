//! The event thread's side of the watcher's self-test: right after the watches are registered
//! it places a probe file in the git dir (`glowfly_core::watch_probe`) and waits for notify to
//! report it. The git dir sits inside the recursively watched work tree, or for a linked
//! worktree or a separate git dir has a recursive watch of its own on the same watcher, so the
//! probe's event takes the path work-tree events take without Glowfly writing to the work
//! tree. Seen: `Probe::Live`. Not seen within `PROBE_TIMEOUT`: `Probe::Silent` (and any event
//! after that sends `Live` after all). No file could be placed (a read-only git dir):
//! `Probe::Unknown`.

use crate::state::watch::{PROBE_TIMEOUT, Probe};
use glowfly_core::watch_probe::ProbeFile;
use notify::Event;
use std::path::Path;
use std::time::{Duration, Instant};

/// Where the probe stands.
#[derive(Debug)]
pub(super) enum Prober {
    /// The file is out; its event is due by `deadline`.
    Waiting { file: ProbeFile, deadline: Instant },
    /// No event came in time: the next one of any kind shows that events do arrive.
    Silent,
    /// Answered for this watcher.
    Done,
}

impl Prober {
    /// Places the probe in `git_dir`; with no file, `Done` and `Probe::Unknown` to report.
    pub(super) fn start(git_dir: &Path, now: Instant) -> (Prober, Option<Probe>) {
        match ProbeFile::place(git_dir) {
            Some(file) => (Prober::Waiting { file, deadline: now + PROBE_TIMEOUT }, None),
            None => (Prober::Done, Some(Probe::Unknown)),
        }
    }

    /// How long until the probe times out; None when nothing is waiting.
    pub(super) fn wait(&self, now: Instant) -> Option<Duration> {
        match self {
            Prober::Waiting { deadline, .. } => Some(deadline.saturating_duration_since(now)),
            _ => None,
        }
    }

    /// A raw event arrived: `Live` when it is the probe's (removing the file) or the first of
    /// any kind after a timeout.
    pub(super) fn event(&mut self, ev: &Event) -> Option<Probe> {
        let live = match self {
            Prober::Waiting { file, .. } => ev.paths.iter().any(|p| file.is(p)),
            Prober::Silent => true,
            Prober::Done => false,
        };
        live.then(|| {
            *self = Prober::Done; // drops (removes) the file
            Probe::Live
        })
    }

    /// `Silent` once the probe has waited out its time (removing the file).
    pub(super) fn due(&mut self, now: Instant) -> Option<Probe> {
        let late = matches!(self, Prober::Waiting { deadline, .. } if now >= *deadline);
        late.then(|| {
            *self = Prober::Silent;
            Probe::Silent
        })
    }
}

/// The sooner of two waits.
pub(super) fn sooner(a: Option<Duration>, b: Option<Duration>) -> Option<Duration> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

#[cfg(test)]
#[path = "probe_tests.rs"]
mod tests;
