//! Terminal colours: the 16 basic colours from the theme's own terminal table
//! (`color::ansi`), the xterm 256-colour cube and grey ramp, and truecolour as given.

use crate::ui::theme::color;
use eframe::egui::Color32;
use trellis_core::terminal::{CellStyle, TermColor, xterm_rgb};

/// The colour for `c`, with `default` for the program's default colour.
pub(super) fn resolve(c: TermColor, default: Color32) -> Color32 {
    match c {
        TermColor::Default => default,
        TermColor::Indexed(i) => xterm_rgb(i).map(|(r, g, b)| Color32::from_rgb(r, g, b)).unwrap_or_else(|| color::ansi(i)),
        TermColor::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
    }
}

/// The text colour and, unless it is the default, the background of a span: bold text in a
/// basic colour uses its bright variant, and inverse swaps the two.
pub(super) fn span_colors(style: &CellStyle) -> (Color32, Option<Color32>) {
    let fg = match style.fg {
        TermColor::Indexed(i) if style.bold && i < 8 => TermColor::Indexed(i + 8),
        other => other,
    };
    let fg = resolve(fg, color::text());
    let bg = (style.bg != TermColor::Default).then(|| resolve(style.bg, color::input()));
    if style.inverse { (bg.unwrap_or_else(color::input), Some(fg)) } else { (fg, bg) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_colours_come_from_the_theme_table() {
        color::set_dark(true);
        let red = resolve(TermColor::Indexed(1), Color32::BLACK);
        assert_eq!(red, color::ansi(1));
        let (bold, _) = span_colors(&CellStyle { fg: TermColor::Indexed(1), bold: true, ..Default::default() });
        assert_eq!(bold, color::ansi(9), "bold uses the bright colour");
        assert_ne!(bold, red);
        assert_eq!(resolve(TermColor::Indexed(196), Color32::BLACK), Color32::from_rgb(255, 0, 0));
        let (fg, bg) = span_colors(&CellStyle { inverse: true, ..Default::default() });
        assert_eq!((fg, bg), (color::input(), Some(color::text())));
        color::set_dark(false);
        assert_ne!(resolve(TermColor::Indexed(1), Color32::BLACK), red, "light has its own table");
        assert_eq!(resolve(TermColor::Indexed(1), Color32::BLACK), color::ansi(1));
    }
}
