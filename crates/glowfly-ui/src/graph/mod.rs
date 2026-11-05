//! The commit graph: a table with configurable, resizable columns (branch / tag, graph, message,
//! changes, author, commit, date) that give way in a narrow graph (`fit`), single-line rows as
//! tall as the row density (`row_h`) banded in their branch's colour (`lane_colours`), author
//! nodes, ref chips (double-click a branch chip to check it out), ghost refs, branch highlighting, this tab's WIP
//! row (`wip_row`: its label, change chips and line total), other worktrees' WIP rows, the New
//! branch… ghost button on the checked-out commit, and the end row below the last commit
//! (`more`).

pub(crate) mod branch_chip;
mod cells;
mod commit_row;
mod find_marks;
mod fit;
mod frame_asks;
mod header;
mod labels;
mod lane_colours;
mod lantern;
mod layout;
mod marks;
mod more;
mod nodes;
mod paint;
mod ref_checkout;
pub(crate) mod ref_group;
mod ref_menu;
mod refs_fit;
mod resize;
mod row;
mod row_look;
mod search_bar;
mod select;
mod wip_row;
mod worktree_wip;

#[cfg(test)]
pub(crate) use lantern::peak as lantern_peak;
pub use paint::draw_avatar;
#[cfg(test)]
pub(crate) use row_look::dim_inks;

use super::UiCache;
use crate::theme::sizes::ControlSize;
use crate::widgets::ghost;
use eframe::egui;
use git2::Oid;
use glowfly_core::repo::Snapshot;
use glowfly_core::settings::Density;
use glowfly_model::msg::GraphMsg;
use glowfly_model::msg::LoadMsg;
use glowfly_model::msg::Msg;
use glowfly_model::state::graph_rows::GraphRows;
use glowfly_model::state::{AppState, ViewState};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// The height of a graph row at `density`.
pub(crate) fn row_h(density: Density) -> f32 {
    match density {
        Density::Comfortable => 36.0,
        Density::Compact => 28.0,
    }
}

const LANE_W: f32 = 24.0;
const MAX_VISIBLE_LANES: usize = 12;
/// The most of the message's room chips leading it take, and the gap after them.
const INLINE_SHARE: f32 = 0.4;
const INLINE_GAP: f32 = 8.0;

/// The commit graph with its column header.
pub fn show(ui: &mut egui::Ui, st: &AppState, view: &mut ViewState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    let Some(snap) = &st.snap else { return };
    // The central view's pane key: the graph takes the keyboard focus.
    let want_focus = crate::pane_focus::take(view, &[glowfly_model::state::layout::Pane::Central]);
    if view.show_search {
        search_bar::show(ui, st, view, msgs);
        ui.add_space(4.0);
    }
    if st.session.tab_view.history_freed.is_some() {
        return super::graph_empty::reloading(ui, st, msgs);
    }
    if snap.commits.is_empty() && snap.graph.wip.is_none() {
        return super::graph_empty::no_commits(ui);
    }
    table(ui, st, snap, view, cache, want_focus, msgs);
}

/// The column header, then the rows (with the end row below the last commit) in their scroll
/// area; `want_focus` gives the rows the keyboard focus.
fn table(
    ui: &mut egui::Ui,
    st: &AppState,
    snap: &Snapshot,
    view: &mut ViewState,
    cache: &mut UiCache,
    want_focus: bool,
    msgs: &mut Vec<Msg>,
) {
    // The search dims or hides rows only while its bar is shown.
    let rows = GraphRows::new(snap, &st.session.search, view.show_search);
    let filtered = rows.filtered();
    let searching = view.show_search && st.session.search.active();
    let total = rows.len();
    // While filtering every node sits in the first lane, without lane lines.
    let lanes = if filtered { 1 } else { snap.graph.max_width.clamp(1, MAX_VISIBLE_LANES) };
    let t = st.settings.lang();
    let graph_w = header::graph_width(ui, t, lanes);
    let (row_h, node_r) = (row_h(st.settings.graph_density), nodes::node_r(st.settings.graph_density));

    // Not under a dialog, the palette or a credential prompt, which keep their keys.
    if st.modal.is_none() && st.palette.is_none() && !st.askpass.showing() {
        let (offset, height) = view.tab.graph_scroll;
        let port = select::Viewport { offset, height, row_h };
        select::keyboard_nav(ui, (&st.session.selection, st.session.wip_lit), snap, &rows, port, view, msgs);
    }

    // What grouping a row's ref labels into chips needs, once per frame.
    let ref_cx = ref_group::RefContext::of(snap, cache.ref_upstreams.of(snap.status_stamp, &snap.locals));
    // The columns as they fit this frame: some narrowed or hidden in a narrow graph (`fit`).
    let s = &st.settings;
    let oldest = snap.commits.iter().map(|c| c.time).min();
    let dates = cells::wide_dates(s, st.cues.now, st.cues.utc_offset, oldest);
    let chips_w = refs_fit::chips_w(ui.painter(), snap, &ref_cx);
    let input = header::HeaderIn { graph_w, cfg: &s.columns, saved: s.column_widths, chips_w, dates: &dates };
    let fitted = header::column_header(ui, t, input, &mut view.column_drag, msgs);
    // A tip about the graph sits pinned under the header, pushing the rows down.
    if let Some(h) =
        glowfly_model::state::hints::view(st, view.sections.prs).filter(|h| h.place == glowfly_model::state::hints::Place::Graph)
    {
        crate::widgets::hint_row::show(ui, t, &h, msgs);
    }
    // How many matches fill the graph, for a filtering search loading further back.
    view.graph_fit_rows = (ui.available_height() / row_h).floor() as usize;
    if filtered && total == 0 {
        return super::graph_empty::no_matches(ui, st, view, msgs);
    }
    // Pull requests by head commit, for graph badges.
    let pr_at: HashMap<Oid, (String, bool)> = st.session.forge.as_ref().map(cells::pr_badges).unwrap_or_default();
    let highlight = highlight(ui, st, msgs);

    ui.spacing_mut().item_spacing.y = 0.0;
    let rows_rect = ui.available_rect_before_wrap();
    let page = ((rows_rect.height() / row_h).floor() as usize).saturating_sub(1);
    select::target(ui, rows_rect, t.tr("graph.focus.name"), page, want_focus);
    // The selection shows as focused while the graph has the keyboard focus or nothing has.
    let active = crate::pane_focus::graph_active(ui.ctx());
    let area = scroll_area(view, row_h);
    let ghost_text = t.tr("graph.ghost_branch.button");
    let frame = row::Frame {
        st,
        snap,
        rows,
        t,
        cols_cfg: fitted.cfg,
        widths: fitted.widths,
        graph_w: fitted.graph_w,
        inline: fitted.refs_inline,
        row_h,
        node_r,
        filtered,
        searching,
        total,
        active,
        wip_lit: st.session.wip_drawn_selected(),
        idle: st.job.is_none(),
        show_avatars: crate::avatars::shown(st),
        highlight,
        ref_cx,
        pr_at,
        commands: glowfly_model::state::custom_commands::MenuCommands::for_state(st),
        tips: st.session.forge.as_ref().map(|f| &f.tips),
        wip_label: t.tr("graph.wip.label"),
        wip_total: st.session.reads.line_counts.wip().and_then(|w| w.total()),
        pointer: ui.ctx().pointer_hover_pos(),
        // One more row below the last commit: more history loading, loading it failed, or the
        // history's end notes (Load more, a shallow clone's Fetch full history).
        end: st.end_row(view),
        // The New branch… ghost button: on the checked-out commit when hovered or anchoring the
        // selection, and never while a menu is open.
        menu_open: ui.ctx().is_context_menu_open(),
        anchor: select::anchor(&st.session.selection),
        ghost_text,
        ghost_w: ghost::ghost_width(ui, ControlSize::Compact, true, Some(ghost_text), 0.0),
    };
    let shown_rows = total + frame.end.is_some() as usize;
    let mut asks = frame_asks::Asks::default();
    let out = area.show_rows(ui, row_h, shown_rows, |ui, range| {
        let last = range.end.min(total);
        for row in range {
            row::draw(ui, &frame, row, &mut cache.avatars, &mut asks, msgs);
        }
        last
    });
    view.tab.graph_scroll = (out.state.offset.y, out.inner_rect.height());
    if more::wants_more(st, out.inner, total, filtered) {
        msgs.push(Msg::Load(LoadMsg::NearHistoryEnd));
    }
    asks.apply(view, msgs);
}

/// The rows belonging to the hovered branch, when the setting is on: update works them out
/// (asked here when the hovered branch changed), then the next frame shows them.
fn highlight<'a>(ui: &egui::Ui, st: &'a AppState, msgs: &mut Vec<Msg>) -> Option<&'a Arc<HashSet<Oid>>> {
    if st.settings.highlight_ref_rows && st.cues.highlight.as_ref().map(|(t, _)| *t) != st.cues.hover_tip {
        msgs.push(Msg::Graph(GraphMsg::HoverRef(st.cues.hover_tip)));
        ui.ctx().request_repaint();
    }
    st.cues.highlight.as_ref().filter(|_| st.settings.highlight_ref_rows).map(|(_, s)| s)
}

/// The rows' scroll area, scrolled to the row or offset the view asked for.
fn scroll_area(view: &mut ViewState, row_h: f32) -> egui::ScrollArea {
    let area = egui::ScrollArea::vertical().auto_shrink([false, false]);
    if let Some(row) = view.scroll_to_row.take() {
        area.vertical_scroll_offset(row as f32 * row_h)
    } else if let Some(off) = view.scroll_offset.take() {
        area.vertical_scroll_offset(off)
    } else {
        area
    }
}
