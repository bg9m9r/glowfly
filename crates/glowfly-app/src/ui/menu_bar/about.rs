//! Help > About Glowfly (`Modal::About`): the version, licence and git in use and the project's
//! website. Third-party notices… opens the bundled licenses in their own dialog
//! (`notices.rs`).

use crate::msg::{AppMsg, Msg};
use crate::state::git_env::GitState;
use crate::state::menu_bar::{LICENSE, WEBSITE, git_line, version_info};
use crate::ui::dialog_frame::{Cx, Dialog, Footer};
use crate::ui::widgets::buttons;
use eframe::egui;

/// Draws the dialog for Glowfly `version` (`menu_bar::VERSION`; tests pass a fixed one so a
/// release doesn't change their images) in `cx.t`; true when it should close. The copied version
/// info stays English, for bug reports.
pub(crate) fn about(ui: &mut egui::Ui, cx: Cx, git: &GitState, version: &str, msgs: &mut Vec<Msg>) -> bool {
    let t = cx.t;
    let mut d = Dialog::new(cx, "Glowfly").subtitle(t.trf("about.version", &[("version", &version)])).begin(ui);
    d.body(ui, |ui| {
        ui.label(t.tr("about.tagline"));
        ui.label(t.trf("about.license", &[("license", &LICENSE)]));
        if let Some(line) = git_line(git) {
            ui.label(line);
        }
        ui.add_space(8.0);
        if ui.add(buttons::secondary(t.tr("about.notices.button")).small()).clicked() {
            msgs.push(Msg::App(AppMsg::ShowNotices));
        }
    });
    let footer = Footer::new()
        .extra(buttons::secondary(t.tr("about.website.button")))
        .extra(buttons::secondary(t.tr("about.copy_info.button")).tip(t.tr("about.copy_info.tip")))
        .close();
    let out = d.finish(ui, footer);
    match out.extra {
        Some(0) => msgs.push(Msg::OpenUrl(WEBSITE.to_owned())),
        Some(_) => msgs.push(Msg::Copy(version_info(git))),
        None => {}
    }
    out.dismiss
}
