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

/// The opaque sRGB colour of OKLCH (`l`, `c`, `h` in degrees), each channel clipped to the gamut.
pub fn from_oklch(l: f64, c: f64, h: f64) -> Color32 {
    let (a, b) = (c * h.to_radians().cos(), c * h.to_radians().sin());
    let l_ = (l + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let m_ = (l - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let s_ = (l - 0.089_484_177_5 * a - 1.291_485_548_0 * b).powi(3);
    let r = 4.076_741_662_1 * l_ - 3.307_711_591_3 * m_ + 0.230_969_929_2 * s_;
    let g = -1.268_438_004_6 * l_ + 2.609_757_401_1 * m_ - 0.341_319_396_5 * s_;
    let bl = -0.004_196_086_3 * l_ - 0.703_418_614_7 * m_ + 1.707_614_701_0 * s_;
    let enc = |v: f64| {
        let v = v.clamp(0.0, 1.0);
        let s = if v <= 0.003_130_8 { 12.92 * v } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
        (s * 255.0).round() as u8
    };
    Color32::from_rgb(enc(r), enc(g), enc(bl))
}

/// `a` moved `t` (0 to 1) of the way to `b`, byte by byte in gamma space as egui blends. Opaque.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let lerp = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgb(lerp(a.r(), b.r()), lerp(a.g(), b.g()), lerp(a.b(), b.b()))
}

/// The least OKLCH chroma that makes a dark surface a coloured field (Dusk's indigo is 0.028 to
/// 0.036; Garden's green-grey and the high-contrast greys sit under 0.015).
const COLOURED_FIELD: f64 = 0.02;
/// How far a tint may turn a coloured dark field's hue, in degrees.
const HUE_FENCE: f64 = 20.0;

/// Surface `base` tinted `t` (0 to 1) toward `toward`, for the faint tints of a surface: graph
/// row bands, the toolbar groups' backing, the dialog guide and field fills (docs/COLOUR.md,
/// "Tinting a surface"). It is [`mix`], except on a dark palette whose `base` is a coloured
/// field: there a tint toward a far hue (the gold on indigo) would cancel the field's colour and
/// leave a grey-brown, so the tint keeps the mix's lightness, moves the field's chroma `t` of
/// the way toward `toward`'s and turns the field's hue at most `HUE_FENCE` degrees toward the
/// mix's. Light palettes keep the plain mix (a warm tint on pale paper reads as cream, not
/// brown).
pub fn tint(base: Color32, toward: Color32, t: f32, dark: bool) -> Color32 {
    let m = mix(base, toward, t);
    let (_, bc, bh) = oklch(base);
    if !dark || bc < COLOURED_FIELD {
        return m;
    }
    let (ml, mc, mh) = oklch(m);
    let turn = (mh - bh + 540.0).rem_euclid(360.0) - 180.0;
    if mc >= bc && turn.abs() <= HUE_FENCE {
        return m;
    }
    // The chroma moves `t` of the way toward the tint colour's, as an OKLCH mix would.
    let c = bc + (oklch(toward).1 - bc).max(0.0) * f64::from(t);
    from_oklch(ml, mc.max(c), bh + turn.clamp(-HUE_FENCE, HUE_FENCE))
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

/// The fill of a selected `+` (`add`) or `-` diff line: the palette's selected-line tint pushed
/// up to as far again from the plain tint, in the same hue, as far as the text keeps 4.5:1 and
/// the syntax colours 2:1 on it. (The high-contrast palettes keep 7:1 and 4.5:1 at the full
/// push; `diff_select_tests` checks every built-in.)
pub fn diff_line_selected(p: &Palette, add: bool) -> Color32 {
    let (plain, sel) = if add { (p.diff_add, p.diff_add_sel) } else { (p.diff_del, p.diff_del_sel) };
    let push = |k: f32| {
        let ch = |s: u8, b: u8| (f32::from(s) + (f32::from(s) - f32::from(b)) * k).round().clamp(0.0, 255.0) as u8;
        Color32::from_rgb(ch(sel.r(), plain.r()), ch(sel.g(), plain.g()), ch(sel.b(), plain.b()))
    };
    let readable = |c: Color32| {
        ratio(p.text, c) >= 4.5
            && Palette::KEYS.iter().filter(|k| k.starts_with("syn_")).filter_map(|k| p.get(k)).all(|s| ratio(s, c) >= 2.0)
    };
    (0..=10).rev().map(|i| push(i as f32 / 10.0)).find(|&c| readable(c)).unwrap_or(sel)
}
