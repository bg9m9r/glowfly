//! The banner under the toolbar once the merge a stopped Finish was waiting for is committed:
//! what is still to do, with Continue finish and Forget (also used by the Git Flow panel's row).

use crate::gitflow::GitFlowMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::gitflow_resume::{offered, ready_text};
use crate::ui::widgets::banner::{self, Banner, BannerKind};
use crate::ui::widgets::buttons;
use eframe::egui;
use trellis_core::i18n::Lang;

pub fn show(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some(r) = offered(st) else { return };
    let t = st.settings.lang();
    let idle = st.job.is_none();
    let b = Banner::new(BannerKind::Info).text(ready_text(t, &r.resume));
    banner::top(ctx, "gitflow_resume_banner", b, |ui| buttons(ui, t, true, idle, msgs));
}

/// Forget, then (once the merge is committed, `ready`) Continue finish, added right to left;
/// Continue is disabled while a job runs (`idle` false).
pub fn buttons(ui: &mut egui::Ui, t: Lang, ready: bool, idle: bool, msgs: &mut Vec<Msg>) {
    let forget = buttons::secondary(t.tr("gitflow.resume.forget.button")).tip(t.tr("gitflow.resume.forget.tip"));
    if ui.add(forget).clicked() {
        msgs.push(Msg::GitFlow(GitFlowMsg::ForgetFinish));
    }
    if ready {
        let why = (!idle).then(|| t.tr("common.busy"));
        let cont = buttons::primary(t.tr("gitflow.resume.continue.button")).enabled(idle).why(why);
        if ui.add(cont).clicked() {
            msgs.push(Msg::GitFlow(GitFlowMsg::ContinueFinish));
        }
    }
}
