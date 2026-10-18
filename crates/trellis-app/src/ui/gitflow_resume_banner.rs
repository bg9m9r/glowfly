//! The banner under the toolbar once the merge a stopped Finish was waiting for is committed:
//! what is still to do, with Continue finish and Forget (also used by the Git Flow panel's row).

use crate::gitflow::GitFlowMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::gitflow_resume::{offered, ready_text};
use crate::ui::theme::color;
use eframe::egui::{self, RichText};

const FORGET_TIP: &str = "Drop the remaining steps. Run them yourself or finish the branch again.";
const BUSY_TIP: &str = "Another operation is still running.";

pub fn show(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(r) = offered(st) else { return };
    let text = ready_text(&r.resume);
    let idle = st.job.is_none();
    egui::TopBottomPanel::top("gitflow_resume_banner")
        .frame(egui::Frame::new().fill(color::banner()).inner_margin(egui::Margin::symmetric(12, 6)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    buttons(ui, true, idle, msgs);
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add(egui::Label::new(RichText::new(text).color(color::text())).wrap());
                    });
                });
            });
        });
}

/// Forget, then (once the merge is committed, `ready`) Continue finish, added right to left;
/// Continue is disabled while a job runs (`idle` false).
pub fn buttons(ui: &mut egui::Ui, ready: bool, idle: bool, msgs: &mut Vec<Msg>) {
    if ui.button("Forget").on_hover_text(FORGET_TIP).clicked() {
        msgs.push(Msg::GitFlow(GitFlowMsg::ForgetFinish));
    }
    if ready {
        let r = ui.add_enabled(idle, egui::Button::new("Continue finish")).on_disabled_hover_text(BUSY_TIP);
        if r.clicked() {
            msgs.push(Msg::GitFlow(GitFlowMsg::ContinueFinish));
        }
    }
}
