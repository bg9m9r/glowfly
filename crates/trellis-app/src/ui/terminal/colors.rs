//! Terminal colours: the 16 basic colours from Trellis's own theme tokens, the xterm 256-colour
//! cube and grey ramp, and truecolour as given.

use crate::ui::theme::color;
use eframe::egui::Color32;
use trellis_core::terminal::{CellStyle, TermColor, xterm_rgb};

/// The colour for `c`, with `default` for the program's default colour.
pub(super) fn resolve(c: TermColor, default: Color32) -> Color32 {
    match c {
        TermColor::Default => default,
        TermColor::Indexed(i) => xterm_rgb(i).map(|(r, g, b)| Color32::from_rgb(r, g, b)).unwrap_or_else(|| ansi(i)),
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

/// Basic colour `i` (0–15) in the active palette: 0–7 from the theme's tokens, 8–15 the same
/// hues a step brighter (lighter on dark, darker on light).
fn ansi(i: u8) -> Color32 {
    let dark = color::is_dark();
    let base = match i % 8 {
        0 if dark => color::toolbar(),
        0 => color::text(),
        1 => color::red(),
        2 => color::green(),
        3 => color::amber(),
        4 => color::blue(),
        5 => color::purple(),
        6 => color::accent(),
        _ if dark => color::text(),
        _ => color::weak(),
    };
    if i < 8 {
        base
    } else if dark {
        mix(base, Color32::WHITE, 0.25)
    } else {
        mix(base, Color32::BLACK, 0.25)
    }
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let ch = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgb(ch(a.r(), b.r()), ch(a.g(), b.g()), ch(a.b(), b.b()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bold_brightens_and_inverse_swaps() {
        color::set_dark(true);
        let red = resolve(TermColor::Indexed(1), Color32::BLACK);
        assert_eq!(red, color::red());
        let (bold, _) = span_colors(&CellStyle { fg: TermColor::Indexed(1), bold: true, ..Default::default() });
        assert_eq!(bold, resolve(TermColor::Indexed(9), Color32::BLACK));
        assert_ne!(bold, red);
        assert_eq!(resolve(TermColor::Indexed(196), Color32::BLACK), Color32::from_rgb(255, 0, 0));
        let (fg, bg) = span_colors(&CellStyle { inverse: true, ..Default::default() });
        assert_eq!((fg, bg), (color::input(), Some(color::text())));
    }
}
