//! A pane that is loading or has failed, in the empty state's style (`empty_state.rs`): the
//! centred block with a spinner or the warning triangle, or a compact line for a list inside a
//! panel. An error shows a plain title saying what failed, the error without its kind prefix
//! ("not found: ", ...), Try again when the pane can retry, and Copy details.

use super::empty_state::{Glyph, PaneButton, block};
use super::theme::color;
use super::widgets::buttons;
use super::widgets::severity::{self, Severity};
use crate::msg::Msg;
use crate::ui::theme::type_scale;
use eframe::egui::{self, RichText, Ui, vec2};
use trellis_core::i18n::Lang;

/// The inline spinner's size, and the inline mark's.
const LINE_SPINNER: f32 = 12.0;
const LINE_MARK: f32 = 14.0;

/// The pane is loading: a spinner and `text`, centred.
pub fn loading(ui: &mut Ui, text: &str) {
    block(ui, Glyph::Spinner, RichText::new(text).color(color::weak()), None, &[]);
}

/// A list inside a panel is loading: a small spinner and `text` on one line.
pub fn loading_line(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.add(egui::Spinner::new().size(LINE_SPINNER).color(color::weak()));
        ui.label(RichText::new(text).color(color::weak()));
    });
}

/// What failed (`title`), the error as reported, and the message that tries again.
pub struct Failure<'a> {
    pub title: &'a str,
    pub error: &'a str,
    pub retry: Option<Msg>,
}

impl Failure<'_> {
    /// The error as shown: without its kind prefix, starting with a capital.
    fn message(&self) -> String {
        super::common::capitalize(trellis_core::error::plain_message(self.error).trim())
    }

    /// What Copy details copies: the title and the error as reported.
    fn details(&self) -> String {
        format!("{}\n\n{}", self.title, self.error)
    }
}

/// The pane failed: the warning triangle, `f.title`, the message, Try again (with a retry)
/// and Copy details, centred.
pub fn failed(ui: &mut Ui, t: Lang, f: Failure, msgs: &mut Vec<Msg>) {
    let message = f.message();
    let mut list = Vec::with_capacity(2);
    if f.retry.is_some() {
        list.push(PaneButton { text: t.tr("common.try_again.button"), primary: false });
    }
    list.push(PaneButton { text: t.tr("common.copy_details.button"), primary: false });
    let title = RichText::new(f.title).font(type_scale::strong(ui, type_scale::BODY)).color(color::text());
    let Some(i) = block(ui, Glyph::Severity(Severity::Warning), title, Some(&message), &list) else { return };
    let copy = i + 1 == list.len();
    match f.retry {
        Some(retry) if !copy => msgs.push(retry),
        _ => msgs.push(Msg::Copy(f.details())),
    }
}

/// A list inside a panel failed: a small triangle and `f.title`, the message under it, and
/// small Try again (with a retry) and Copy details buttons.
pub fn failed_line(ui: &mut Ui, t: Lang, f: Failure, msgs: &mut Vec<Msg>) {
    let message = f.message();
    let (mut again, mut copy) = (false, false);
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(LINE_MARK, LINE_MARK), egui::Sense::hover());
            severity::paint(ui.painter(), rect.center(), LINE_MARK, Severity::Warning, color::warn());
            ui.label(RichText::new(f.title).font(type_scale::strong(ui, type_scale::BODY)).color(color::text()));
        });
        ui.add(egui::Label::new(RichText::new(&message).color(color::weak())).wrap());
        ui.horizontal(|ui| {
            if f.retry.is_some() {
                again = ui.add(buttons::secondary(t.tr("common.try_again.button")).small()).clicked();
            }
            copy = ui.add(buttons::secondary(t.tr("common.copy_details.button")).small()).clicked();
        });
    });
    if copy {
        msgs.push(Msg::Copy(f.details()));
    } else if again && let Some(retry) = f.retry {
        msgs.push(retry);
    }
}
