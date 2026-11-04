//! Colour vision, simulated: how far apart two colours look under normal, protan, deutan and
//! tritan vision, and the floors under which two lines pass for each other. The colour-blind
//! tests and the trunk's near slots (`branch_colours::trunk_near`) both measure with it.
//!
//! Vision is simulated with Machado, Oliveira and Fernandes (2009) at severity 1.0, applied to
//! linear sRGB and clamped; distance is Euclidean in OKLab.

use eframe::egui::Color32;

/// The least OKLab distance between dark tones, and between light line tones, in every vision.
pub(crate) const DARK_FLOOR: f64 = 0.08;
pub(crate) const LIGHT_FLOOR: f64 = 0.05;

/// A vision's linear-sRGB transform.
pub type Matrix = [[f64; 3]; 3];

const PROTAN: Matrix = [[0.152286, 1.052583, -0.204868], [0.114503, 0.786281, 0.099216], [-0.003882, -0.048116, 1.051998]];
pub(crate) const DEUTAN: Matrix = [[0.367322, 0.860646, -0.227968], [0.280085, 0.672501, 0.047413], [-0.011820, 0.042940, 0.968881]];
const TRITAN: Matrix = [[1.255528, -0.076749, -0.178779], [-0.078411, 0.930809, 0.147602], [0.004733, 0.691367, 0.303900]];
const NORMAL: Matrix = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
/// Normal vision first, then the three colour blindnesses the colour-blind preset is built for.
pub const VISIONS: [(&str, Matrix); 4] = [("normal", NORMAL), ("protan", PROTAN), ("deutan", DEUTAN), ("tritan", TRITAN)];

/// The floor for lines on a dark (`dark`) or light background.
pub fn floor(dark: bool) -> f64 {
    if dark { DARK_FLOOR } else { LIGHT_FLOOR }
}

fn linear(c: Color32) -> [f64; 3] {
    let lin = |v: u8| {
        let s = f64::from(v) / 255.0;
        if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    };
    [lin(c.r()), lin(c.g()), lin(c.b())]
}

/// OKLab of linear sRGB `rgb`.
fn oklab([r, g, b]: [f64; 3]) -> [f64; 3] {
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    [
        0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s,
    ]
}

/// `c` as seen through `vision`, in OKLab.
fn seen(c: Color32, vision: &Matrix) -> [f64; 3] {
    let v = linear(c);
    let row = |k: usize| (vision[k][0] * v[0] + vision[k][1] * v[1] + vision[k][2] * v[2]).clamp(0.0, 1.0);
    oklab([row(0), row(1), row(2)])
}

/// The OKLab lightness `c` has through `vision`.
#[cfg(test)]
pub(crate) fn lightness_seen(c: Color32, vision: &Matrix) -> f64 {
    seen(c, vision)[0]
}

/// How far apart `a` and `b` look through `vision` (OKLab distance).
pub fn distance(a: Color32, b: Color32, vision: &Matrix) -> f64 {
    let (x, y) = (seen(a, vision), seen(b, vision));
    ((x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2) + (x[2] - y[2]).powi(2)).sqrt()
}
