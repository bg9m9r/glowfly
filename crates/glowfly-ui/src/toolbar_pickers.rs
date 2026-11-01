//! The toolbar's Repository › Branch pickers: one framed segment bar (`widgets::segment_bar`)
//! of two segments parted by a full-height divider, each a caption over a name with its caret,
//! lighting its whole segment while hovered, held or open (the repository picker alone, with
//! no repository open, lights its whole frame). Each part has a fixed slot for its name
//! (`REPO_SLOT`, `BRANCH_SLOT`), so switching tabs or branches never resizes the bar or moves
//! the groups after it; only a window too narrow for everything shrinks the slots
//! (`name_widths`). A longer branch name keeps its start and end around a "…"
//! (`truncate::cut_middle_in`), a longer repository name is cut at its end; groups after them
//! fold away before the names go below `COMFORT`. A cut name shows whole on hover, and the
//! accessible name is always the whole name. The branch name follows a dot in the checked-out
//! branch's colour (a ring while HEAD is detached), and each branch in its menu has its colour
//! dot, the checked-out one checked.

use super::menus::repo_menu;
use crate::msg::{BranchMsg, Msg};
use crate::state::AppState;
use crate::ui::branch_dot;
use crate::ui::common::{menu_item, short};
use crate::ui::menu_anchor;
use crate::ui::theme::branch_colours::line;
use crate::ui::theme::color;
use crate::ui::theme::sizes::{self, ControlSize};
use crate::ui::theme::tokens::{BORDER_W, radius};
use crate::ui::theme::type_scale::{self, BODY, CAPTION};
use crate::ui::widgets;
use crate::ui::widgets::menu::{self, separator};
use crate::ui::widgets::segment_bar::{BarStyle, Seg, segment_bar};
use crate::ui::widgets::truncate::{cut_galley, cut_job, cut_middle_in};
use eframe::egui::{self, Color32, Galley, Id, Response, WidgetInfo, WidgetType, pos2};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Snapshot;
use std::sync::Arc;

/// The room for the repository and branch names, whatever names they show.
const REPO_SLOT: f32 = 120.0;
const BRANCH_SLOT: f32 = 168.0;
/// The narrowest the repository and branch names are cut to in a small window.
const REPO_MIN: f32 = 32.0;
const BRANCH_MIN: f32 = 40.0;
/// The names' room the pickers keep before toolbar groups start to fold
/// (`toolbar_fold.rs`): about a repository of 64 px and a branch of 96, so a 1400 px window
/// with a hosting service still shows every group in English.
const COMFORT_NAMES: f32 = 64.0 + 96.0;
/// A part's side padding, and the room between its caption and its name (the two centred as one
/// block in the segment).
const PAD_X: f32 = 10.0;
const CAPTION_GAP: f32 = 2.0;
/// The size whose focus style the segments take (their corners are the frame's).
const PICKER_SIZE: ControlSize = ControlSize::Large;
/// The caret's room after a name, and the branch icon's before it.
const CARET_ROOM: f32 = 14.0;
const ICON_ROOM: f32 = 20.0;
/// One part's room besides its name.
const PART_FRAME: f32 = 2.0 * PAD_X + CARET_ROOM;
/// The bar's room besides the names: both parts' frames, the branch icon and the border.
const FRAMES: f32 = 2.0 * PART_FRAME + ICON_ROOM + 2.0 * BORDER_W;
/// Where the toolbar keeps the width of everything after the pickers, measured each frame.
const REST: &str = "toolbar-after-pickers";
/// The pickers' menus: one bar, so hovering the other part moves an open menu to it.
const MENU_ID: &str = "toolbar-pickers";

/// The widths of the repository and branch name slots, from their full `slots` and the `room`
/// the window leaves for both: the full slots when they fit; otherwise the wider shrinks toward
/// the narrower, then both evenly, never below `REPO_MIN` and `BRANCH_MIN` (nor above their full
/// widths). The names shown never change them.
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

/// The pickers' group with its names in their comfortable room (`COMFORT_NAMES`).
pub(super) const COMFORT: f32 = FRAMES + COMFORT_NAMES;

/// Notes the width of everything after the pickers (the gaps and the button groups), drawing
/// once more when it changed so the names follow.
pub(super) fn remember_rest(ctx: &egui::Context, rest: f32) {
    let old = ctx.data_mut(|d| std::mem::replace(d.get_temp_mut_or(Id::new(REST), f32::NAN), rest));
    if old.is_nan() || (old - rest).abs() >= 0.5 {
        ctx.request_repaint();
    }
}

/// The pickers' group.
pub(super) fn pickers(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
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
    let (repo_w, branch_w) = name_widths((REPO_SLOT, BRANCH_SLOT), names_room);
    let repo_g = cut_galley(ui.ctx(), cut_job(repo.clone(), font.clone(), color::text()), repo_w);
    let branch = st.snap.as_ref().zip(head.as_ref()).map(|(snap, head)| {
        let shown = cut_middle_in(ui.painter(), head, &font, branch_w);
        let cut = shown != *head;
        let g = ui.painter().layout_no_wrap(shown, font.clone(), color::text());
        // A dot in the checked-out branch's colour; a ring while HEAD is detached.
        (snap, head, g, cut, snap.head_swatch().map(line))
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
    let cut = repo_g.elided;
    part(ui, responses.next().expect("the repository segment"), t.tr("toolbar.repository"), (repo_g, None), &repo, cut);
    if let (Some((_, head, g, cut, tone)), Some(r)) = (branch, responses.next()) {
        part(ui, r, t.tr("toolbar.branch"), (g, Some(tone)), head, cut);
    }
}

/// The branch picker's menu: the local branches with their colour dots, the checked-out one
/// checked, then New branch.
fn branch_menu(ui: &mut egui::Ui, t: Lang, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    ui.set_min_width(220.0);
    for b in &snap.locals {
        // The current branch is checked and stays enabled: a click just closes the menu.
        let why = (!b.is_head).then(|| crate::state::worktree_branches::checkout_blocked(t, snap, &b.name)).flatten();
        let dot = Some(line(snap.branch_swatch(&b.name)));
        let r = menu::item(b.name.as_str()).current(b.is_head).icon(branch_dot::paint_slot, dot).blocked(why.as_deref()).show(ui);
        let r = if b.is_head { r.on_hover_text(t.trf("toolbar.branch_menu.current_blocked", &[("branch", &b.name)])) } else { r };
        if r.clicked() {
            if !b.is_head {
                msgs.push(Msg::Branch(BranchMsg::Checkout(b.name.clone())));
            }
            ui.close_menu();
        }
    }
    separator(ui);
    let new_branch = Msg::Branch(BranchMsg::NewBranch { start: None, name: String::new() });
    menu_item(msgs, ui, t.tr("toolbar.new_branch"), new_branch);
}

/// A part's width around a name slot `slot` wide, with room for a branch dot before it when
/// `dot`. The slot, not the name, sets the width.
fn part_w(slot: f32, dot: bool) -> f32 {
    2.0 * PAD_X + if dot { ICON_ROOM } else { 0.0 } + slot + CARET_ROOM
}

/// Paints one picker over its segment `r`: `caption` over the name galley (with its dot before
/// it: a branch dot, or a ring for None), and the caret pinned at the segment's right. Named by
/// `full`, which also shows on hover when the name was `cut`.
fn part(ui: &egui::Ui, r: Response, caption: &str, (name, dot): (Arc<Galley>, Option<Option<Color32>>), full: &str, cut: bool) -> Response {
    let rect = r.rect;
    let icon_w = if dot.is_some() { ICON_ROOM } else { 0.0 };
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
    p.galley(pos2(x + icon_w, row_y - name.size().y / 2.0), name, color::text());
    widgets::caret(p, pos2(rect.right() - PAD_X - 6.0, row_y + 1.0), color::weak());
    r.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, full));
    if cut { r.on_hover_text(full) } else { r }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLOTS: (f32, f32) = (REPO_SLOT, BRANCH_SLOT);

    #[test]
    fn the_slots_keep_their_width_when_they_fit() {
        assert_eq!(name_widths(SLOTS, 400.0), SLOTS);
        assert_eq!(name_widths(SLOTS, f32::INFINITY), SLOTS);
        assert_eq!(name_widths(SLOTS, REPO_SLOT + BRANCH_SLOT), SLOTS);
    }

    #[test]
    fn a_narrow_window_shrinks_the_wider_slot_first() {
        assert_eq!(name_widths(SLOTS, 260.0), (REPO_SLOT, 140.0));
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
}
