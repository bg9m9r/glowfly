//! The type scale: five text sizes and nothing smaller than `FLOOR`. Views name a step (`font`,
//! `mono`, `strong`) instead of a pixel size; `install` sets egui's own text styles (`Body`,
//! `Button`, `Small`, ...) to the same steps, so egui's widgets match.
//!
//! - `CAPTION` (11): badges, tiny marks, column titles
//! - `SMALL` (12): secondary text, hints, counters, toolbar captions, tooltip footers
//! - `BODY` (14): labels, buttons, menus, list rows, fields, paths and inline code
//! - `SUBHEAD` (16): panel and dialog titles, file-path headers, the commit summary
//! - `TITLE` (20): page titles
//!
//! Headers are marked by weight: `section` is the one section-header style (SemiBold at body
//! size, in `color::text()`), and titles are SemiBold through `strong`. The semibold family is
//! not put into egui's text styles: egui panics on a family before the fonts arrive on the next
//! pass, which `fonts::semibold` guards against.

use super::{color, fonts};
use eframe::egui::{self, FontId, RichText, TextStyle, Ui};

/// Badges, tiny marks and column titles; also the floor.
pub const CAPTION: f32 = 11.0;
/// Secondary text: help lines, hints, counters, captions.
pub const SMALL: f32 = 12.0;
/// Labels, buttons, menus, list rows, fields, paths and inline code.
pub const BODY: f32 = 14.0;
/// Panel and dialog titles, file-path headers, the commit summary.
pub const SUBHEAD: f32 = 16.0;
/// Page titles.
pub const TITLE: f32 = 20.0;
/// The five steps, smallest first.
pub const STEPS: [f32; 5] = [CAPTION, SMALL, BODY, SUBHEAD, TITLE];
/// No text is drawn smaller.
const FLOOR: f32 = CAPTION;
/// The welcome screen's wordmark: the logo, not a text step.
pub const WORDMARK: f32 = 2.0 * TITLE;

/// The names of the extra egui text styles (`TextStyle::Name`).
const CAPTION_STYLE: &str = "caption";
const SMALL_STYLE: &str = "small";
const SUBHEAD_STYLE: &str = "subhead";

/// Sets egui's text styles to the scale: `Small` is the caption step, `Body`, `Button` and
/// `Monospace` the body step, `Heading` the title step, plus the named caption, small and
/// subhead styles.
pub fn install(style: &mut egui::Style) {
    let styles = [
        (TextStyle::Small, font(CAPTION)),
        (TextStyle::Body, font(BODY)),
        (TextStyle::Button, font(BODY)),
        (TextStyle::Monospace, mono(BODY)),
        (TextStyle::Heading, font(TITLE)),
        (TextStyle::Name(CAPTION_STYLE.into()), font(CAPTION)),
        (TextStyle::Name(SMALL_STYLE.into()), font(SMALL)),
        (TextStyle::Name(SUBHEAD_STYLE.into()), font(SUBHEAD)),
    ];
    debug_assert!(styles.iter().all(|(_, f)| STEPS.contains(&f.size)), "every style is a step");
    style.text_styles = styles.into_iter().collect();
}

/// Proportional text at `px` (a step).
pub fn font(px: f32) -> FontId {
    FontId::proportional(px)
}

/// Monospace text at `px` (a step): SHAs, paths, code and fingerprints.
pub fn mono(px: f32) -> FontId {
    FontId::monospace(px)
}

/// SemiBold text at `px`: titles and emphasis (regular until the bundled fonts are in).
pub fn strong(ui: &Ui, px: f32) -> FontId {
    fonts::semibold(ui, px)
}

/// The section-header font: SemiBold at body size.
pub fn section(ui: &Ui) -> FontId {
    strong(ui, BODY)
}

/// `text` in the section-header style.
pub fn section_text(ui: &Ui, text: impl Into<String>) -> RichText {
    RichText::new(text).font(section(ui)).color(color::text())
}

/// `px`, or less when only `room` fits, but never under the floor.
pub fn fit(px: f32, room: f32) -> f32 {
    px.min(room).max(FLOOR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_are_five_ascending_with_body_14_and_floor_11() {
        assert_eq!(STEPS.len(), 5);
        assert!(STEPS.windows(2).all(|w| w[0] < w[1]), "{STEPS:?}");
        assert_eq!(BODY, 14.0);
        assert_eq!((FLOOR, CAPTION), (11.0, 11.0));
        assert_eq!(WORDMARK, 2.0 * TITLE);
    }

    #[test]
    fn install_sets_egui_text_styles() {
        let mut style = egui::Style::default();
        install(&mut style);
        let size = |s: TextStyle| style.text_styles[&s].size;
        assert_eq!(size(TextStyle::Small), 11.0);
        assert_eq!(size(TextStyle::Body), 14.0);
        assert_eq!(size(TextStyle::Button), 14.0);
        assert_eq!(size(TextStyle::Monospace), 14.0);
        assert_eq!(style.text_styles[&TextStyle::Monospace].family, egui::FontFamily::Monospace);
        assert_eq!(size(TextStyle::Heading), 20.0);
        assert_eq!(size(TextStyle::Name(CAPTION_STYLE.into())), CAPTION);
        assert_eq!(size(TextStyle::Name(SMALL_STYLE.into())), SMALL);
        assert_eq!(size(TextStyle::Name(SUBHEAD_STYLE.into())), SUBHEAD);
        for (s, f) in &style.text_styles {
            assert!(f.size >= FLOOR, "{s:?} at {}", f.size);
            assert!(STEPS.contains(&f.size), "{s:?} at {} is a step", f.size);
        }
    }

    #[test]
    fn fit_never_goes_under_the_floor() {
        assert_eq!(fit(BODY, 7.0), 11.0);
        assert_eq!(fit(BODY, 15.0), 14.0);
        assert_eq!(fit(SMALL, 9.0), 11.0);
    }

    #[test]
    fn apply_style_installs_the_scale_in_both_themes() {
        let ctx = egui::Context::default();
        super::super::apply_style(&ctx, &super::super::DARK);
        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            assert_eq!(ctx.style_of(theme).text_styles[&TextStyle::Body].size, BODY, "{theme:?}");
        }
    }
}
