//! Per-frame drawing metrics of the diff viewer, from Preferences > Diff viewer: the text font, the
//! row height and the gutter positions, which scale with the font size so the default size
//! draws exactly as the fixed layout did.

use eframe::egui::FontId;
use trellis_core::settings::{DEFAULT_DIFF_FONT_SIZE, Settings};

pub(in crate::ui) struct DiffStyle {
    pub font: FontId,
    pub row_h: f32,
    /// Scale of the gutter positions: the font size over the default size.
    pub k: f32,
    /// Columns per tab stop.
    pub tab: usize,
    /// Whether the line-number columns are drawn.
    pub numbers: bool,
    /// Whether changed words are filled.
    pub words: bool,
    /// Whether long lines wrap to the viewer's width.
    pub wrap: bool,
}

/// Inline gutter: the right edges of the old and new numbers (`None` when hidden), then the
/// left edges of the sign and the text, relative to the row's left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::ui) struct InlineCols {
    pub old: Option<f32>,
    pub new: Option<f32>,
    pub sign: f32,
    pub text: f32,
}

/// One side of the split layout: the number's right edge (`None` when hidden), then the left
/// edges of the sign and the text, relative to the cell's left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::ui) struct SplitCols {
    pub num: Option<f32>,
    pub sign: f32,
    pub text: f32,
}

impl DiffStyle {
    pub fn new(s: &Settings) -> Self {
        let size = s.diff_font_px();
        Self {
            font: FontId::monospace(size),
            row_h: (size + 6.5).round(),
            k: size / DEFAULT_DIFF_FONT_SIZE,
            tab: s.tab_columns(),
            numbers: s.diff_line_numbers,
            words: s.diff_word_highlights,
            wrap: s.diff_word_wrap,
        }
    }

    pub fn inline_cols(&self) -> InlineCols {
        let k = self.k;
        if self.numbers {
            InlineCols { old: Some(40.0 * k), new: Some(84.0 * k), sign: 94.0 * k, text: 110.0 * k }
        } else {
            InlineCols { old: None, new: None, sign: 8.0 * k, text: 24.0 * k }
        }
    }

    pub fn split_cols(&self) -> SplitCols {
        let k = self.k;
        if self.numbers {
            SplitCols { num: Some(40.0 * k), sign: 48.0 * k, text: 62.0 * k }
        } else {
            SplitCols { num: None, sign: 8.0 * k, text: 22.0 * k }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_size_keeps_the_fixed_layout() {
        let st = DiffStyle::new(&Settings::default());
        assert_eq!((st.row_h, st.k, st.tab), (19.0, 1.0, 4));
        assert_eq!(st.font, FontId::monospace(12.5));
        assert_eq!(st.inline_cols(), InlineCols { old: Some(40.0), new: Some(84.0), sign: 94.0, text: 110.0 });
        assert_eq!(st.split_cols().text, 62.0);
        assert!(st.numbers && st.words);
        assert!(!st.wrap, "lines don't wrap by default");
        assert!(DiffStyle::new(&Settings { diff_word_wrap: true, ..Settings::default() }).wrap);
    }

    #[test]
    fn hiding_numbers_moves_the_text_left() {
        let st = DiffStyle::new(&Settings { diff_line_numbers: false, ..Settings::default() });
        let c = st.inline_cols();
        assert_eq!((c.old, c.new, c.text), (None, None, 24.0));
        assert_eq!(st.split_cols(), SplitCols { num: None, sign: 8.0, text: 22.0 });
    }

    #[test]
    fn larger_font_scales_rows_and_gutters() {
        let st = DiffStyle::new(&Settings { diff_font_size: 16.0, ..Settings::default() });
        assert_eq!(st.row_h, 23.0);
        assert!((st.inline_cols().text - 140.8).abs() < 1e-4);
        let clamped = DiffStyle::new(&Settings { diff_font_size: f32::NAN, tab_size: 0, ..Settings::default() });
        assert_eq!((clamped.row_h, clamped.tab), (19.0, 1));
    }
}
