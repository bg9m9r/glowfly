//! The colours one graph row draws in, read from the snapshot's lane owners: its node (and band
//! and chip join), each lane above and below the node, and each ref chip's own branch.

use super::ref_group::Chip;
use crate::ui::theme::branch_colours::{chip, line};
use crate::ui::theme::color;
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
}

impl<'a> RowColours<'a> {
    fn new(snap: &'a Snapshot, pos: RowPos, node: Option<u32>) -> Self {
        Self { owners: &snap.graph.owners, layout: &snap.graph, pos, node }
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

    /// `lane` coming into the row from above.
    pub fn above(&self, lane: usize) -> Color32 {
        self.tone(self.owners.lane_before(lane, self.pos))
    }

    /// `lane` leaving the row downward.
    pub fn below(&self, lane: usize) -> Color32 {
        self.tone(self.owners.lane_after(lane, self.pos))
    }
}

/// Each chip's tone: its branch's chip colour; the node's for HEAD and tags (their styles don't
/// use it).
pub(super) fn chip_tones(snap: &Snapshot, chips: &[Chip], node: Color32) -> Vec<Color32> {
    chips
        .iter()
        .map(|c| match c.label.kind {
            RefKind::Local | RefKind::Remote => snap.ref_swatch(&c.label.full).map_or(node, chip),
            RefKind::Head | RefKind::Tag => node,
        })
        .collect()
}
