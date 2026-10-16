//! The release notes dialog (`Modal::ReleaseNotes`): a newer release on offer on top, then the
//! notes bundled with this build, newest first. Esc closes it through the usual `CloseModal`.

use super::theme::color;
use super::widgets::{self, Icon};
use crate::msg::{Msg, UpdateMsg};
use crate::state::release_notes::NotesDialog;
use eframe::egui::{self, RichText, Sense, Stroke, pos2, vec2};
use trellis_core::changelog::{NoteSection, ReleaseNotes, Span, SpanStyle};

const WIDTH: f32 = 600.0;

/// Draws the dialog; true when Close was clicked.
pub fn show(ui: &mut egui::Ui, d: &NotesDialog, msgs: &mut Vec<Msg>) -> bool {
    ui.set_width(WIDTH);
    ui.heading("Release notes");
    ui.label(RichText::new(format!("You're running Trellis {}", d.current)).color(color::weak()));
    ui.add_space(10.0);
    if let Some(offer) = &d.offer {
        egui::Frame::new()
            .fill(color::panel())
            .stroke(Stroke::new(1.0_f32, color::border()))
            .corner_radius(6)
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(WIDTH - 26.0);
                ui.label(RichText::new(format!("Trellis {} is available", offer.version)).strong().size(15.0).color(color::text()));
                if let Some(notes) = &d.offered_notes {
                    ui.add_space(4.0);
                    for s in &notes.sections {
                        section(ui, s, msgs);
                    }
                }
                ui.add_space(8.0);
                let (label, msg) = if offer.asset.is_some() {
                    ("Download", UpdateMsg::Download)
                } else {
                    ("Open release page", UpdateMsg::OpenReleasePage)
                };
                if ui.add(widgets_primary(label)).clicked() {
                    msgs.push(Msg::Updates(msg));
                }
            });
        ui.add_space(10.0);
    }
    if d.releases.is_empty() {
        ui.label(RichText::new("No release notes are bundled with this build.").color(color::weak()));
        ui.add_space(6.0);
        if ui.button("Open releases page").clicked() {
            msgs.push(Msg::Updates(UpdateMsg::OpenReleasePage));
        }
    } else {
        egui::ScrollArea::vertical().id_salt("trellis-release-notes").max_height(480.0).auto_shrink([false, true]).show(ui, |ui| {
            for (i, r) in d.releases.iter().enumerate() {
                if i > 0 {
                    ui.add_space(8.0);
                    ui.separator();
                }
                release(ui, r, d.is_new(r), msgs);
            }
        });
    }
    ui.add_space(12.0);
    let mut close = false;
    ui.horizontal(|ui| {
        close = ui.button("Close").clicked();
        ui.label(RichText::new("Esc to close").color(color::faint()));
    });
    close
}

fn widgets_primary(label: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(label).color(egui::Color32::WHITE))
        .fill(color::primary())
        .stroke(Stroke::new(1.0_f32, color::primary_border()))
}

/// One release: its version (a link to the comparison, when there is one), date, "New" chip
/// and sections.
fn release(ui: &mut egui::Ui, r: &ReleaseNotes, new: bool, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        let version = RichText::new(&r.version).strong().size(15.0);
        match &r.url {
            Some(url) => {
                let resp = ui.add(egui::Label::new(version.color(color::blue())).sense(Sense::click()));
                if resp.on_hover_text(url).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    msgs.push(Msg::OpenUrl(url.clone()));
                }
            }
            None => {
                ui.label(version.color(color::text()));
            }
        }
        if let Some(date) = &r.date {
            ui.label(RichText::new(date).color(color::weak()));
        }
        if new {
            let (rect, _) = ui.allocate_exact_size(vec2(36.0, 18.0), Sense::hover());
            widgets::chip(ui.painter(), rect.left(), rect.center().y, "New", color::green().gamma_multiply(0.25), color::green());
        }
    });
    for s in &r.sections {
        section(ui, s, msgs);
    }
}

fn section(ui: &mut egui::Ui, s: &NoteSection, msgs: &mut Vec<Msg>) {
    if !s.title.is_empty() {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if s.breaking {
                // The font has no warning sign: paint one.
                let (rect, _) = ui.allocate_exact_size(vec2(18.0, 16.0), Sense::hover());
                widgets::paint_icon(ui.painter(), rect.center(), Icon::Warning, color::amber());
            }
            let col = if s.breaking { color::amber() } else { color::strong() };
            ui.label(RichText::new(&s.title).strong().color(col));
        });
    }
    for item in &s.items {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let (rect, _) = ui.allocate_exact_size(vec2(16.0, 14.0), Sense::hover());
            ui.painter().circle_filled(pos2(rect.left() + 6.0, rect.center().y), 2.0, color::weak());
            for span in item {
                span_label(ui, span, msgs);
            }
        });
    }
}

fn span_label(ui: &mut egui::Ui, span: &Span, msgs: &mut Vec<Msg>) {
    match &span.style {
        SpanStyle::Plain => {
            ui.label(RichText::new(&span.text).color(color::text()));
        }
        SpanStyle::Strong => {
            ui.label(RichText::new(&span.text).strong().color(color::strong()));
        }
        SpanStyle::Code => {
            let text = RichText::new(&span.text).monospace().color(color::text()).background_color(color::input());
            ui.label(text);
        }
        SpanStyle::Link(url) => {
            // A commit's short sha is dim; other links read as links.
            let sha = (7..=40).contains(&span.text.len()) && span.text.bytes().all(|b| b.is_ascii_hexdigit());
            let text = RichText::new(&span.text);
            let text = if sha { text.monospace().color(color::faint()) } else { text.color(color::blue()) };
            let resp = ui.add(egui::Label::new(text).sense(Sense::click()));
            if resp.on_hover_text(url).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                msgs.push(Msg::OpenUrl(url.clone()));
            }
        }
    }
}
