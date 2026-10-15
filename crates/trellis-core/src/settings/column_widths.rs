//! Widths of the graph's resizable columns (dragged from the header dividers), with the
//! per-column defaults and limits.

use serde::{Deserialize, Serialize};
use std::ops::RangeInclusive;

/// A graph column whose width can be dragged. GRAPH is sized by its lanes and COMMIT MESSAGE
/// takes what is left, so neither is here.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum ResizableColumn {
    Refs,
    Sha,
    Changes,
    Author,
    Date,
}

impl ResizableColumn {
    pub const ALL: [Self; 5] = [Self::Refs, Self::Sha, Self::Changes, Self::Author, Self::Date];

    /// The column's name in accessible labels ("Resize <title> column").
    pub fn title(self) -> &'static str {
        match self {
            Self::Refs => "branch / tag",
            Self::Sha => "SHA",
            Self::Changes => "changes",
            Self::Author => "author",
            Self::Date => "date",
        }
    }

    /// Width in px, including the gap before the column.
    pub fn default_width(self) -> f32 {
        match self {
            Self::Refs => 190.0,
            Self::Sha => 74.0,
            Self::Changes => 100.0,
            Self::Author => 148.0,
            Self::Date => 150.0,
        }
    }

    /// The widths a drag or a hand-edited file may set.
    pub fn range(self) -> RangeInclusive<f32> {
        match self {
            Self::Refs => 100.0..=480.0,
            Self::Sha => 60.0..=160.0,
            Self::Changes => 60.0..=220.0,
            Self::Author => 60.0..=360.0,
            Self::Date => 80.0..=360.0,
        }
    }

    /// Whether the column is resized from its right divider (dragging right widens it). The
    /// right-aligned columns are resized from their left divider instead.
    pub fn grows_right(self) -> bool {
        matches!(self, Self::Refs | Self::Sha)
    }
}

/// Graph column widths in px; read them through `get`, which sanitizes hand-edited values.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ColumnWidths {
    pub refs: f32,
    pub sha: f32,
    pub changes: f32,
    pub author: f32,
    pub date: f32,
}

impl Default for ColumnWidths {
    fn default() -> Self {
        use ResizableColumn::*;
        Self {
            refs: Refs.default_width(),
            sha: Sha.default_width(),
            changes: Changes.default_width(),
            author: Author.default_width(),
            date: Date.default_width(),
        }
    }
}

impl ColumnWidths {
    fn slot(&mut self, col: ResizableColumn) -> &mut f32 {
        match col {
            ResizableColumn::Refs => &mut self.refs,
            ResizableColumn::Sha => &mut self.sha,
            ResizableColumn::Changes => &mut self.changes,
            ResizableColumn::Author => &mut self.author,
            ResizableColumn::Date => &mut self.date,
        }
    }

    /// The column's width within its range (the default for a non-number).
    pub fn get(&self, col: ResizableColumn) -> f32 {
        let w = match col {
            ResizableColumn::Refs => self.refs,
            ResizableColumn::Sha => self.sha,
            ResizableColumn::Changes => self.changes,
            ResizableColumn::Author => self.author,
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
    fn column_width_defaults_match_old_layout() {
        let w = ColumnWidths::default();
        assert_eq!([Refs, Sha, Changes, Author, Date].map(|c| w.get(c)), [190.0, 74.0, 100.0, 148.0, 150.0]);
        assert_eq!(ResizableColumn::ALL.map(|c| w.get(c)), ResizableColumn::ALL.map(|c| c.default_width()));
    }

    #[test]
    fn column_widths_clamp_and_sanitize() {
        let mut w = ColumnWidths::default();
        assert!(w.set(Refs, 5000.0));
        assert_eq!(w.refs, 480.0);
        assert!(w.set(Sha, 3.0));
        assert_eq!(w.sha, 60.0);
        assert!(w.set(Author, 200.4));
        assert_eq!(w.author, 200.0);
        assert!(!w.set(Author, 200.0), "the same width is no change");
        assert!(!w.set(Date, f32::NAN));
        assert_eq!(w.date, 150.0);

        let edited = ColumnWidths { refs: f32::INFINITY, author: 9999.0, ..ColumnWidths::default() };
        assert_eq!(edited.get(Refs), 190.0);
        assert_eq!(edited.get(Author), 360.0);
    }

    #[test]
    fn reset_restores_the_default() {
        let mut w = ColumnWidths::default().with(Author, 220.0);
        assert_eq!(w.get(Author), 220.0);
        assert!(w.reset(Author));
        assert_eq!(w.get(Author), 148.0);
        assert!(!w.reset(Author));
    }

    #[test]
    fn drag_width_direction_and_room() {
        assert_eq!(drag_width(Refs, 190.0, 50.0, 500.0), 240.0);
        // Dragging a right-side column's divider left widens it.
        assert_eq!(drag_width(Author, 148.0, -60.0, 500.0), 208.0);
        assert_eq!(drag_width(Date, 150.0, 40.0, 500.0), 110.0);
        // The message column keeps its minimum.
        assert_eq!(drag_width(Author, 148.0, -100.0, 30.0), 178.0);
        assert_eq!(drag_width(Author, 148.0, 500.0, 0.0), 60.0);
        // Negative room counts as none.
        assert_eq!(drag_width(Sha, 74.0, 10.0, -5.0), 74.0);
    }
}
