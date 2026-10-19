//! The history table's column layout. Each column is a cell, divider to divider, whose stored
//! width (`ColumnWidths`) is the whole cell; its content sits `CELL_PAD` in from a divider and
//! `EDGE_INSET` in from the table's edge, so neighbouring columns' content is 24 px apart with
//! the divider in the middle.

use eframe::egui::Rect;
use trellis_core::settings::{ColumnWidths, GraphColumns, ResizableColumn};

/// The first and last shown columns' content from the table's edges; also the search bar's and
/// the header notes line's inset.
pub(super) const EDGE_INSET: f32 = 16.0;
/// Each cell's content from the divider beside it.
pub(super) const CELL_PAD: f32 = 12.0;
/// The narrowest a drag leaves the message column's content.
const MIN_MSG_W: f32 = 120.0;

/// Each column's cell, divider to divider (empty ranges for hidden columns).
pub struct Edges {
    pub refs: (f32, f32),
    pub graph: (f32, f32),
    pub msg: (f32, f32),
    pub changes: (f32, f32),
    pub commit: (f32, f32),
    pub date: (f32, f32),
}

/// Horizontal extent of each column's content for one row (empty ranges for hidden columns),
/// left to right: BRANCH / TAG, GRAPH, MESSAGE, CHANGES, COMMIT, DATE; with each column's cell
/// and the table's inner extent.
pub struct Columns {
    pub refs: (f32, f32),
    pub graph: (f32, f32),
    pub msg: (f32, f32),
    pub changes: (f32, f32),
    pub commit: (f32, f32),
    pub date: (f32, f32),
    pub cell: Edges,
    /// The table inset by `EDGE_INSET` on each side.
    pub inner: (f32, f32),
}

impl Edges {
    /// The cells left to right.
    pub fn all(&self) -> [(f32, f32); 6] {
        [self.refs, self.graph, self.msg, self.changes, self.commit, self.date]
    }
}

/// `cell`'s content: inset by `EDGE_INSET` on a side at the table's edge, `CELL_PAD` otherwise.
fn content(cell: (f32, f32), first: bool, last: bool) -> (f32, f32) {
    let pad = |edge: bool| if edge { EDGE_INSET } else { CELL_PAD };
    let left = cell.0 + pad(first);
    (left, (cell.1 - pad(last)).max(left))
}

impl Columns {
    /// Lays out the columns in `rect`; `graph_w` is the GRAPH column's content width.
    pub fn new(rect: Rect, graph_w: f32, c: &GraphColumns, w: &ColumnWidths) -> Self {
        use ResizableColumn::*;
        let (l, r) = (rect.left(), rect.right());
        let refs = if c.refs { (l, l + w.get(Refs)) } else { (l, l) };
        let lead = if c.refs { CELL_PAD } else { EDGE_INSET };
        let graph = if c.tree { (refs.1, refs.1 + lead + graph_w + CELL_PAD) } else { (refs.1, refs.1) };
        let date = if c.date { (r - w.get(Date), r) } else { (r, r) };
        let commit = if c.commit { (date.0 - w.get(Commit), date.0) } else { (date.0, date.0) };
        let changes = if c.changes { (commit.0 - w.get(Changes), commit.0) } else { (commit.0, commit.0) };
        let msg = (graph.1, changes.0);
        let cell = Edges { refs, graph, msg, changes, commit, date };
        // Which column is the first shown from the left and the last from the right; the
        // message column always takes part.
        let shown = [c.refs, c.tree, true, c.changes, c.commit, c.date];
        let first = shown.iter().position(|&s| s).unwrap_or(2);
        let last = shown.iter().rposition(|&s| s).unwrap_or(2);
        let inset = |i: usize, cell: (f32, f32)| if shown[i] { content(cell, i == first, i == last) } else { (cell.0, cell.0) };
        Self {
            refs: inset(0, refs),
            graph: inset(1, graph),
            msg: inset(2, msg),
            changes: inset(3, changes),
            commit: inset(4, commit),
            date: inset(5, date),
            cell,
            inner: (l + EDGE_INSET, r - EDGE_INSET),
        }
    }

    /// The columns' content left to right.
    pub fn content(&self) -> [(f32, f32); 6] {
        [self.refs, self.graph, self.msg, self.changes, self.commit, self.date]
    }

    /// Each shown resizable column with the x of the divider that resizes it: the one on the
    /// right of BRANCH / TAG, the one on the left of the right-aligned columns.
    pub fn grips(&self, c: &GraphColumns) -> impl Iterator<Item = (ResizableColumn, f32)> {
        [
            (c.refs, ResizableColumn::Refs, self.cell.refs.1),
            (c.changes, ResizableColumn::Changes, self.cell.changes.0),
            (c.commit, ResizableColumn::Commit, self.cell.commit.0),
            (c.date, ResizableColumn::Date, self.cell.date.0),
        ]
        .into_iter()
        .filter(|(on, ..)| *on)
        .map(|(_, col, x)| (col, x))
    }

    /// How much wider the columns may grow before the message column drops below `MIN_MSG_W`.
    pub fn msg_room(&self) -> f32 {
        (self.msg.1 - self.msg.0 - MIN_MSG_W).max(0.0)
    }

    /// This tab's WIP row's cell: from the message column's content to the table's inset right
    /// edge, across the changes, commit and date columns that the row leaves empty.
    pub fn wip_span(&self) -> (f32, f32) {
        (self.msg.0, self.inner.1.max(self.msg.0))
    }

    pub fn rect(&self, col: (f32, f32), row: Rect) -> Rect {
        Rect::from_x_y_ranges(col.0..=col.1.max(col.0), row.y_range())
    }
}

#[cfg(test)]
mod tests {
    use super::super::nodes::NODE_R;
    use super::super::paint::lane_x;
    use super::*;
    use eframe::egui::{pos2, vec2};

    const GRAPH_W: f32 = 72.0;

    fn cols(c: GraphColumns) -> (Rect, Columns) {
        let rect = Rect::from_min_size(pos2(10.0, 0.0), vec2(740.0, 40.0));
        (rect, Columns::new(rect, GRAPH_W, &c, &ColumnWidths::default()))
    }

    #[test]
    fn default_columns_are_contiguous_cells_with_padded_content() {
        let (rect, c) = cols(GraphColumns::default());
        assert_eq!(c.refs.0, rect.left() + EDGE_INSET);
        assert_eq!(c.date.1, rect.right() - EDGE_INSET);
        let e = &c.cell;
        assert_eq!(e.refs.0, rect.left());
        assert_eq!((e.refs.1, e.graph.1, e.msg.1, e.commit.1, e.date.1), (e.graph.0, e.msg.0, e.commit.0, e.date.0, rect.right()));
        assert_eq!(c.graph.1 - c.graph.0, GRAPH_W);
        for (content, cell) in [(c.refs, e.refs), (c.graph, e.graph), (c.msg, e.msg), (c.commit, e.commit), (c.date, e.date)] {
            if cell.0 > rect.left() {
                assert_eq!(content.0 - cell.0, CELL_PAD, "{content:?} in {cell:?}");
            }
            if cell.1 < rect.right() {
                assert_eq!(cell.1 - content.1, CELL_PAD, "{content:?} in {cell:?}");
            }
        }
    }

    #[test]
    fn hidden_refs_put_the_graph_at_the_inset() {
        let (rect, c) = cols(GraphColumns { refs: false, ..GraphColumns::default() });
        assert_eq!(c.graph.0, rect.left() + EDGE_INSET);
        assert_eq!(c.cell.graph.0, rect.left());
    }

    #[test]
    fn the_message_takes_the_inset_when_it_is_first_or_last() {
        let (rect, c) = cols(GraphColumns { changes: false, commit: false, date: false, ..GraphColumns::default() });
        assert_eq!(c.msg.1, rect.right() - EDGE_INSET);
        let (rect, c) = cols(GraphColumns { refs: false, tree: false, ..GraphColumns::default() });
        assert_eq!(c.msg.0, rect.left() + EDGE_INSET);
    }

    #[test]
    fn grips_sit_on_cell_boundaries() {
        for cfg in [GraphColumns::default(), GraphColumns { tree: false, changes: true, ..GraphColumns::default() }] {
            let (_, c) = cols(cfg);
            let grips: Vec<_> = c.grips(&cfg).collect();
            let want: Vec<_> = [
                (cfg.refs, ResizableColumn::Refs, c.cell.refs.1),
                (cfg.changes, ResizableColumn::Changes, c.cell.changes.0),
                (cfg.commit, ResizableColumn::Commit, c.cell.commit.0),
                (cfg.date, ResizableColumn::Date, c.cell.date.0),
            ]
            .into_iter()
            .filter(|w| w.0)
            .map(|w| (w.1, w.2))
            .collect();
            assert_eq!(grips, want);
        }
        let cfg = GraphColumns { tree: false, ..GraphColumns::default() };
        let (_, c) = cols(cfg);
        assert_eq!(c.cell.refs.1, c.cell.msg.0, "with the graph hidden the refs divider is the message's left edge");
    }

    #[test]
    fn the_wip_span_ends_at_the_inset() {
        let (rect, c) = cols(GraphColumns::default());
        assert_eq!(c.wip_span(), (c.msg.0, rect.right() - EDGE_INSET));
    }

    #[test]
    fn a_narrow_table_never_inverts_the_message() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 40.0));
        let c = Columns::new(rect, GRAPH_W, &GraphColumns::default(), &ColumnWidths::default());
        assert!(c.msg.1 >= c.msg.0, "{:?}", c.msg);
    }

    #[test]
    fn the_first_node_stays_inside_its_cell() {
        for refs in [true, false] {
            let (_, c) = cols(GraphColumns { refs, ..GraphColumns::default() });
            assert!(lane_x(c.graph.0, 0) - NODE_R >= c.cell.graph.0);
        }
    }
}
