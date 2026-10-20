//! The toolbar's Repository › Branch pickers: one group of two parts, each a caption over a
//! name with its caret, sized to its name up to `NAME_MAX`. A longer branch name keeps its
//! start and end around a "…" (`truncate::cut_middle_in`), a longer repository name is cut at
//! its end, and in a window too narrow for everything both shrink further (`name_widths`). A cut
//! name shows whole on hover, and the accessible name is always the whole name. The
//! conflict-check chip follows the group.

use super::menus::repo_menu;
use crate::msg::{BranchMsg, Msg};
use crate::state::AppState;
use crate::ui::common::{menu_item, short};
use crate::ui::menu_anchor;
use crate::ui::theme::color;
use crate::ui::theme::type_scale::{self, BODY, CAPTION};
use crate::ui::widgets::truncate::{cut_galley, cut_job, cut_middle_in};
use crate::ui::widgets::{self, Icon};
use eframe::egui::{self, Color32, CornerRadius, Galley, Id, Response, Sense, WidgetInfo, WidgetType, pos2, vec2};
use std::sync::Arc;

/// The widest a name is drawn: about 30 characters.
pub const NAME_MAX: f32 = 224.0;
/// The narrowest the repository and branch names are cut to in a small window.
pub const REPO_MIN: f32 = 32.0;
pub const BRANCH_MIN: f32 = 40.0;
/// Room for the conflict-check chip: its gap, a two-digit count and the spacing after it.
pub const CHIP_ROOM: f32 = 68.0;
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

/// The widths the repository and branch names are drawn at, from their `natural` widths and the
/// `names_room` left for both (less `CHIP_ROOM` while the chip is shown): each at most
/// `NAME_MAX`; when they don't fit, the wider shrinks toward the narrower, then both evenly,
/// never below `REPO_MIN` and `BRANCH_MIN` (nor above their natural widths).
pub fn name_widths(natural: (f32, f32), names_room: f32, chip: bool) -> (f32, f32) {
    let (repo, branch) = (natural.0.min(NAME_MAX), natural.1.min(NAME_MAX));
    let room = names_room - if chip { CHIP_ROOM } else { 0.0 };
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

/// The pickers' group and the conflict chip after it.
pub(super) fn pickers(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let font = type_scale::strong(ui, BODY);
    let repo = st.snap.as_ref().map(|s| s.name.clone()).unwrap_or_else(|| t.tr("toolbar.open").into());
    let head = st.snap.as_ref().map(|snap| {
        snap.head_branch
            .clone()
            .unwrap_or_else(|| snap.head_oid.map(|o| t.trf("toolbar.detached", &[("sha", &short(o))])).unwrap_or_default())
    });
    let chip = st.snap.is_some() && crate::ui::conflict_check::shown(st).is_some();
    let measure = |s: &str| ui.painter().layout_no_wrap(s.to_owned(), font.clone(), Color32::PLACEHOLDER).size().x;
    // The room the rest of the bar (last frame's width) leaves the names.
    let rest = ui.ctx().data(|d| d.get_temp::<f32>(Id::new(REST)));
    let names_room = rest.map_or(f32::INFINITY, |rest| ui.available_width() - rest - FRAMES - ui.spacing().item_spacing.x);
    let (repo_w, branch_w) = name_widths((measure(&repo), head.as_deref().map_or(0.0, measure)), names_room, chip);
    widgets::tool_group(ui, |ui| {
        ui.set_min_height(widgets::TOOL_H);
        let g = cut_galley(ui.ctx(), cut_job(repo.clone(), font.clone(), color::text()), repo_w);
        let cut = g.elided;
        part(ui, t.tr("toolbar.repository"), g, None, &repo, cut, |ui| repo_menu(ui, t, st, msgs));
        if let (Some(snap), Some(head)) = (&st.snap, &head) {
            let (rect, _) = ui.allocate_exact_size(vec2(CHEVRON_W, PART_H), Sense::hover());
            widgets::paint_icon_sized(ui.painter(), rect.center(), Icon::ExpandRight, color::faint(), 14.0);
            let shown = cut_middle_in(ui.painter(), head, &font, branch_w);
            let cut = shown != *head;
            let g = ui.painter().layout_no_wrap(shown, font.clone(), color::text());
            part(ui, t.tr("toolbar.branch"), g, Some(Icon::Branch), head, cut, |ui| {
                ui.set_min_width(220.0);
                for b in &snap.locals {
                    let text = if b.is_head { format!("✔ {}", b.name) } else { b.name.clone() };
                    let elsewhere = crate::state::worktree_branches::checkout_blocked(snap, &b.name);
                    let mut r = ui.add_enabled(!b.is_head && elsewhere.is_none(), egui::Button::new(text));
                    if let Some(why) = &elsewhere {
                        r = r.on_disabled_hover_text(why);
                    }
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
    if st.snap.is_some() {
        crate::ui::conflict_check::indicator(ui, st, msgs);
    }
}

/// One picker: `caption` over `name` (with `icon` before it) and the caret, opening `menu`
/// below it. Named by `full`, which also shows on hover when the name was `cut`.
fn part(
    ui: &mut egui::Ui,
    caption: &str,
    name: Arc<Galley>,
    icon: Option<Icon>,
    full: &str,
    cut: bool,
    menu: impl FnOnce(&mut egui::Ui),
) -> Response {
    let caption = ui.painter().layout_no_wrap(caption.to_owned(), type_scale::font(CAPTION), color::weak());
    let icon_w = if icon.is_some() { ICON_ROOM } else { 0.0 };
    let w = 2.0 * PAD_X + caption.size().x.max(icon_w + name.size().x + CARET_ROOM);
    let (rect, r) = ui.allocate_exact_size(vec2(w, PART_H), Sense::click());
    let open = menu_anchor::menu_below(ui, Id::new(MENU_ID), &r, false, menu);
    let p = ui.painter();
    if open {
        p.rect_filled(rect, CornerRadius::same(PART_RADIUS), color::selected());
    } else if r.hovered() {
        p.rect_filled(rect, CornerRadius::same(PART_RADIUS), color::hover());
    }
    let x = rect.left() + PAD_X;
    p.galley(pos2(x, rect.top() + 12.0 - caption.size().y / 2.0), caption, color::weak());
    let row_y = rect.top() + 27.0;
    if let Some(icon) = icon {
        widgets::paint_icon_sized(p, pos2(x + 7.0, row_y), icon, color::accent(), 14.0);
    }
    let (name_x, name_w) = (x + icon_w, name.size().x);
    p.galley(pos2(name_x, row_y - name.size().y / 2.0), name, color::text());
    widgets::chevron(p, pos2(name_x + name_w + 8.0, row_y + 1.0), true, color::weak());
    r.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, full));
    if cut { r.on_hover_text(full) } else { r }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_that_fit_keep_their_width() {
        assert_eq!(name_widths((60.0, 90.0), 400.0, false), (60.0, 90.0));
        assert_eq!(name_widths((60.0, 90.0), f32::INFINITY, true), (60.0, 90.0));
    }

    #[test]
    fn each_name_is_capped() {
        assert_eq!(name_widths((400.0, 500.0), f32::INFINITY, false), (NAME_MAX, NAME_MAX));
        assert_eq!(name_widths((40.0, 500.0), f32::INFINITY, false), (40.0, NAME_MAX));
    }

    #[test]
    fn a_long_branch_beside_a_short_repo_shrinks_alone() {
        assert_eq!(name_widths((60.0, 400.0), 200.0, false), (60.0, 140.0));
        assert_eq!(name_widths((300.0, 50.0), 200.0, false), (150.0, 50.0));
    }

    #[test]
    fn two_long_names_shrink_evenly() {
        assert_eq!(name_widths((300.0, 300.0), 300.0, false), (150.0, 150.0));
        assert_eq!(name_widths((120.0, 400.0), 200.0, false), (100.0, 100.0));
    }

    #[test]
    fn the_chip_takes_its_room() {
        assert_eq!(name_widths((100.0, 100.0), 200.0 + CHIP_ROOM, true), (100.0, 100.0));
        assert_eq!(name_widths((100.0, 100.0), 200.0, true), (66.0, 66.0));
    }

    #[test]
    fn the_floors_hold_in_a_tiny_room() {
        assert_eq!(name_widths((200.0, 200.0), 10.0, false), (REPO_MIN, BRANCH_MIN));
        assert_eq!(name_widths((200.0, 200.0), -50.0, true), (REPO_MIN, BRANCH_MIN));
        // A name narrower than its floor keeps its own width.
        assert_eq!(name_widths((20.0, 200.0), 10.0, false), (20.0, BRANCH_MIN));
    }
}
