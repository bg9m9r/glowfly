//! How long the UI thread spends on each frame (ui/app.rs times them): the last, average and
//! slowest of the recent frames for the frame-time overlay (Preferences > General >
//! Diagnostics), and a line for the performance log when a frame takes `SLOW_FRAME` or more,
//! saying which phases and message areas the time went to. At most one line per
//! `SLOW_LOG_EVERY`; the frames skipped meanwhile are counted on the next line.
//!
//! Timing costs a few clock reads per frame and one per message, so it is always on; only the
//! overlay is a setting.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// A frame at least this long is logged.
pub const SLOW_FRAME: Duration = Duration::from_millis(50);
/// The shortest gap between two slow-frame lines.
pub const SLOW_LOG_EVERY: Duration = Duration::from_secs(2);
/// How many recent frames the average and the slowest are taken over.
pub const KEPT_FRAMES: usize = 120;
/// How many message areas a slow-frame line names, slowest first.
const AREAS_NAMED: usize = 3;

/// The recent frames, as the overlay shows them (milliseconds).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameSummary {
    pub last_ms: f32,
    pub avg_ms: f32,
    pub max_ms: f32,
}

/// A slow frame for the performance log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlowFrame {
    pub took: Duration,
    /// Where the time went, e.g. "results 2 ms, draw 41 ms, messages 9 ms; slowest: repo 8 ms".
    pub detail: String,
}

/// The recent frames and the one being timed.
#[derive(Default)]
pub struct FrameTimes {
    recent: VecDeque<Duration>,
    /// The frame's phases in the order they ran.
    phases: Vec<(&'static str, Duration)>,
    /// Time spent handling messages, by area, in the order first seen.
    areas: Vec<(&'static str, Duration)>,
    last_logged: Option<Instant>,
    /// Slow frames not logged since the last line.
    skipped: u32,
}

impl FrameTimes {
    /// Forgets the last frame's phases and areas.
    pub fn begin(&mut self) {
        self.phases.clear();
        self.areas.clear();
    }

    /// One phase of the frame took `took`.
    pub fn phase(&mut self, name: &'static str, took: Duration) {
        self.phases.push((name, took));
    }

    /// Handling a message of `area` took `took` (added to the area's time this frame).
    pub fn area(&mut self, area: &'static str, took: Duration) {
        match self.areas.iter_mut().find(|(a, _)| *a == area) {
            Some((_, t)) => *t += took,
            None => self.areas.push((area, took)),
        }
    }

    /// The frame took `took` and ended at `now`: kept for the summary, and returned as a line
    /// for the performance log when it is slow and no line went out in the last
    /// `SLOW_LOG_EVERY`.
    pub fn finish(&mut self, took: Duration, now: Instant) -> Option<SlowFrame> {
        if self.recent.len() == KEPT_FRAMES {
            self.recent.pop_front();
        }
        self.recent.push_back(took);
        if took < SLOW_FRAME {
            return None;
        }
        if self.last_logged.is_some_and(|at| now.saturating_duration_since(at) < SLOW_LOG_EVERY) {
            self.skipped += 1;
            return None;
        }
        self.last_logged = Some(now);
        let detail = self.detail();
        self.skipped = 0;
        Some(SlowFrame { took, detail })
    }

    fn detail(&self) -> String {
        let ms = |d: &Duration| d.as_millis();
        let mut out = self.phases.iter().map(|(n, d)| format!("{n} {} ms", ms(d))).collect::<Vec<_>>().join(", ");
        let mut areas: Vec<_> = self.areas.iter().filter(|(_, d)| !d.is_zero()).collect();
        areas.sort_by_key(|a| std::cmp::Reverse(a.1));
        if !areas.is_empty() {
            let named: Vec<_> = areas.iter().take(AREAS_NAMED).map(|(n, d)| format!("{n} {} ms", ms(d))).collect();
            out.push_str(&format!("; slowest messages: {}", named.join(", ")));
        }
        if self.skipped > 0 {
            out.push_str(&format!("; {} more slow frames since the last line", self.skipped));
        }
        out
    }

    /// The last, average and slowest of the recent frames.
    pub fn summary(&self) -> FrameSummary {
        let ms = |d: Duration| d.as_secs_f32() * 1000.0;
        let Some(last) = self.recent.back() else { return FrameSummary::default() };
        let total: Duration = self.recent.iter().sum();
        FrameSummary {
            last_ms: ms(*last),
            avg_ms: ms(total) / self.recent.len() as f32,
            max_ms: self.recent.iter().copied().max().map(ms).unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    #[test]
    fn the_summary_covers_the_recent_frames() {
        let mut f = FrameTimes::default();
        assert_eq!(f.summary(), FrameSummary::default());
        let now = Instant::now();
        for ms in [2, 4, 12] {
            assert!(f.finish(MS * ms, now).is_none(), "fast frames aren't logged");
        }
        assert_eq!(f.summary(), FrameSummary { last_ms: 12.0, avg_ms: 6.0, max_ms: 12.0 });
        for _ in 0..KEPT_FRAMES {
            f.finish(MS, now);
        }
        assert_eq!(f.summary(), FrameSummary { last_ms: 1.0, avg_ms: 1.0, max_ms: 1.0 }, "older frames drop out");
    }

    #[test]
    fn a_slow_frame_names_its_phases_and_slowest_areas() {
        let mut f = FrameTimes::default();
        f.begin();
        f.phase("results", MS * 3);
        f.phase("draw", MS * 40);
        f.phase("messages", MS * 20);
        for (area, ms) in [("tick", 1), ("repo", 12), ("search", 5), ("repo", 3), ("diff", 2), ("watch", 0)] {
            f.area(area, MS * ms);
        }
        let slow = f.finish(MS * 63, Instant::now()).expect("logged");
        assert_eq!(slow.took, MS * 63);
        assert_eq!(slow.detail, "results 3 ms, draw 40 ms, messages 20 ms; slowest messages: repo 15 ms, search 5 ms, diff 2 ms");
    }

    #[test]
    fn slow_frames_are_logged_at_most_once_per_interval_and_counted_meanwhile() {
        let mut f = FrameTimes::default();
        let t0 = Instant::now();
        f.begin();
        f.phase("draw", MS * 60);
        assert!(f.finish(MS * 60, t0).is_some());
        assert!(f.finish(MS * 70, t0 + MS * 500).is_none());
        assert!(f.finish(MS * 80, t0 + MS * 1500).is_none());
        assert!(f.finish(MS * 10, t0 + MS * 1900).is_none(), "fast");
        let next = f.finish(MS * 55, t0 + SLOW_LOG_EVERY).expect("the interval passed");
        assert_eq!(next.detail, "draw 60 ms; 2 more slow frames since the last line");
        f.begin();
        let after = f.finish(MS * 55, t0 + SLOW_LOG_EVERY * 2).expect("logged");
        assert_eq!(after.detail, "", "the count starts again");
    }
}
