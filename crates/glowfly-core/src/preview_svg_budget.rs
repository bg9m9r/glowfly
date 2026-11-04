//! How much work an SVG may ask of the renderer before the preview refuses it: resvg has no
//! limits of its own on filters, so a few bytes of `feMorphology` with a huge radius or
//! `feTurbulence` with a huge `numOctaves` could keep a worker thread busy for hours, and a
//! deep stack of filtered or translucent groups holds a whole layer for each (about 100 MB at
//! the largest filter region), where a failed allocation aborts the app. `within` walks the
//! parsed tree, nested pictures, clip paths, masks, patterns and `feImage` included, and
//! never weakens a filter: a drawing with weaker filters would be a wrong picture. The
//! renderer may instead draw it smaller (`preview_svg::render`), and refuses it past that.

use crate::preview_image::SHOWN_MAX;
use resvg::usvg::{Group, Node, Tree, filter};

/// The deepest stack of groups that each draw on a layer of their own (filter, opacity, mask,
/// clip path or isolation).
const MAX_LAYERS: usize = 16;
/// The most filter primitives applied, counting a filter once for each group that uses it.
const MAX_PRIMITIVES: usize = 128;
/// The most `feTurbulence` octaves (browsers gain nothing visible past about 10).
const MAX_OCTAVES: u32 = 10;
/// The largest `feMorphology` radius, in output pixels.
const MAX_RADIUS_PX: f32 = 32.0;
/// The most pixel operations the filters may add up to: a few hundred milliseconds of work.
const MAX_COST: f64 = 4.0e8;
/// The largest filter region resvg draws: five times the canvas each way.
const MAX_REGION_PX: f64 = (5 * SHOWN_MAX as u64 * 5 * SHOWN_MAX as u64) as f64;

#[derive(Default)]
struct Spent {
    primitives: usize,
    cost: f64,
}

/// Whether `tree`, drawn at `scale` output pixels per user unit, is within the budget.
pub(crate) fn within(tree: &Tree, scale: f32) -> bool {
    walk(tree.root(), scale, false, 0, &mut Spent::default())
}

/// Walks `g`, `layers` deep in layered groups. In a nested picture (`nested`) the transforms
/// don't say its output size, so each filter region counts as the largest.
fn walk(g: &Group, scale: f32, nested: bool, layers: usize, spent: &mut Spent) -> bool {
    let layered = g.isolate() || !g.filters().is_empty() || g.opacity().get() < 1.0 || g.mask().is_some() || g.clip_path().is_some();
    let layers = layers + usize::from(layered);
    if layers > MAX_LAYERS {
        return false;
    }
    let (sx, sy) = g.abs_transform().get_scale();
    let (sx, sy) = (sx.abs() * scale, sy.abs() * scale);
    for f in g.filters() {
        let r = f.rect();
        let region = if nested { MAX_REGION_PX } else { (f64::from(r.width() * sx) * f64::from(r.height() * sy)).min(MAX_REGION_PX) };
        for p in f.primitives() {
            spent.primitives += 1;
            if spent.primitives > MAX_PRIMITIVES {
                return false;
            }
            let per_pixel = match p.kind() {
                filter::Kind::Morphology(m) => {
                    let (rx, ry) = (m.radius_x().get() * sx, m.radius_y().get() * sy);
                    if !(rx <= MAX_RADIUS_PX && ry <= MAX_RADIUS_PX) {
                        return false;
                    }
                    f64::from((2.0 * rx.ceil() + 1.0) * (2.0 * ry.ceil() + 1.0))
                }
                filter::Kind::Turbulence(t) if t.num_octaves() > MAX_OCTAVES => return false,
                filter::Kind::Turbulence(t) => 4.0 * f64::from(t.num_octaves().max(1)),
                filter::Kind::ConvolveMatrix(c) => f64::from(c.matrix().columns()) * f64::from(c.matrix().rows()),
                _ => 1.0,
            };
            spent.cost += region * per_pixel;
            if spent.cost > MAX_COST {
                return false;
            }
        }
    }
    for node in g.children() {
        if let Node::Group(child) = node
            && !walk(child, scale, nested, layers, spent)
        {
            return false;
        }
        let mut ok = true;
        node.subroots(|root| ok = ok && walk(root, scale, true, layers, spent));
        if !ok {
            return false;
        }
    }
    true
}
