//! How much work an SVG may ask of the renderer before the preview refuses it. resvg has no
//! limits of its own: a few bytes of `feMorphology` with a huge radius, `feTurbulence` with a
//! huge `numOctaves` or `feMerge` with a great many inputs could keep a worker thread busy for
//! hours, `<use>` can fan a file out into a million shapes, and each group drawn on a layer of
//! its own (filter, opacity, mask or clip, each link of a mask or clip chain, a nested picture)
//! holds a whole layer while its children draw, where a failed allocation aborts the app.
//!
//! `within` walks the parsed tree the way resvg draws it, carrying the transform down from the
//! output canvas into groups, clip paths and masks (to the end of their chains), patterns,
//! `feImage` and nested pictures, so every size is measured in output pixels. It counts drawn
//! shapes, their painted area, pattern tiles, filter work and layer depth, and refuses rather
//! than weakens: a drawing with weaker filters would be a wrong picture. The renderer may
//! instead draw it smaller (`preview_svg::render`), and refuses it past that. This is a
//! backstop for the worst cases, not an exact cost: the runner also renders one SVG at a time
//! and stops waiting for one that runs long.

use crate::preview_image::SHOWN_MAX;
use resvg::tiny_skia::Transform;
use resvg::usvg::{ClipPath, Group, ImageKind, Mask, Node, Paint, Path, Rect, Tree, filter};

/// The deepest stack of layers drawn at once.
const MAX_LAYERS: usize = 16;
/// The most shapes, pictures and texts drawn, after `<use>` and patterns are expanded.
const MAX_NODES: usize = 50_000;
/// The most pixels the shapes may paint, added up (a full 1024 px canvas is about a million).
const MAX_PAINT: f64 = 1.0e9;
/// The most filter primitives applied, counting a filter once for each group that uses it.
const MAX_PRIMITIVES: usize = 128;
/// The most `feTurbulence` octaves (browsers gain nothing visible past about 10).
const MAX_OCTAVES: u32 = 10;
/// The largest `feMorphology` radius, in output pixels.
const MAX_RADIUS_PX: f32 = 32.0;
/// The most pixel operations the filters may add up to.
const MAX_COST: f64 = 4.0e8;
/// The largest pattern tile, in output pixels.
const MAX_TILE_PX: f64 = 2048.0 * 2048.0;
/// The largest area a layer or filter region covers: five times the canvas each way.
const MAX_REGION_PX: f64 = (5 * SHOWN_MAX as u64 * 5 * SHOWN_MAX as u64) as f64;

/// What the walk has counted so far; `layers` is the depth at the node being walked.
#[derive(Default)]
struct Walk {
    layers: usize,
    nodes: usize,
    paint: f64,
    primitives: usize,
    cost: f64,
}

/// Whether `tree`, drawn at `scale` output pixels per user unit, is within the budget.
pub(crate) fn within(tree: &Tree, scale: f32) -> bool {
    Walk::default().group(tree.root(), Transform::from_scale(scale, scale))
}

/// A `width` × `height` area in output pixels under `ts`, at most `MAX_REGION_PX`.
fn area(width: f32, height: f32, ts: Transform) -> f64 {
    let (sx, sy) = ts.get_scale();
    (f64::from((width * sx).abs()) * f64::from((height * sy).abs())).min(MAX_REGION_PX)
}

fn rect_area(r: Rect, ts: Transform) -> f64 {
    area(r.width(), r.height(), ts)
}

impl Walk {
    /// Walks `f` one layer deeper.
    fn layer(&mut self, f: impl FnOnce(&mut Self) -> bool) -> bool {
        self.layers += 1;
        let ok = self.layers <= MAX_LAYERS && f(self);
        self.layers -= 1;
        ok
    }

    /// `g` drawn under its parent's transform `parent`.
    fn group(&mut self, g: &Group, parent: Transform) -> bool {
        let ts = parent.pre_concat(g.transform());
        let layered = g.isolate() || !g.filters().is_empty() || g.opacity().get() < 1.0 || g.mask().is_some() || g.clip_path().is_some();
        if layered { self.layer(|w| w.group_parts(g, ts)) } else { self.group_parts(g, ts) }
    }

    fn group_parts(&mut self, g: &Group, ts: Transform) -> bool {
        self.filters(g, ts) && clips(self, g.clip_path(), ts) && masks(self, g.mask(), ts) && self.children(g, ts)
    }

    fn children(&mut self, g: &Group, ts: Transform) -> bool {
        g.children().iter().all(|node| match node {
            Node::Group(child) => self.group(child, ts),
            Node::Path(p) => self.path(p, ts),
            Node::Image(i) => {
                self.count(rect_area(i.bounding_box(), ts))
                    && match i.kind() {
                        // Drawn on a canvas-sized layer of its own, under this transform.
                        ImageKind::SVG(tree) => self.layer(|w| w.group(tree.root(), ts)),
                        _ => true,
                    }
            }
            Node::Text(t) => self.group(t.flattened(), ts),
        })
    }

    /// One more drawn node painting `px` pixels.
    fn count(&mut self, px: f64) -> bool {
        self.nodes += 1;
        self.paint += px;
        self.nodes <= MAX_NODES && self.paint <= MAX_PAINT
    }

    fn path(&mut self, p: &Path, ts: Transform) -> bool {
        let paints = [p.fill().map(|f| f.paint()), p.stroke().map(|s| s.paint())];
        let px = rect_area(p.stroke_bounding_box(), ts) * paints.iter().flatten().count() as f64;
        if !self.count(px) {
            return false;
        }
        paints.into_iter().flatten().all(|paint| match paint {
            // A pattern draws its tile on a pixmap of its own each time the shape paints.
            Paint::Pattern(pat) => {
                let (sx, sy) = ts.pre_concat(pat.transform()).get_scale();
                let r = pat.rect();
                f64::from(r.width() * sx) * f64::from(r.height() * sy) <= MAX_TILE_PX
                    && self.group(pat.root(), Transform::from_scale(sx, sy))
            }
            _ => true,
        })
    }

    fn filters(&mut self, g: &Group, ts: Transform) -> bool {
        let (sx, sy) = ts.get_scale();
        for f in g.filters() {
            let region = area(f.rect().width(), f.rect().height(), ts);
            for p in f.primitives() {
                self.primitives += 1;
                if self.primitives > MAX_PRIMITIVES {
                    return false;
                }
                let per_pixel = match p.kind() {
                    filter::Kind::Morphology(m) => {
                        let (rx, ry) = (m.radius_x().get() * sx.abs(), m.radius_y().get() * sy.abs());
                        if !(rx <= MAX_RADIUS_PX && ry <= MAX_RADIUS_PX) {
                            return false;
                        }
                        f64::from((2.0 * rx.ceil() + 1.0) * (2.0 * ry.ceil() + 1.0))
                    }
                    filter::Kind::Turbulence(t) if t.num_octaves() > MAX_OCTAVES => return false,
                    filter::Kind::Turbulence(t) => 4.0 * f64::from(t.num_octaves().max(1)),
                    filter::Kind::ConvolveMatrix(c) => f64::from(c.matrix().columns()) * f64::from(c.matrix().rows()),
                    // Each input is fetched and drawn over the whole region.
                    filter::Kind::Merge(m) => m.inputs().len().max(1) as f64,
                    filter::Kind::Image(i) => {
                        if !self.group(i.root(), ts) {
                            return false;
                        }
                        1.0
                    }
                    _ => 1.0,
                };
                self.cost += region * per_pixel;
                if self.cost > MAX_COST {
                    return false;
                }
            }
        }
        true
    }
}

/// The clip path's chain, each link on a layer held while the next draws.
fn clips(w: &mut Walk, clip: Option<&ClipPath>, ts: Transform) -> bool {
    let Some(c) = clip else { return true };
    w.layer(|w| w.group(c.root(), ts.pre_concat(c.transform())) && clips(w, c.clip_path(), ts))
}

/// The mask's chain, each link on a layer held while the next draws.
fn masks(w: &mut Walk, mask: Option<&Mask>, ts: Transform) -> bool {
    let Some(m) = mask else { return true };
    w.layer(|w| w.group(m.root(), ts) && masks(w, m.mask(), ts))
}
