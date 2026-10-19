//! Contrast maths (WCAG relative luminance and ratio, CIE lightness, OKLCH for the tests) and the colours of the
//! graph's ref chips, worked out so their text stays readable on every palette and lane.

use super::color::Palette;
use eframe::egui::Color32;

/// How many 5% steps `readable_on` takes at most.
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

/// OKLCH (lightness 0 to 1, chroma, hue in degrees 0 to 360) of an sRGB colour.
#[cfg(test)]
pub fn oklch(c: Color32) -> (f64, f64, f64) {
    let lin = |v: u8| {
        let s = f64::from(v) / 255.0;
        if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    };
    let (r, g, b) = (lin(c.r()), lin(c.g()), lin(c.b()));
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    let lightness = 0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s;
    let a = 1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s;
    let bb = 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s;
    (lightness, a.hypot(bb), bb.atan2(a).to_degrees().rem_euclid(360.0))
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

/// A ref chip's (fill, text) colours for a ref on a lane of colour `lane`: soft tints of the
/// lane (a little stronger for the checked-out branch), the palette's tag colours, and a neutral
/// tint for a detached HEAD. Every fill is opaque, so the text reaches 4.5:1 on selected and
/// hovered rows too.
pub fn ref_chip(style: ChipStyle, lane: Color32, p: &Palette) -> (Color32, Color32) {
    let tint = |dark_t: f32, light_t: f32| {
        let fill = mix(p.bg, lane, if p.dark { dark_t } else { light_t });
        (fill, readable_on(lane, fill, p.dark, 4.5))
    };
    match style {
        ChipStyle::Head => (mix(p.bg, p.text, 0.16), p.text),
        ChipStyle::Tag => (p.tag_bg, p.tag_fg),
        ChipStyle::Branch | ChipStyle::Remote => tint(0.28, 0.20),
        ChipStyle::CurrentBranch => tint(0.40, 0.30),
    }
}

/// A change-kind chip's (fill, text) colours for a change of colour `col` (the WIP row's M / A / D
/// chips): a soft opaque tint of it, with the text pushed until it reaches 4.5:1 on the tint.
/// Light palettes tint at 10%, so the status colour itself reads at 4.5:1 there and only the
/// odd one (Light's modified) is pushed; dark ones at 22%.
pub fn change_chip(col: Color32, p: &Palette) -> (Color32, Color32) {
    let fill = mix(p.bg, col, if p.dark { 0.22 } else { 0.10 });
    (fill, readable_on(col, fill, p.dark, 4.5))
}
