//! Lane assignment for the commit graph.
//!
//! Commits arrive in topological order (children before parents). Each lane holds the commit it
//! is waiting for; a commit takes the first lane waiting for it, and its parents then occupy lanes
//! below it. Lanes never shift sideways, so pass-through lines stay vertical.

use crate::repo::CommitInfo;
use git2::Oid;

#[derive(Default, Clone)]
pub struct GraphRow {
    /// Lane holding this row's node.
    pub col: usize,
    /// Lines from a lane at the top of the row into the node: (lane, col).
    pub top: Vec<(usize, usize)>,
    /// Lines from the node to a lane at the bottom of the row: (col, lane).
    pub bottom: Vec<(usize, usize)>,
    /// Lanes passing straight through the row.
    pub through: Vec<usize>,
}

#[derive(Default)]
pub struct Layout {
    /// Row for uncommitted changes, drawn above HEAD.
    pub wip: Option<GraphRow>,
    pub rows: Vec<GraphRow>,
    pub max_width: usize,
}

fn free_slot(lanes: &mut Vec<Option<Oid>>) -> usize {
    match lanes.iter().position(Option::is_none) {
        Some(i) => i,
        None => {
            lanes.push(None);
            lanes.len() - 1
        }
    }
}

pub fn layout(commits: &[CommitInfo], head: Option<Oid>, wip: bool) -> Layout {
    let mut lanes: Vec<Option<Oid>> = Vec::new();
    let mut out = Layout::default();

    if wip {
        let mut row = GraphRow::default();
        if let Some(h) = head {
            // Reserve lane 0 for HEAD so the WIP node connects straight down to it.
            lanes.push(Some(h));
            row.bottom.push((0, 0));
        }
        out.wip = Some(row);
        out.max_width = 1;
    }

    for c in commits {
        let matches: Vec<usize> = (0..lanes.len()).filter(|&i| lanes[i] == Some(c.oid)).collect();
        let col = match matches.first() {
            Some(&i) => i,
            None => free_slot(&mut lanes),
        };
        let through: Vec<usize> = (0..lanes.len()).filter(|&i| lanes[i].is_some() && lanes[i] != Some(c.oid)).collect();
        let top = matches.iter().map(|&i| (i, col)).collect();
        for &i in &matches {
            lanes[i] = None;
        }

        let mut bottom = Vec::new();
        if let Some(&first) = c.parents.first() {
            lanes[col] = Some(first);
            bottom.push((col, col));
        }
        for p in c.parents.iter().skip(1) {
            let lane = match lanes.iter().position(|l| *l == Some(*p)) {
                Some(j) => j,
                None => {
                    let k = free_slot(&mut lanes);
                    lanes[k] = Some(*p);
                    k
                }
            };
            bottom.push((col, lane));
        }

        out.max_width = out.max_width.max(lanes.len()).max(col + 1);
        while lanes.last() == Some(&None) {
            lanes.pop();
        }
        out.rows.push(GraphRow { col, top, bottom, through });
    }
    out
}
