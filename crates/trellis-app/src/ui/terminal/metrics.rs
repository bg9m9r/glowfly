//! The terminal grid's cell size: the monospace advance rounded to whole device pixels, which is
//! where epaint really places each glyph of a run, so text, cursor and mouse share one grid.

use eframe::egui::FontId;
use eframe::egui::epaint::text::Fonts;

/// The width of one column: epaint rounds the pen position to a whole device pixel after every
/// glyph, so a run of text advances by the rounded advance, not by the font's exact one.
pub(crate) fn cell_width(fonts: &Fonts, font: &FontId) -> f32 {
    let ppp = fonts.pixels_per_point();
    ((fonts.glyph_width(font, 'M') * ppp).round() / ppp).max(1.0 / ppp)
}

/// (column width, row height) of the terminal grid in points.
pub(crate) fn cell_size(fonts: &Fonts, font: &FontId) -> (f32, f32) {
    (cell_width(fonts, font), fonts.row_height(font).round())
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Color32;
    use trellis_core::settings::TERMINAL_FONT_SIZES;

    fn fonts(ppp: f32) -> Fonts {
        Fonts::new(ppp, 8192, crate::ui::theme::fonts::definitions().clone())
    }

    #[test]
    fn ascii_runs_land_on_their_cells() {
        let text = "PS C:\\work\\src> 0123456789:".repeat(5);
        let text: String = text.chars().take(120).collect();
        for ppp in [1.0, 1.25, 1.5, 1.75, 2.0, 2.25] {
            let fonts = fonts(ppp);
            let mut size = *TERMINAL_FONT_SIZES.start();
            while size <= *TERMINAL_FONT_SIZES.end() {
                let font = FontId::monospace(size);
                let cw = cell_width(&fonts, &font);
                let galley = fonts.layout_no_wrap(text.clone(), font.clone(), Color32::WHITE);
                let glyphs = &galley.rows[0].glyphs;
                assert_eq!(glyphs.len(), 120);
                for (k, g) in glyphs.iter().enumerate() {
                    let off = g.pos.x - k as f32 * cw;
                    assert!(off.abs() < 0.01, "ppp {ppp}, {size} px: glyph {k} is {off} px off its cell");
                }
                let exact = fonts.glyph_width(&font, 'M');
                assert!((cw - exact).abs() <= 0.5 / ppp + 1e-4, "ppp {ppp}, {size} px: {cw} vs {exact}");
                size += 0.5;
            }
        }
    }

    #[test]
    fn cell_size_rounds_the_row_height() {
        let fonts = fonts(1.0);
        let font = FontId::monospace(12.5);
        assert_eq!(cell_size(&fonts, &font).1, fonts.row_height(&font).round());
    }
}
