//! How the history table's columns give way when the graph is narrow, so MESSAGE keeps room and
//! no column runs over another. GRAPH is capped at about a third of the table (its lanes beyond
//! that clipped to its cell). Then, while MESSAGE has less than `MSG_WANT`, in turn: BRANCH / TAG
//! narrows to 120 px, COMMIT to 80 (its SHA still fits), DATE to 130; then CHANGES, DATE and
//! COMMIT hide; and while MESSAGE has less than `MSG_MIN`, BRANCH / TAG narrows to its floor.
//! None of this is saved: the stored widths come back as the window widens.

use super::layout::CELL_PAD;
use super::layout::EDGE_INSET;
use glowfly_core::settings::{ColumnWidths, GraphColumns, ResizableColumn};

/// The message content width the other columns give way for, and the least it keeps while a
/// column can still give way. A 1400 px window's default columns leave it about 76 px, which
/// stays as it was.
pub(super) const MSG_WANT: f32 = 72.0;
pub(super) const MSG_MIN: f32 = 48.0;
/// The share of the table the GRAPH column's lanes may take (at least its title's width).
const GRAPH_SHARE: f32 = 0.3;

/// One step of giving way.
#[derive(Clone, Copy)]
enum Step {
    /// Narrow the column toward this width (never below its floor).
    Narrow(ResizableColumn, f32),
    Hide(ResizableColumn),
}

/// The steps in turn, each with the message width it gives way for.
const STEPS: [(f32, Step); 7] = [
    (MSG_WANT, Step::Narrow(ResizableColumn::Refs, 120.0)),
    (MSG_WANT, Step::Narrow(ResizableColumn::Commit, 80.0)),
    (MSG_WANT, Step::Narrow(ResizableColumn::Date, 130.0)),
    (MSG_WANT, Step::Hide(ResizableColumn::Changes)),
    (MSG_WANT, Step::Hide(ResizableColumn::Date)),
    (MSG_WANT, Step::Hide(ResizableColumn::Commit)),
    (MSG_MIN, Step::Narrow(ResizableColumn::Refs, 0.0)),
];

/// The columns as drawn this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Fitted {
    pub cfg: GraphColumns,
    pub widths: ColumnWidths,
    /// The GRAPH column's content width.
    pub graph_w: f32,
    /// The columns hidden to fit, left to right.
    pub hidden: Vec<ResizableColumn>,
}

fn shown(cfg: &GraphColumns, col: ResizableColumn) -> bool {
    match col {
        ResizableColumn::Refs => cfg.refs,
        ResizableColumn::Changes => cfg.changes,
        ResizableColumn::Commit => cfg.commit,
        ResizableColumn::Date => cfg.date,
    }
}

fn hide(cfg: &mut GraphColumns, col: ResizableColumn) {
    match col {
        ResizableColumn::Refs => cfg.refs = false,
        ResizableColumn::Changes => cfg.changes = false,
        ResizableColumn::Commit => cfg.commit = false,
        ResizableColumn::Date => cfg.date = false,
    }
}

/// The message content's width in a table `width` wide.
fn message_w(width: f32, graph_w: f32, cfg: &GraphColumns, w: &ColumnWidths) -> f32 {
    let col = |c: ResizableColumn| if shown(cfg, c) { w.get(c) } else { 0.0 };
    let lead = if cfg.refs { CELL_PAD } else { EDGE_INSET };
    let graph = if cfg.tree { lead + graph_w + CELL_PAD } else { 0.0 };
    let right = [ResizableColumn::Changes, ResizableColumn::Commit, ResizableColumn::Date].map(col).iter().sum::<f32>();
    let pads = if right > 0.0 { 2.0 * CELL_PAD } else { CELL_PAD + EDGE_INSET };
    width - col(ResizableColumn::Refs) - graph - right - pads
}

/// The columns for a table `width` wide: `graph` is the GRAPH column's content width for its
/// lanes and the least it takes (its title), `w` the widths to draw at (each at least its
/// `floors` entry).
pub fn fit(width: f32, graph: (f32, f32), cfg: &GraphColumns, w: ColumnWidths, floors: &[(ResizableColumn, f32)]) -> Fitted {
    let (mut cfg, mut w) = (*cfg, w);
    let graph_w = graph.0.min((width * GRAPH_SHARE).max(graph.1));
    let floor = |c: ResizableColumn| floors.iter().find(|f| f.0 == c).map_or(0.0, |f| f.1);
    let mut hidden = Vec::new();
    for (want, step) in STEPS {
        let short = want - message_w(width, graph_w, &cfg, &w);
        if short <= 0.0 {
            continue;
        }
        match step {
            Step::Narrow(col, to) if shown(&cfg, col) => {
                let (now, least) = (w.get(col), to.max(floor(col)));
                if now > least {
                    w = w.with(col, (now - short).max(least).ceil());
                }
            }
            Step::Hide(col) if shown(&cfg, col) => {
                hide(&mut cfg, col);
                hidden.push(col);
            }
            _ => {}
        }
    }
    hidden.sort_by_key(|c| ResizableColumn::ALL.iter().position(|a| a == c));
    Fitted { cfg, widths: w, graph_w, hidden }
}

#[cfg(test)]
mod tests {
    use super::super::layout::Columns;
    use super::*;
    use ResizableColumn::*;
    use eframe::egui::{Rect, pos2, vec2};

    const FLOORS: [(ResizableColumn, f32); 4] = [(Refs, 100.0), (Changes, 90.0), (Commit, 84.0), (Date, 60.0)];
    /// Five lanes, and the GRAPH title's width.
    const GRAPH: (f32, f32) = (120.0, 40.0);

    fn at(width: f32) -> Fitted {
        fit(width, GRAPH, &GraphColumns::default(), ColumnWidths::default(), &FLOORS)
    }

    #[test]
    fn a_wide_table_keeps_every_column() {
        let f = at(1200.0);
        assert_eq!((f.cfg, f.widths, f.graph_w), (GraphColumns::default(), ColumnWidths::default(), 120.0));
        assert!(f.hidden.is_empty());
    }

    #[test]
    fn columns_give_way_in_turn() {
        // BRANCH / TAG first, then COMMIT, then DATE narrows.
        assert_eq!(at(760.0).widths, ColumnWidths::default(), "the message has its room");
        let f = at(740.0);
        assert!(f.widths.get(Refs) < 190.0 && f.widths.get(Commit) == 170.0, "{f:?}");
        let f = at(680.0);
        assert_eq!(f.widths.get(Refs), 120.0);
        assert!(f.widths.get(Commit) < 170.0 && f.widths.get(Date) == 160.0, "{f:?}");
        let f = at(580.0);
        assert_eq!((f.widths.get(Commit), f.widths.get(Date)), (84.0, 136.0), "COMMIT at its floor");
        // Then DATE hides, then COMMIT.
        let f = at(520.0);
        assert_eq!(f.hidden, vec![Date], "{f:?}");
        let f = at(420.0);
        assert_eq!(f.hidden, vec![Commit, Date], "{f:?}");
        assert!(!f.cfg.commit && !f.cfg.date && f.cfg.refs && f.cfg.tree);
    }

    #[test]
    fn the_message_keeps_its_room_until_nothing_can_give() {
        for width in (300..=1400).step_by(10).map(|w| w as f32) {
            let f = at(width);
            let msg = message_w(width, f.graph_w, &f.cfg, &f.widths);
            let floors_reached = f.widths.get(Refs) <= 100.0 && f.hidden.len() == 2;
            assert!(msg >= MSG_MIN - 1.0 || floors_reached, "{width}: message {msg} with {f:?}");
            if width >= 450.0 {
                assert!(msg >= MSG_WANT - 1.0, "{width}: message {msg} with {f:?}");
            }
        }
    }

    #[test]
    fn no_column_runs_over_another() {
        for width in (300..=1400).step_by(10).map(|w| w as f32) {
            for cfg in [GraphColumns::default(), GraphColumns { changes: true, ..GraphColumns::default() }] {
                let f = fit(width, (12.0 * 24.0, 40.0), &cfg, ColumnWidths::default(), &FLOORS);
                let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(width, 40.0));
                let c = Columns::new(rect, f.graph_w, &f.cfg, &f.widths);
                let cells = c.cell.all();
                for pair in cells.windows(2) {
                    assert!(pair[1].0 >= pair[0].1 - 0.5, "{width}: {cells:?}");
                }
                assert!(c.cell.graph.1 <= c.cell.msg.0 && c.msg.1 >= c.msg.0, "{width}: lanes under the next column, {cells:?}");
            }
        }
    }

    #[test]
    fn the_graph_takes_at_most_its_share() {
        let f = fit(400.0, (12.0 * 24.0, 40.0), &GraphColumns::default(), ColumnWidths::default(), &FLOORS);
        assert!((f.graph_w - 120.0).abs() < 0.01, "{}", f.graph_w);
        let f = fit(100.0, (12.0 * 24.0, 40.0), &GraphColumns::default(), ColumnWidths::default(), &FLOORS);
        assert_eq!(f.graph_w, 40.0, "never narrower than its title");
    }
}
