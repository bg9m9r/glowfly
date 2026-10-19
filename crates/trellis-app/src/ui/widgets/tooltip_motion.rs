//! Tooltips rise 4 px into place while egui fades them in, unless one is replacing a tooltip
//! that was just showing (sweeping along the toolbar switches at once) or Reduce motion is on.
//! The rise is a translation of the tooltip's layer, set once a frame after everything is
//! drawn (`animate`); egui's own area fade does the opacity.

use eframe::egui::{self, Context, Id, LayerId, Order, emath::TSTransform, vec2};

/// How long the fade and the rise take (egui's `animation_time`).
pub const FADE: f32 = 0.12;
/// How far below its place a tooltip starts.
pub const RISE: f32 = 4.0;
/// A tooltip shown within this long of another one closing replaces it without motion (egui's
/// tooltip grace time).
pub const GRACE: f32 = 0.3;

/// The tooltip layer seen last frame.
#[derive(Clone, Copy)]
struct Motion {
    layer: Option<LayerId>,
    /// When a tooltip was last visible (egui's clock).
    last_seen: f64,
    /// `layer` replaced a tooltip within `GRACE`: it doesn't move.
    switched: bool,
}

impl Default for Motion {
    fn default() -> Self {
        Self { layer: None, last_seen: f64::NEG_INFINITY, switched: false }
    }
}

fn key() -> Id {
    Id::new("trellis-tooltip-motion")
}

/// How far below its place a tooltip `age` seconds old is drawn: `RISE` at first, easing out to
/// 0 at `FADE`; always 0 with reduced motion.
pub fn rise(age: f32, reduce: bool) -> f32 {
    if reduce || age >= FADE {
        return 0.0;
    }
    RISE * (1.0 - egui::emath::easing::quadratic_out((age / FADE).clamp(0.0, 1.0)))
}

/// Whether the tooltip drawn on `layer` replaces one that was just showing (so it shouldn't
/// fade or rise). True on its first frame too, before `animate` has seen it.
pub fn switching(ctx: &Context, layer: LayerId) -> bool {
    let m = ctx.data(|d| d.get_temp::<Motion>(key())).unwrap_or_default();
    if m.layer == Some(layer) {
        return m.switched;
    }
    ctx.input(|i| i.time) - m.last_seen <= f64::from(GRACE)
}

/// Moves this frame's tooltip layer to where its rise has got to. Run after everything is drawn.
pub fn animate(ctx: &Context, reduce: bool) {
    let now = ctx.input(|i| i.time);
    let mut m = ctx.data(|d| d.get_temp::<Motion>(key())).unwrap_or_default();
    let painted = |layer: &LayerId| ctx.graphics(|g| g.get(*layer).is_some_and(|list| !list.is_empty()));
    let visible = ctx.memory(|mem| mem.areas().visible_layer_ids());
    let shown_at = |layer: &LayerId| egui::AreaState::load(ctx, layer.id).and_then(|a| a.last_became_visible_at).unwrap_or(now);
    // The newest tooltip painted this frame.
    let current =
        visible.into_iter().filter(|l| l.order == Order::Tooltip && painted(l)).max_by(|a, b| shown_at(a).total_cmp(&shown_at(b)));
    match current {
        Some(layer) => {
            if m.layer != Some(layer) {
                if let Some(prev) = m.layer {
                    ctx.set_transform_layer(prev, TSTransform::IDENTITY);
                }
                m.switched = now - m.last_seen <= f64::from(GRACE);
                m.layer = Some(layer);
            }
            m.last_seen = now;
            let r = if m.switched { 0.0 } else { rise((now - shown_at(&layer)) as f32, reduce) };
            let transform = if r > 0.0 { TSTransform::from_translation(vec2(0.0, r)) } else { TSTransform::IDENTITY };
            ctx.set_transform_layer(layer, transform);
            if r > 0.0 {
                ctx.request_repaint();
            }
        }
        None => {
            if let Some(prev) = m.layer.take() {
                ctx.set_transform_layer(prev, TSTransform::IDENTITY);
            }
        }
    }
    ctx.data_mut(|d| d.insert_temp(key(), m));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rise_eases_out_and_stops() {
        assert_eq!(rise(0.0, false), RISE);
        let mid = rise(0.06, false);
        assert!(0.0 < mid && mid < RISE, "{mid}");
        assert_eq!(rise(FADE, false), 0.0);
        assert_eq!(rise(1.0, false), 0.0);
        for age in [0.0, 0.06, 0.12, 2.0] {
            assert_eq!(rise(age, true), 0.0, "reduced motion at {age}");
        }
    }
}
