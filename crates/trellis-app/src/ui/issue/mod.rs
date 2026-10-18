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
use eframe::egui::{self, Align, Layout, Margin, RichText, Sense, Ui, vec2};
use trellis_core::text::printable;

pub fn show(ui: &mut Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let Some(view) = &st.issue else { return };
    let open_label = st.forge.as_ref().map_or("Open in browser", |f| f.open_issue_label());
    ui.add_space(6.0);
    header(ui, view, open_label, msgs);
    ui.horizontal(|ui| {
        ui.add_space(32.0);
        let line = printable(&view.kind_line(&st.settings, st.view.utc_offset));
        let r = ui.add(egui::Label::new(RichText::new(&line).color(color::weak())).truncate());
        if let Some(c) = view.created() {
            // Keep the full line (the truncated label's own hover) above the date.
            let tip = st.settings.date_tooltip(c, st.view.now, st.view.utc_offset);
            r.on_hover_text(format!("{line}\n{tip}"));
        }
    });
    ui.add_space(2.0);
    ui.separator();
    egui::SidePanel::left("issue-side")
        .resizable(true)
        .default_width(320.0)
        .width_range(260.0..=520.0)
        .frame(egui::Frame::new().fill(color::panel()).inner_margin(Margin::symmetric(10, 8)))
        .show_inside(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("issue-side-scroll").auto_shrink([false, false]).show(ui, |ui| info::show(ui, st, msgs));
        });
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(color::bg()).inner_margin(Margin::symmetric(16, 10)))
        .show_inside(ui, |ui| conversation::show(ui, st, msgs));
}

/// The icon, mention and title on the left; Close, Copy link and Open in browser on the right.
fn header(ui: &mut Ui, v: &IssueView, open_label: &str, msgs: &mut Vec<Msg>) {
    let url = &v.issue.url;
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(8.0);
            if ui.button("Close  (Esc)").clicked() {
                msgs.push(Msg::Issue(IssueMsg::Close));
            }
            let no_link = format!("The service sent no link for this {}.", v.noun);
            let has_link = !url.is_empty();
            if ui.add_enabled(has_link, egui::Button::new("Copy link")).on_disabled_hover_text(&no_link).clicked() {
                msgs.push(Msg::Copy(url.clone()));
            }
            if ui.add_enabled(has_link, egui::Button::new(open_label)).on_disabled_hover_text(&no_link).clicked() {
                msgs.push(Msg::OpenUrl(url.clone()));
            }
            if v.reloading {
                ui.label(RichText::new("Updating…").color(color::weak()));
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.add_space(8.0);
                let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                let tint = if v.closed() { color::purple() } else { color::green() };
                widgets::paint_icon(ui.painter(), rect.center(), Icon::Issue, tint);
                ui.label(RichText::new(&v.mention).color(color::weak()));
                let title = RichText::new(printable(&v.issue.title)).strong().size(15.0).color(color::text());
                ui.add(egui::Label::new(title).truncate());
            });
        });
    });
}
