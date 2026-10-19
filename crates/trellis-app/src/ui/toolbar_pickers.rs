//! The toolbar's repository › branch pickers and the conflict-check chip, in one fixed-width
//! slot so the buttons after them never move: long names are cut with an ellipsis (the whole
//! name on hover and as the accessible name) and short ones leave the slack after the slot.

use super::menus::repo_menu;
use crate::msg::{BranchMsg, Msg};
use crate::state::AppState;
use crate::ui::common::{caret_after, menu_item, short};
use crate::ui::theme::{color, spacing};
use crate::ui::widgets::truncate::{cut_galley, cut_job};
use eframe::egui::{self, FontId, RichText, WidgetText};

/// The widest a repository name is drawn. The slot leaves the toolbar's groups apart in a
/// 1400 px window with captions shown (German's are the widest).
pub const REPO_NAME_MAX: f32 = 96.0;
/// The widest a branch name (or "detached at …") is drawn; less `CHIP_ROOM` while the
/// conflict-check chip is shown beside it.
pub const BRANCH_NAME_MAX: f32 = 128.0;
/// Room for the conflict-check chip: its gap, a two-digit count and the spacing after it.
const CHIP_ROOM: f32 = 64.0;
/// The slot's width, its worst case: both names at their limits inside their menu buttons'
/// padding, and the gap and "›" between them with the row's spacing (the chip takes its room
/// from the branch name's).
pub const PICKERS_W: f32 = REPO_NAME_MAX + BRANCH_NAME_MAX + 4.0 * spacing::BUTTON_PADDING.x + 12.0 + 10.0 + 3.0 * spacing::ITEM_SPACING.x;

/// The pickers and chip, then the space left in the slot (the caller adds the group gap).
pub(super) fn pickers(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let start = ui.cursor().min.x;
    let t = st.settings.lang();
    let name_font = FontId::proportional(16.0);
    ui.vertical(|ui| {
        ui.add_space(4.0);
        ui.label(RichText::new(t.tr("toolbar.repository")).size(11.0).color(color::weak()));
        let name = st.snap.as_ref().map(|s| s.name.clone()).unwrap_or_else(|| t.tr("toolbar.open").into());
        let g = cut_galley(ui.ctx(), cut_job(name, name_font.clone(), color::text()), REPO_NAME_MAX);
        let r = ui.menu_button(WidgetText::Galley(g.clone()), |ui| repo_menu(ui, t, st, msgs));
        caret_after(ui, &r.response);
        if g.elided {
            r.response.on_hover_text(g.text());
        }
    });
    if let Some(snap) = &st.snap {
        let chip = crate::ui::conflict_check::shown(st).is_some();
        ui.add_space(12.0);
        ui.label(RichText::new("›").size(20.0).color(color::weak()));
        ui.vertical(|ui| {
            ui.add_space(4.0);
            ui.label(RichText::new(t.tr("toolbar.branch")).size(11.0).color(color::weak()));
            let head = snap
                .head_branch
                .clone()
                .unwrap_or_else(|| snap.head_oid.map(|o| t.trf("toolbar.detached", &[("sha", &short(o))])).unwrap_or_default());
            let max = if chip { BRANCH_NAME_MAX - CHIP_ROOM } else { BRANCH_NAME_MAX };
            let g = cut_galley(ui.ctx(), cut_job(head, name_font, color::text()), max);
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
    ui.add_space((start + PICKERS_W - ui.cursor().min.x).max(0.0));
}
