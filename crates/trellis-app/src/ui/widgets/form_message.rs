//! Messages in forms, in three kinds (`Severity`): a field's message directly under the field,
//! and a form's message (a job's failure, a refusal, a notice) in a tinted box above the
//! footer. A field's error or warning waits until the user has edited that field or tried to
//! submit (`state::touched::Touched`); information always shows.

use super::super::theme::color;
use super::severity::{self, Severity};
use crate::state::touched::Touched;
use eframe::egui::{self, CornerRadius, FontId, Response, RichText, Sense, Stroke, Ui, vec2};
use std::borrow::Cow;

/// A field message's mark and text size.
const FIELD_MARK: f32 = 12.0;
const FIELD_TEXT: f32 = 12.5;
/// A form message's mark.
const FORM_MARK: f32 = 14.0;

/// What a message says and how serious it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message<'a> {
    pub kind: Severity,
    pub text: Cow<'a, str>,
}

impl<'a> Message<'a> {
    pub fn error(text: impl Into<Cow<'a, str>>) -> Self {
        Self { kind: Severity::Error, text: text.into() }
    }

    pub fn warning(text: impl Into<Cow<'a, str>>) -> Self {
        Self { kind: Severity::Warning, text: text.into() }
    }

    pub fn info(text: impl Into<Cow<'a, str>>) -> Self {
        Self { kind: Severity::Info, text: text.into() }
    }
}

/// Marks `field` as edited when its widget `r` changed.
pub fn track(touched: &mut Touched, field: &'static str, r: &Response) {
    if r.changed() {
        touched.mark(field);
    }
}

/// `msg` under the field named `field`: information always, an error or warning once the field
/// was edited or a submit was tried.
pub fn field(ui: &mut Ui, touched: &Touched, field: &str, msg: Option<Message>) {
    let Some(msg) = msg else { return };
    if msg.kind != Severity::Info && !touched.shows(field) {
        return;
    }
    ui.add_space(2.0 - ui.spacing().item_spacing.y);
    let row_h = ui.fonts(|f| f.row_height(&FontId::proportional(FIELD_TEXT)));
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        let (rect, _) = ui.allocate_exact_size(vec2(FIELD_MARK, row_h), Sense::hover());
        severity::paint(ui.painter(), rect.center(), FIELD_MARK, msg.kind, msg.kind.color());
        let text = RichText::new(msg.text.as_ref()).size(FIELD_TEXT).color(msg.kind.text_color());
        ui.add(egui::Label::new(text).wrap());
    });
}

/// `msg` for the whole form, in a box tinted with its kind's colour.
pub fn form(ui: &mut Ui, msg: Message) {
    let tint = msg.kind.color();
    egui::Frame::new()
        .fill(tint.gamma_multiply(0.12))
        .stroke(Stroke::new(1.0_f32, tint.gamma_multiply(0.5)))
        .corner_radius(CornerRadius::same(4))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_top(|ui| {
                let row_h = ui.text_style_height(&egui::TextStyle::Body);
                let (rect, _) = ui.allocate_exact_size(vec2(FORM_MARK, row_h), Sense::hover());
                severity::paint(ui.painter(), rect.center(), FORM_MARK, msg.kind, tint);
                ui.add(egui::Label::new(RichText::new(msg.text.as_ref()).color(color::text())).wrap());
            });
        });
}
