//! How often other worktrees' changes are counted again while the snapshot stays (after a
//! watcher or status-only reload): at most once per `RECOUNT_EVERY`, so many worktrees aren't
//! read on every file event. One asked for sooner, or while a count runs, is kept and runs once
//! the interval has passed (on a tick, or when that count ends). Returning to the tab or the
//! window counts at once (`forget`).

use std::time::{Duration, Instant};

/// The shortest time between two recounts.
pub const RECOUNT_EVERY: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Recount {
    /// When the last count started (a recount, or the load that counted).
    last: Option<Instant>,
    /// When the recount asked for meanwhile may run.
    due: Option<Instant>,
}

impl Recount {
    /// When a recount may start: `now`, or the end of the interval since the last one.
    fn next(&self, now: Instant) -> Instant {
        self.last.map_or(now, |l| (l + RECOUNT_EVERY).max(now))
    }

    /// A recount is asked for at `now`, with no count running: true when it may start now;
    /// otherwise it is kept for the end of the interval.
    pub fn ask(&mut self, now: Instant) -> bool {
        let next = self.next(now);
        if next > now {
            self.due = Some(next);
        }
        next <= now
    }

    /// A recount asked for at `now` while a count runs: kept for when it ends, after the
    /// interval.
    pub fn wait(&mut self, now: Instant) {
        self.due = Some(self.next(now));
    }

    /// A count of the other worktrees started at `now`.
    pub fn started(&mut self, now: Instant) {
        self.last = Some(now);
        self.due = None;
    }

    /// When the recount kept for later is due, if one is.
    pub fn due(&self) -> Option<Instant> {
        self.due
    }

    /// Whether the recount kept for later may run at `now`.
    pub fn is_due(&self, now: Instant) -> bool {
        self.due.is_some_and(|d| now >= d)
    }

    /// Nothing is left to count again: no recount is kept for later.
    pub fn drop_due(&mut self) {
        self.due = None;
    }

    /// The next recount runs at once (back in the window).
    pub fn forget(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_most_one_per_interval_and_the_last_ask_is_kept() {
        let t0 = Instant::now();
        let mut r = Recount::default();
        assert!(r.ask(t0), "the first runs at once");
        r.started(t0);
        assert!(!r.ask(t0 + Duration::from_secs(1)));
        assert_eq!(r.due(), Some(t0 + RECOUNT_EVERY), "kept for the end of the interval");
        assert!(!r.is_due(t0 + Duration::from_secs(9)));
        assert!(r.is_due(t0 + RECOUNT_EVERY));
        assert!(r.ask(t0 + RECOUNT_EVERY));
        r.started(t0 + RECOUNT_EVERY);
        assert_eq!(r.due(), None);

        r.wait(t0 + RECOUNT_EVERY * 3);
        assert_eq!(r.due(), Some(t0 + RECOUNT_EVERY * 3), "past the interval: as soon as the running count ends");
        r.forget();
        assert!(r.ask(t0 + RECOUNT_EVERY + Duration::from_secs(1)), "back in the window: at once");
    }
}
