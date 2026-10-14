//! The commit graph: configurable columns (branch/tag, graph, SHA, message, changes, author,
//! date), avatars, ghost refs, and branch highlighting.

mod paint;
mod select;

pub use paint::draw_avatar;

use super::UiCache;
use super::common::lane_color;
use super::menus::{commit_menu, multi_commit_menu};
use super::theme::color;
use super::widgets;
use crate::msg::Msg;
use crate::state::{AppState, Selection};
use crate::update::commit_matches;
use eframe::egui::{self, Align2, CornerRadius, FontId, Key, Rect, RichText, Sense, Stroke, pos2, vec2};
use git2::Oid;
use paint::{Columns, branch_rows, default_ancestry, draw_lanes, draw_ref_labels, lane_x, paint_pencil, wip_counts};
use std::collections::HashMap;
use trellis_core::settings::DescriptionMode;

const ROW_H: f32 = 30.0;
const LANE_W: f32 = 22.0;
const MAX_VISIBLE_LANES: usize = 12;
const REF_COL_W: f32 = 190.0;

/// The commit graph with its column header.
pub fn show(ui: &mut egui::Ui, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    {
        let Some(snap) = &st.snap else { return };
        let has_wip = snap.graph.wip.is_some();
        let total = snap.commits.len() + has_wip as usize;
        let lanes = snap.graph.max_width.clamp(1, MAX_VISIBLE_LANES);
        let graph_w = lanes as f32 * LANE_W + 10.0;
        let query = st.view.search.trim().to_lowercase();
        let cols_cfg = st.settings.columns;

        if st.view.show_search {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                let r = ui.add(
                    egui::TextEdit::singleline(&mut st.view.search)
                        .hint_text("Search commit messages, authors and SHAs, then press Enter")
                        .desired_width(420.0),
                );
                // A credential prompt or the command palette above keeps the keyboard.
                let covered = st.askpass.showing() || st.palette.is_some();
                if !r.has_focus() && st.view.search.is_empty() && !covered {
                    r.request_focus();
                }
                // Search commits from the command palette focuses the field even with text in it.
                if !covered && std::mem::take(&mut st.view.focus_search) {
                    r.request_focus();
                }
                if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    msgs.push(Msg::SearchNext);
                    r.request_focus();
                }
                if r.changed() {
                    st.view.search_cursor = snap.commits.len().saturating_sub(1);
                }
                if ui.small_button("Close").clicked() {
                    st.view.show_search = false;
                    st.view.search.clear();
                }
            });
            ui.add_space(4.0);
        }

        if snap.commits.is_empty() && !has_wip {
            ui.centered_and_justified(|ui| ui.label(RichText::new("This repository has no commits yet.").color(color::weak())));
            return;
        }

        select::keyboard_nav(ui, &st.selection, snap, &mut st.view, total, has_wip, msgs);

        // Column header.
        let (hdr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::hover());
        let cols = Columns::new(hdr, graph_w, &cols_cfg);
        let p = ui.painter();
        let font = FontId::proportional(10.5);
        let weak = color::weak();
        p.line_segment([hdr.left_bottom(), hdr.right_bottom()], Stroke::new(1.0_f32, color::border()));
        let header = |col: (f32, f32), text: &str, on: bool| {
            if on {
                p.text(pos2(col.0 + 6.0, hdr.center().y), Align2::LEFT_CENTER, text, font.clone(), weak);
                if col.0 > hdr.left() + 1.0 {
                    p.line_segment([pos2(col.0, hdr.top() + 4.0), pos2(col.0, hdr.bottom() - 4.0)], Stroke::new(1.0_f32, color::border()));
                }
            }
        };
        header(cols.refs, "BRANCH / TAG", cols_cfg.refs);
        header(cols.graph, "GRAPH", cols_cfg.tree);
        header(cols.sha, "SHA", cols_cfg.sha);
        header((cols.msg.0 - 6.0, cols.msg.1), "COMMIT MESSAGE", cols_cfg.message);
        header(cols.changes, "CHANGES", cols_cfg.changes);
        header(cols.author, "AUTHOR", cols_cfg.author);
        header(cols.date, "DATE", cols_cfg.date);
        if snap.truncated {
            p.text(
                pos2(cols.msg.1, hdr.center().y),
                Align2::RIGHT_CENTER,
                format!("newest {} commits", snap.commits.len()),
                FontId::proportional(10.5),
                color::faint(),
            );
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

        // Rows belonging to the hovered branch.
        if st.settings.highlight_ref_rows {
            match st.view.hover_tip {
                Some(tip) if st.view.highlight.as_ref().map(|(t, _)| *t) != Some(tip) => {
                    if st.view.base_set.is_none() {
                        st.view.base_set = Some(default_ancestry(snap).1);
                    }
                    let rows = branch_rows(snap, tip, st.view.base_set.as_ref().unwrap());
                    st.view.highlight = Some((tip, rows));
                }
                None => st.view.highlight = None,
                _ => {}
            }
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
        let pointer = ui.ctx().pointer_hover_pos();
        let out = area.show_rows(ui, ROW_H, total, |ui, range| {
            for row in range {
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
                let painter = ui.painter_at(rect);
                let cols = Columns::new(rect, graph_w, &cols_cfg);
                let is_wip = has_wip && row == 0;
                let commit = if is_wip { None } else { Some(&snap.commits[row - has_wip as usize]) };
                let g = if is_wip { snap.graph.wip.as_ref().unwrap() } else { &snap.graph.rows[row - has_wip as usize] };
                let selected = match (selection, commit) {
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
                let node_x = lane_x(cols.graph.0, g.col);
                let mid = rect.center().y;
                if cols_cfg.tree {
                    draw_lanes(&gp, cols.graph.0, rect, g);
                }

                let Some(c) = commit else {
                    if cols_cfg.tree {
                        widgets::dashed_circle(&gp, pos2(node_x, mid), 10.0, Stroke::new(1.6_f32, color::weak()));
                    }
                    let mp = painter.with_clip_rect(cols.rect(cols.msg, rect));
                    let box_r = Rect::from_min_size(pos2(cols.msg.0, mid - 11.0), vec2(96.0, 22.0));
                    mp.rect_filled(box_r, CornerRadius::same(3), color::input());
                    mp.text(pos2(box_r.left() + 8.0, mid), Align2::LEFT_CENTER, "// WIP", FontId::proportional(13.0), color::weak());
                    let (m, a, d) = wip_counts(snap);
                    let mut x = box_r.right() + 12.0;
                    for (n, sym, col) in [(m, "", color::amber()), (a, "+", color::green()), (d, "−", color::red())] {
                        if n > 0 {
                            if sym.is_empty() {
                                paint_pencil(&mp, pos2(x + 5.0, mid), col);
                                x += 14.0;
                            }
                            let g = mp.layout_no_wrap(format!("{sym} {n}").trim().to_owned(), FontId::proportional(13.0), col);
                            let w = g.size().x;
                            mp.galley(pos2(x, mid - g.size().y / 2.0), g, col);
                            x += w + 12.0;
                        }
                    }
                    if resp.clicked() {
                        msgs.push(Msg::SelectWip);
                    }
                    continue;
                };

                let dim = !query.is_empty() && !commit_matches(c, &query);
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
                        let (label_rect, end) = draw_ref_labels(&rp, cols.refs.0 + 6.0, cols.refs.1 - 6.0, mid, labels, lane_col);
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
                    painter.text(
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
                        Msg::NewBranch { start: None, name: String::new() }
                    } else {
                        select::row_click(c.oid, mods)
                    });
                }
                resp.context_menu(|ui| match selection {
                    Selection::Commits(m) if m.contains(c.oid) => multi_commit_menu(ui, m, msgs),
                    _ => commit_menu(ui, c.oid, &c.summary, msgs),
                });
            }
        });
        st.view.graph_scroll = (out.state.offset.y, out.inner_rect.height());
        if hover_tip_next.is_some() {
            st.view.hover_tip_next = hover_tip_next;
        }
        if !want_stats.is_empty() {
            msgs.push(Msg::WantStats(want_stats));
        }
    }
}
