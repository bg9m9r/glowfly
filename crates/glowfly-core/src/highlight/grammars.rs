//! The syntax set the parser runs on, with its compiled regexes, held only while highlighting is
//! in use. Syntect compiles a grammar's regexes the first time it parses that language, and each
//! regex keeps a search cache per thread that has used it. They live as long as the set: about
//! 9 MB for Rust and 19 MB for Markdown, plus up to 3 MB per highlighting thread. So the parse
//! set sits in a `Holder` that drops it (and everything compiled into it) once no highlight has
//! asked for it for `IDLE`; the next highlight loads it again from the dump (under 1 ms) and pays
//! the first-use compile again (about 75 ms for Rust or Markdown).
//!
//! Language detection uses the permanent set in `syntaxes` (`SYNTAXES`), which is never parsed
//! with, so its regexes stay uncompiled. Both sets come from the same dump, so a context id, a
//! `SyntaxReference` from one and a parse state built against either (checkpoints in `memo`
//! outlive a drop) are valid in the other.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};
use syntect::parsing::SyntaxSet;

/// How long the parse set is kept after the last highlight that asked for it.
pub(super) const IDLE: Duration = Duration::from_secs(3 * 60);
/// How often the reaper thread checks whether the set has been idle for `IDLE`.
const CHECK_EVERY: Duration = Duration::from_secs(30);

/// The parse set: see the module doc.
pub(super) static GRAMMARS: Holder<SyntaxSet> = Holder::new();

/// The syntax set to parse with, loaded if it was dropped (or never loaded). Hold the `Arc` for a
/// whole pass: it keeps the set alive however long the pass takes.
pub(super) fn grammars() -> Arc<SyntaxSet> {
    let (set, loaded) = GRAMMARS.get(Instant::now(), super::syntaxes::load_set);
    if loaded {
        spawn_reaper();
    }
    set
}

/// A thread that drops the parse set once it has been idle for `IDLE`, then ends; `grammars`
/// starts a new one with the next load. Without a thread (spawning failed) the set is just kept.
fn spawn_reaper() {
    let reaper = || {
        loop {
            std::thread::sleep(CHECK_EVERY);
            if GRAMMARS.release_idle(Instant::now(), IDLE) != Kept::Held {
                return;
            }
        }
    };
    if let Err(e) = std::thread::Builder::new().name("glowfly-grammars".into()).spawn(reaper) {
        tracing::debug!("no thread to release idle grammars: {e}");
    }
}

/// What `release_idle` did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Kept {
    /// Dropped (from the holder; a pass still running keeps its `Arc` until it ends).
    Released,
    /// Still held: used within the idle time.
    Held,
    /// Nothing was loaded.
    Empty,
}

/// A value loaded on first use and dropped after it has gone unused for a while (a cache: what
/// it holds never changes a result, only how soon one comes).
pub(super) struct Holder<T> {
    slot: Mutex<Option<(Arc<T>, Instant)>>,
}

impl<T> Holder<T> {
    pub(super) const fn new() -> Self {
        Holder { slot: Mutex::new(None) }
    }

    /// The value, loaded with `load` when there is none (then the flag is true), and marked used
    /// at `now`.
    pub(super) fn get(&self, now: Instant, load: impl FnOnce() -> T) -> (Arc<T>, bool) {
        let mut slot = self.slot.lock().unwrap_or_else(PoisonError::into_inner);
        let loaded = slot.is_none();
        let value = slot.take().map_or_else(|| Arc::new(load()), |(v, _)| v);
        *slot = Some((value.clone(), now));
        (value, loaded)
    }

    /// Drops the value when it was last asked for at least `idle` before `now`. A pass still
    /// running when it is dropped finishes with its own `Arc`, and the memory goes with it.
    pub(super) fn release_idle(&self, now: Instant, idle: Duration) -> Kept {
        let mut slot = self.slot.lock().unwrap_or_else(PoisonError::into_inner);
        match &*slot {
            None => Kept::Empty,
            Some((_, used)) if now.saturating_duration_since(*used) < idle => Kept::Held,
            Some(_) => {
                *slot = None;
                Kept::Released
            }
        }
    }
}
