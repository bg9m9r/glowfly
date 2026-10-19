//! The toolbar's repository › branch pickers and the conflict-check chip, in one slot so the
//! buttons after them never move: the slot's width follows only the window (its widest when
//! everything fits, narrower down to `PICKERS_MIN` so the groups stay apart in a smaller
//! window), long names are cut with an ellipsis (the whole name on hover and as the accessible
//! name) and short ones leave the slack after the slot.

use super::menus::repo_menu;
use crate::msg::{BranchMsg, Msg};
use crate::state::AppState;
use crate::ui::common::{caret_after, menu_item, short};
use crate::ui::theme::type_scale::{self, SUBHEAD};
use crate::ui::theme::{color, spacing};
use crate::ui::widgets::truncate::{cut_galley, cut_job};
use eframe::egui::{self, Id, RichText, WidgetText};

/// The widest a repository name is drawn (in a window with room for it).
pub const REPO_NAME_MAX: f32 = 96.0;
/// The widest a branch name (or "detached at …") is drawn; less `CHIP_ROOM` while the
/// conflict-check chip is shown beside it.
pub const BRANCH_NAME_MAX: f32 = 128.0;
/// Room for the conflict-check chip: its gap, a two-digit count and the spacing after it.
const CHIP_ROOM: f32 = 68.0;
/// The narrowest a branch name is cut to beside the chip; the repository name gives way first.
const BRANCH_BESIDE_CHIP: f32 = 40.0;
/// The slot's room besides the names: their menu buttons' padding, and the gap and "›" between
/// them with the row's spacing.
const SLOT_FRAME: f32 = 4.0 * spacing::BUTTON_PADDING.x + 12.0 + 10.0 + 3.0 * spacing::ITEM_SPACING.x;
/// The slot's widest: both names at their limits (the chip takes its room from the branch's).
pub const PICKERS_W: f32 = SLOT_FRAME + REPO_NAME_MAX + BRANCH_NAME_MAX;
/// The slot's narrowest, in a window too small for the rest at `PICKERS_W`: the names keep
/// about 60% of their widest, and with the chip shown the repository name keeps 32 px.
pub const PICKERS_MIN: f32 = SLOT_FRAME + 140.0;
/// Where the toolbar keeps the width of everything after the slot, measured each frame.
const REST: &str = "toolbar-after-pickers";

/// The widths the repository and branch names are cut to in a slot `slot` wide.
pub fn name_limits(slot: f32, chip: bool) -> (f32, f32) {
    let names = slot - SLOT_FRAME;
    let repo = names * REPO_NAME_MAX / (REPO_NAME_MAX + BRANCH_NAME_MAX);
    let branch = names - repo;
    if !chip {
        return (repo, branch);
    }
    let branch = (branch - CHIP_ROOM).max(BRANCH_BESIDE_CHIP);
    (names - CHIP_ROOM - branch, branch)
}

/// The slot's width: as wide as it goes while the rest of the toolbar (last frame's width,
/// which never depends on the names or the pull request button) still fits after it.
fn slot_width(ui: &egui::Ui) -> f32 {
    let rest = ui.ctx().data(|d| d.get_temp::<f32>(Id::new(REST)));
    rest.map_or(PICKERS_W, |rest| (ui.available_width() - rest).clamp(PICKERS_MIN, PICKERS_W))
}

/// Notes the width of everything after the slot (the gaps and both button groups), drawing
/// once more when it changed so the slot follows.
pub(super) fn remember_rest(ctx: &egui::Context, rest: f32) {
    let old = ctx.data_mut(|d| std::mem::replace(d.get_temp_mut_or(Id::new(REST), f32::NAN), rest));
    if old.is_nan() || (old - rest).abs() >= 0.5 {
        ctx.request_repaint();
    }
}

/// The pickers and chip, then the space left in the slot (the caller adds the group gap).
pub(super) fn pickers(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let (start, slot) = (ui.cursor().min.x, slot_width(ui));
    let chip = st.snap.is_some() && crate::ui::conflict_check::shown(st).is_some();
    let (repo_max, branch_max) = name_limits(slot, chip);
    let t = st.settings.lang();
    let (repo_font, branch_font) = (type_scale::strong(ui, SUBHEAD), type_scale::font(SUBHEAD));
    ui.vertical(|ui| {
        ui.add_space(4.0);
        ui.label(RichText::new(t.tr("toolbar.repository")).size(type_scale::CAPTION).color(color::weak()));
        let name = st.snap.as_ref().map(|s| s.name.clone()).unwrap_or_else(|| t.tr("toolbar.open").into());
        let g = cut_galley(ui.ctx(), cut_job(name, repo_font, color::text()), repo_max);
        let r = ui.menu_button(WidgetText::Galley(g.clone()), |ui| repo_menu(ui, t, st, msgs));
        caret_after(ui, &r.response);
        if g.elided {
            r.response.on_hover_text(g.text());
        }
    });
    if let Some(snap) = &st.snap {
        ui.add_space(12.0);
        ui.label(RichText::new("›").size(type_scale::TITLE).color(color::weak()));
        ui.vertical(|ui| {
            ui.add_space(4.0);
            ui.label(RichText::new(t.tr("toolbar.branch")).size(type_scale::CAPTION).color(color::weak()));
            let head = snap
                .head_branch
                .clone()
                .unwrap_or_else(|| snap.head_oid.map(|o| t.trf("toolbar.detached", &[("sha", &short(o))])).unwrap_or_default());
            let g = cut_galley(ui.ctx(), cut_job(head, branch_font, color::text()), branch_max);
            let r = ui.menu_button(WidgetText::Galley(g.clone()), |ui| {
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
            caret_after(ui, &r.response);
            if g.elided {
                r.response.on_hover_text(g.text());
            }
        });
        crate::ui::conflict_check::indicator(ui, st, msgs);
    }
    ui.add_space((start + slot - ui.cursor().min.x).max(0.0));
}
