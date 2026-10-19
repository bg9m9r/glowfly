//! Help > About Trellis (`Modal::About`): the version, licence and git in use, the project's
//! website, and the licenses of the bundled syntax definitions. The dialog can be resized.

use crate::msg::Msg;
use crate::state::git_env::GitState;
use crate::state::menu_bar::{LICENSE, WEBSITE, git_line, reflow, version_info};
use crate::ui::dialog_frame::{Cx, Dialog, Footer};
use crate::ui::theme::color;
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use eframe::egui::{self, CollapsingHeader, Label, RichText};

/// Draws the dialog for Trellis `version` (`menu_bar::VERSION`; tests pass a fixed one so a
/// release doesn't change their images) in `cx.t`; true when it should close. The copied version
/// info stays English, for bug reports.
pub(crate) fn about(ui: &mut egui::Ui, cx: Cx, git: &GitState, version: &str, msgs: &mut Vec<Msg>) -> bool {
    let t = cx.t;
    let mut d = Dialog::new(cx, "Trellis").subtitle(t.trf("about.version", &[("version", &version)])).resizable("about").begin(ui);
    d.body(ui, |ui| {
        ui.label(t.tr("about.tagline"));
        ui.label(t.trf("about.license", &[("license", &LICENSE)]));
        if let Some(line) = git_line(git) {
            ui.label(line);
        }
        ui.add_space(10.0);
        ui.separator();
        ui.label(type_scale::section_text(ui, t.tr("about.notices.title")));
        ui.add(Label::new(RichText::new(t.tr("about.notices.intro")).color(color::weak())).wrap());
        for n in trellis_core::notices::licenses() {
            CollapsingHeader::new(&n.name).id_salt(&n.source).default_open(false).show(ui, |ui| {
                ui.add(Label::new(RichText::new(reflow(&n.text)).size(type_scale::SMALL)).wrap());
            });
        }
        ui.add_space(4.0);
        if ui.add(buttons::secondary(t.tr("about.copy_notices.button")).small()).clicked() {
            msgs.push(Msg::Copy(trellis_core::notices::TEXT.to_owned()));
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
