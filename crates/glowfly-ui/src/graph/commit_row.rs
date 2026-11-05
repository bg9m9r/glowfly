//! A commit's row in the graph, after its look and lanes (`row.rs`): its node and the lantern's
//! rim, the ref chips with the New branch… ghost button, the text cells, the tooltips, a click
//! or a double-click on a branch chip, and the row's context menu.

use super::frame_asks::Asks;
use super::labels::{self, Tip as ChipTip};
use super::layout::Columns;
use super::nodes::{self, NodeKind};
use super::ref_group::Chip;
use super::row::Frame;
use super::row_look::{self, RowLook};
use super::{INLINE_GAP, INLINE_SHARE, cells, find_marks, lane_colours, lantern, ref_checkout, ref_group, ref_menu, refs_fit, select};
use crate::avatars::Avatars;
use crate::theme::color;
use crate::theme::ref_chip::BranchTone;
use crate::theme::sizes::ControlSize;
use crate::widgets::{Icon, ghost, tooltip};
use eframe::egui::{self, Color32, Painter, Pos2, Rect, Response};
use glowfly_core::repo::{CommitInfo, RefKind};
use glowfly_core::settings::DescriptionMode;
use glowfly_model::keymap::BindingId;
use glowfly_model::msg::{BranchMsg, Msg};
use glowfly_model::state::tip::Tip;

/// One row as `row.rs` laid it out: where it is, its painters (`gp` clipped to the GRAPH
/// column), columns and look, and the HEAD lantern's glow (its centre and node edge) when it
/// has one.
pub(super) struct RowCx {
    pub rect: Rect,
    pub resp: Response,
    pub painter: Painter,
    pub gp: Painter,
    pub cols: Columns,
    pub look: RowLook,
    pub lane_col: Color32,
    pub selected: bool,
    pub hovered: bool,
    /// The row matches the search, and the search dims the rows that don't (`dim`).
    pub matched: bool,
    pub dim: bool,
    pub node_x: f32,
    pub mid: f32,
    pub glow: Option<(Pos2, f32)>,
}

/// Draws commit `c`'s row: node, chips, cells, tooltips, clicks and menu.
pub(super) fn draw(ui: &mut egui::Ui, f: &Frame, cx: RowCx, c: &CommitInfo, avatars: &mut Avatars, asks: &mut Asks, msgs: &mut Vec<Msg>) {
    let (t, snap, settings) = (f.t, f.snap, &f.st.settings);
    let kind = if c.parents.len() > 1 { NodeKind::Merge } else { NodeKind::Commit };
    let at = cx.glow.map_or(egui::pos2(cx.node_x, cx.mid), |(at, _)| at);
    if f.cols_cfg.tree {
        if kind == NodeKind::Merge {
            nodes::merge_dot(&cx.gp, at, cx.lane_col);
        } else {
            let av = if f.show_avatars { Some(&mut *avatars) } else { None };
            nodes::commit(&cx.gp, (at, f.node_r), c.author.name(), c.author.email(), cx.lane_col, av);
        }
    }
    if let Some((at, edge)) = cx.glow {
        lantern::rim(&cx.gp, at, edge);
    }

    // BRANCH / TAG column, joined to the node; in a narrow table, the chips lead the message.
    let chips = snap.refs.get(&c.oid).map(|l| ref_group::chips(l, &f.ref_cx)).unwrap_or_default();
    let tones = lane_colours::chip_tones(snap, &chips, cx.lane_col);
    let refs = refs_cell(ui, f, &cx, c, (&chips, &tones), asks, msgs);

    let ink = if cx.selected { color::strong() } else { color::text() }; // As the sidebar's selected rows.
    let show_body = match f.cols_cfg.description {
        DescriptionMode::Always => true,
        DescriptionMode::OnHover => cx.hovered || cx.selected,
        DescriptionMode::Never => false,
    };
    let found = cx.matched.then(|| find_marks::RowRuns::of(&f.st.session.search.query, c, show_body.then(|| c.body_preview().0)));
    let stat = f.st.session.reads.stats.get(&c.oid).copied().filter(|_| f.cols_cfg.changes);
    if f.cols_cfg.changes && stat.is_none() {
        asks.want_stats.push(c.oid);
    }
    let (now, offset) = (f.st.cues.now, f.st.cues.utc_offset);
    let cells = cells::Cells {
        cols: &cx.cols,
        rect: cx.rect,
        cfg: &f.cols_cfg,
        msg_x: refs.labels_hit.filter(|_| f.inline).map_or(cx.cols.msg.0, |r| r.right() + INLINE_GAP),
        commit: c,
        pr: f.pr_at.get(&c.oid),
        show_body,
        inks: cells::Inks::new(ink, cx.dim),
        found: found.as_ref().map(|r| (r, find_marks::Mark { fill: color::find_fill(row_look::top_fill(cx.look)), ink })),
        date: f.cols_cfg.date.then(|| settings.format_graph_time(c.time, now, offset)),
        stats: stat,
    };
    let text = cells::draw(&cx.painter, &cells);

    let (pointer, hovered) = (f.pointer, cx.hovered);
    let in_date = f.cols_cfg.date && pointer.is_some_and(|p| cx.cols.rect(cx.cols.cell.date, cx.rect).contains(p));
    let resp = match refs.hovered_tip {
        Some(ChipTip::All) if hovered => cx.resp.on_hover_ui_at_pointer(|ui| labels::tip_list(ui, t, &chips, &tones)),
        Some(ChipTip::One(i)) if hovered => cx.resp.on_hover_ui_at_pointer(|ui| labels::tip_row(ui, t, &chips[i], tones[i])),
        _ if hovered && text.msg.hovered(pointer) => cx.resp.on_hover_ui_at_pointer(|ui| cells::message_tip(ui, c, show_body)),
        _ if hovered && text.sha.hovered(pointer) => cx.resp.on_hover_text_at_pointer(&c.oid.to_string()[..7]),
        _ if hovered && text.author.hovered(pointer) => cx.resp.on_hover_text_at_pointer(c.author.name()),
        _ if hovered && in_date => cx.resp.on_hover_text_at_pointer(settings.date_tooltip(c.time, now, offset)),
        _ => cx.resp,
    };
    click(ui, f, &resp, c, (&refs, &chips), msgs);
    let menu = ref_menu::RowMenu {
        commit: c,
        labels_hit: refs.labels_hit,
        snap,
        selection: &f.st.session.selection,
        settings,
        forge: f.st.session.forge.as_ref(),
        idle: f.idle,
        commands: &f.commands,
        keys: &f.st.keymap,
    };
    ref_menu::context_menu(ui, &resp, menu, msgs);
}

/// The row's ref chips (in BRANCH / TAG, or leading the message in a narrow table), with the
/// New branch… ghost button on the checked-out commit; notes the tips whose builds were looked
/// up and the commit whose chip the pointer is over.
fn refs_cell(
    ui: &mut egui::Ui,
    f: &Frame,
    cx: &RowCx,
    c: &CommitInfo,
    (chips, tones): (&[Chip], &[BranchTone]),
    asks: &mut Asks,
    msgs: &mut Vec<Msg>,
) -> labels::RefsCell {
    let (t, snap, cols, rect, inline) = (f.t, f.snap, &cx.cols, cx.rect, f.inline);
    let (span, clip) = match inline {
        false => (cols.refs, cols.cell.refs),
        true => {
            let room = refs_fit::row_w(&cx.painter, chips).min((cols.msg.1 - cols.msg.0) * INLINE_SHARE);
            let span = (cols.msg.0, cols.msg.0 + room);
            (span, labels::inline_clip(span))
        }
    };
    if !f.cols_cfg.refs && !inline {
        return labels::RefsCell::default();
    }
    let labels = snap.refs.get(&c.oid).map_or(&[][..], |l| l.as_slice());
    let mut build = None;
    if f.tips.is_some() && labels.iter().any(|l| matches!(l.kind, RefKind::Local | RefKind::Remote)) {
        asks.tips_seen.push(c.oid);
        build = f.tips.and_then(|t| t.checks(c.oid)).map(|c| c.state);
    }
    let ghost =
        !inline && f.st.settings.ghost_refs && !f.menu_open && snap.head_oid == Some(c.oid) && (cx.hovered || f.anchor == Some(c.oid));
    let kind = if c.parents.len() > 1 { NodeKind::Merge } else { NodeKind::Commit };
    let node_edge_x = (f.cols_cfg.tree && !inline).then(|| cx.node_x - nodes::radius(kind, f.node_r));
    let (lane, pointer, ghost_w) = (cx.lane_col, f.pointer, f.ghost_w);
    let row = labels::RefsRow { chips, tones, lane, build, node_edge_x, pointer, ghost, ghost_w };
    let refs = labels::refs_cell(&cx.painter, span, clip, rect, row);
    if let Some((r, labelled)) = refs.ghost_at {
        let id = ui.id().with(("ghost-branch", c.oid));
        let clip = cols.rect(cols.cell.refs, rect);
        let label = labelled.then_some(f.ghost_text);
        let g = ghost::ghost_at(ui, id, r, clip, ControlSize::Compact, Icon::Plus, label, f.ghost_text);
        let tip = || Tip::new(t.tr("graph.ghost_branch.tip")).keys(BindingId::CreateBranch);
        if tooltip::attach(g, &f.st.keymap, t, tip).clicked() {
            msgs.push(Msg::Branch(BranchMsg::NewBranch { start: None, name: String::new() }));
        }
    }
    if refs.on_chip {
        asks.hover_tip_next = Some(c.oid);
    }
    refs
}

/// A plain double-click on a branch chip checks it out; otherwise a click selects the row.
fn click(ui: &egui::Ui, f: &Frame, resp: &Response, c: &CommitInfo, (refs, chips): (&labels::RefsCell, &[Chip]), msgs: &mut Vec<Msg>) {
    let mods = ui.input(|i| i.modifiers);
    let checkout = resp
        .interact_pointer_pos()
        .filter(|_| resp.double_clicked() && mods.is_none())
        .and_then(|p| refs.chip_at(p))
        .and_then(|i| chips.get(i))
        .and_then(|chip| ref_checkout::double_click(chip, f.snap));
    if let Some(m) = checkout {
        msgs.push(m);
    } else if resp.clicked() {
        select::focus(ui.ctx());
        msgs.push(select::row_click(c.oid, mods));
    }
}
