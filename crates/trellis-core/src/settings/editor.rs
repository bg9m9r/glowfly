//! Diff text size and tab stops (Preferences > Editor).

use super::Settings;
use std::ops::RangeInclusive;

/// The diff viewer's text size when nothing else is set.
pub const DEFAULT_DIFF_FONT_SIZE: f32 = 12.5;
/// The diff text sizes Preferences > Editor offers.
pub const DIFF_FONT_SIZES: RangeInclusive<f32> = 8.0..=24.0;
/// The tab sizes Preferences > Editor offers.
pub const TAB_SIZES: RangeInclusive<u8> = 1..=16;

impl Settings {
    /// The diff text size, kept within `DIFF_FONT_SIZES` (the default for a non-number from a
    /// hand-edited file).
    pub fn diff_font_px(&self) -> f32 {
        if !self.diff_font_size.is_finite() {
            return DEFAULT_DIFF_FONT_SIZE;
        }
        self.diff_font_size.clamp(*DIFF_FONT_SIZES.start(), *DIFF_FONT_SIZES.end())
    }

    /// Columns per tab stop, kept within `TAB_SIZES`.
    pub fn tab_columns(&self) -> usize {
        self.tab_size.clamp(*TAB_SIZES.start(), *TAB_SIZES.end()) as usize
    }
}
