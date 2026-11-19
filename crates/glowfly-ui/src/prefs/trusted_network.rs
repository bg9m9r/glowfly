//! Preferences > Trusted repositories, under the decisions: the folders opened with Open anyway,
//! in the decisions' own list style. Each row is the folder's name with Forget at the right and,
//! weak and small under it, its path (cut in the middle, whole on hover) and the network places
//! its Git data may point to. Nothing is drawn while there is none (or the filter matches none).

use crate::theme::sizes::ControlSize;
use crate::theme::tokens::radius;
use crate::theme::{color, type_scale};
use crate::widgets::buttons;
use crate::widgets::truncate::{cut_path_in, truncate_label};
use eframe::egui::{self, Align, CornerRadius, FontId, Layout, RichText, Stroke, Ui, WidgetInfo, WidgetType};
use glowfly_core::i18n::Lang;
use glowfly_core::native_path;
use glowfly_model::msg::{Msg, TrustMsg};
use glowfly_model::state::trust_list::{NetworkRow, network_shown};

/// Room kept at the right of the name for the button.
const ACTIONS_W: f32 = 120.0;

/// The section, with the rows `filter` matches.
pub(super) fn section(ui: &mut Ui, t: Lang, rows: &[NetworkRow], filter: &str, msgs: &mut Vec<Msg>) {
    let listed = network_shown(rows, filter);
    if listed.is_empty() {
        return;
    }
    ui.add_space(18.0);
    let heading = RichText::new(t.tr("prefs.trust.network.label")).font(type_scale::strong(ui, type_scale::SMALL)).color(color::weak());
    ui.label(heading).on_hover_text(t.tr("prefs.trust.network.help"));
    ui.add_space(4.0);
    egui::Frame::new()
        .fill(color::panel())
        .stroke(Stroke::new(1.0_f32, color::border()))
        .corner_radius(CornerRadius::same(radius::LG))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            for (i, r) in listed.into_iter().enumerate() {
                if i > 0 {
                    let x = ui.max_rect().x_range();
                    let y = ui.cursor().top();
                    ui.painter().hline(x, y, Stroke::new(1.0_f32, color::border()));
                }
                row(ui, t, r, msgs);
            }
        });
}

fn row(ui: &mut Ui, t: Lang, r: &NetworkRow, msgs: &mut Vec<Msg>) {
    egui::Frame::new().inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.horizontal(|ui| {
            ui.set_min_height(24.0);
            let name = RichText::new(&r.name).font(type_scale::strong(ui, type_scale::BODY)).color(color::text());
            truncate_label(ui, name, (ui.available_width() - ACTIONS_W).max(80.0));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let access = t.trf("prefs.trust.network.forget.name", &[("name", &r.name)]);
                let button = buttons::secondary(t.tr("prefs.trust.forget.button"))
                    .size(ControlSize::Small)
                    .access_name(&access)
                    .tip(t.tr("prefs.trust.network.forget.tip"));
                if ui.add(button).clicked() {
                    msgs.push(Msg::Trust(TrustMsg::ForgetNetworkPlaces(r.path.clone())));
                }
            });
        });
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
            ui.label(RichText::new(format!("· {}", r.places.join(", "))).font(font).color(color::weak()));
        });
    });
}
