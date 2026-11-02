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

/// The linear-light sRGB channels of OKLCH (`l`, `c`, `h` in degrees), unclipped.
fn oklch_linear(l: f64, c: f64, h: f64) -> [f64; 3] {
    let (a, b) = (c * h.to_radians().cos(), c * h.to_radians().sin());
    let l_ = (l + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let m_ = (l - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let s_ = (l - 0.089_484_177_5 * a - 1.291_485_548_0 * b).powi(3);
    [
        4.076_741_662_1 * l_ - 3.307_711_591_3 * m_ + 0.230_969_929_2 * s_,
        -1.268_438_004_6 * l_ + 2.609_757_401_1 * m_ - 0.341_319_396_5 * s_,
        -0.004_196_086_3 * l_ - 0.703_418_614_7 * m_ + 1.707_614_701_0 * s_,
    ]
}

/// The opaque sRGB colour of OKLCH (`l`, `c`, `h` in degrees), each channel clipped to the gamut.
pub(super) fn from_oklch(l: f64, c: f64, h: f64) -> Color32 {
    let [r, g, bl] = oklch_linear(l, c, h);
    let enc = |v: f64| {
        let v = v.clamp(0.0, 1.0);
        let s = if v <= 0.003_130_8 { 12.92 * v } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
        (s * 255.0).round() as u8
    };
    Color32::from_rgb(enc(r), enc(g), enc(bl))
}

/// Whether `c` reads as brown, tan, khaki, olive or rust (docs/COLOUR.md, "No brown"), judged
/// from a rendered grid of OKLCH swatches. Greys (chroma under 0.025, or 0.010 darker than
/// lightness 0.5) and pale cream or ivory
/// (lightness 0.90 and up) never do. A warm hue, orange through olive (40 to 125 deg; from 55
/// at lightness 0.75 and up, where 40 to 55 is peach) does unless it is a clear, saturated
/// orange, amber, gold or yellow: chroma at least 0.15 at lightness 0.60, falling to 0.074 at
/// 0.88, and never under lightness 0.55, where every warm colour is brown. The rust side of
/// red (20 to 40 deg) does when it is dull (chroma under 0.08) and darker than 0.62. So a warm
/// tint or wash on a dark surface is always brown, whatever its strength.
pub fn brownish(c: Color32) -> bool {
    let (l, ch, h) = oklch(c);
    // A dark warm grey turns taupe sooner than a light one.
    let grey = if l < 0.5 { 0.010 } else { 0.025 };
    if ch < grey || l >= 0.90 {
        return false;
    }
    let warm_from = if l >= 0.75 { 55.0 } else { 40.0 };
    if (warm_from..=125.0).contains(&h) {
        let clear = l >= 0.55 && ch >= (0.15 - (l - 0.60) * 0.27).clamp(0.074, 0.16);
        return !clear;
    }
    (20.0..40.0).contains(&h) && ch < 0.08 && l < 0.62
}

/// The most OKLCH chroma sRGB holds at lightness `l` and hue `h`.
#[cfg(test)]
pub fn max_chroma(l: f64, h: f64) -> f64 {
    let fits = |c: f64| oklch_linear(l, c, h).iter().all(|v| (-1e-4..=1.0 + 1e-4).contains(v));
    let (mut lo, mut hi) = (0.0, 0.4);
    for _ in 0..40 {
        let mid = (lo + hi) / 2.0;
        if fits(mid) { lo = mid } else { hi = mid }
    }
    lo
}

/// Whether coloured text `c` (a status, syntax or terminal colour, not a neutral ink) reads
/// brown (docs/COLOUR.md, "No brown"). Text dark enough to read on paper can't take the
/// chroma [`brownish`] asks of a warm fill, so a warm text colour is clean only as an orange at
/// the edge of the sRGB gamut: hue 40 to 66 deg (past that, amber turns ochre at text
/// darkness), lightness 0.50 and up (below it, russet) and chroma at least 95% of the most its
/// lightness and hue hold.
#[cfg(test)]
pub fn brownish_text(c: Color32) -> bool {
    if !brownish(c) {
        return false;
    }
    let (l, ch, h) = oklch(c);
    !((40.0..=66.0).contains(&h) && l >= 0.50 && ch >= 0.95 * max_chroma(l, h))
}

/// `a` moved `t` (0 to 1) of the way to `b`, byte by byte in gamma space as egui blends. Opaque.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let lerp = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgb(lerp(a.r(), b.r()), lerp(a.g(), b.g()), lerp(a.b(), b.b()))
}

/// The least OKLCH chroma that makes a dark surface a coloured field (Dusk's indigo is 0.028 to
/// 0.036, Pine's slate and Ember's charcoal about 0.015; Garden's green-grey and the greys of
/// Graphite and high contrast sit under 0.012). On one, a gold tint mixed in gamma space would
/// turn olive-green or grey against the field's own colour.
const COLOURED_FIELD: f64 = 0.012;
/// How far a tint may turn a coloured dark field's hue, in degrees.
const HUE_FENCE: f64 = 20.0;

/// Surface `base` tinted `t` (0 to 1) toward `toward`, for the faint tints of a surface: graph
/// row bands, the toolbar groups' backing, the dialog guide and field fills (docs/COLOUR.md,
/// "Tinting a surface"). It is [`mix`], except on a dark palette whose `base` is a coloured
/// field: there a tint toward a far hue (the gold on indigo) would cancel the field's colour and
/// leave a grey-brown, so the tint keeps the mix's lightness, moves the field's chroma `t` of
/// the way toward `toward`'s and turns the field's hue at most `HUE_FENCE` degrees toward the
/// mix's. Whatever would still read brown (a warm tint on a dark grey) is [`unbrown`]ed.
pub fn tint(base: Color32, toward: Color32, t: f32, dark: bool) -> Color32 {
    let m = mix(base, toward, t);
    let (_, bc, bh) = oklch(base);
    if !dark || bc < COLOURED_FIELD {
        return unbrown(base, m);
    }
    let (ml, mc, mh) = oklch(m);
    let turn = (mh - bh + 540.0).rem_euclid(360.0) - 180.0;
    if mc >= bc && turn.abs() <= HUE_FENCE {
        return unbrown(base, m);
    }
    // The chroma moves `t` of the way toward the tint colour's, as an OKLCH mix would.
    let c = bc + (oklch(toward).1 - bc).max(0.0) * f64::from(t);
    unbrown(base, from_oklch(ml, mc.max(c), bh + turn.clamp(-HUE_FENCE, HUE_FENCE)))
}

/// `c`, a tint or wash painted on surface `base`, unless it reads brown ([`brownish`]): then a
/// colour of its lightness in `base`'s own hue and chroma, so the tint still lifts (or sinks)
/// the surface as much but takes no colour. Warm colours on a dark surface go this way: the
/// colour moves to the tint's edge, text or mark instead (docs/COLOUR.md, "No brown").
pub fn unbrown(base: Color32, c: Color32) -> Color32 {
    if brownish(c) { field_at(base, oklch(c).0) } else { c }
}

/// The colour of OKLCH lightness `l` in surface `base`'s hue and chroma; a grey when that
/// would read brown (a warm paper's tint at a darker step).
pub fn field_at(base: Color32, l: f64) -> Color32 {
    let (_, bc, bh) = oklch(base);
    let c = from_oklch(l, bc, bh);
    if brownish(c) { from_oklch(l, 0.0, 0.0) } else { c }
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

/// `tone` pushed until it reads at 4.5:1 on `fill`, for a chip's or initials' letters; the
/// palette's own text when that would read brown (a warm branch tone dark enough for paper).
pub fn clean_ink(tone: Color32, fill: Color32, p: &Palette) -> Color32 {
    let ink = readable_on(tone, fill, p.dark, 4.5);
    if brownish(ink) { readable_on(p.text, fill, p.dark, 4.5) } else { ink }
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

/// How far a dark palette's branch chip lifts from the background toward `text`.
const CHIP_LIFT: f32 = 0.10;
/// How far a branch chip's edge moves from its fill toward the branch colour.
const CHIP_EDGE: f32 = 0.70;
/// A light palette's branch chip: a pale wash of the branch's hue, OKLCH lightness and the most
/// chroma it takes; the checked-out branch's is a step deeper.
const PALE: (f64, f64) = (0.955, 0.035);
const PALE_CURRENT: (f64, f64) = (0.90, 0.09);
/// The lightness and chroma [`clear`] lifts a brown colour to at least: a clear orange, gold or
/// lime, never a tan.
const CLEAR: (f64, f64) = (0.80, 0.13);

/// `c`, a line, edge or solid fill, unless it reads brown ([`brownish`]): then lifted in its own
/// hue to a clear, saturated colour (OKLCH lightness 0.80 and chroma 0.13 at least).
pub fn clear(c: Color32) -> Color32 {
    if !brownish(c) {
        return c;
    }
    let (l, ch, h) = oklch(c);
    from_oklch(l.max(CLEAR.0), ch.max(CLEAR.1), h)
}

/// A ref chip's colours for a ref on a lane of colour `lane` (docs/COLOUR.md, "Ref chips").
/// A branch or remote branch: on a dark palette a neutral chip a step off the background with
/// an edge and text in the branch colour (a tint of a warm colour on a dark field reads brown,
/// so no chip is tinted); on a light one a pale wash of the branch's hue, edged and lettered in
/// it. The checked-out branch: on a dark palette a solid fill of the branch colour with dark
/// text, on a light one a deeper wash with a full-colour edge. Tags take the palette's tag
/// colours, a detached HEAD a neutral fill. Every fill is opaque and its text reaches 4.5:1.
pub fn ref_chip(style: ChipStyle, lane: Color32, p: &Palette) -> ChipLook {
    let (_, lc, lh) = oklch(lane);
    let pale = |(l, c): (f64, f64)| from_oklch(l, lc.min(c), lh);
    let edged = |fill: Color32, edge: Color32| ChipLook { fill, edge: Some(edge), text: clean_ink(lane, fill, p) };
    match (style, p.dark) {
        (ChipStyle::Head, _) => ChipLook { fill: mix(p.bg, p.text, 0.16), edge: None, text: p.text },
        (ChipStyle::Tag, _) => ChipLook { fill: p.tag_bg, edge: None, text: p.tag_fg },
        (ChipStyle::Branch | ChipStyle::Remote, true) => {
            let fill = mix(p.bg, p.text, CHIP_LIFT);
            edged(fill, clear(mix(fill, lane, CHIP_EDGE)))
        }
        (ChipStyle::Branch | ChipStyle::Remote, false) => {
            let fill = pale(PALE);
            edged(fill, clear(mix(fill, lane, CHIP_EDGE)))
        }
        (ChipStyle::CurrentBranch, true) => {
            let fill = clear(lane);
            ChipLook { fill, edge: None, text: readable_on(p.bg, fill, false, 4.5) }
        }
        (ChipStyle::CurrentBranch, false) => edged(pale(PALE_CURRENT), clear(lane)),
    }
}

/// How a ref chip is painted: its opaque fill, the 1 px edge drawn inside it (if any) and its
/// text and marks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ChipLook {
    pub fill: Color32,
    pub edge: Option<Color32>,
    pub text: Color32,
}

/// A change-kind chip's (fill, text) colours for a change of colour `col` (the WIP row's M / A / D
/// chips): a soft opaque tint of it ([`unbrown`]ed: a warm one on a dark palette is a neutral
/// step), with the text pushed until it reaches 4.5:1 on the tint.
/// Light palettes tint at 10%, so the status colour itself reads at 4.5:1 there and only the
/// odd one (Light's modified) is pushed; dark ones at 22%.
pub fn change_chip(col: Color32, p: &Palette) -> (Color32, Color32) {
    let fill = unbrown(p.bg, mix(p.bg, col, if p.dark { 0.22 } else { 0.10 }));
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
