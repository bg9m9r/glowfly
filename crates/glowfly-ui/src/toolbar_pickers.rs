//! The toolbar's Repository › Branch pickers: one framed segment bar (`widgets::segment_bar`)
//! of two segments parted by a full-height divider, each a caption over a name with its caret,
//! lighting its whole segment while hovered, held or open (the repository picker alone, with
//! no repository open, lights its whole frame). Each part's name slot is fitted to names
//! (`toolbar_fit.rs`): the repository's to its own, the branch's to the widest local branch
//! name and the current label, so checking out a branch never resizes the bar or moves the
//! groups after it; a re-fit (a tab switch, or a new widest branch) is a single jump. Only a
//! window too narrow for everything shrinks the fitted slots (`name_widths`). A longer
//! repository or branch name keeps its start and end around a "…" (`truncate::middle`); groups
//! after them fold away before the names go below `comfort`.
//! A cut name shows whole on hover, and the accessible name is always the whole name. The
//! branch name follows a dot in the checked-out branch's colour (a ring while HEAD is
//! detached), and each branch in its menu has its colour dot, the checked-out one checked.
//! After the branch name, its counts against its upstream ("2↑ 3↓", the sidebar's painted
//! arrows, `list_row::paint_parts`) take room inside the slot, cutting the name rather than
//! widening the bar, and hide when they would leave the name less than its floor; they show
//! only when out of sync, and the accessible name always says them, hidden or not.

use super::fit;
use super::menus::repo_menu;
use crate::branch_dot;
use crate::common::{menu_item, short};
use crate::list_row::{Part, paint_parts, parts_width};
use crate::menu_anchor;
use crate::theme::branch_colours::line;
use crate::theme::color;
use crate::theme::sizes::{self, ControlSize};
use crate::theme::tokens::{BORDER_W, radius};
use crate::theme::type_scale::{self, BODY, CAPTION};
use crate::widgets;
use crate::widgets::menu::{self, separator};
use crate::widgets::segment_bar::{BarStyle, Seg, segment_bar};
use crate::widgets::truncate::{cut_galley, cut_job, middle};
use eframe::egui::{self, Color32, Galley, Id, Response, WidgetInfo, WidgetType, pos2};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Snapshot;
use glowfly_model::msg::{BranchMsg, Msg};
use glowfly_model::state::AppState;
use glowfly_model::state::worktree_branches::Elsewhere;
use std::sync::Arc;

/// The narrowest the repository and branch names are cut to in a small window.
const REPO_MIN: f32 = 32.0;
const BRANCH_MIN: f32 = 40.0;
/// The most of each name's room the pickers keep before toolbar groups start to fold
/// (`toolbar_fold.rs`): a repository of 64 px and a branch of 96, so a 1400 px window with a
/// hosting service still shows every group in English.
const COMFORT_NAMES: (f32, f32) = (64.0, 96.0);
/// A part's side padding (a ghost's: the parts are borderless targets inside one frame), and the
/// room between its caption and its name (the two centred as one block in the segment).
const PAD_X: f32 = PICKER_SIZE.metrics().ghost_pad_x;
const CAPTION_GAP: f32 = 2.0;
/// The size whose focus style the segments take (their corners are the frame's).
const PICKER_SIZE: ControlSize = ControlSize::Large;
/// The caret's room after a name, and the branch icon's before it.
const CARET_ROOM: f32 = 14.0;
const ICON_ROOM: f32 = 20.0;
/// Between the branch name and its ahead/behind counts.
const COUNTS_GAP: f32 = 6.0;
/// One part's room besides its name.
const PART_FRAME: f32 = 2.0 * PAD_X + CARET_ROOM;
/// The bar's room besides the names: both parts' frames, the branch icon and the border.
const FRAMES: f32 = 2.0 * PART_FRAME + ICON_ROOM + 2.0 * BORDER_W;
/// Where the toolbar keeps the width of everything after the pickers, measured each frame.
const REST: &str = "toolbar-after-pickers";
/// The pickers' menus: one bar, so hovering the other part moves an open menu to it.
const MENU_ID: &str = "toolbar-pickers";

/// The widths of the repository and branch name slots, from their fitted `slots` and the
/// `room` the window leaves for both: the fitted slots when they fit; otherwise the wider
/// shrinks toward the narrower, then both evenly, never below `REPO_MIN` and `BRANCH_MIN` (nor
/// above their fitted widths).
fn name_widths(slots: (f32, f32), room: f32) -> (f32, f32) {
    let (repo, branch) = slots;
    if repo + branch <= room {
        return (repo, branch);
    }
    let narrower = repo.min(branch);
    let (r, b) = if room >= 2.0 * narrower {
        let wider = room - narrower;
        if repo >= branch { (wider, narrower) } else { (narrower, wider) }
    } else {
        (room / 2.0, room / 2.0)
    };
    (r.max(REPO_MIN.min(repo)), b.max(BRANCH_MIN.min(branch)))
}

/// The pickers' group with its fitted name `slots` in their comfortable room: each slot, up
/// to its share of `COMFORT_NAMES`, so short names don't fold groups earlier than they need to.
pub(super) fn comfort((repo, branch): (f32, f32)) -> f32 {
    FRAMES + repo.min(COMFORT_NAMES.0) + branch.min(COMFORT_NAMES.1)
}

/// Notes the width of everything after the pickers (the gaps and the button groups), drawing
/// once more when it changed so the names follow.
pub(super) fn remember_rest(ctx: &egui::Context, rest: f32) {
    let old = ctx.data_mut(|d| std::mem::replace(d.get_temp_mut_or(Id::new(REST), f32::NAN), rest));
    if old.is_nan() || (old - rest).abs() >= 0.5 {
        ctx.request_repaint();
    }
}

/// The pickers' group. Returns the fitted name slots, before a narrow window shrinks them.
pub(super) fn pickers(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) -> (f32, f32) {
    let t = st.settings.lang();
    let font = type_scale::strong(ui, BODY);
    let repo = st.snap.as_ref().map(|s| s.name.clone()).unwrap_or_else(|| t.tr("toolbar.open").into());
    let head = st.snap.as_ref().map(|snap| {
        snap.head_branch
            .clone()
            .unwrap_or_else(|| snap.head_oid.map(|o| t.trf("toolbar.detached", &[("sha", &short(o))])).unwrap_or_default())
    });
    // The room the rest of the bar (last frame's width) leaves the names.
    let rest = ui.ctx().data(|d| d.get_temp::<f32>(Id::new(REST)));
    let names_room = rest.map_or(f32::INFINITY, |rest| ui.available_width() - rest - FRAMES - ui.spacing().item_spacing.x);
    let slots = fit::slots(ui, st, &repo, head.as_deref(), (CARET_ROOM, ICON_ROOM + CARET_ROOM));
    let (repo_w, branch_w) = name_widths(slots, names_room);
    let repo_shown = middle(ui.painter(), &repo, &font, repo_w);
    let repo_cut = repo_shown != repo;
    let repo_g = ui.painter().layout_no_wrap(repo_shown, font.clone(), color::text());
    let branch = st.snap.as_ref().zip(head.as_ref()).map(|(snap, head)| {
        let mut counts = counts(snap);
        // The accessible name says the counts even when there is no room to show them.
        let name = access_name(t, head, &counts);
        let mut counts_w = if counts.is_empty() { 0.0 } else { parts_width(ui.painter(), &counts) + COUNTS_GAP };
        // Counts that would cut the name below its floor (a shorter name, its own width) hide;
        // the badges and tips still say them.
        let name_need = fit::text_w(ui.ctx(), head, &font).min(fit::BRANCH_FIT.0);
        if branch_w - counts_w < name_need {
            (counts, counts_w) = (Vec::new(), 0.0);
        }
        let shown = middle(ui.painter(), head, &font, (branch_w - counts_w).max(0.0));
        let cut = shown != *head;
        let g = ui.painter().layout_no_wrap(shown, font.clone(), color::text());
        // A dot in the checked-out branch's colour; a ring while HEAD is detached.
        (snap, head, g, cut, snap.head_swatch().map(line), counts, name)
    });
    let mut segs = vec![Seg { w: part_w(repo_w, false), enabled: true }];
    if branch.is_some() {
        segs.push(Seg { w: part_w(branch_w, true), enabled: true });
    }
    let style = BarStyle::Framed { fill: color::tool_backing() };
    let bar = segment_bar(ui, PICKER_SIZE, widgets::TOOL_H, radius::LG, style, &segs);
    let menu_id = Id::new(MENU_ID);
    let mut open =
        vec![menu_anchor::menu_below(ui, menu_id, &bar.responses[0], menu_anchor::MenuCmd::None, |ui| repo_menu(ui, t, st, msgs))];
    if let (Some((snap, ..)), Some(r)) = (&branch, bar.responses.get(1)) {
        open.push(menu_anchor::menu_below(ui, menu_id, r, menu_anchor::MenuCmd::None, |ui| branch_menu(ui, t, snap, msgs)));
    }
    bar.paint(ui, &open);
    let mut responses = bar.responses.into_iter();
    let cut = repo_cut;
    part(ui, responses.next().expect("the repository segment"), t.tr("toolbar.repository"), (repo_g, None, &[]), (&repo, &repo), cut);
    if let (Some((_, head, g, cut, tone, counts, name)), Some(r)) = (branch, responses.next()) {
        part(ui, r, t.tr("toolbar.branch"), (g, Some(tone), &counts), (head, &name), cut);
    }
    slots
}

/// The head branch's counts against its upstream: ahead, then behind, each only when not 0;
/// none on a detached HEAD or without an upstream.
fn counts(snap: &Snapshot) -> Vec<Part<'static>> {
    let Some(b) = snap.head_branch.as_ref().and_then(|_| snap.head_info()).filter(|b| b.upstream.is_some()) else { return Vec::new() };
    let mut parts = Vec::new();
    if b.ahead > 0 {
        parts.push(Part::Ahead(b.ahead));
    }
    if b.behind > 0 {
        parts.push(Part::Behind(b.behind));
    }
    parts
}

/// The branch picker's accessible name: the branch, with its `counts` ("main, 2 ahead, 3 behind").
fn access_name(t: Lang, head: &str, counts: &[Part<'_>]) -> String {
    match counts {
        [Part::Ahead(a), Part::Behind(b)] => t.trf("toolbar.branch_name_both", &[("branch", &head), ("ahead", a), ("behind", b)]),
        [Part::Ahead(n)] => t.trf("toolbar.branch_name_ahead", &[("branch", &head), ("count", n)]),
        [Part::Behind(n)] => t.trf("toolbar.branch_name_behind", &[("branch", &head), ("count", n)]),
        _ => head.to_owned(),
    }
}

/// The branch picker's menu: the local branches with their colour dots, the checked-out one
/// checked, then New branch.
fn branch_menu(ui: &mut egui::Ui, t: Lang, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    ui.set_min_width(220.0);
    for b in &snap.locals {
        // The current branch is checked and stays enabled: a click just closes the menu. One
        // another worktree has opens that worktree (or offers to prune a missing one), saying so
        // on hover.
        let dot = Some(line(snap.branch_swatch(&b.name)));
        let r = menu::item(b.name.as_str()).current(b.is_head).icon(branch_dot::paint_slot, dot).show(ui);
        let elsewhere = (!b.is_head).then(|| glowfly_model::state::worktree_branches::checkout_elsewhere(t, snap, &b.name)).flatten();
        let r = match &elsewhere {
            _ if b.is_head => r.on_hover_text(t.trf("toolbar.branch_menu.current_blocked", &[("branch", &b.name)])),
            Some(e) => r.on_hover_text(e.tip()),
            None => r,
        };
        if r.clicked() {
            if !b.is_head {
                msgs.push(Msg::Branch(Elsewhere::msg(elsewhere.as_ref(), BranchMsg::Checkout(b.name.clone()))));
            }
            ui.close_menu();
        }
    }
    separator(ui);
    let new_branch = Msg::Branch(BranchMsg::NewBranch { start: None, name: String::new() });
    menu_item(msgs, ui, t.tr("toolbar.new_branch"), new_branch);
}

/// A part's width around a name slot `slot` wide, with room for a branch dot before it when
/// `dot`. The slot, not the name shown, sets the width.
pub(super) fn part_w(slot: f32, dot: bool) -> f32 {
    2.0 * PAD_X + if dot { ICON_ROOM } else { 0.0 } + slot + CARET_ROOM
}

/// Paints one picker over its segment `r`: `caption` over the name galley (with its dot before
/// it: a branch dot, or a ring for None, and its `counts` after it), and the caret pinned at the
/// segment's right. Named by `caption` and `access`; `full`, the whole name, shows on hover when cut.
fn part(
    ui: &egui::Ui,
    r: Response,
    caption: &str,
    (name, dot, counts): (Arc<Galley>, Option<Option<Color32>>, &[Part<'_>]),
    (full, access): (&str, &str),
    cut: bool,
) -> Response {
    let rect = r.rect;
    let icon_w = if dot.is_some() { ICON_ROOM } else { 0.0 };
    // Named with its caption and counts ("Branch: main, 2 ahead, 3 behind"), as it reads on screen.
    let named = format!("{caption}: {access}");
    // In a very narrow window the caption is cut too.
    let caption = cut_galley(ui.ctx(), cut_job(caption.to_owned(), type_scale::font(CAPTION), color::weak()), rect.width() - 2.0 * PAD_X);
    let p = ui.painter();
    let x = rect.left() + PAD_X;
    let cap_h = caption.size().y;
    let top = sizes::centred_top(rect, cap_h + CAPTION_GAP + name.size().y);
    p.galley(pos2(x, top), caption, color::weak());
    let row_y = top + cap_h + CAPTION_GAP + name.size().y / 2.0;
    match dot {
        Some(Some(c)) => {
            p.circle_filled(pos2(x + 7.0, row_y), 4.0, c);
        }
        Some(None) => {
            p.circle_stroke(pos2(x + 7.0, row_y), 3.5, egui::Stroke::new(1.5_f32, color::weak()));
        }
        None => {}
    }
    let name_w = name.size().x;
    p.galley(pos2(x + icon_w, row_y - name.size().y / 2.0), name, color::text());
    if !counts.is_empty() {
        paint_parts(p, pos2(x + icon_w + name_w + COUNTS_GAP, row_y), counts, color::weak());
    }
    widgets::caret(p, pos2(rect.right() - PAD_X - 6.0, row_y + 1.0), color::weak());
    r.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &named));
    if cut { r.on_hover_text(full) } else { r }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLOTS: (f32, f32) = (120.0, 168.0);

    #[test]
    fn the_slots_keep_their_width_when_they_fit() {
        assert_eq!(name_widths(SLOTS, 400.0), SLOTS);
        assert_eq!(name_widths(SLOTS, f32::INFINITY), SLOTS);
        assert_eq!(name_widths(SLOTS, 120.0 + 168.0), SLOTS);
    }

    #[test]
    fn a_narrow_window_shrinks_the_wider_slot_first() {
        assert_eq!(name_widths(SLOTS, 260.0), (120.0, 140.0));
        assert_eq!(name_widths((168.0, 120.0), 260.0), (140.0, 120.0));
    }

    #[test]
    fn then_both_shrink_evenly() {
        assert_eq!(name_widths(SLOTS, 200.0), (100.0, 100.0));
        assert_eq!(name_widths((150.0, 150.0), 200.0), (100.0, 100.0));
    }

    #[test]
    fn the_floors_hold_in_a_tiny_room() {
        assert_eq!(name_widths(SLOTS, 10.0), (REPO_MIN, BRANCH_MIN));
        assert_eq!(name_widths(SLOTS, -50.0), (REPO_MIN, BRANCH_MIN));
        // A slot narrower than its floor keeps its own width.
        assert_eq!(name_widths((20.0, 200.0), 10.0), (20.0, BRANCH_MIN));
    }

    #[test]
    fn fitted_slots_shrink_by_the_same_rule() {
        // A short repository name and a long branch list's slot.
        let fitted = (48.0, 192.0);
        assert_eq!(name_widths(fitted, 400.0), fitted);
        assert_eq!(name_widths(fitted, 200.0), (48.0, 152.0), "the wider shrinks first");
        assert_eq!(name_widths(fitted, 80.0), (40.0, 40.0), "halves, above the floors");
        // Both short: they fit almost anywhere, and never grow past their fit.
        assert_eq!(name_widths((48.0, 48.0), 1000.0), (48.0, 48.0));
        assert_eq!(name_widths((48.0, 56.0), 100.0), (48.0, 52.0));
    }

    #[test]
    fn the_comfort_takes_short_slots_as_they_are() {
        assert_eq!(comfort((48.0, 48.0)), FRAMES + 96.0);
        assert_eq!(comfort((144.0, 192.0)), FRAMES + 64.0 + 96.0);
    }
}
