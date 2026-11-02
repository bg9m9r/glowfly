//! How the history table's columns give way when the graph is narrow, so MESSAGE keeps
//! `MSG_MIN` (its cell, divider to divider) and no column runs over another. GRAPH is capped at
//! about a third of the table (its lanes beyond that clipped to its cell). Then, while MESSAGE
//! has less than `MSG_MIN`, in turn: CHANGES hides, then COMMIT, AUTHOR and DATE; BRANCH / TAG
//! narrows to its floor (100 px); and last its chips move inline before the message
//! (`Fitted::refs_inline`), its column giving MESSAGE its room. BRANCH / TAG never hides
//! otherwise. None of this is saved: the stored widths come back as the window widens.

use super::layout::CELL_PAD;
use super::layout::EDGE_INSET;
use glowfly_core::settings::{ColumnWidths, GraphColumns, ResizableColumn};

/// The message cell's width the other columns give way for.
pub(super) const MSG_MIN: f32 = 240.0;
/// The share of the table the GRAPH column's lanes may take (at least its title's width).
const GRAPH_SHARE: f32 = 0.3;
/// The narrowest BRANCH / TAG gets before its chips move inline.
const REFS_FLOOR: f32 = 100.0;

/// One step of giving way.
#[derive(Clone, Copy)]
enum Step {
    Hide(ResizableColumn),
    /// Narrow the column toward this width (never below its floor).
    Narrow(ResizableColumn, f32),
    /// BRANCH / TAG's chips move inline before the message.
    Inline,
}

/// The steps in turn.
const STEPS: [Step; 6] = [
    Step::Hide(ResizableColumn::Changes),
    Step::Hide(ResizableColumn::Commit),
    Step::Hide(ResizableColumn::Author),
    Step::Hide(ResizableColumn::Date),
    Step::Narrow(ResizableColumn::Refs, REFS_FLOOR),
    Step::Inline,
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
    /// BRANCH / TAG has no column (`cfg.refs` is off) and its chips lead the message.
    pub refs_inline: bool,
}

pub(super) fn shown(cfg: &GraphColumns, col: ResizableColumn) -> bool {
    match col {
        ResizableColumn::Refs => cfg.refs,
        ResizableColumn::Changes => cfg.changes,
        ResizableColumn::Author => cfg.author,
        ResizableColumn::Commit => cfg.commit,
        ResizableColumn::Date => cfg.date,
    }
}

fn hide(cfg: &mut GraphColumns, col: ResizableColumn) {
    match col {
        ResizableColumn::Refs => cfg.refs = false,
        ResizableColumn::Changes => cfg.changes = false,
        ResizableColumn::Author => cfg.author = false,
        ResizableColumn::Commit => cfg.commit = false,
        ResizableColumn::Date => cfg.date = false,
    }
}

/// The message cell's width in a table `width` wide.
fn message_w(width: f32, graph_w: f32, cfg: &GraphColumns, w: &ColumnWidths) -> f32 {
    let col = |c: ResizableColumn| if shown(cfg, c) { w.get(c) } else { 0.0 };
    let lead = if cfg.refs { CELL_PAD } else { EDGE_INSET };
    let graph = if cfg.tree { lead + graph_w + CELL_PAD } else { 0.0 };
    let right: f32 =
        [ResizableColumn::Changes, ResizableColumn::Author, ResizableColumn::Commit, ResizableColumn::Date].map(col).iter().sum();
    width - col(ResizableColumn::Refs) - graph - right
}

/// The columns for a table `width` wide: `graph` is the GRAPH column's content width for its
/// lanes and the least it takes (its title), `w` the widths to draw at (each at least its
/// `floors` entry).
pub fn fit(width: f32, graph: (f32, f32), cfg: &GraphColumns, w: ColumnWidths, floors: &[(ResizableColumn, f32)]) -> Fitted {
    let (mut cfg, mut w) = (*cfg, w);
    let graph_w = graph.0.min((width * GRAPH_SHARE).max(graph.1));
    let floor = |c: ResizableColumn| floors.iter().find(|f| f.0 == c).map_or(0.0, |f| f.1);
    let mut hidden = Vec::new();
    let mut refs_inline = false;
    for step in STEPS {
        let short = MSG_MIN - message_w(width, graph_w, &cfg, &w);
        if short <= 0.0 {
            break;
        }
        match step {
            Step::Hide(col) if shown(&cfg, col) => {
                hide(&mut cfg, col);
                hidden.push(col);
            }
            Step::Narrow(col, to) if shown(&cfg, col) => {
                let (now, least) = (w.get(col), to.max(floor(col)));
                if now > least {
                    w = w.with(col, (now - short).max(least).ceil());
                }
            }
            Step::Inline if cfg.refs => {
                cfg.refs = false;
                refs_inline = true;
            }
            _ => {}
        }
    }
    hidden.sort_by_key(|c| ResizableColumn::ALL.iter().position(|a| a == c));
    Fitted { cfg, widths: w, graph_w, hidden, refs_inline }
}

#[cfg(test)]
mod tests {
    use super::super::layout::Columns;
    use super::*;
    use ResizableColumn::*;
    use eframe::egui::{Rect, pos2, vec2};

    const FLOORS: [(ResizableColumn, f32); 5] = [(Refs, 100.0), (Changes, 90.0), (Author, 74.0), (Commit, 74.0), (Date, 60.0)];
    /// Three lanes, and the GRAPH title's width.
    const GRAPH: (f32, f32) = (72.0, 40.0);

    fn fit_cfg(width: f32, cfg: GraphColumns) -> Fitted {
        fit(width, GRAPH, &cfg, ColumnWidths::default(), &FLOORS)
    }

    fn at(width: f32) -> Fitted {
        fit_cfg(width, GraphColumns::default())
    }

    /// The columns `f` lays out in a table `width` wide.
    fn columns(width: f32, f: &Fitted) -> Columns {
        Columns::new(Rect::from_min_size(pos2(0.0, 0.0), vec2(width, 36.0)), f.graph_w, &f.cfg, &f.widths)
    }

    fn msg_cell(width: f32, f: &Fitted) -> f32 {
        let c = columns(width, f);
        c.cell.msg.1 - c.cell.msg.0
    }

    /// What gives way first, then next, as the table narrows from 1400 px.
    fn drop_order(cfg: GraphColumns) -> Vec<String> {
        let mut seen: Vec<String> = Vec::new();
        for width in (200..=1400).rev().map(|w| w as f32) {
            let f = fit_cfg(width, cfg);
            let mut now: Vec<String> = f.hidden.iter().map(|c| format!("{c:?}")).collect();
            if f.widths.get(Refs) < ColumnWidths::default().get(Refs) {
                now.push("narrow Refs".into());
            }
            if f.refs_inline {
                now.push("inline Refs".into());
            }
            for step in now {
                if !seen.contains(&step) {
                    seen.push(step);
                }
            }
        }
        seen
    }

    #[test]
    fn a_wide_table_keeps_every_column() {
        let f = at(760.0);
        assert_eq!((f.cfg, f.widths, f.graph_w, f.refs_inline), (GraphColumns::default(), ColumnWidths::default(), 72.0, false));
        assert!(f.hidden.is_empty());
        assert!(msg_cell(760.0, &f) >= MSG_MIN, "{}", msg_cell(760.0, &f));
    }

    #[test]
    fn commit_then_author_then_date_hide_then_branch_tag_narrows_and_moves_inline() {
        assert_eq!(drop_order(GraphColumns::default()), ["Commit", "Author", "Date", "narrow Refs", "inline Refs"]);
        // A shown CHANGES column goes before them all.
        let with_changes = GraphColumns { changes: true, ..GraphColumns::default() };
        assert_eq!(drop_order(with_changes), ["Changes", "Commit", "Author", "Date", "narrow Refs", "inline Refs"]);
        // BRANCH / TAG never hides: inline, its chips lead the message.
        let f = at(300.0);
        assert!(f.refs_inline && !f.cfg.refs && !f.hidden.contains(&Refs), "{f:?}");
        assert_eq!(f.hidden, vec![Author, Commit, Date]);
    }

    #[test]
    fn the_message_keeps_its_room_until_nothing_can_give() {
        for width in (200..=1400).step_by(2).map(|w| w as f32) {
            let f = at(width);
            let msg = msg_cell(width, &f);
            assert!(msg >= MSG_MIN - 0.5 || f.refs_inline, "{width}: message {msg} with {f:?}");
        }
    }

    #[test]
    fn a_column_hides_only_when_the_message_needs_its_room() {
        let order = [Commit, Author, Date];
        for width in (200..=1400).step_by(2).map(|w| w as f32) {
            let f = at(width);
            let Some(&last) = order.iter().rev().find(|c| f.hidden.contains(c)) else { continue };
            if f.refs_inline || f.widths.get(Refs) < 130.0 {
                continue;
            }
            let mut back = f.clone();
            match last {
                Commit => back.cfg.commit = true,
                Author => back.cfg.author = true,
                _ => back.cfg.date = true,
            }
            assert!(msg_cell(width, &back) < MSG_MIN, "{width}: {last:?} hid with room to spare");
        }
    }

    #[test]
    fn no_column_runs_over_another() {
        for width in (200..=1400).step_by(10).map(|w| w as f32) {
            for cfg in [GraphColumns::default(), GraphColumns { changes: true, ..GraphColumns::default() }] {
                let f = fit(width, (12.0 * 24.0, 40.0), &cfg, ColumnWidths::default(), &FLOORS);
                let c = columns(width, &f);
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
