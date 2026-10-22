//! The commit graph: a table with configurable, resizable columns (branch / tag, graph, message,
//! changes, commit, date), rows banded in their branch's colour (`lane_colours`), author nodes, ref chips
//! (double-click a branch chip to check it out), ghost refs, branch highlighting, this tab's WIP
//! row (`wip_row`: its label, change chips and line total), other worktrees' WIP rows, the New
//! branch… ghost button on the checked-out commit, and the end row below the last commit
//! (`more`).

mod cells;
mod header;
mod labels;
mod lane_colours;
mod layout;
mod marks;
mod more;
mod nodes;
mod paint;
mod ref_checkout;
mod ref_group;
mod ref_menu;
mod resize;
mod row_look;
mod search_bar;
mod select;
mod wip_row;
mod worktree_wip;

pub use paint::draw_avatar;
#[cfg(test)]
pub(in crate::ui) use row_look::band_color;

use super::UiCache;
use super::theme::color;
use crate::keymap::BindingId;
use crate::msg::BranchMsg;
use crate::msg::GraphMsg;
use crate::msg::LoadMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::state::AppState;
use crate::state::build_status::MAX_SEEN;
use crate::state::graph_rows::{DisplayRow, GraphRows};
use crate::state::tip::Tip;
use crate::state::worktrees::{open_blocked, wip_tooltip};
use crate::ui::widgets::row_menu::RowMenuExt;
use crate::ui::widgets::{Icon, ghost, tooltip};
use crate::worktree::WorktreeMsg;
use eframe::egui::{self, Rect, Sense, pos2, vec2};
use git2::Oid;
use glowfly_core::repo::RefKind;
use glowfly_core::settings::DescriptionMode;
use labels::Tip as ChipTip;
use lane_colours::RowColours;
use layout::Columns;
use nodes::NodeKind;
use paint::{draw_lanes, lane_x};
use std::collections::HashMap;

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
        let t = st.settings.lang();
        let graph_w = header::graph_width(ui, t, lanes);
        let cols_cfg = st.settings.columns;

        select::keyboard_nav(ui, &st.selection, snap, &rows, &mut st.view, msgs);

        let widths = header::column_header(ui, t, graph_w, &cols_cfg, st.settings.column_widths, &mut st.view.column_drag, msgs);
        // How many matches fill the graph, for a filtering search loading further back.
        st.view.graph_fit_rows = (ui.available_height() / ROW_H).floor() as usize;
        if filtered && total == 0 {
            return super::graph_empty::no_matches(ui, st, msgs);
        }

        // What grouping a row's ref labels into chips needs, once per frame.
        let ref_cx = ref_group::RefContext::of(snap);
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
        let commands = crate::state::custom_commands::MenuCommands::for_state(st);
        let now = st.view.now;
        let offset = st.view.utc_offset;
        let avatars = &mut cache.avatars;
        let stats = &st.stats;
        let wip_label = t.tr("graph.wip.label");
        let wip_total = st.line_counts.wip().and_then(|w| w.total());
        let mut want_stats: Vec<Oid> = Vec::new();
        let mut hover_tip_next = None;
        let tips = st.forge.as_ref().map(|f| &f.tips);
        let mut tips_seen: Vec<Oid> = Vec::new();
        let pointer = ui.ctx().pointer_hover_pos();
        // One more row below the last commit: more history loading, loading it failed, or the
        // history's end notes (Load more, a shallow clone's Fetch full history).
        let end = st.end_row();
        let shown_rows = total + end.is_some() as usize;
        // The New branch… ghost button: on the checked-out commit when hovered or anchoring the
        // selection, and never while a menu is open.
        let menu_open = ui.ctx().is_context_menu_open();
        let anchor = select::anchor(selection);
        let keymap = &st.keymap;
        let ghost_text = t.tr("graph.ghost_branch.button");
        let ghost_w = ghost::small_width(ui, true, ghost_text);
        let out = area.show_rows(ui, ROW_H, shown_rows, |ui, range| {
            let last = range.end.min(total);
            for row in range {
                if row == total
                    && let Some(r) = &end
                {
                    let line = Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), ROW_H));
                    more::row(ui, t, ROW_H, Columns::new(line, graph_w, &cols_cfg, &widths).msg.0, r, snap, msgs);
                    continue;
                }
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
                let painter = ui.painter_at(rect);
                let cols = Columns::new(rect, graph_w, &cols_cfg, &widths);
                // A layout that doesn't match the history or the worktrees draws nothing.
                let (index, commit, g, worktree, colours) = match rows.at(row) {
                    Some(DisplayRow::Commit(i)) => match (snap.commits.get(i), snap.graph.rows.get(i)) {
                        (Some(c), Some(g)) => (i, Some(c), g, None, RowColours::commit(snap, i)),
                        _ => continue,
                    },
                    Some(DisplayRow::Wip) => match snap.graph.wip.as_ref() {
                        Some(g) => (0, None, g, None, RowColours::wip(snap)),
                        None => continue,
                    },
                    Some(DisplayRow::Worktree(k)) => match snap.graph.worktree_wips.get(k) {
                        Some(w) => match snap.worktrees.get(w.worktree) {
                            Some(wt) => (0, None, &w.row, Some((w, wt)), RowColours::worktree(snap, k)),
                            None => continue,
                        },
                        None => continue,
                    },
                    None => continue,
                };
                // The row's accessible name: its summary, or the text of its WIP box.
                match (commit, worktree) {
                    (Some(c), _) => super::access::name_button(&resp, true, &c.summary),
                    (None, Some((_, wt))) => super::access::name_button(&resp, true, &crate::state::worktrees::wip_label(t, wt)),
                    (None, None) => super::access::name_button(&resp, true, wip_label),
                }
                // Another worktree's row is never selected.
                let selected = match (selection, commit) {
                    _ if worktree.is_some() => false,
                    (s, None) if s.wip_shown() => true,
                    (s, Some(c)) => s.includes(c.oid),
                    _ => false,
                };
                // The pointer over the row's ghost button still hovers the row.
                let hovered = resp.hovered() || ui.rect_contains_pointer(rect);
                let lane_col = colours.node();
                let highlighted = commit.is_some_and(|c| highlight.is_some_and(|h| h.contains(&c.oid)));
                row_look::paint(&painter, rect, row_look::RowLook { lane: lane_col, highlighted, selected, hovered });

                let gp = painter.with_clip_rect(cols.rect(cols.cell.graph, rect));
                let node_x = lane_x(cols.graph.0, if filtered { 0 } else { g.col });
                let mid = rect.center().y;
                // Another worktree's row: a click opens (or switches to) that worktree's tab; its
                // menu is the WORKTREES row's.
                if let Some((w, wt)) = worktree {
                    let label = crate::state::worktrees::wip_label(t, wt);
                    worktree_wip::draw(&painter, cols_cfg.tree.then_some(&gp), &cols, rect, (w, wt, &colours), &label);
                    let resp = resp.on_hover_text(wip_tooltip(wt));
                    if resp.clicked() && open_blocked(wt).is_none() {
                        msgs.push(Msg::Worktree(WorktreeMsg::Open(wt.path.clone())));
                    }
                    resp.row_menu(|ui| super::worktrees::row_menu(ui, wt, msgs));
                    continue;
                }
                if cols_cfg.tree && !filtered {
                    draw_lanes(&gp, cols.graph.0, rect, g, &colours);
                }

                let Some(c) = commit else {
                    if cols_cfg.tree {
                        nodes::wip_ring(&gp, pos2(node_x, mid), lane_col);
                    }
                    let mp = painter.with_clip_rect(cols.rect(cols.wip_span(), rect));
                    wip_row::draw(&mp, &cols, mid, wip_label, snap.changes, wip_total);
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
                let chips = snap.refs.get(&c.oid).map(|l| ref_group::chips(l, &ref_cx)).unwrap_or_default();
                let tones = lane_colours::chip_tones(snap, &chips, lane_col);
                let mut refs = labels::RefsCell::default();
                if cols_cfg.refs {
                    let labels = snap.refs.get(&c.oid).map_or(&[][..], |l| l.as_slice());
                    let mut build = None;
                    if tips.is_some() && labels.iter().any(|l| matches!(l.kind, RefKind::Local | RefKind::Remote)) {
                        tips_seen.push(c.oid);
                        build = tips.and_then(|t| t.checks(c.oid)).map(|c| c.state);
                    }
                    let ghost = settings.ghost_refs && !menu_open && snap.head_oid == Some(c.oid) && (hovered || anchor == Some(c.oid));
                    let node_edge_x = cols_cfg.tree.then(|| node_x - nodes::radius(kind));
                    let row = labels::RefsRow { chips: &chips, tones: &tones, lane: lane_col, build, node_edge_x, pointer, ghost, ghost_w };
                    refs = labels::refs_cell(&painter, &cols, rect, row);
                    if let Some((r, labelled)) = refs.ghost_at {
                        let id = ui.id().with(("ghost-branch", c.oid));
                        let clip = cols.rect(cols.cell.refs, rect);
                        let label = labelled.then_some(ghost_text);
                        let g = ghost::small_ghost_at(ui, id, r, clip, Icon::Plus, label, ghost_text);
                        let tip = || Tip::new(t.tr("graph.ghost_branch.tip")).keys(BindingId::CreateBranch);
                        if tooltip::attach(g, keymap, t, tip).clicked() {
                            msgs.push(Msg::Branch(BranchMsg::NewBranch { start: None, name: String::new() }));
                        }
                    }
                    if refs.on_chip {
                        hover_tip_next = Some(c.oid);
                    }
                }

                // A selected row's message is in `strong`, as the sidebar's selected rows are.
                let ink = if selected { color::strong() } else { color::text() };
                let fg = if dim { ink.gamma_multiply(0.35) } else { ink };
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

                let in_date = cols_cfg.date && pointer.is_some_and(|p| cols.rect(cols.cell.date, rect).contains(p));
                let resp = match refs.hovered_tip {
                    Some(ChipTip::All) if hovered => resp.on_hover_ui_at_pointer(|ui| labels::tip_list(ui, t, &chips, &tones)),
                    Some(ChipTip::One(i)) if hovered => resp.on_hover_ui_at_pointer(|ui| labels::tip_row(ui, t, &chips[i], tones[i])),
                    _ if hovered && text.msg.hovered(pointer) => resp.on_hover_ui_at_pointer(|ui| cells::message_tip(ui, c, show_body)),
                    _ if hovered && text.sha.hovered(pointer) => resp.on_hover_text_at_pointer(&c.oid.to_string()[..7]),
                    _ if hovered && text.author.hovered(pointer) => resp.on_hover_text_at_pointer(&c.author),
                    _ if hovered && in_date => resp.on_hover_text_at_pointer(settings.date_tooltip(c.time, now, offset)),
                    _ => resp,
                };
                // A plain double-click on a branch chip checks it out instead of clicking the row.
                let mods = ui.input(|i| i.modifiers);
                let checkout = resp
                    .interact_pointer_pos()
                    .filter(|_| resp.double_clicked() && mods.is_none())
                    .and_then(|p| refs.chip_at(p))
                    .and_then(|i| chips.get(i))
                    .and_then(|chip| ref_checkout::double_click(chip, snap));
                if let Some(m) = checkout {
                    msgs.push(m);
                } else if resp.clicked() {
                    msgs.push(select::row_click(c.oid, mods));
                }
                let menu = ref_menu::RowMenu {
                    commit: c,
                    labels_hit: refs.labels_hit,
                    snap,
                    selection,
                    settings,
                    forge,
                    idle,
                    commands: &commands,
                };
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
