//! The commit graph: a table with configurable, resizable columns (branch / tag, graph, message,
//! changes, commit, date), rows banded in their lane's colour, author nodes, ghost refs, branch
//! highlighting, and other worktrees' WIP rows.

mod cells;
mod header;
mod labels;
mod marks;
mod more;
mod nodes;
mod paint;
mod ref_group;
mod ref_menu;
mod resize;
mod row_look;
mod search_bar;
mod select;
mod worktree_wip;

pub use paint::draw_avatar;

use super::UiCache;
use super::common::lane_color;
use super::theme::color;
use crate::msg::BranchMsg;
use crate::msg::GraphMsg;
use crate::msg::LoadMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::state::AppState;
use crate::state::build_status::MAX_SEEN;
use crate::state::graph_rows::{DisplayRow, GraphRows};
use crate::state::worktrees::{open_blocked, wip_tooltip};
use crate::worktree::WorktreeMsg;
use eframe::egui::{self, Rect, Sense, pos2, vec2};
use git2::Oid;
use labels::Tip;
use nodes::NodeKind;
use paint::{Columns, draw_lanes, lane_x};
use std::collections::HashMap;
use trellis_core::repo::RefKind;
use trellis_core::settings::DescriptionMode;

/// The height of a graph row.
pub(crate) const ROW_H: f32 = 40.0;
const LANE_W: f32 = 24.0;
const MAX_VISIBLE_LANES: usize = 12;

/// The commit graph with its column header.
pub fn show(ui: &mut egui::Ui, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    if st.snap.is_none() {
        return;
    }
    if st.view.show_search {
        search_bar::show(ui, st, msgs);
        ui.add_space(4.0);
    }
    {
        let Some(snap) = &st.snap else { return };
        if snap.commits.is_empty() && snap.graph.wip.is_none() {
            return super::graph_empty::no_commits(ui);
        }
        // The search dims or hides rows only while its bar is shown.
        let rows = GraphRows::new(snap, &st.search, st.view.show_search);
        let filtered = rows.filtered();
        let searching = st.view.show_search && st.search.active();
        let total = rows.len();
        // While filtering every node sits in the first lane, without lane lines.
        let lanes = if filtered { 1 } else { snap.graph.max_width.clamp(1, MAX_VISIBLE_LANES) };
        let graph_w = header::graph_width(ui, lanes);
        let cols_cfg = st.settings.columns;

        select::keyboard_nav(ui, &st.selection, snap, &rows, &mut st.view, msgs);

        let widths = header::column_header(ui, snap, graph_w, &cols_cfg, st.settings.column_widths, &mut st.view.column_drag, msgs);
        if filtered && total == 0 {
            return super::graph_empty::no_matches(ui, st, msgs);
        }

        // Pull requests by head commit, for graph badges.
        let pr_at: HashMap<Oid, (String, bool)> = st
            .forge
            .as_ref()
            .map(|f| {
                let word = if f.pr_noun() == "merge request" { "MR" } else { "PR" };
                f.prs
                    .iter()
                    .filter_map(|p| {
                        Some((*f.pr_heads.get(&p.id)?, (format!("{} {}", if p.draft { "Draft" } else { word }, f.pr_label(p.id)), p.draft)))
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Rows belonging to the hovered branch: update works them out, then the next frame shows them.
        if st.settings.highlight_ref_rows && st.view.highlight.as_ref().map(|(t, _)| *t) != st.view.hover_tip {
            msgs.push(Msg::Graph(GraphMsg::HoverRef(st.view.hover_tip)));
            ui.ctx().request_repaint();
        }
        let highlight = st.view.highlight.as_ref().filter(|_| st.settings.highlight_ref_rows).map(|(_, s)| s);

        ui.spacing_mut().item_spacing.y = 0.0;
        let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]);
        if let Some(row) = st.view.scroll_to_row.take() {
            area = area.vertical_scroll_offset(row as f32 * ROW_H);
        } else if let Some(off) = st.view.scroll_offset.take() {
            area = area.vertical_scroll_offset(off);
        }
        let selection = &st.selection;
        let settings = &st.settings;
        let idle = st.job.is_none();
        let forge = st.forge.as_ref();
        let now = st.view.now;
        let offset = st.view.utc_offset;
        let avatars = &mut cache.avatars;
        let stats = &st.stats;
        let mut want_stats: Vec<Oid> = Vec::new();
        let mut hover_tip_next = None;
        let tips = st.forge.as_ref().map(|f| &f.tips);
        let mut tips_seen: Vec<Oid> = Vec::new();
        let pointer = ui.ctx().pointer_hover_pos();
        // While more history loads, one more row below the last commit says so.
        let more_row = st.shows_loading_more_row();
        let shown_rows = total + more_row as usize;
        let out = area.show_rows(ui, ROW_H, shown_rows, |ui, range| {
            let last = range.end.min(total);
            for row in range {
                if more_row && row == total {
                    let line = Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), ROW_H));
                    more::loading_row(ui, ROW_H, Columns::new(line, graph_w, &cols_cfg, &widths).msg.0);
                    continue;
                }
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
                let painter = ui.painter_at(rect);
                let cols = Columns::new(rect, graph_w, &cols_cfg, &widths);
                // A layout that doesn't match the history or the worktrees draws nothing.
                let (index, commit, g, worktree) = match rows.at(row) {
                    Some(DisplayRow::Commit(i)) => match (snap.commits.get(i), snap.graph.rows.get(i)) {
                        (Some(c), Some(g)) => (i, Some(c), g, None),
                        _ => continue,
                    },
                    Some(DisplayRow::Wip) => match snap.graph.wip.as_ref() {
                        Some(g) => (0, None, g, None),
                        None => continue,
                    },
                    Some(DisplayRow::Worktree(k)) => match snap.graph.worktree_wips.get(k) {
                        Some(w) => match snap.worktrees.get(w.worktree) {
                            Some(wt) => (0, None, &w.row, Some((w, wt))),
                            None => continue,
                        },
                        None => continue,
                    },
                    None => continue,
                };
                // The row's accessible name: its summary, or the text of its WIP box.
                match (commit, worktree) {
                    (Some(c), _) => super::access::name_button(&resp, true, &c.summary),
                    (None, Some((_, wt))) => super::access::name_button(&resp, true, &crate::state::worktrees::wip_label(wt)),
                    (None, None) => super::access::name_button(&resp, true, "// WIP"),
                }
                // Another worktree's row is never selected.
                let selected = match (selection, commit) {
                    _ if worktree.is_some() => false,
                    (s, None) if s.wip_shown() => true,
                    (s, Some(c)) => s.includes(c.oid),
                    _ => false,
                };
                let hovered = resp.hovered();
                let lane_col = lane_color(g.col);
                let highlighted = commit.is_some_and(|c| highlight.is_some_and(|h| h.contains(&c.oid)));
                row_look::paint(&painter, rect, row_look::RowLook { lane: lane_col, highlighted, selected, hovered });

                let gp = painter.with_clip_rect(cols.rect(cols.graph, rect));
                let node_x = lane_x(cols.graph.0, if filtered { 0 } else { g.col });
                let mid = rect.center().y;
                // Another worktree's row: a click opens (or switches to) that worktree's tab; its
                // menu is the WORKTREES row's.
                if let Some((w, wt)) = worktree {
                    worktree_wip::draw(&painter, &gp, &cols, rect, w, wt, cols_cfg.tree);
                    let resp = resp.on_hover_text(wip_tooltip(wt));
                    if resp.clicked() && open_blocked(wt).is_none() {
                        msgs.push(Msg::Worktree(WorktreeMsg::Open(wt.path.clone())));
                    }
                    resp.context_menu(|ui| super::worktrees::row_menu(ui, wt, msgs));
                    continue;
                }
                if cols_cfg.tree && !filtered {
                    draw_lanes(&gp, cols.graph.0, rect, g);
                }

                let Some(c) = commit else {
                    if cols_cfg.tree {
                        nodes::wip_ring(&gp, pos2(node_x, mid), lane_col);
                    }
                    let mp = painter.with_clip_rect(cols.rect(cols.msg, rect));
                    let right = worktree_wip::wip_box(&mp, cols.msg.0, mid, "// WIP");
                    worktree_wip::draw_counts(&mp, right + 12.0, mid, snap.changes);
                    if resp.clicked() {
                        msgs.push(select::wip_click(ui.input(|i| i.modifiers)));
                    }
                    continue;
                };

                let dim = searching && !filtered && st.search.position(index).is_none();
                let kind = if c.parents.len() > 1 { NodeKind::Merge } else { NodeKind::Commit };
                if cols_cfg.tree {
                    if kind == NodeKind::Merge {
                        nodes::merge_dot(&gp, pos2(node_x, mid), lane_col);
                    } else {
                        let av = if settings.use_initials { None } else { Some(&mut *avatars) };
                        nodes::commit(&gp, pos2(node_x, mid), &c.author, &c.email, lane_col, av);
                    }
                }

                // BRANCH / TAG column, joined to the node.
                let chips = snap.refs.get(&c.oid).map(|l| ref_group::chips(l, &snap.locals)).unwrap_or_default();
                let mut refs = labels::RefsCell::default();
                if cols_cfg.refs {
                    let labels = snap.refs.get(&c.oid).map_or(&[][..], |l| l.as_slice());
                    let mut build = None;
                    if tips.is_some() && labels.iter().any(|l| matches!(l.kind, RefKind::Local | RefKind::Remote)) {
                        tips_seen.push(c.oid);
                        build = tips.and_then(|t| t.checks(c.oid)).map(|c| c.state);
                    }
                    let ghost = settings.ghost_refs && snap.head_oid == Some(c.oid) && (hovered || selected);
                    let node_edge_x = cols_cfg.tree.then(|| node_x - nodes::radius(kind));
                    let row = labels::RefsRow { chips: &chips, lane: lane_col, build, node_edge_x, pointer, ghost };
                    refs = labels::refs_cell(&painter, &cols, rect, row);
                    if refs.on_chip {
                        hover_tip_next = Some(c.oid);
                    }
                }

                let fg = if dim { color::text().gamma_multiply(0.35) } else { color::text() };
                let weak = if dim { color::weak().gamma_multiply(0.35) } else { color::weak() };
                let show_body = match cols_cfg.description {
                    DescriptionMode::Always => true,
                    DescriptionMode::OnHover => hovered || selected,
                    DescriptionMode::Never => false,
                };
                let stat = stats.get(&c.oid).copied().filter(|_| cols_cfg.changes);
                if cols_cfg.changes && stat.is_none() {
                    want_stats.push(c.oid);
                }
                let cells = cells::Cells {
                    cols: &cols,
                    rect,
                    cfg: &cols_cfg,
                    commit: c,
                    pr: pr_at.get(&c.oid),
                    show_body,
                    colors: (fg, weak),
                    date: cols_cfg.date.then(|| settings.format_time(c.time, now, offset)),
                    stats: stat,
                };
                let text = cells::draw(&painter, &cells);

                let in_date = cols_cfg.date && pointer.is_some_and(|p| cols.rect(cols.date, rect).contains(p));
                let resp = match refs.hovered_tip {
                    Some(Tip::All) if hovered => resp.on_hover_ui_at_pointer(|ui| labels::tip_list(ui, &chips, lane_col)),
                    Some(Tip::One(i)) if hovered => resp.on_hover_ui_at_pointer(|ui| labels::tip_row(ui, &chips[i], lane_col)),
                    _ if hovered && text.msg.hovered(pointer) => resp.on_hover_ui_at_pointer(|ui| cells::message_tip(ui, c, show_body)),
                    _ if hovered && text.sha.hovered(pointer) => resp.on_hover_text_at_pointer(&c.oid.to_string()[..7]),
                    _ if hovered && text.author.hovered(pointer) => resp.on_hover_text_at_pointer(&c.author),
                    _ if hovered && in_date => resp.on_hover_text_at_pointer(settings.date_tooltip(c.time, now, offset)),
                    _ => resp,
                };
                if resp.clicked() {
                    let mods = ui.input(|i| i.modifiers);
                    let on_ghost = refs.ghost.zip(pointer).is_some_and(|(r, p)| r.contains(p));
                    msgs.push(if on_ghost && mods.is_none() {
                        Msg::Branch(BranchMsg::NewBranch { start: None, name: String::new() })
                    } else {
                        select::row_click(c.oid, mods)
                    });
                }
                let menu = ref_menu::RowMenu { commit: c, labels_hit: refs.labels_hit, snap, selection, settings, forge, idle };
                ref_menu::context_menu(ui, &resp, menu, msgs);
            }
            last
        });
        st.view.graph_scroll = (out.state.offset.y, out.inner_rect.height());
        if more::wants_more(st, out.inner, total, filtered) {
            msgs.push(Msg::Load(LoadMsg::NearHistoryEnd));
        }
        if hover_tip_next.is_some() {
            st.view.hover_tip_next = hover_tip_next;
        }
        let seen = &mut st.view.tips_seen;
        for o in tips_seen {
            if seen.len() < MAX_SEEN && !seen.contains(&o) {
                seen.push(o);
            }
        }
        if !want_stats.is_empty() {
            msgs.push(Msg::Repo(RepoMsg::WantStats(want_stats)));
        }
    }
}
