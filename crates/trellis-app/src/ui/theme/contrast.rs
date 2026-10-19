//! Contrast maths (WCAG relative luminance and ratio, CIE lightness) and the colours of the
//! graph's ref chips, worked out so their text stays readable on every palette and lane.

use super::color::Palette;
use eframe::egui::Color32;

/// The dark text colour chips use when white would be less readable.
const NEAR_BLACK: Color32 = Color32::from_rgb(0x10, 0x12, 0x16);
/// How many 5% steps `ink` and `readable_on` take at most.
const MAX_STEPS: usize = 20;

/// WCAG relative luminance.
pub fn luminance(c: Color32) -> f64 {
    let lin = |v: u8| {
        let s = f64::from(v) / 255.0;
        if s <= 0.03928 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
}

/// WCAG contrast ratio of two colours (1 to 21).
pub fn ratio(a: Color32, b: Color32) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// CIE L* lightness (0 black to 100 white).
#[cfg(test)]
pub fn lightness(c: Color32) -> f64 {
    let y = luminance(c);
    const E: f64 = 216.0 / 24389.0;
    const K: f64 = 24389.0 / 27.0;
    if y > E { 116.0 * y.cbrt() - 16.0 } else { K * y }
}

/// `a` moved `t` (0 to 1) of the way to `b`, byte by byte in gamma space as egui blends. Opaque.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let lerp = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgb(lerp(a.r(), b.r()), lerp(a.g(), b.g()), lerp(a.b(), b.b()))
}

/// `c` seen through a black `backdrop` (only its alpha matters).
#[cfg(test)]
pub fn dimmed(c: Color32, backdrop: Color32) -> Color32 {
    let keep = 1.0 - f32::from(backdrop.a()) / 255.0;
    let scale = |v: u8| (f32::from(v) * keep).round() as u8;
    Color32::from_rgb(scale(c.r()), scale(c.g()), scale(c.b()))
}

/// White or near-black text for `fill`, whichever contrasts more. While the ratio is below
/// `min`, the fill moves 5% toward the opposite pole of the text. Returns (fill, text).
pub fn ink(fill: Color32, min: f64) -> (Color32, Color32) {
    let fg = if ratio(Color32::WHITE, fill) >= ratio(NEAR_BLACK, fill) { Color32::WHITE } else { NEAR_BLACK };
    let away = if fg == Color32::WHITE { Color32::BLACK } else { Color32::WHITE };
    let mut fill = fill;
    for _ in 0..MAX_STEPS {
        if ratio(fg, fill) >= min {
            break;
        }
        fill = mix(fill, away, 0.05);
    }
    (fill, fg)
}

/// `fg` moved 5% at a time toward white (on a `dark` palette) or black until it reaches `min`
/// on `fill`.
pub fn readable_on(fg: Color32, fill: Color32, dark: bool, min: f64) -> Color32 {
    let pole = if dark { Color32::WHITE } else { Color32::BLACK };
    let mut fg = fg;
    for _ in 0..MAX_STEPS {
        if ratio(fg, fill) >= min {
            break;
        }
        fg = mix(fg, pole, 0.05);
    }
    fg
}

/// What a ref chip in the graph stands for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChipStyle {
    /// A detached HEAD.
    Head,
    /// The checked-out branch.
    CurrentBranch,
    /// Another local branch.
    Branch,
    /// A remote-tracking branch.
    Remote,
    Tag,
}

#[cfg(test)]
impl ChipStyle {
    pub const ALL: [ChipStyle; 5] = [Self::Head, Self::CurrentBranch, Self::Branch, Self::Remote, Self::Tag];
}

/// A ref chip's (fill, text) colours for a ref on a lane of colour `lane`. Every fill is
/// opaque, so the text reaches 4.5:1 on selected and hovered rows too.
pub fn ref_chip(style: ChipStyle, lane: Color32, p: &Palette) -> (Color32, Color32) {
    match style {
        ChipStyle::Head => (p.text, p.bg),
        ChipStyle::Tag => (p.tag_bg, p.tag_fg),
        ChipStyle::CurrentBranch | ChipStyle::Branch => {
            let strength = if style == ChipStyle::CurrentBranch { 0.75 } else { 0.65 };
            let base = if p.dark { mix(p.bg, lane, strength) } else { lane };
            ink(base, 4.5)
        }
        ChipStyle::Remote => {
            let fill = mix(p.bg, lane, 0.25);
            (fill, readable_on(lane, fill, p.dark, 4.5))
        }
    }
}
