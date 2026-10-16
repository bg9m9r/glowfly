//! The pull request panel, in place of the graph: a header with the title and links, the pull
//! request's details and changed files on the left (`info`, `files`), and the open file's diff
//! on the right.

mod files;
mod info;

use super::UiCache;
use super::theme::color;
use super::widgets::{self, Icon};
use crate::msg::Msg;
use crate::pull_request::PullRequestMsg;
use crate::state::AppState;
use crate::state::pull_request::PrView;
use eframe::egui::{self, Align, FontId, Layout, Margin, RichText, Sense, Ui, vec2};
use trellis_core::text::printable;

pub fn show(ui: &mut Ui, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    let Some(view) = &st.pull_request else { return };
    let shows_diff = st.diff.as_ref().is_some_and(|(t, _)| view.shows(t));
    ui.add_space(6.0);
    header(ui, view, msgs);
    ui.separator();
    egui::SidePanel::left("pull-request-info")
        .resizable(true)
        .default_width(380.0)
        .width_range(280.0..=600.0)
        .frame(egui::Frame::new().fill(color::panel()).inner_margin(Margin::symmetric(10, 8)))
        .show_inside(ui, |ui| {
            let Some(view) = &st.pull_request else { return };
            egui::ScrollArea::vertical()
                .id_salt("pr-info")
                .max_height(ui.available_height() * 0.5)
                .auto_shrink([false, true])
                .show(ui, |ui| info::show(ui, view));
            ui.separator();
            files::show(ui, st, view, msgs);
        });
    egui::CentralPanel::default().frame(egui::Frame::new().fill(color::bg())).show_inside(ui, |ui| {
        if shows_diff {
            super::diff::show(ui, st, &mut cache.diff_rows, msgs);
        } else {
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(RichText::new("Select a file to see its changes.").color(color::weak()));
            });
        }
    });
}

/// The icon, label, title and Draft chip, the links and Close; below, the author and branches.
fn header(ui: &mut Ui, v: &PrView, msgs: &mut Vec<Msg>) {
    let pr = &v.pr;
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(8.0);
            if ui.button("Close  (Esc)").clicked() {
                msgs.push(Msg::PullRequest(PullRequestMsg::Close));
            }
            let no_link = format!("The service sent no link for this {}.", v.noun);
            let has_link = !pr.url.is_empty();
            if ui.add_enabled(has_link, egui::Button::new("Copy link")).on_disabled_hover_text(&no_link).clicked() {
                msgs.push(Msg::Copy(pr.url.clone()));
            }
            if ui.add_enabled(has_link, egui::Button::new("Open in browser")).on_disabled_hover_text(&no_link).clicked() {
                msgs.push(Msg::OpenUrl(pr.url.clone()));
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.add_space(8.0);
                let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                widgets::paint_icon(ui.painter(), rect.center(), Icon::PullRequest, color::purple());
                ui.label(RichText::new(&v.label).color(color::weak()));
                const DRAFT: &str = "Draft";
                let chip_w = if pr.draft {
                    ui.painter().layout_no_wrap(DRAFT.to_owned(), FontId::proportional(11.5), color::weak()).size().x + 12.0
                } else {
                    0.0
                };
                ui.scope(|ui| {
                    ui.set_max_width((ui.available_width() - chip_w - 8.0).max(40.0));
                    let title = RichText::new(printable(&pr.title)).strong().size(15.0).color(color::text());
                    ui.add(egui::Label::new(title).truncate());
                });
                if pr.draft {
                    let (rect, _) = ui.allocate_exact_size(vec2(chip_w, 20.0), Sense::hover());
                    widgets::chip(ui.painter(), rect.left(), rect.center().y, DRAFT, color::toolbar(), color::weak());
                }
            });
        });
    });
    ui.horizontal(|ui| {
        ui.add_space(32.0);
        let weak = |t: String| RichText::new(t).color(color::weak());
        ui.label(weak(format!("by {} · {}", printable(&pr.author), printable(&pr.source))));
        // Painted: the bundled fonts have no arrow glyph.
        let (rect, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
        widgets::paint_icon(ui.painter(), rect.center(), Icon::Into, color::weak());
        ui.add(egui::Label::new(weak(printable(&pr.target))).truncate());
    });
}
