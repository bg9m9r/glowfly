//! The toolbar's Repository › Branch pickers: one group of two parts, each a caption over a
//! name with its caret. Each part has a fixed slot for its name (`REPO_SLOT`, `BRANCH_SLOT`), so
//! switching tabs or branches never resizes the group or moves the groups after it; only a
//! window too narrow for everything shrinks the slots (`name_widths`). A longer branch name keeps
//! its start and end around a "…" (`truncate::cut_middle_in`), a longer repository name is cut
//! at its end. A cut name shows whole on hover, and the accessible name is always the whole name.
//! The branch name follows a dot in the checked-out branch's colour (a ring while HEAD is
//! detached), and each branch in its menu starts with its colour dot.

use super::menus::repo_menu;
use crate::msg::{BranchMsg, Msg};
use crate::state::AppState;
use crate::ui::branch_dot;
use crate::ui::common::{menu_item, short};
use crate::ui::menu_anchor;
use crate::ui::theme::branch_colours::line;
use crate::ui::theme::color;
use crate::ui::theme::type_scale::{self, BODY, CAPTION};
use crate::ui::widgets::truncate::{cut_galley, cut_job, cut_middle_in};
use crate::ui::widgets::{self, Icon};
use eframe::egui::{self, Color32, CornerRadius, Galley, Id, Response, Sense, WidgetInfo, WidgetType, pos2, vec2};
use std::sync::Arc;

/// The room for the repository and branch names, whatever names they show.
pub const REPO_SLOT: f32 = 120.0;
pub const BRANCH_SLOT: f32 = 168.0;
/// The narrowest the repository and branch names are cut to in a small window.
pub const REPO_MIN: f32 = 32.0;
pub const BRANCH_MIN: f32 = 40.0;
/// A part's side padding, height and corners.
const PAD_X: f32 = 10.0;
const PART_H: f32 = 40.0;
const PART_RADIUS: u8 = 7;
/// The caret's room after a name, and the branch icon's before it.
const CARET_ROOM: f32 = 14.0;
const ICON_ROOM: f32 = 20.0;
/// The chevron between the parts.
const CHEVRON_W: f32 = 14.0;
/// One part's room besides its name.
const PART_FRAME: f32 = 2.0 * PAD_X + CARET_ROOM;
/// The group's room besides the names: both parts' frames, the branch icon, the chevron with
/// the spacing either side of it, and the group's padding and stroke.
const FRAMES: f32 = 2.0 * PART_FRAME + ICON_ROOM + CHEVRON_W + 2.0 * widgets::TOOL_SPACING + 2.0 * (widgets::GROUP_PAD as f32 + 1.0);
/// Where the toolbar keeps the width of everything after the pickers, measured each frame.
const REST: &str = "toolbar-after-pickers";
/// The pickers' menus: one bar, so hovering the other part moves an open menu to it.
const MENU_ID: &str = "toolbar-pickers";

/// The widths of the repository and branch name slots, from their full `slots` and the `room`
/// the window leaves for both: the full slots when they fit; otherwise the wider shrinks toward
/// the narrower, then both evenly, never below `REPO_MIN` and `BRANCH_MIN` (nor above their full
/// widths). The names shown never change them.
pub fn name_widths(slots: (f32, f32), room: f32) -> (f32, f32) {
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
    widgets::tool_group(ui, |ui| {
        ui.set_min_height(widgets::TOOL_H);
        let g = cut_galley(ui.ctx(), cut_job(repo.clone(), font.clone(), color::text()), repo_w);
        let cut = g.elided;
        part(ui, t.tr("toolbar.repository"), (g, repo_w), None, &repo, cut, |ui| repo_menu(ui, t, st, msgs));
        if let (Some(snap), Some(head)) = (&st.snap, &head) {
            let (rect, _) = ui.allocate_exact_size(vec2(CHEVRON_W, PART_H), Sense::hover());
            widgets::paint_icon_sized(ui.painter(), rect.center(), Icon::ExpandRight, color::faint(), 14.0);
            let shown = cut_middle_in(ui.painter(), head, &font, branch_w);
            let cut = shown != *head;
            let g = ui.painter().layout_no_wrap(shown, font.clone(), color::text());
            // A dot in the checked-out branch's colour; a ring while HEAD is detached.
            let tone = snap.head_swatch().map(line);
            part(ui, t.tr("toolbar.branch"), (g, branch_w), Some(tone), head, cut, |ui| {
                ui.set_min_width(220.0);
                for b in &snap.locals {
                    let text = if b.is_head { format!("✔ {}", b.name) } else { b.name.clone() };
                    let why = if b.is_head {
                        Some(t.trf("toolbar.branch_menu.current_blocked", &[("branch", &b.name)]))
                    } else {
                        crate::state::worktree_branches::checkout_blocked(snap, &b.name)
                    };
                    let r = branch_dot::menu_row_if(ui, &text, Some(line(snap.branch_swatch(&b.name))), why.as_deref());
                    if r.clicked() {
                        msgs.push(Msg::Branch(BranchMsg::Checkout(b.name.clone())));
                        ui.close_menu();
                    }
                }
                ui.separator();
                let new_branch = Msg::Branch(BranchMsg::NewBranch { start: None, name: String::new() });
                menu_item(msgs, ui, t.tr("toolbar.new_branch"), new_branch);
            });
        }
    });
}

/// One picker: `caption` over `name` (with `dot` before it: a branch dot, or a ring for None) in a slot of the
/// width paired with it, and the caret pinned at the part's right, opening `menu` below it.
/// Named by `full`, which also shows on hover when the name was `cut`.
fn part(
    ui: &mut egui::Ui,
    caption: &str,
    (name, slot): (Arc<Galley>, f32),
    dot: Option<Option<Color32>>,
    full: &str,
    cut: bool,
    menu: impl FnOnce(&mut egui::Ui),
) -> Response {
    let icon_w = if dot.is_some() { ICON_ROOM } else { 0.0 };
    // The slot, not the name, sets the width; in a very narrow window the caption is cut too.
    let w = 2.0 * PAD_X + icon_w + slot + CARET_ROOM;
    let caption = cut_galley(ui.ctx(), cut_job(caption.to_owned(), type_scale::font(CAPTION), color::weak()), w - 2.0 * PAD_X);
    let (rect, r) = ui.allocate_exact_size(vec2(w, PART_H), Sense::click());
    let open = menu_anchor::menu_below(ui, Id::new(MENU_ID), &r, false, menu);
    let p = ui.painter();
    if open {
        p.rect_filled(rect, CornerRadius::same(PART_RADIUS), color::tool_pressed());
    } else if r.hovered() {
        p.rect_filled(rect, CornerRadius::same(PART_RADIUS), color::tool_hover());
    }
    let x = rect.left() + PAD_X;
    p.galley(pos2(x, rect.top() + 12.0 - caption.size().y / 2.0), caption, color::weak());
    let row_y = rect.top() + 27.0;
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
