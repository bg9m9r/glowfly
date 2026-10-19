//! The issue panel, in the pull request panel's place: a header with the title and links and a
//! line with its type, author and time; the details, linked branches and actions on the left
//! (`info`) and the description and comments in the middle (`conversation`). The Create issue
//! dialog is `create_dialog` (with its pickers in `picker`).

mod conversation;
pub mod create_dialog;
mod info;
mod picker;

use super::theme::color;
use super::widgets::{self, Icon};
use crate::msg::{IssueMsg, Msg};
use crate::state::AppState;
use crate::state::issue::IssueView;
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use eframe::egui::{self, Align, Layout, Margin, RichText, Sense, Ui, vec2};
use trellis_core::i18n::Lang;
use trellis_core::text::printable;

pub fn show(ui: &mut Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let Some(view) = &st.issue else { return };
    let open_label = st.forge.as_ref().map_or("Open in browser", |f| f.open_issue_label());
    ui.add_space(6.0);
    header(ui, st.settings.lang(), view, open_label, msgs);
    ui.horizontal(|ui| {
        ui.add_space(32.0);
        let line = printable(&view.kind_line(&st.settings, st.view.utc_offset));
        let text = RichText::new(&line).color(color::weak());
        match view.created() {
            // One tooltip: the whole line above the date.
            Some(c) => {
                let (r, _) = widgets::cut_label(ui, text, ui.available_width());
                let tip = st.settings.date_tooltip(c, st.view.now, st.view.utc_offset);
                r.on_hover_text(format!("{line}\n{tip}"));
            }
            None => {
                widgets::truncate_label(ui, text, ui.available_width());
            }
        }
    });
    ui.add_space(2.0);
    ui.separator();
    let room = ui.available_width();
    crate::ui::panel_size::inner_side(egui::SidePanel::left("issue-side"), crate::state::panel_layout::ISSUE_SIDE, room)
        .frame(egui::Frame::new().fill(color::panel()).inner_margin(Margin::same(super::theme::spacing::PANEL_PAD)))
        .show_inside(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("issue-side-scroll").auto_shrink([false, false]).show(ui, |ui| info::show(ui, st, msgs));
        });
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(color::bg()).inner_margin(Margin::symmetric(16, 10)))
        .show_inside(ui, |ui| conversation::show(ui, st, msgs));
}

/// The icon, mention and title on the left; Close, Copy link and Open in browser on the right.
fn header(ui: &mut Ui, t: Lang, v: &IssueView, open_label: &str, msgs: &mut Vec<Msg>) {
    let url = &v.issue.url;
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(8.0);
            if ui.add(buttons::secondary(t.tr("common.close_esc.button"))).clicked() {
                msgs.push(Msg::Issue(IssueMsg::Close));
            }
            let no_link = format!("The service sent no link for this {}.", v.noun);
            let has_link = !url.is_empty();
            let why = (!has_link).then_some(no_link.as_str());
            if ui.add(buttons::secondary("Copy link").enabled(has_link).why(why)).clicked() {
                msgs.push(Msg::Copy(url.clone()));
            }
            if ui.add(buttons::secondary(open_label).enabled(has_link).why(why)).clicked() {
                msgs.push(Msg::OpenUrl(url.clone()));
            }
            if v.reloading {
                ui.label(RichText::new("Updating…").color(color::weak()));
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.add_space(8.0);
                let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                let tint = if v.closed() { color::progress() } else { color::added() };
                widgets::paint_icon(ui.painter(), rect.center(), Icon::Issue, tint);
                ui.label(RichText::new(&v.mention).color(color::weak()));
                let title = RichText::new(printable(&v.issue.title)).font(type_scale::strong(ui, type_scale::SUBHEAD)).color(color::text());
                ui.add(egui::Label::new(title).truncate());
            });
        });
    });
}
