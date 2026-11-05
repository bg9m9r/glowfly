//! One row of the commit graph, drawn from what the frame worked out once (`Frame`): which
//! commit, WIP or other worktree it shows, its look, lanes and the HEAD lantern's glow; this
//! tab's WIP row and other worktrees' WIP rows are drawn here, a commit's node, chips and cells
//! in `commit_row.rs`.

use super::commit_row::{self, RowCx};
use super::frame_asks::Asks;
use super::lane_colours::RowColours;
use super::layout::Columns;
use super::paint::{draw_lanes, lane_x};
use super::{cells, lantern, more, nodes, ref_group, row_look, select, wip_row, worktree_wip};
use crate::avatars::Avatars;
use crate::widgets::row_menu::RowMenuExt;
use eframe::egui::{self, Pos2, Rect, Sense, pos2, vec2};
use git2::Oid;
use glowfly_core::graph::{GraphRow, WorktreeWipRow};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{CommitInfo, LineCount, Snapshot, WorktreeInfo};
use glowfly_core::settings::{ColumnWidths, GraphColumns};
use glowfly_model::msg::Msg;
use glowfly_model::state::AppState;
use glowfly_model::state::build_status::TipChecks;
use glowfly_model::state::custom_commands::MenuCommands;
use glowfly_model::state::graph_rows::{DisplayRow, GraphRows};
use glowfly_model::state::load_more::EndRow;
use glowfly_model::state::worktrees::{open_blocked, wip_label, wip_tooltip};
use glowfly_model::worktree::WorktreeMsg;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// What every row of one frame shares, worked out before the rows are drawn.
pub(super) struct Frame<'a> {
    pub st: &'a AppState,
    pub snap: &'a Snapshot,
    pub rows: GraphRows<'a>,
    pub t: Lang,
    /// The columns as they fit this frame, their widths and the GRAPH column's content width.
    pub cols_cfg: GraphColumns,
    pub widths: ColumnWidths,
    pub graph_w: f32,
    /// BRANCH / TAG has no column and its chips lead the message.
    pub inline: bool,
    pub row_h: f32,
    pub node_r: f32,
    pub filtered: bool,
    pub searching: bool,
    /// The rows' count, without the end row.
    pub total: usize,
    /// The graph has the keyboard focus, or nothing has.
    pub active: bool,
    /// This tab's WIP row shows as selected.
    pub wip_lit: bool,
    /// No job is running.
    pub idle: bool,
    pub show_avatars: bool,
    /// Rows belonging to the hovered branch.
    pub highlight: Option<&'a Arc<HashSet<Oid>>>,
    pub ref_cx: ref_group::RefContext<'a>,
    /// Pull requests by head commit, for graph badges.
    pub pr_at: HashMap<Oid, (String, bool)>,
    pub commands: MenuCommands<'a>,
    pub tips: Option<&'a TipChecks>,
    pub wip_label: &'static str,
    pub wip_total: Option<LineCount>,
    pub pointer: Option<Pos2>,
    /// The row below the last commit, if any.
    pub end: Option<EndRow<'a>>,
    pub menu_open: bool,
    /// The commit anchoring the selection.
    pub anchor: Option<Oid>,
    pub ghost_text: &'static str,
    pub ghost_w: f32,
}

impl Frame<'_> {
    /// The columns of a row at `rect`.
    pub(super) fn columns(&self, rect: Rect) -> Columns {
        Columns::new(rect, self.graph_w, &self.cols_cfg, &self.widths)
    }
}

/// What a display row shows: its commit index, the commit (None for a WIP row), its graph row,
/// another worktree's WIP (row and worktree), and its colours.
struct Shown<'a> {
    index: usize,
    commit: Option<&'a CommitInfo>,
    g: &'a GraphRow,
    worktree: Option<(&'a WorktreeWipRow, &'a WorktreeInfo)>,
    colours: RowColours<'a>,
}

/// A row's place in the history or the worktrees; None for a layout that doesn't match them.
fn resolve<'a>(f: &Frame<'a>, row: usize) -> Option<Shown<'a>> {
    let snap = f.snap;
    let shown = |index, commit, g, worktree, colours| Some(Shown { index, commit, g, worktree, colours });
    match f.rows.at(row)? {
        DisplayRow::Commit(i) => match (snap.commits.get(i), snap.graph.rows.get(i)) {
            (Some(c), Some(g)) => shown(i, Some(c), g, None, RowColours::commit(snap, i)),
            _ => None,
        },
        DisplayRow::Wip => shown(0, None, snap.graph.wip.as_ref()?, None, RowColours::wip(snap)),
        DisplayRow::Worktree(k) => {
            let w = snap.graph.worktree_wips.get(k)?;
            let wt = snap.worktrees.get(w.worktree)?;
            shown(0, None, &w.row, Some((w, wt)), RowColours::worktree(snap, k))
        }
    }
}

/// Draws display row `row`, or the end row below the last commit.
pub(super) fn draw(ui: &mut egui::Ui, f: &Frame, row: usize, avatars: &mut Avatars, asks: &mut Asks, msgs: &mut Vec<Msg>) {
    let (t, snap, st, row_h) = (f.t, f.snap, f.st, f.row_h);
    if row == f.total
        && let Some(r) = &f.end
    {
        let line = Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), row_h));
        more::row(ui, t, row_h, f.columns(line).msg.0, r, snap, msgs);
        return;
    }
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), row_h), Sense::click());
    let painter = ui.painter_at(rect);
    let cols = f.columns(rect);
    // A layout that doesn't match the history or the worktrees draws nothing.
    let Some(Shown { index, commit, g, worktree, colours }) = resolve(f, row) else { return };
    let colours = colours.with_origin(rect.top() - row as f32 * row_h);
    // The row's accessible name: its summary, or the text of its WIP box.
    let (now, offset) = (st.cues.now, st.cues.utc_offset);
    match (commit, worktree) {
        (Some(c), _) => cells::name_row(&resp, c, snap.refs.get(&c.oid), st.settings.format_graph_time(c.time, now, offset)),
        (None, Some((_, wt))) => crate::access::name_button(&resp, true, &wip_label(t, wt)),
        (None, None) => crate::access::name_button(&resp, true, f.wip_label),
    }
    // Another worktree's row is never selected.
    let selected = match (&st.session.selection, commit) {
        _ if worktree.is_some() => false,
        (_, None) => f.wip_lit,
        (s, Some(c)) => s.includes(c.oid),
    };
    // The pointer over the row's ghost button still hovers the row.
    let hovered = resp.hovered() || ui.rect_contains_pointer(rect);
    let lane_col = colours.node();
    let highlighted = commit.is_some_and(|c| f.highlight.is_some_and(|h| h.contains(&c.oid)));
    let (matched, dim) = row_look::found(f.searching && commit.is_some(), f.filtered, selected, || st.session.search.position(index));
    let wip = commit.is_none();
    let look = row_look::RowLook { lane: lane_col, wip, highlighted, selected, active: f.active, hovered, marked: matched && !f.filtered };
    row_look::paint(&painter, rect, look);

    let mut gp = painter.with_clip_rect(cols.rect(cols.cell.graph, rect));
    gp.set_opacity(if dim { row_look::DIM_ALPHA } else { 1.0 });
    let node_x = lane_x(cols.graph.0, if f.filtered { 0 } else { g.col });
    let mid = rect.center().y;
    let mut cx = RowCx { rect, resp, painter, gp, cols, look, lane_col, selected, hovered, matched, dim, node_x, mid, glow: None };
    if let Some((w, wt)) = worktree {
        return draw_worktree(f, cx, (w, wt, &colours), msgs);
    }
    // The HEAD lantern's glow under the lanes, from HEAD's node snapped to pixels; its rim goes on after the node.
    let shown = f.cols_cfg.tree && !dim;
    cx.glow = lantern::glow(&cx.gp, pos2(node_x, mid), (commit, snap.head_oid), (f.node_r, row_h), (look, shown));
    if f.cols_cfg.tree && !f.filtered {
        draw_lanes(&cx.gp, cx.cols.graph.0, rect, g, &colours);
    }
    match commit {
        Some(c) => commit_row::draw(ui, f, cx, c, avatars, asks, msgs),
        None => draw_wip(ui, f, &cx, msgs),
    }
}

/// Another worktree's row: a click opens (or switches to) that worktree's tab; its menu is the
/// WORKTREES row's.
fn draw_worktree(f: &Frame, cx: RowCx, (w, wt, colours): (&WorktreeWipRow, &WorktreeInfo, &RowColours), msgs: &mut Vec<Msg>) {
    let label = wip_label(f.t, wt);
    let gp = f.cols_cfg.tree.then_some(&cx.gp);
    worktree_wip::draw(&cx.painter, gp, &cx.cols, cx.rect, (w, wt, colours), &label, f.node_r);
    let resp = cx.resp.on_hover_text(wip_tooltip(wt));
    if resp.clicked() && open_blocked(wt).is_none() {
        msgs.push(Msg::Worktree(WorktreeMsg::Open(wt.path.clone())));
    }
    resp.row_menu(|ui| crate::worktrees::row_menu(ui, wt, msgs));
}

/// This tab's WIP row: its lantern node, then its label, change chips and line total.
fn draw_wip(ui: &egui::Ui, f: &Frame, cx: &RowCx, msgs: &mut Vec<Msg>) {
    let (cols, rect) = (&cx.cols, cx.rect);
    if f.cols_cfg.tree {
        nodes::wip_lantern(&cx.gp, pos2(cx.node_x, cx.mid), f.node_r, row_look::top_fill(cx.look), f.snap.is_dirty());
    }
    let mp = cx.painter.with_clip_rect(cols.rect(cols.wip_span(), rect));
    wip_row::draw(&mp, cols, cx.mid, (f.wip_label, cx.selected), f.snap.changes, f.wip_total);
    if cx.resp.clicked() {
        select::focus(ui.ctx());
        msgs.push(select::wip_click(ui.input(|i| i.modifiers)));
    }
}
