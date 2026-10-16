//! The commit graph: configurable, resizable columns (branch/tag, graph, SHA, message, changes,
//! author, date), avatars, ghost refs, branch highlighting, and other worktrees' WIP rows.

mod header;
mod more;
mod paint;
mod resize;
mod search_bar;
mod select;
mod worktree_wip;

pub use paint::draw_avatar;

use super::UiCache;
use super::common::lane_color;
use super::menus::{commit_menu, multi_commit_menu};
use super::theme::color;
use super::widgets;
use crate::msg::BranchMsg;
use crate::msg::GraphMsg;
use crate::msg::LoadMsg;
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::state::build_status::MAX_SEEN;
use crate::state::graph_rows::{DisplayRow, GraphRows};
use crate::state::worktrees::{open_blocked, wip_tooltip};
use crate::state::{AppState, Selection};
use crate::worktree::WorktreeMsg;
use eframe::egui::{self, Align2, CornerRadius, FontId, Rect, RichText, Sense, Stroke, pos2, vec2};
use git2::Oid;
use paint::{Columns, draw_lanes, draw_ref_labels, lane_x};
use std::collections::HashMap;
use trellis_core::repo::{ChangeCounts, RefKind};
use trellis_core::settings::DescriptionMode;

const ROW_H: f32 = 30.0;
const LANE_W: f32 = 22.0;
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
            ui.centered_and_justified(|ui| ui.label(RichText::new("This repository has no commits yet.").color(color::weak())));
            return;
        }
        // The search dims or hides rows only while its bar is shown.
        let rows = GraphRows::new(snap, &st.search, st.view.show_search);
        let filtered = rows.filtered();
        let searching = st.view.show_search && st.search.active();
        let total = rows.len();
        // While filtering every node sits in the first lane, without lane lines.
        let lanes = if filtered { 1 } else { snap.graph.max_width.clamp(1, MAX_VISIBLE_LANES) };
        let graph_w = lanes as f32 * LANE_W + 10.0;
        let cols_cfg = st.settings.columns;

        select::keyboard_nav(ui, &st.selection, snap, &rows, &mut st.view, msgs);

        let widths = header::column_header(ui, snap, graph_w, &cols_cfg, st.settings.column_widths, &mut st.view.column_drag, msgs);
        if filtered && total == 0 {
            let text = if st.search.searching_files() { "Searching changed files…" } else { "No commits match this search." };
            ui.centered_and_justified(|ui| ui.label(RichText::new(text).color(color::weak())));
            return;
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
                // Another worktree's row is never selected.
                let selected = match (selection, commit) {
                    _ if worktree.is_some() => false,
                    (Selection::Wip, None) => true,
                    (s, Some(c)) => s.includes(c.oid),
                    _ => false,
                };
                let hovered = resp.hovered();
                let lane_col = lane_color(g.col);
                let highlighted = commit.is_some_and(|c| highlight.is_some_and(|h| h.contains(&c.oid)));
                if selected {
                    painter.rect_filled(rect, 0.0, color::selected());
                    painter.rect_filled(Rect::from_min_size(pos2(cols.msg.0 - 6.0, rect.top()), vec2(2.0, ROW_H)), 0.0, lane_col);
                } else if highlighted {
                    painter.rect_filled(rect, 0.0, lane_col.gamma_multiply(0.16));
                } else if hovered {
                    painter.rect_filled(rect, 0.0, color::hover());
                }

                let gp = painter.with_clip_rect(cols.rect(cols.graph, rect));
                let node_x = lane_x(cols.graph.0, if filtered { 0 } else { g.col });
                let mid = rect.center().y;
                // Another worktree's row: a click opens (or switches to) that worktree's tab.
                if let Some((w, wt)) = worktree {
                    worktree_wip::draw(&painter, &gp, &cols, rect, w, wt, cols_cfg.tree);
                    let resp = resp.on_hover_text(wip_tooltip(wt));
                    if resp.clicked() && open_blocked(wt).is_none() {
                        msgs.push(Msg::Worktree(WorktreeMsg::Open(wt.path.clone())));
                    }
                    continue;
                }
                if cols_cfg.tree && !filtered {
                    draw_lanes(&gp, cols.graph.0, rect, g);
                }

                let Some(c) = commit else {
                    if cols_cfg.tree {
                        widgets::dashed_circle(&gp, pos2(node_x, mid), 10.0, Stroke::new(1.6_f32, color::weak()));
                    }
                    let mp = painter.with_clip_rect(cols.rect(cols.msg, rect));
                    let right = worktree_wip::wip_box(&mp, cols.msg.0, mid, "// WIP");
                    worktree_wip::draw_counts(&mp, right + 12.0, mid, ChangeCounts::of(&snap.staged, &snap.unstaged));
                    if resp.clicked() {
                        msgs.push(Msg::Repo(RepoMsg::SelectWip));
                    }
                    continue;
                };

                let dim = searching && !filtered && st.search.position(index).is_none();
                if cols_cfg.tree {
                    if c.parents.len() > 1 {
                        gp.circle_filled(pos2(node_x, mid), 5.0, lane_col);
                    } else {
                        let av = if settings.use_initials { None } else { Some(&mut *avatars) };
                        draw_avatar(&gp, pos2(node_x, mid), 10.0, &c.author, &c.email, lane_col, av);
                    }
                }

                // BRANCH / TAG column, connected to the node.
                let mut ghost_rect = None;
                if cols_cfg.refs {
                    let rp = painter.with_clip_rect(cols.rect(cols.refs, rect));
                    let mut labels_end = cols.refs.0;
                    if let Some(labels) = snap.refs.get(&c.oid) {
                        let mut mark = None;
                        if tips.is_some() && labels.iter().any(|l| matches!(l.kind, RefKind::Local | RefKind::Remote)) {
                            tips_seen.push(c.oid);
                            mark = tips.and_then(|t| t.checks(c.oid)).map(|c| c.state);
                        }
                        let (label_rect, end) = draw_ref_labels(&rp, cols.refs.0 + 6.0, cols.refs.1 - 6.0, mid, labels, lane_col, mark);
                        if cols_cfg.tree {
                            gp.line_segment([pos2(cols.graph.0, mid), pos2(node_x - 10.0, mid)], Stroke::new(1.5_f32, lane_col));
                            rp.line_segment([pos2(end, mid), pos2(cols.refs.1, mid)], Stroke::new(1.5_f32, lane_col));
                        }
                        if pointer.is_some_and(|p| label_rect.contains(p)) {
                            hover_tip_next = Some(c.oid);
                        }
                        labels_end = end;
                    }
                    // A faded "+ branch" after the checked-out commit's labels: click it to branch
                    // from the current branch (other commits: right-click > Create branch here).
                    let r = Rect::from_min_size(pos2(labels_end + 6.0, mid - 11.0), vec2(80.0, 22.0));
                    let is_head = snap.head_oid == Some(c.oid);
                    if settings.ghost_refs && is_head && (hovered || selected) && r.right() <= cols.refs.1 - 6.0 {
                        rp.rect_filled(r, CornerRadius::same(3), color::bg());
                        rp.rect_stroke(
                            r,
                            CornerRadius::same(3),
                            Stroke::new(1.0_f32, lane_col.gamma_multiply(0.6)),
                            egui::StrokeKind::Inside,
                        );
                        rp.text(
                            pos2(r.left() + 8.0, mid),
                            Align2::LEFT_CENTER,
                            "+ branch",
                            FontId::proportional(12.0),
                            lane_col.gamma_multiply(0.8),
                        );
                        ghost_rect = Some(r);
                    }
                }

                if cols_cfg.sha {
                    painter.with_clip_rect(cols.rect(cols.sha, rect)).text(
                        pos2(cols.sha.0, mid),
                        Align2::LEFT_CENTER,
                        &c.oid.to_string()[..7],
                        FontId::monospace(12.0),
                        color::weak(),
                    );
                }

                let fg = if dim { color::text().gamma_multiply(0.35) } else { color::text() };
                let weak = if dim { color::weak().gamma_multiply(0.35) } else { color::weak() };
                if cols_cfg.message {
                    let mp = painter.with_clip_rect(cols.rect(cols.msg, rect));
                    let mut x = cols.msg.0;
                    if let Some((label, draft)) = pr_at.get(&c.oid) {
                        let col = if *draft { color::weak() } else { color::purple() };
                        x += widgets::chip(&mp, x, mid, label, col.gamma_multiply(0.35), col) + 6.0;
                    }
                    let g = mp.layout_no_wrap(c.summary.clone(), FontId::proportional(13.0), fg);
                    let w = g.size().x;
                    mp.galley(pos2(x, mid - g.size().y / 2.0), g, fg);
                    let show_body = match cols_cfg.description {
                        DescriptionMode::Always => true,
                        DescriptionMode::OnHover => hovered || selected,
                        DescriptionMode::Never => false,
                    };
                    if show_body && !c.body.is_empty() {
                        mp.text(pos2(x + w + 10.0, mid), Align2::LEFT_CENTER, &c.body, FontId::proportional(12.5), weak);
                    }
                }
                if cols_cfg.changes {
                    match stats.get(&c.oid) {
                        Some(&(_files, adds, dels)) => {
                            let cp = painter.with_clip_rect(cols.rect(cols.changes, rect));
                            let g = cp.layout_no_wrap(format!("+{adds}"), FontId::proportional(12.0), color::green());
                            let w = g.size().x;
                            cp.galley(pos2(cols.changes.0, mid - g.size().y / 2.0), g, color::green());
                            cp.text(
                                pos2(cols.changes.0 + w + 6.0, mid),
                                Align2::LEFT_CENTER,
                                format!("−{dels}"),
                                FontId::proportional(12.0),
                                color::red(),
                            );
                        }
                        None => want_stats.push(c.oid),
                    }
                }
                if cols_cfg.author {
                    painter.with_clip_rect(cols.rect(cols.author, rect)).text(
                        pos2(cols.author.0, mid),
                        Align2::LEFT_CENTER,
                        &c.author,
                        FontId::proportional(12.5),
                        weak,
                    );
                }
                if cols_cfg.date {
                    painter.with_clip_rect(cols.rect(cols.date, rect)).text(
                        pos2(cols.date.0, mid),
                        Align2::LEFT_CENTER,
                        settings.format_time(c.time, now, offset),
                        FontId::proportional(12.5),
                        weak,
                    );
                }

                let in_refs = pointer.is_some_and(|p| cols.rect(cols.refs, rect).contains(p));
                let resp = match snap.refs.get(&c.oid) {
                    Some(labels) if hovered && in_refs && labels.len() > 1 => resp.on_hover_ui_at_pointer(|ui| {
                        for l in labels {
                            ui.label(&l.name);
                        }
                    }),
                    _ => resp,
                };
                if resp.clicked() {
                    let mods = ui.input(|i| i.modifiers);
                    let on_ghost = ghost_rect.zip(pointer).is_some_and(|(r, p)| r.contains(p));
                    msgs.push(if on_ghost && mods.is_none() {
                        Msg::Branch(BranchMsg::NewBranch { start: None, name: String::new() })
                    } else {
                        select::row_click(c.oid, mods)
                    });
                }
                resp.context_menu(|ui| match selection {
                    Selection::Commits(m) if m.contains(c.oid) => multi_commit_menu(ui, m, msgs),
                    _ => commit_menu(ui, c, msgs),
                });
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
