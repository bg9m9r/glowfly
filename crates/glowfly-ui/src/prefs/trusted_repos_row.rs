//! One row of Preferences > Trusted repositories: the folder's name with its decision's chip
//! and Revoke (or Forget, for Don't trust) at the right; under it, weak and small, the folder's
//! path cut in the middle (whole on hover), when it was decided, the repository it was passed
//! on from, how many were passed on from it, and a painted warning mark with Not found when the
//! folder check didn't find it.

use crate::theme::sizes::ControlSize;
use crate::theme::{color, type_scale};
use crate::widgets::chip::{Chip, ChipSize, Look};
use crate::widgets::truncate::{cut_path_in, truncate_label};
use crate::widgets::{self, Icon, buttons};
use eframe::egui::{self, Align, FontId, Layout, RichText, Sense, Ui, WidgetInfo, WidgetType, vec2};
use glowfly_core::i18n::Lang;
use glowfly_core::native_path;
use glowfly_core::settings::RepoTrust;
use glowfly_model::msg::{Msg, TrustMsg};
use glowfly_model::state::trust_list::TrustRow;

/// Room kept at the right of the name for the chip and the button.
const ACTIONS_W: f32 = 230.0;

/// `when`: the row's date as the user reads dates; `checked`: the folder check reported.
pub(super) fn row(ui: &mut Ui, t: Lang, r: &TrustRow, when: Option<&str>, checked: bool, msgs: &mut Vec<Msg>) {
    egui::Frame::new().inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.horizontal(|ui| {
            ui.set_min_height(24.0);
            let name = RichText::new(&r.name).font(type_scale::strong(ui, type_scale::BODY)).color(color::text());
            truncate_label(ui, name, (ui.available_width() - ACTIONS_W).max(80.0));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                action(ui, t, r, msgs);
                ui.add_space(8.0);
                chip(ui, t, r.trust);
            });
        });
        details(ui, t, r, when, checked);
    });
}

/// The decision's chip: Trusted, or a quiet Not trusted.
fn chip(ui: &mut Ui, t: Lang, trust: RepoTrust) {
    let (text, look) = match trust {
        RepoTrust::Trusted => (t.tr("prefs.trust.trusted"), Look::new(color::wash(color::added(), 0.25), color::text())),
        RepoTrust::NotTrusted => (t.tr("prefs.trust.not_trusted"), Look::new(color::wash(color::weak(), 0.15), color::weak())),
    };
    Chip::new(ChipSize::Badge, text).look(look).show(ui);
}

/// Revoke on a Trusted row, Forget on a Not trusted one, named for its repository.
fn action(ui: &mut Ui, t: Lang, r: &TrustRow, msgs: &mut Vec<Msg>) {
    let name = [("name", &r.name as &dyn std::fmt::Display)];
    let (label, access) = match r.trust {
        RepoTrust::Trusted => (t.tr("prefs.trust.revoke.button"), t.trf("prefs.trust.revoke.name", &name)),
        RepoTrust::NotTrusted => (t.tr("prefs.trust.forget.button"), t.trf("prefs.trust.forget.name", &name)),
    };
    let tip = match (r.trust, r.heirs) {
        (_, n) if n > 0 => t.trn("prefs.trust.revoke.heirs.tip", n, &[]),
        (RepoTrust::Trusted, _) => t.tr("prefs.trust.revoke.tip").to_owned(),
        (RepoTrust::NotTrusted, _) => t.tr("prefs.trust.forget.tip").to_owned(),
    };
    let button = buttons::secondary(label).size(ControlSize::Small).access_name(&access).tip(&tip);
    if ui.add(button).clicked() {
        msgs.push(Msg::Trust(TrustMsg::Revoke { real: r.path.clone() }));
    }
}

/// The weak line under the name.
fn details(ui: &mut Ui, t: Lang, r: &TrustRow, when: Option<&str>, checked: bool) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let font = FontId::proportional(type_scale::SMALL);
        let full = native_path::shown(&r.path);
        let shown = cut_path_in(ui.painter(), &full, &font, ui.available_width() * 0.55, std::path::MAIN_SEPARATOR);
        let cut = shown != full;
        let path = ui.label(RichText::new(shown).font(font.clone()).color(color::weak()));
        path.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &full));
        if cut {
            path.on_hover_text(&full);
        }
        let mut facts: Vec<String> = Vec::new();
        if let Some(when) = when {
            facts.push(t.trf("prefs.trust.decided", &[("when", &when)]));
        }
        if let Some(from) = &r.from_name {
            facts.push(t.trf("prefs.trust.inherited_from", &[("name", from)]));
        }
        if r.heirs > 0 {
            facts.push(t.trn("prefs.trust.heirs", r.heirs, &[]));
        }
        if !facts.is_empty() {
            ui.label(RichText::new(format!("· {}", facts.join(" · "))).font(font.clone()).color(color::weak()));
        }
        if checked && r.missing {
            ui.add_space(4.0);
            let (mark, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
            widgets::paint_icon_sized(ui.painter(), mark.center(), Icon::Warning, color::warn(), 13.0);
            ui.label(RichText::new(t.tr("prefs.trust.not_found")).font(font).color(color::warn()))
                .on_hover_text(t.tr("prefs.trust.not_found.tip"));
        }
    });
}
