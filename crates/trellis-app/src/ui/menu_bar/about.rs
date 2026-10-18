//! Help > About Trellis (`Modal::About`): the version, licence and git in use, the project's
//! website, and the licenses of the bundled syntax definitions.

use crate::msg::Msg;
use crate::state::git_env::GitState;
use crate::state::menu_bar::{LICENSE, WEBSITE, git_line, reflow, version_info};
use crate::ui::theme::color;
use eframe::egui::{self, CollapsingHeader, Label, RichText, ScrollArea};
use trellis_core::i18n::Lang;

/// The notices list's height before it scrolls.
const NOTICES_HEIGHT: f32 = 320.0;

/// Draws the dialog for Trellis `version` (`menu_bar::VERSION`; tests pass a fixed one so a
/// release doesn't change their images) in `t`; true when it should close. The copied version
/// info stays English, for bug reports.
pub(crate) fn about(ui: &mut egui::Ui, t: Lang, git: &GitState, version: &str, msgs: &mut Vec<Msg>) -> bool {
    ui.heading("Trellis");
    ui.label(RichText::new(t.trf("about.version", &[("version", &version)])).color(color::weak()));
    ui.add_space(6.0);
    ui.label(t.tr("about.tagline"));
    ui.label(t.trf("about.license", &[("license", &LICENSE)]));
    if let Some(line) = git_line(git) {
        ui.label(line);
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.button(t.tr("about.website.button")).clicked() {
            msgs.push(Msg::OpenUrl(WEBSITE.to_owned()));
        }
        if ui.button(t.tr("about.copy_info.button")).on_hover_text(t.tr("about.copy_info.tip")).clicked() {
            msgs.push(Msg::Copy(version_info(git)));
        }
    });
    ui.add_space(10.0);
    ui.separator();
    ui.label(RichText::new(t.tr("about.notices.title")).strong());
    ui.add(Label::new(RichText::new(t.tr("about.notices.intro")).color(color::weak())).wrap());
    ScrollArea::vertical().max_height(NOTICES_HEIGHT).auto_shrink([false, true]).show(ui, |ui| {
        for n in trellis_core::notices::licenses() {
            CollapsingHeader::new(&n.name).id_salt(&n.source).default_open(false).show(ui, |ui| {
                ui.add(Label::new(RichText::new(reflow(&n.text)).monospace().size(11.5)).wrap());
            });
        }
    });
    if ui.button(t.tr("about.copy_notices.button")).clicked() {
        msgs.push(Msg::Copy(trellis_core::notices::TEXT.to_owned()));
    }
    ui.add_space(10.0);
    let mut close = false;
    ui.horizontal(|ui| {
        if ui.button(t.tr("common.close.button")).clicked() {
            close = true;
        }
    });
    close
}
