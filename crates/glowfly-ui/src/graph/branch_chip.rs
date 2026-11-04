//! A branch's ref chip outside the graph, drawn as the Branch / tag column draws it (its
//! colour, and the checked-out branch's tick in semibold): the rebase panel's tip row.

use super::labels::{ChipSpec, draw_chip, spec_w};
use super::ref_group::Chip;
use crate::ui::theme::color;
use crate::ui::theme::ref_chip::BranchTone;
use eframe::egui::{Painter, Rect};
use glowfly_core::repo::{RefKind, RefLabel, Snapshot};

/// Paints the chip of local branch `name` (with the tick when it is checked out) from `x`,
/// centred on `y`, at most `room` wide (a longer name is cut), and returns its rect.
pub(crate) fn paint(p: &Painter, x: f32, y: f32, snap: Option<&Snapshot>, name: &str, room: f32) -> Rect {
    let is_head = snap.is_some_and(|s| s.head_branch.as_deref() == Some(name));
    let label = RefLabel { name: name.to_owned(), full: format!("refs/heads/{name}"), kind: RefKind::Local, is_head };
    let spec = ChipSpec::of(&Chip { label: &label, remotes: Vec::new() }, None);
    let swatch = snap.and_then(|s| s.ref_swatch(&label.full));
    let tone = swatch.map_or_else(|| BranchTone::plain(color::accent()), |s| BranchTone::of(s, &color::current()));
    draw_chip(p, x, y, &spec, tone, spec_w(p, &spec).min(room))
}
