//! Widths of the graph's resizable columns (dragged from the header dividers), with the
//! per-column defaults and limits.

use serde::{Deserialize, Serialize};
use std::ops::RangeInclusive;

/// A graph column whose width can be dragged. GRAPH is sized by its lanes and MESSAGE takes
/// what is left, so neither is here.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum ResizableColumn {
    Refs,
    Changes,
    /// The COMMIT column: the author with the short SHA under it.
    Commit,
    Date,
}

impl ResizableColumn {
    pub const ALL: [Self; 4] = [Self::Refs, Self::Changes, Self::Commit, Self::Date];

    /// The column's name in accessible labels ("Resize <title> column").
    pub fn title(self) -> &'static str {
        match self {
            Self::Refs => "branch / tag",
            Self::Changes => "changes",
            Self::Commit => "commit",
            Self::Date => "date",
        }
    }

    /// Width in px, including the gap before the column.
    pub fn default_width(self) -> f32 {
        match self {
            Self::Refs => 190.0,
            Self::Changes => 100.0,
            Self::Commit => 170.0,
            Self::Date => 160.0,
        }
    }

    /// The widths a drag or a hand-edited file may set.
    pub fn range(self) -> RangeInclusive<f32> {
        match self {
            Self::Refs => 100.0..=480.0,
            Self::Changes => 60.0..=220.0,
            Self::Commit => 80.0..=360.0,
            Self::Date => 80.0..=360.0,
        }
    }

    /// Whether the column is resized from its right divider (dragging right widens it). The
    /// right-aligned columns are resized from their left divider instead.
    pub fn grows_right(self) -> bool {
        matches!(self, Self::Refs)
    }
}

/// Graph column widths in px; read them through `get`, which sanitizes hand-edited values.
/// A width is the column's whole cell, divider to divider, including its 12 px padding and, at
/// the table's edge, the 16 px inset.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ColumnWidths {
    pub refs: f32,
    pub changes: f32,
    /// Files written before the COMMIT column stored the AUTHOR column's width; an old `sha`
    /// width is ignored.
    #[serde(alias = "author")]
    pub commit: f32,
    pub date: f32,
}

impl Default for ColumnWidths {
    fn default() -> Self {
        use ResizableColumn::*;
        Self { refs: Refs.default_width(), changes: Changes.default_width(), commit: Commit.default_width(), date: Date.default_width() }
    }
}

impl ColumnWidths {
    fn slot(&mut self, col: ResizableColumn) -> &mut f32 {
        match col {
            ResizableColumn::Refs => &mut self.refs,
            ResizableColumn::Changes => &mut self.changes,
            ResizableColumn::Commit => &mut self.commit,
            ResizableColumn::Date => &mut self.date,
        }
    }

    /// The column's width within its range (the default for a non-number).
    pub fn get(&self, col: ResizableColumn) -> f32 {
        let w = match col {
            ResizableColumn::Refs => self.refs,
            ResizableColumn::Changes => self.changes,
            ResizableColumn::Commit => self.commit,
            ResizableColumn::Date => self.date,
        };
        if !w.is_finite() {
            return col.default_width();
        }
        let r = col.range();
        w.clamp(*r.start(), *r.end())
    }

    /// Stores `w` rounded to whole pixels and clamped to the column's range (a non-number is
    /// ignored). Returns whether the stored value changed.
    pub fn set(&mut self, col: ResizableColumn, w: f32) -> bool {
        if !w.is_finite() {
            return false;
        }
        let r = col.range();
        let w = w.round().clamp(*r.start(), *r.end());
        let slot = self.slot(col);
        let changed = *slot != w;
        *slot = w;
        changed
    }

    /// Restores the column's default width; returns whether it changed.
    pub fn reset(&mut self, col: ResizableColumn) -> bool {
        let slot = self.slot(col);
        let changed = *slot != col.default_width();
        *slot = col.default_width();
        changed
    }

    /// A copy with one column set (the preview while a divider is dragged).
    pub fn with(mut self, col: ResizableColumn, w: f32) -> Self {
        self.set(col, w);
        self
    }
}

/// The width of `col` after dragging its divider `dx` px from a start width of `start`. It
/// grows by at most `room` (the space the message column can give up), and stays in range.
pub fn drag_width(col: ResizableColumn, start: f32, dx: f32, room: f32) -> f32 {
    let grown = start + if col.grows_right() { dx } else { -dx };
    let r = col.range();
    grown.min(start + room.max(0.0)).clamp(*r.start(), *r.end())
}

#[cfg(test)]
mod tests {
    use super::ResizableColumn::*;
    use super::*;

    #[test]
    fn column_width_defaults() {
        let w = ColumnWidths::default();
        assert_eq!([Refs, Changes, Commit, Date].map(|c| w.get(c)), [190.0, 100.0, 170.0, 160.0]);
        assert_eq!(ResizableColumn::ALL.map(|c| w.get(c)), ResizableColumn::ALL.map(|c| c.default_width()));
    }

    #[test]
    fn old_author_width_becomes_the_commit_width() {
        let w: ColumnWidths = serde_json::from_str(r#"{"refs":200,"sha":90,"author":210,"date":120}"#).unwrap();
        assert_eq!([Refs, Changes, Commit, Date].map(|c| w.get(c)), [200.0, 100.0, 210.0, 120.0]);
        let json = serde_json::to_value(w).unwrap();
        assert!(json.get("author").is_none() && json.get("sha").is_none(), "{json}");
        assert_eq!(serde_json::from_value::<ColumnWidths>(json).unwrap(), w);
    }

    #[test]
    fn column_widths_clamp_and_sanitize() {
        let mut w = ColumnWidths::default();
        assert!(w.set(Refs, 5000.0));
        assert_eq!(w.refs, 480.0);
        assert!(w.set(Commit, 3.0));
        assert_eq!(w.commit, 80.0);
        assert!(w.set(Commit, 200.4));
        assert_eq!(w.commit, 200.0);
        assert!(!w.set(Commit, 200.0), "the same width is no change");
        assert!(!w.set(Date, f32::NAN));
        assert_eq!(w.date, 160.0);

        let edited = ColumnWidths { refs: f32::INFINITY, commit: 9999.0, ..ColumnWidths::default() };
        assert_eq!(edited.get(Refs), 190.0);
        assert_eq!(edited.get(Commit), 360.0);
    }

    #[test]
    fn reset_restores_the_default() {
        let mut w = ColumnWidths::default().with(Commit, 220.0);
        assert_eq!(w.get(Commit), 220.0);
        assert!(w.reset(Commit));
        assert_eq!(w.get(Commit), 170.0);
        assert!(!w.reset(Commit));
    }

    #[test]
    fn drag_width_direction_and_room() {
        assert_eq!(drag_width(Refs, 190.0, 50.0, 500.0), 240.0);
        // Dragging a right-side column's divider left widens it.
        assert_eq!(drag_width(Commit, 170.0, -60.0, 500.0), 230.0);
        assert_eq!(drag_width(Date, 150.0, 40.0, 500.0), 110.0);
        // The message column keeps its minimum.
        assert_eq!(drag_width(Commit, 170.0, -100.0, 30.0), 200.0);
        assert_eq!(drag_width(Commit, 170.0, 500.0, 0.0), 80.0);
        // Negative room counts as none.
        assert_eq!(drag_width(Refs, 190.0, 10.0, -5.0), 190.0);
    }
}
