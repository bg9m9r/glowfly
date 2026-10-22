//! The shell's frame timing: each phase of a frame and each message's handling are timed and
//! traced (`tracing` spans "frame", "phase" and "update"), and the frame's total goes to
//! `state/frame_times.rs` for the overlay and, when slow, to the performance log. Also when the
//! next frame is due while nothing happens (`schedule_repaints`).

use super::GlowflyApp;
use eframe::egui;
use std::time::{Duration, Instant};

/// The frame-time overlay, drawn by the shell over the window.
#[path = "frame_overlay.rs"]
mod frame_overlay;

impl GlowflyApp {
    /// The frame-time overlay over everything `ui::draw` drew (when Preferences turns it on).
    pub(super) fn draw_overlay(&self, ctx: &egui::Context) {
        frame_overlay::show(ctx, &self.st);
    }

    /// Runs `f` as the frame phase `name`: timed, and traced as a span.
    pub(super) fn phase<R>(&mut self, name: &'static str, f: impl FnOnce(&mut Self) -> R) -> R {
        let _span = tracing::trace_span!("phase", name).entered();
        let started = Instant::now();
        let r = f(self);
        self.frames.phase(name, started.elapsed());
        r
    }

    /// Records the frame begun at `started`: its times for the overlay, and a slow one for the
    /// performance log.
    pub(super) fn end_frame(&mut self, started: Instant) {
        let took = started.elapsed();
        if let Some(slow) = self.frames.finish(took, Instant::now()) {
            tracing::debug!(ms = took.as_millis(), "slow frame: {}", slow.detail);
            self.runner.log_perf(slow.took, "frame", slow.detail);
        }
        self.st.view.frame_times = self.frames.summary();
    }

    /// Asks for the frames that come without input: while work runs or toasts fade, for
    /// auto-fetch, the Saved tick, the status poll and the retry of a failed load for more.
    pub(super) fn schedule_repaints(&self, ctx: &egui::Context, now: Instant) {
        // Keep polling while work is in flight or toasts are fading.
        if self.st.job.is_some() || self.st.loading || !self.st.toasts.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(150));
        } else if self.st.settings.auto_fetch_minutes > 0 {
            ctx.request_repaint_after(Duration::from_secs(5));
        }
        // The "Saved" tick beside a Preferences title fades out.
        if self.st.prefs.as_ref().is_some_and(|p| p.saved.showing() || p.saved.pending) {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        // The status is re-read on a timer where the watcher can be silent (see `poll_every`).
        if let Some(every) = self.st.watch.poll_interval() {
            ctx.request_repaint_after(every);
        }
        // A failed load for more tries again by itself (`Msg::Tick`) at this time.
        if let Some(at) = self.st.refresh.more_retry_at {
            ctx.request_repaint_after(at.saturating_duration_since(now));
        }
    }
}
