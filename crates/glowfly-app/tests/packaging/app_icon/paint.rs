//! The geometry and paint the icon is drawn with, in the 1024-unit design space (y down): frames
//! for rotated and scaled groups, gradients with stops, cubic curves flattened to polylines, and
//! the distance and inside tests that decide coverage. Only `+ - * /` and `sqrt` are used, so
//! every system computes the same pixels.

/// A colour in sRGB, 0 to 255 per channel.
pub type Rgb = [f64; 3];
pub type Point = (f64, f64);

pub const fn hex(c: u32) -> Rgb {
    [((c >> 16) & 0xff) as f64, ((c >> 8) & 0xff) as f64, (c & 0xff) as f64]
}

/// `src` at opacity `a` over the opaque `dst`.
pub fn over(dst: &mut Rgb, src: Rgb, a: f64) {
    if a > 0.0 {
        for i in 0..3 {
            dst[i] += (src[i] - dst[i]) * a;
        }
    }
}

/// A gradient stop: where (0 to 1), its colour and its opacity.
pub type Stop = (f64, Rgb, f64);

/// The colour and opacity of `stops` at `t`, interpolated as SVG does (colour and opacity apart,
/// padded past the ends).
pub fn at(stops: &[Stop], t: f64) -> (Rgb, f64) {
    let first = stops[0];
    if t <= first.0 {
        return (first.1, first.2);
    }
    for w in stops.windows(2) {
        let (a, b) = (w[0], w[1]);
        if t <= b.0 {
            let k = if b.0 > a.0 { (t - a.0) / (b.0 - a.0) } else { 1.0 };
            return ([0, 1, 2].map(|i| a.1[i] + (b.1[i] - a.1[i]) * k), a.2 + (b.2 - a.2) * k);
        }
    }
    let last = stops[stops.len() - 1];
    (last.1, last.2)
}

/// A group's placement, as SVG's `translate(tx ty) rotate(θ) scale(s)`: `cos` and `sin` of θ
/// are given as numbers so no trigonometry runs.
#[derive(Clone, Copy)]
pub struct Frame {
    pub tx: f64,
    pub ty: f64,
    pub cos: f64,
    pub sin: f64,
    pub scale: f64,
}

impl Frame {
    /// A design-space point in the frame's own units.
    pub fn local(&self, p: Point) -> Point {
        let (dx, dy) = ((p.0 - self.tx) / self.scale, (p.1 - self.ty) / self.scale);
        (dx * self.cos + dy * self.sin, -dx * self.sin + dy * self.cos)
    }
}

/// An ellipse at `c` with radii `r`, in some frame's units.
#[derive(Clone, Copy)]
pub struct Ellipse {
    pub c: Point,
    pub r: Point,
}

impl Ellipse {
    pub fn inside(&self, p: Point) -> bool {
        let (u, v) = ((p.0 - self.c.0) / self.r.0, (p.1 - self.c.1) / self.r.1);
        u * u + v * v <= 1.0
    }

    /// The radial gradient position of `p` for SVG's bounding-box gradient on this ellipse,
    /// centred at (`cx`, `cy`) of the box with radius `r` (fractions of the box).
    pub fn gradient_t(&self, p: Point, cx: f64, cy: f64, r: f64) -> f64 {
        let (w, h) = (2.0 * self.r.0, 2.0 * self.r.1);
        let (gx, gy) = (self.c.0 - self.r.0 + cx * w, self.c.1 - self.r.1 + cy * h);
        let (u, v) = ((p.0 - gx) / (r * w), (p.1 - gy) / (r * h));
        (u * u + v * v).sqrt()
    }
}

/// Whether `p` is inside the square from `lo` to `hi` (both axes) with corner radius `r`.
pub fn in_round_rect(p: Point, lo: f64, hi: f64, r: f64) -> bool {
    let c = (lo + hi) / 2.0;
    let half = (hi - lo) / 2.0;
    let (dx, dy) = ((p.0 - c).abs(), (p.1 - c).abs());
    if dx > half || dy > half {
        return false;
    }
    let (qx, qy) = ((dx - (half - r)).max(0.0), (dy - (half - r)).max(0.0));
    qx * qx + qy * qy <= r * r
}

pub fn dist2(a: Point, b: Point) -> f64 {
    let (dx, dy) = (a.0 - b.0, a.1 - b.1);
    dx * dx + dy * dy
}

fn dist2_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
    let len2 = abx * abx + aby * aby;
    let t = if len2 == 0.0 { 0.0 } else { (((p.0 - a.0) * abx + (p.1 - a.1) * aby) / len2).clamp(0.0, 1.0) };
    dist2(p, (a.0 + abx * t, a.1 + aby * t))
}

/// A polyline with its bounding box, for strokes with round caps and joins.
pub struct Polyline {
    pub points: Vec<Point>,
    lo: Point,
    hi: Point,
}

impl Polyline {
    pub fn new(points: Vec<Point>) -> Polyline {
        let lo = points.iter().fold((f64::MAX, f64::MAX), |m, p| (m.0.min(p.0), m.1.min(p.1)));
        let hi = points.iter().fold((f64::MIN, f64::MIN), |m, p| (m.0.max(p.0), m.1.max(p.1)));
        Polyline { points, lo, hi }
    }

    /// The squared distance from `p` to the line, or None when `p` is further than `reach` from
    /// its bounding box.
    pub fn dist2_within(&self, p: Point, reach: f64) -> Option<f64> {
        if p.0 < self.lo.0 - reach || p.0 > self.hi.0 + reach || p.1 < self.lo.1 - reach || p.1 > self.hi.1 + reach {
            return None;
        }
        let r2 = reach * reach;
        let mut best = f64::MAX;
        for w in self.points.windows(2) {
            let (a, b) = (w[0], w[1]);
            // A segment whose box is out of reach can't be nearer.
            if p.0 < a.0.min(b.0) - reach || p.0 > a.0.max(b.0) + reach || p.1 < a.1.min(b.1) - reach || p.1 > a.1.max(b.1) + reach {
                continue;
            }
            best = best.min(dist2_to_segment(p, a, b));
        }
        (best <= r2).then_some(best)
    }

    /// Even-odd inside test, the polyline taken as a closed polygon.
    pub fn contains(&self, p: Point) -> bool {
        if p.0 < self.lo.0 || p.0 > self.hi.0 || p.1 < self.lo.1 || p.1 > self.hi.1 {
            return false;
        }
        let n = self.points.len();
        let mut inside = false;
        for i in 0..n {
            let (a, b) = (self.points[i], self.points[(i + 1) % n]);
            if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) / (b.1 - a.1) * (b.0 - a.0) {
                inside = !inside;
            }
        }
        inside
    }
}

/// A cubic from `p0` to `p3`, flattened to `steps` segments (`p0` itself left out).
pub fn cubic(p0: Point, p1: Point, p2: Point, p3: Point, steps: u32) -> Vec<Point> {
    (1..=steps)
        .map(|i| {
            let t = i as f64 / steps as f64;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            (a * p0.0 + b * p1.0 + c * p2.0 + d * p3.0, a * p0.1 + b * p1.1 + c * p2.1 + d * p3.1)
        })
        .collect()
}
