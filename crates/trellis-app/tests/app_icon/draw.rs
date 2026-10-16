//! Trellis's icon, drawn in unit coordinates (0..1, y down) and rendered with 4x4 supersampling:
//! a rounded green square with a faint diagonal lattice and a cream commit-graph vine (a trunk,
//! one branch curving off it, and round commit nodes). Only `+ - * /` and `sqrt` are used, so
//! every system renders the same pixels.

/// Straight-alpha RGBA.
type Rgba = [f64; 4];

const TOP: [f64; 3] = [0x3F as f64, 0xA3 as f64, 0x6B as f64];
const BOTTOM: [f64; 3] = [0x1E as f64, 0x6B as f64, 0x45 as f64];
const CREAM: [f64; 3] = [0xF4 as f64, 0xF1 as f64, 0xE8 as f64];

const INSET: f64 = 0.06;
const CORNER: f64 = 0.2;
const LATTICE_WIDTH: f64 = 0.025;
const LATTICE_SPACING: f64 = 0.22;
const LATTICE_ALPHA: f64 = 0.16;
const VINE_WIDTH: f64 = 0.07;
const NODE: f64 = 0.075;
const NODE_DOT: f64 = 0.035;
const TRUNK: [(f64, f64); 2] = [(0.40, 0.80), (0.40, 0.22)];
const NODES: [(f64, f64); 3] = [(0.40, 0.76), (0.40, 0.28), (0.64, 0.30)];

/// What a size shows: the lattice only above 32 px, the nodes' inner dots only above 24 px.
struct Detail {
    lattice: bool,
    dots: bool,
}

fn inside_square(x: f64, y: f64) -> bool {
    let half = 0.5 - INSET;
    let (dx, dy) = ((x - 0.5).abs(), (y - 0.5).abs());
    if dx > half || dy > half {
        return false;
    }
    let (qx, qy) = ((dx - (half - CORNER)).max(0.0), (dy - (half - CORNER)).max(0.0));
    qx * qx + qy * qy <= CORNER * CORNER
}

/// The background's colour at height `y`: the gradient across the square.
fn gradient(y: f64) -> [f64; 3] {
    let t = ((y - INSET) / (1.0 - 2.0 * INSET)).clamp(0.0, 1.0);
    [0, 1, 2].map(|i| TOP[i] + (BOTTOM[i] - TOP[i]) * t)
}

/// Distance from `v` to the nearest multiple of the lattice spacing.
fn off_grid(v: f64) -> f64 {
    let k = (v / LATTICE_SPACING + 0.5).floor();
    (v - k * LATTICE_SPACING).abs()
}

fn on_lattice(x: f64, y: f64) -> bool {
    let root2 = 2.0_f64.sqrt();
    // Lines at +45 and -45 degrees, measured across the line.
    off_grid((x + y) / root2) <= LATTICE_WIDTH / 2.0 || off_grid((x - y + 1.0) / root2) <= LATTICE_WIDTH / 2.0
}

fn dist2_to_segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
    let len2 = abx * abx + aby * aby;
    let t = if len2 == 0.0 { 0.0 } else { (((p.0 - a.0) * abx + (p.1 - a.1) * aby) / len2).clamp(0.0, 1.0) };
    let (cx, cy) = (a.0 + abx * t - p.0, a.1 + aby * t - p.1);
    cx * cx + cy * cy
}

/// The branch: a quadratic curve from the trunk at (0.40, 0.66), bending through (0.64, 0.66)
/// to (0.64, 0.46), then straight up to the top node, as a 16-segment polyline.
fn branch() -> Vec<(f64, f64)> {
    let (p0, c, p1) = ((0.40, 0.66), (0.64, 0.66), (0.64, 0.46));
    let mut points: Vec<(f64, f64)> = (0..=16)
        .map(|i| {
            let t = i as f64 / 16.0;
            let u = 1.0 - t;
            (u * u * p0.0 + 2.0 * u * t * c.0 + t * t * p1.0, u * u * p0.1 + 2.0 * u * t * c.1 + t * t * p1.1)
        })
        .collect();
    points.push((0.64, 0.30));
    points
}

fn on_vine(p: (f64, f64), branch: &[(f64, f64)]) -> bool {
    let r2 = (VINE_WIDTH / 2.0) * (VINE_WIDTH / 2.0);
    dist2_to_segment(p, TRUNK[0], TRUNK[1]) <= r2 || branch.windows(2).any(|w| dist2_to_segment(p, w[0], w[1]) <= r2)
}

fn within(p: (f64, f64), c: (f64, f64), r: f64) -> bool {
    let (dx, dy) = (p.0 - c.0, p.1 - c.1);
    dx * dx + dy * dy <= r * r
}

/// The colour of one sample point.
fn sample(x: f64, y: f64, detail: &Detail, branch: &[(f64, f64)]) -> Rgba {
    if !inside_square(x, y) {
        return [0.0; 4];
    }
    let mut c = gradient(y);
    if detail.lattice && on_lattice(x, y) {
        c = c.map(|v| v + (255.0 - v) * LATTICE_ALPHA);
    }
    let p = (x, y);
    if on_vine(p, branch) {
        c = CREAM;
    }
    for node in NODES {
        if within(p, node, NODE) {
            c = if detail.dots && within(p, node, NODE_DOT) { gradient(y) } else { CREAM };
        }
    }
    [c[0], c[1], c[2], 255.0]
}

/// The icon at `size` x `size` pixels, as straight-alpha RGBA bytes.
pub fn render(size: u32) -> Vec<u8> {
    const N: u32 = 4;
    let detail = Detail { lattice: size > 32, dots: size > 24 };
    let branch = branch();
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for py in 0..size {
        for px in 0..size {
            // Premultiplied sums of the 16 samples.
            let mut sum = [0.0_f64; 4];
            for sy in 0..N {
                for sx in 0..N {
                    let x = (px as f64 + (sx as f64 + 0.5) / N as f64) / size as f64;
                    let y = (py as f64 + (sy as f64 + 0.5) / N as f64) / size as f64;
                    let s = sample(x, y, &detail, &branch);
                    let a = s[3] / 255.0;
                    for i in 0..3 {
                        sum[i] += s[i] * a;
                    }
                    sum[3] += s[3];
                }
            }
            let n = (N * N) as f64;
            let alpha = sum[3] / n;
            let rgb = |i: usize| if alpha > 0.0 { sum[i] / n / (alpha / 255.0) } else { 0.0 };
            for v in [rgb(0), rgb(1), rgb(2), alpha] {
                out.push((v + 0.5).floor().clamp(0.0, 255.0) as u8);
            }
        }
    }
    out
}
