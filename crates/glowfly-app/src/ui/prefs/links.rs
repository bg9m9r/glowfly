//! Preferences > General: the Glowfly Links row (which program opens glowfly:// links, and
//! registering this copy of Glowfly for them).

use super::{help, row};
use crate::deep_link::DeepLinkMsg;
use crate::msg::Msg;
use crate::state::deep_link::{SchemeView, can_unregister, status_text};
use crate::ui::theme::color;
use crate::ui::widgets::buttons;
use eframe::egui::{self, RichText, Ui};
use glowfly_core::i18n::Lang;
use glowfly_core::url_scheme::SchemeStatus;

pub fn links_row(ui: &mut Ui, t: Lang, view: &SchemeView, msgs: &mut Vec<Msg>) {
    row(ui, t.tr("prefs.links.label"), |ui| {
        if cfg!(target_os = "macos") {
            ui.add(egui::Label::new(RichText::new(t.tr("prefs.links.macos_note")).color(color::text())).wrap());
            help(ui, t.tr("prefs.links.help"));
            return;
        }
        // Nothing to register here (inside Flatpak, say): only why.
        if matches!(view, SchemeView::Known(SchemeStatus::Unsupported(_))) {
            ui.add(egui::Label::new(RichText::new(status_text(t, view).0).color(color::text())).wrap());
            help(ui, t.tr("prefs.links.help"));
            return;
        }
        let (text, error) = status_text(t, view);
        ui.add(egui::Label::new(RichText::new(text).color(if error { color::removed() } else { color::text() })).wrap());
        let checking = matches!(view, SchemeView::Unknown | SchemeView::Checking);
        let other = matches!(view, SchemeView::Known(SchemeStatus::OtherProgram(_)));
        ui.horizontal(|ui| {
            let register = if other { t.tr("prefs.links.register_this.button") } else { t.tr("prefs.links.register.button") };
            if ui.add(buttons::secondary(register).enabled(!checking)).clicked() {
                msgs.push(Msg::DeepLink(DeepLinkMsg::Register));
            }
            if ui.add(buttons::secondary(t.tr("prefs.links.unregister.button")).enabled(!checking && can_unregister(view))).clicked() {
                msgs.push(Msg::DeepLink(DeepLinkMsg::Unregister));
            }
            if error && ui.add(buttons::secondary(t.tr("prefs.links.check_again.button"))).clicked() {
                msgs.push(Msg::DeepLink(DeepLinkMsg::CheckScheme));
            }
        });
        help(ui, t.tr("prefs.links.help"));
    });
}

#[cfg(test)]
mod tests {
    use glowfly_core::i18n::Lang;
    use glowfly_core::url_scheme::{FLATPAK_NOTE, MACOS_NOTE};

    #[test]
    fn the_macos_note_reads_as_core_wrote_it_in_english() {
        assert_eq!(Lang::En.tr("prefs.links.macos_note"), MACOS_NOTE);
    }

    #[test]
    fn the_flatpak_note_reads_as_core_wrote_it_in_english() {
        assert_eq!(Lang::En.tr("prefs.links.flatpak_note"), FLATPAK_NOTE);
    }
}
