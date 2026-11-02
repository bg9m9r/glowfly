//! How wide BRANCH / TAG's chips want to be: each row's first chip, with a "+N" chip after it
//! for the rest (the most a row folds to), and the widest row's of those. The header fits the
//! column to it (`header::refs_fitted`); a row whose chips lead its message (a narrow table)
//! takes its own.

use super::labels::{self, ChipSpec};
use super::ref_group::{Chip, RefContext, chips};
use eframe::egui::Painter;
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
/// reaches `ENOUGH`.
pub(super) fn chips_w(p: &Painter, snap: &Snapshot, cx: &RefContext) -> f32 {
    let mut widest = 0.0_f32;
    for labels in snap.refs.values() {
        widest = widest.max(row_w(p, &chips(labels, cx)));
        if widest >= ENOUGH {
            break;
        }
    }
    widest
}
