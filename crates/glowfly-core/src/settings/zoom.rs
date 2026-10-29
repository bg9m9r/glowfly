//! Window zoom levels (keys, palette and Preferences > Appearance).

use super::Settings;

/// The zoom levels in percent, smallest first.
pub const ZOOM_STEPS: [u16; 11] = [50, 67, 75, 80, 90, 100, 110, 125, 150, 175, 200];

/// The zoom level when nothing else is set.
pub const DEFAULT_ZOOM_PERCENT: u16 = 100;

/// The step nearest to `p` (a hand-edited value); halfway between two steps picks the larger.
pub fn snap_zoom(p: u16) -> u16 {
    let mut best = ZOOM_STEPS[0];
    for s in ZOOM_STEPS {
        if s.abs_diff(p) <= best.abs_diff(p) {
            best = s;
        }
    }
    best
}

/// One step larger than `p`, or the largest step when there is none.
pub fn zoom_in_from(p: u16) -> u16 {
    ZOOM_STEPS.into_iter().find(|&s| s > p).unwrap_or(ZOOM_STEPS[ZOOM_STEPS.len() - 1])
}

/// One step smaller than `p`, or the smallest step when there is none.
pub fn zoom_out_from(p: u16) -> u16 {
    ZOOM_STEPS.into_iter().rev().find(|&s| s < p).unwrap_or(ZOOM_STEPS[0])
}

impl Settings {
    /// The zoom in percent, snapped to one of `ZOOM_STEPS`.
    pub fn zoom_level(&self) -> u16 {
        snap_zoom(self.zoom_percent)
    }

    /// The zoom as a factor (1.0 is 100%).
    pub fn zoom_factor(&self) -> f32 {
        self.zoom_level() as f32 / 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_picks_the_nearest_step() {
        assert_eq!(snap_zoom(100), 100);
        assert_eq!(snap_zoom(0), 50);
        assert_eq!(snap_zoom(1000), 200);
        assert_eq!(snap_zoom(130), 125);
        assert_eq!(snap_zoom(140), 150);
        // 71 is 4 from both 67 and 75: a tie goes to the larger step.
        assert_eq!(snap_zoom(71), 75);
    }

    #[test]
    fn steps_move_one_and_stop_at_the_ends() {
        assert_eq!(zoom_in_from(100), 110);
        assert_eq!(zoom_in_from(200), 200);
        assert_eq!(zoom_in_from(130), 150);
        assert_eq!(zoom_out_from(100), 90);
        assert_eq!(zoom_out_from(50), 50);
        assert_eq!(zoom_out_from(130), 125);
    }

    #[test]
    fn factor_follows_the_level() {
        assert_eq!(Settings::default().zoom_factor(), 1.0);
        let s = Settings { zoom_percent: 125, ..Settings::default() };
        assert_eq!(s.zoom_factor(), 1.25);
        let s = Settings { zoom_percent: 1000, ..Settings::default() };
        assert_eq!(s.zoom_level(), 200);
    }
}
