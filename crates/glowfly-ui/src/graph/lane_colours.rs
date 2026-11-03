//! The colours one graph row draws in, read from the snapshot's lane owners: its node (and band
//! and chip join), each lane above and below the node, and each ref chip's own branch; and how
//! each lane line is drawn (`LaneLine`): HEAD's branch wider, and in the colour-blind safe
//! colours a dash pattern by the lane's distance from HEAD's lane.

use super::paint::{LaneLine, Pattern};
use super::ref_group::Chip;
use crate::ui::theme::branch_colours::line;
use crate::ui::theme::color;
use crate::ui::theme::ref_chip::BranchTone;
use eframe::egui::Color32;
use glowfly_core::graph::Layout;
use glowfly_core::graph::owners::{Owners, RowPos};
use glowfly_core::repo::{RefKind, Snapshot};

/// One row's colours.
pub(super) struct RowColours<'a> {
    owners: &'a Owners,
    layout: &'a Layout,
    pos: RowPos,
    node: Option<u32>,
    /// HEAD's lane for the whole graph (0 when HEAD isn't loaded) and its branch's owner.
    head: (usize, Option<u32>),
    /// The lanes carry dash patterns (the colour-blind safe colours).
    patterned: bool,
    /// Where the graph's content starts on screen: dashes run on from it across rows.
    origin: f32,
}

impl<'a> RowColours<'a> {
    fn new(snap: &'a Snapshot, pos: RowPos, node: Option<u32>) -> Self {
        let head = match snap.head_oid.and_then(|h| snap.row_of.get(&h).copied()) {
            Some(i) => (snap.graph.rows.get(i).map_or(0, |r| r.col), snap.graph.owners.commit.get(i).copied()),
            None => (0, snap.graph.owners.wip),
        };
        Self { owners: &snap.graph.owners, layout: &snap.graph, pos, node, head, patterned: color::colour_safe(), origin: 0.0 }
    }

    /// These colours with the graph's content starting at `origin` on screen (the row's top less
    /// the rows above it), so dash patterns line up from row to row and scroll with the rows.
    pub fn with_origin(self, origin: f32) -> Self {
        Self { origin, ..self }
    }

    /// Commit row `i`.
    pub fn commit(snap: &'a Snapshot, i: usize) -> Self {
        Self::new(snap, RowPos::commit(i), snap.graph.owners.commit.get(i).copied())
    }

    /// This tab's WIP row.
    pub fn wip(snap: &'a Snapshot) -> Self {
        Self::new(snap, RowPos::wip(), snap.graph.owners.wip)
    }

    /// Another worktree's WIP row, `k` in `Layout::worktree_wips`.
    pub fn worktree(snap: &'a Snapshot, k: usize) -> Self {
        let before = snap.graph.worktree_wips.get(k).map_or(0, |w| w.before);
        Self::new(snap, RowPos::worktree(before), snap.graph.owners.worktree_wips.get(k).copied())
    }

    fn tone(&self, owner: Option<u32>) -> Color32 {
        owner.map_or_else(color::faint, |o| line(self.layout.swatch(o)))
    }

    /// The node's colour, also its row band and the line to its chips.
    pub fn node(&self) -> Color32 {
        self.tone(self.node)
    }

    /// How a line owned by `owner` and ending in `lane` is drawn.
    fn line(&self, owner: Option<u32>, lane: usize) -> LaneLine {
        let head = owner.is_some() && owner == self.head.1;
        let pattern = if self.patterned { Pattern::for_distance(lane.abs_diff(self.head.0)) } else { Pattern::Solid };
        LaneLine { colour: self.tone(owner), width: if head { 2.5 } else { 2.0 }, pattern, origin: self.origin }
    }

    /// The line of `lane` coming into the row from above, drawn to lane `to`.
    pub fn line_above(&self, lane: usize, to: usize) -> LaneLine {
        self.line(self.owners.lane_before(lane, self.pos), to)
    }

    /// The line of `lane` leaving the row downward.
    pub fn line_below(&self, lane: usize) -> LaneLine {
        self.line(self.owners.lane_after(lane, self.pos), lane)
    }
}

/// Each chip's tones: its branch's line and chip colours; the node's for HEAD and tags (their
/// styles don't use them) and for a branch without a swatch.
pub(super) fn chip_tones(snap: &Snapshot, chips: &[Chip], node: Color32) -> Vec<BranchTone> {
    let pal = color::current();
    chips
        .iter()
        .map(|c| match c.label.kind {
            RefKind::Local | RefKind::Remote => snap.ref_swatch(&c.label.full).map_or(BranchTone::plain(node), |s| BranchTone::of(s, &pal)),
            RefKind::Head | RefKind::Tag => BranchTone::plain(node),
        })
        .collect()
}
