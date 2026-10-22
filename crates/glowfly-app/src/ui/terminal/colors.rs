//! Terminal colours: the background, text and 16 basic colours the terminal draws in
//! (`color::term`: the theme's own or the scheme chosen in Preferences), the xterm 256-colour
//! cube and grey ramp, and truecolour as given.

use crate::ui::theme::color;
use eframe::egui::Color32;
use glowfly_core::terminal::{CellStyle, TermColor, xterm_rgb};

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
    let term = color::term();
    let fg = resolve(fg, term.foreground);
    let bg = (style.bg != TermColor::Default).then(|| resolve(style.bg, term.background));
    if style.inverse { (bg.unwrap_or(term.background), Some(fg)) } else { (fg, bg) }
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

    #[test]
    fn a_chosen_scheme_replaces_the_theme_colours() {
        color::set_dark(true);
        let mut t = color::current().ansi;
        t.foreground = Color32::from_rgb(1, 2, 3);
        t.background = Color32::from_rgb(4, 5, 6);
        t.colors[1] = Color32::from_rgb(7, 8, 9);
        color::set_terminal(Some(t));
        assert_eq!(span_colors(&CellStyle::default()), (t.foreground, None));
        assert_eq!(span_colors(&CellStyle { inverse: true, ..Default::default() }), (t.background, Some(t.foreground)));
        assert_eq!(resolve(TermColor::Indexed(1), Color32::BLACK), t.colors[1]);
        color::set_terminal(None);
        assert_eq!(color::term(), color::current().ansi);
    }
}
