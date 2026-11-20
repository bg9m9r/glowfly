//! How wide BRANCH / TAG's chips want to be: each row's first chip, with a "+N" chip after it
//! for the rest (the most a row folds to), and the widest row's of those. The header fits the
//! column to it (`header::refs_fitted`); a row whose chips lead its message (a narrow table)
//! takes its own. The widest is measured once per snapshot, chip font and zoom and kept in egui's
//! temporary data (`frame_memo`), not on every frame.

use super::labels::{self, ChipSpec};
use super::ref_group::{Chip, RefContext, chips};
use crate::frame_memo;
use crate::theme::fonts;
use eframe::egui::{FontId, Id, Painter};
use glowfly_core::repo::Snapshot;

/// The widest BRANCH / TAG fits itself to; no row is measured past it.
const ENOUGH: f32 = 200.0;

/// One row's chips folded to its first and a "+N" chip for the rest.
pub(super) fn row_w(p: &Painter, chips: &[Chip]) -> f32 {
    let Some(first) = chips.first() else { return 0.0 };
    let w = labels::spec_w(p, &ChipSpec::of(first, None));
    if chips.len() > 1 { w + labels::GAP + labels::plus_w(p, chips.len() - 1) } else { w }
}

/// The widest `row_w` over the history's labelled commits, measuring no further once a row
/// reaches `ENOUGH`: kept per snapshot (its status stamp covers the upstreams and remotes `cx` is
/// made of), zoom and chip font.
pub(super) fn chips_w(p: &Painter, snap: &Snapshot, cx: &RefContext) -> f32 {
    let key = (snap.status_stamp, p.ctx().pixels_per_point(), chip_font(p));
    frame_memo::kept(p.ctx(), Id::new("graph-refs-chips-w"), key, || measure(p, snap, cx))
}

/// The one chip font that depends on the installed fonts (the checked-out branch's semibold); the
/// regular one is the constant `FontId::proportional(labels::FONT)`.
fn chip_font(p: &Painter) -> FontId {
    fonts::semibold_in(p.ctx(), labels::FONT)
}

/// `chips_w`, measured.
fn measure(p: &Painter, snap: &Snapshot, cx: &RefContext) -> f32 {
    let mut widest = 0.0_f32;
    for labels in snap.refs.values() {
        widest = widest.max(row_w(p, &chips(labels, cx)));
        if widest >= ENOUGH {
            break;
        }
    }
    widest
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::ref_group::Upstreams;
    use crate::snapshot_tests::{remove_fixture, with_repo};
    use crate::theme::fonts;
    use eframe::egui::{self, LayerId};
    use glowfly_core::settings::ThemeChoice;
    use std::sync::Arc;

    fn cx(snap: &Snapshot) -> RefContext<'_> {
        RefContext::of(snap, Arc::new(Upstreams::of(&snap.locals)))
    }

    /// Runs `body` in a frame with the painter of the background layer.
    fn frame(ctx: &egui::Context, mut body: impl FnMut(&Painter)) {
        let _ = ctx.run(egui::RawInput::default(), |ctx| body(&ctx.layer_painter(LayerId::background())));
    }

    #[test]
    fn the_chip_width_is_kept_until_the_snapshot_or_zoom_changes() {
        let (mut st, dir) = with_repo(ThemeChoice::Dark, "chips-w-kept");
        let snap = st.snap.as_mut().unwrap();
        let ctx = egui::Context::default();
        fonts::use_test_cjk();
        fonts::install(&ctx);
        let _ = ctx.run(egui::RawInput::default(), |_| {});
        let mut w0 = 0.0;
        frame(&ctx, |p| w0 = chips_w(p, &*snap, &cx(snap)));
        assert!(w0 > 0.0);
        for label in snap.refs.values_mut().flatten() {
            label.name.push_str("-with-a-rather-long-name");
        }
        frame(&ctx, |p| assert_eq!(chips_w(p, &*snap, &cx(snap)), w0, "kept for the same snapshot"));
        snap.status_stamp = glowfly_core::repo::next_status_stamp();
        frame(&ctx, |p| {
            let w = chips_w(p, &*snap, &cx(snap));
            assert!(w > w0, "a new snapshot measures again: {w} vs {w0}");
            assert_eq!(w, measure(p, &*snap, &cx(snap)));
        });
        ctx.set_pixels_per_point(1.5);
        frame(&ctx, |p| assert_eq!(chips_w(p, &*snap, &cx(snap)), measure(p, &*snap, &cx(snap)), "a new zoom measures again"));
        remove_fixture(&dir);
    }
}
