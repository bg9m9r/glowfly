//! The release notes dialog (`Modal::ReleaseNotes`): a newer release on offer on top, then the
//! notes bundled with this build, newest first. Esc closes it through the usual `CloseModal`.

use super::dialog_frame::{Cx, Dialog, Footer, Size};
use super::theme::color;
use super::widgets::{self, Icon, buttons};
use crate::msg::{Msg, UpdateMsg};
use crate::state::release_notes::NotesDialog;
use crate::ui::theme::type_scale;
use eframe::egui::{self, RichText, Sense, Stroke, pos2, vec2};
use trellis_core::changelog::{NoteSection, ReleaseNotes, Span, SpanStyle};
use trellis_core::i18n::Lang;

/// Draws the dialog (wide, and resizable); true when Close was clicked.
pub fn show(ui: &mut egui::Ui, cx: Cx, d: &NotesDialog, msgs: &mut Vec<Msg>) -> bool {
    let t = cx.t;
    let running = t.trf("release_notes.running", &[("version", &d.current)]);
    let mut dialog = Dialog::new(cx, t.tr("release_notes.title")).subtitle(running).size(Size::Wide).resizable("release-notes").begin(ui);
    dialog.body(ui, |ui| body(ui, t, d, msgs));
    dialog.finish(ui, Footer::new().note(t.tr("release_notes.esc_hint")).close()).dismiss
}

/// The offer, then the notes of each release.
fn body(ui: &mut egui::Ui, t: Lang, d: &NotesDialog, msgs: &mut Vec<Msg>) {
    if let Some(offer) = &d.offer {
        egui::Frame::new()
            .fill(color::panel())
            .stroke(Stroke::new(1.0_f32, color::border()))
            .corner_radius(6)
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let title = t.trf("release_notes.available", &[("version", &offer.version)]);
                ui.label(RichText::new(title).font(type_scale::strong(ui, type_scale::SUBHEAD)).color(color::text()));
                if let Some(notes) = &d.offered_notes {
                    ui.add_space(4.0);
                    for s in &notes.sections {
                        section(ui, s, msgs);
                    }
                }
                ui.add_space(8.0);
                let (label, msg) = if offer.asset.is_some() {
                    (t.tr("updates.download.button"), UpdateMsg::Download)
                } else {
                    (t.tr("updates.open_release_page.button"), UpdateMsg::OpenReleasePage)
                };
                if ui.add(buttons::primary(label)).clicked() {
                    msgs.push(Msg::Updates(msg));
                }
            });
        ui.add_space(10.0);
    }
    if d.releases.is_empty() {
        ui.label(RichText::new(t.tr("release_notes.none")).color(color::weak()));
        ui.add_space(6.0);
        if ui.add(buttons::secondary(t.tr("release_notes.open_releases.button"))).clicked() {
            msgs.push(Msg::Updates(UpdateMsg::OpenReleasePage));
        }
    } else {
        for (i, r) in d.releases.iter().enumerate() {
            if i > 0 {
                ui.add_space(8.0);
                ui.separator();
            }
            release(ui, t, r, d.is_new(r), msgs);
        }
    }
}

/// One release: its version (a link to the comparison, when there is one), date, "New" chip
/// and sections.
fn release(ui: &mut egui::Ui, t: Lang, r: &ReleaseNotes, new: bool, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        let version = RichText::new(&r.version).font(type_scale::strong(ui, type_scale::SUBHEAD));
        match &r.url {
            Some(url) => {
                let resp = ui.add(egui::Label::new(version.color(color::accent())).sense(Sense::click()));
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
            // As wide as the chip draws its text (`widgets::chip`), at least the English slot.
            let text = t.tr("release_notes.new_chip");
            let text_w = ui.painter().layout_no_wrap(text.into(), type_scale::font(type_scale::CAPTION), color::added()).size().x;
            let (rect, _) = ui.allocate_exact_size(vec2(36.0_f32.max(text_w + 12.0), 18.0), Sense::hover());
            widgets::chip(ui.painter(), rect.left(), rect.center().y, text, color::added().gamma_multiply(0.25), color::added());
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
                widgets::paint_icon(ui.painter(), rect.center(), Icon::Warning, color::warn());
            }
            let col = if s.breaking { color::warn() } else { color::strong() };
            ui.label(RichText::new(&s.title).font(type_scale::strong(ui, type_scale::BODY)).color(col));
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
            ui.label(RichText::new(&span.text).font(type_scale::strong(ui, type_scale::BODY)).color(color::strong()));
        }
        SpanStyle::Code => {
            let text = RichText::new(&span.text).monospace().color(color::text()).background_color(color::input());
            ui.label(text);
        }
        SpanStyle::Link(url) => {
            // Every link is in the accent colour; a commit's short sha is also monospace.
            let sha = (7..=40).contains(&span.text.len()) && span.text.bytes().all(|b| b.is_ascii_hexdigit());
            let text = RichText::new(&span.text);
            let text = if sha { text.font(type_scale::mono(type_scale::SMALL)) } else { text }.color(color::accent());
            let resp = ui.add(egui::Label::new(text).sense(Sense::click()));
            if resp.on_hover_text(url).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                msgs.push(Msg::OpenUrl(url.clone()));
            }
        }
    }
}
