//! Help > About Trellis (`Modal::About`): the version, licence and git in use, the project's
//! website, and the licenses of the bundled syntax definitions.

use crate::msg::Msg;
use crate::state::git_env::GitState;
use crate::state::menu_bar::{LICENSE, WEBSITE, git_line, reflow, version_info};
use crate::ui::theme::color;
use eframe::egui::{self, CollapsingHeader, Label, RichText, ScrollArea};

/// The notices list's height before it scrolls.
const NOTICES_HEIGHT: f32 = 320.0;

/// Draws the dialog for Trellis `version` (`menu_bar::VERSION`; tests pass a fixed one so a
/// release doesn't change their images); true when it should close.
pub(crate) fn about(ui: &mut egui::Ui, git: &GitState, version: &str, msgs: &mut Vec<Msg>) -> bool {
    ui.heading("Trellis");
    ui.label(RichText::new(format!("Version {version}")).color(color::weak()));
    ui.add_space(6.0);
    ui.label("A fast, native Git client for Windows, macOS and Linux.");
    ui.label(format!("Licensed under {LICENSE}"));
    if let Some(line) = git_line(git) {
        ui.label(line);
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.button("Website").clicked() {
            msgs.push(Msg::OpenUrl(WEBSITE.to_owned()));
        }
        if ui.button("Copy version info").on_hover_text("For bug reports").clicked() {
            msgs.push(Msg::Copy(version_info(git)));
        }
    });
    ui.add_space(10.0);
    ui.separator();
    ui.label(RichText::new("Third-party notices").strong());
    ui.add(
        Label::new(RichText::new("Trellis bundles syntax definitions for highlighting diffs; their licenses follow.").color(color::weak()))
            .wrap(),
    );
    ScrollArea::vertical().max_height(NOTICES_HEIGHT).auto_shrink([false, true]).show(ui, |ui| {
        for n in trellis_core::notices::licenses() {
            CollapsingHeader::new(&n.name).id_salt(&n.source).default_open(false).show(ui, |ui| {
                ui.add(Label::new(RichText::new(reflow(&n.text)).monospace().size(11.5)).wrap());
            });
        }
    });
    if ui.button("Copy all notices").clicked() {
        msgs.push(Msg::Copy(trellis_core::notices::TEXT.to_owned()));
    }
    ui.add_space(10.0);
    let mut close = false;
    ui.horizontal(|ui| {
        if ui.button("Close").clicked() {
            close = true;
        }
    });
    close
}
