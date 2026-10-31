//! Every drawer's default size and limits, in one place, and the arithmetic that fits the
//! drawers into the window each frame. Sizes are logical px (points). A drawer's size comes
//! from what it was dragged to (or its default for the window), clamped to its limits, and
//! the drawers sharing an axis then give way so the main view keeps `MAIN_MIN_*`: an old,
//! too-small saved size or a shrunken window is repaired here, never stored.

use glowfly_core::settings::SizedPanel;

/// The graph, diff or other main view never gets narrower than this while the window allows:
/// below it the commit messages are cut to a few words.
pub const MAIN_MIN_WIDTH: f32 = 360.0;
/// The main view's least height beside the bottom drawers: a handful of graph rows.
pub const MAIN_MIN_HEIGHT: f32 = 160.0;

/// One drawer's size rules along its axis. `room` is the length of that axis the drawer
/// shares with the main view (and any drawer beside it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spec {
    /// Below this the content is unusable.
    pub min: f32,
    /// The default is this share of `room`, kept within `default_lo..=default_hi`.
    pub default_frac: f32,
    pub default_lo: f32,
    pub default_hi: f32,
    /// The drawer may take at most this share of `room`, and never more than `max_abs`.
    pub max_frac: f32,
    pub max_abs: f32,
}

impl Spec {
    pub fn default_for(&self, room: f32) -> f32 {
        (room * self.default_frac).clamp(self.default_lo, self.default_hi)
    }

    /// The largest size for `room`; at least `min` (the fit gives way below that).
    pub fn max_for(&self, room: f32) -> f32 {
        (room * self.max_frac).min(self.max_abs).max(self.min)
    }

    /// A dragged size as it is saved: whole px within `min..=max_abs` (None for a non-number).
    pub fn sanitize(&self, v: f32) -> Option<f32> {
        v.is_finite().then(|| v.round().clamp(self.min, self.max_abs))
    }

    /// The `lo..=hi` a secondary drawer that egui sizes by itself may be dragged within.
    pub fn range_for(&self, room: f32) -> (f32, f32) {
        let hi = self.max_for(room.max(0.0));
        (self.min.min(hi), hi)
    }
}

/// Left panel of refs: branch names stay readable at 180; past 40% it crowds the graph.
pub const SIDEBAR: Spec = Spec { min: 180.0, default_frac: 0.15, default_lo: 260.0, default_hi: 320.0, max_frac: 0.4, max_abs: 560.0 };
/// Commit details: 260 keeps a title, the author line and file names legible; it grows with
/// wide windows (400 at 1600, 520 from 2080) and may take up to half the window.
pub const DETAILS: Spec = Spec { min: 260.0, default_frac: 0.25, default_lo: 400.0, default_hi: 520.0, max_frac: 0.5, max_abs: 900.0 };
/// Built-in terminal: 120 keeps the header and a few lines of output.
pub const TERMINAL: Spec = Spec { min: 120.0, default_frac: 0.3, default_lo: 220.0, default_hi: 360.0, max_frac: 0.7, max_abs: 1200.0 };
/// Logs panel: 100 keeps the tab row and a few entries; the default shows about five.
pub const ACTIVITY: Spec = Spec { min: 100.0, default_frac: 0.25, default_lo: 180.0, default_hi: 300.0, max_frac: 0.5, max_abs: 800.0 };
/// File history's commit list (inside the main view): room for a summary, author and date.
pub const HISTORY_LIST: Spec = Spec { min: 260.0, default_frac: 0.0, default_lo: 380.0, default_hi: 380.0, max_frac: 0.5, max_abs: 600.0 };
/// An issue's side column: labels, assignees and dates.
pub const ISSUE_SIDE: Spec = Spec { min: 260.0, default_frac: 0.0, default_lo: 320.0, default_hi: 320.0, max_frac: 0.45, max_abs: 520.0 };
/// A pull request's side column: checks, reviewers and the file list.
pub const PR_SIDE: Spec = Spec { min: 280.0, default_frac: 0.0, default_lo: 380.0, default_hi: 380.0, max_frac: 0.5, max_abs: 600.0 };
/// A pull request file's review threads under its diff: a comment and its reply box.
pub const PR_THREADS: Spec = Spec { min: 100.0, default_frac: 0.0, default_lo: 220.0, default_hi: 220.0, max_frac: 0.6, max_abs: 480.0 };
/// The conflict view's output editor: the panes above keep `CONFLICT_PANES_MIN`.
pub const CONFLICT_OUTPUT: Spec =
    Spec { min: 90.0, default_frac: 0.45, default_lo: 90.0, default_hi: f32::INFINITY, max_frac: 1.0, max_abs: f32::INFINITY };
/// The conflict view's ours/theirs panes above the output: a few lines each.
pub const CONFLICT_PANES_MIN: f32 = 140.0;

/// The rules of a drawer whose dragged size is saved in the settings.
pub fn spec(p: SizedPanel) -> Spec {
    match p {
        SizedPanel::Sidebar => SIDEBAR,
        SizedPanel::Details => DETAILS,
        SizedPanel::Activity => ACTIVITY,
    }
}

/// A drawer's size this frame and the range a drag of its edge may move it in (which keeps
/// the main view and the drawers beside it at their sizes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    pub size: f32,
    pub lo: f32,
    pub hi: f32,
}

/// Fits drawers sharing one axis of length `room` with a main view that keeps `main_min`.
/// Each drawer is `(spec, saved size)`; the result is in the same order. Each size starts
/// as the saved one (or the default) within its limits; when they don't all fit, each gives
/// way toward its minimum in proportion to its slack, and when even the minimums don't fit
/// they shrink together so the main view keeps priority. Sizes are never negative.
pub fn fit(room: f32, main_min: f32, drawers: &[(Spec, Option<f32>)]) -> Vec<Fit> {
    let room = if room.is_finite() { room.max(0.0) } else { 0.0 };
    let budget = (room - main_min).max(0.0);
    let mut sizes: Vec<f32> = drawers
        .iter()
        .map(|(s, saved)| saved.filter(|v| v.is_finite()).unwrap_or_else(|| s.default_for(room)).clamp(s.min, s.max_for(room)))
        .collect();
    let total: f32 = sizes.iter().sum();
    if total > budget {
        let slack: f32 = sizes.iter().zip(drawers).map(|(w, (s, _))| w - s.min).sum();
        let excess = total - budget;
        if slack >= excess {
            for (w, (s, _)) in sizes.iter_mut().zip(drawers) {
                *w -= excess * (*w - s.min) / slack;
            }
        } else {
            let mins: f32 = drawers.iter().map(|(s, _)| s.min).sum();
            let scale = if mins > 0.0 { budget / mins } else { 0.0 };
            for (w, (s, _)) in sizes.iter_mut().zip(drawers) {
                *w = (s.min * scale).floor();
            }
        }
    }
    let used: f32 = sizes.iter().sum();
    sizes
        .iter()
        .zip(drawers)
        .map(|(&size, (s, _))| {
            let lo = s.min.min(size);
            let others = used - size;
            let hi = s.max_for(room).min(budget - others).max(lo).max(size);
            Fit { size, lo, hi }
        })
        .collect()
}

/// Fits one drawer (see `fit`).
pub fn fit_one(room: f32, main_min: f32, spec: Spec, saved: Option<f32>) -> Fit {
    fit(room, main_min, &[(spec, saved)])[0]
}

#[cfg(test)]
#[path = "panel_layout_tests.rs"]
mod tests;
