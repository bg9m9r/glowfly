//! A dialog's label-field-help group: the label above the field (14 px text, with a weak
//! "Optional" at the row's right end for an optional field), a 4 px gap, the field, and one line
//! under it: the field's message when one may show (`form_message::field`'s rules: information
//! at once, an error or warning once the field was edited or a submit was tried), otherwise the
//! group's weak help. The message takes the help's place, so the layout changes once. 16 px
//! follow the group (12 px of space plus the item spacing).

use super::form_message::{self, Message};
use super::severity::Severity;
use crate::state::touched::Touched;
use crate::ui::access;
use crate::ui::theme::{color, type_scale};
use eframe::egui::{self, Align, Layout, Response, RichText, Ui};
use std::borrow::Cow;

/// The space after a group, before the item spacing.
const AFTER: f32 = 12.0;

/// When a message may show.
enum Gate<'a> {
    /// Once the field named by the key was edited or a submit was tried (errors and warnings).
    Touched(&'a Touched, &'a str),
    /// At once.
    Always,
}

impl Gate<'_> {
    fn shows(&self, m: &Message) -> bool {
        match self {
            Gate::Touched(t, key) => m.kind == Severity::Info || t.shows(key),
            Gate::Always => true,
        }
    }
}

/// A field with its label, help and messages; `show` draws it.
#[derive(Default)]
pub struct FieldGroup<'a> {
    label: Option<Cow<'a, str>>,
    optional: Option<&'a str>,
    help: Option<Cow<'a, str>>,
    messages: Vec<(Message<'a>, Gate<'a>)>,
}

impl<'a> FieldGroup<'a> {
    /// A group labelled `label`, which also becomes the field's accessible name.
    pub fn new(label: impl Into<Cow<'a, str>>) -> Self {
        Self { label: Some(label.into()), ..Self::default() }
    }

    /// Marks the field optional: `text` (`dialog.field.optional`) at the label row's right end.
    pub fn optional(mut self, text: &'a str) -> Self {
        self.optional = Some(text);
        self
    }

    /// The weak line under the field while no message shows.
    pub fn help(mut self, text: impl Into<Cow<'a, str>>) -> Self {
        self.help = Some(text.into());
        self
    }

    /// `msg` under the field, gated as `form_message::field` gates it for the field `key`.
    pub fn message(mut self, touched: &'a Touched, key: &'a str, msg: Option<Message<'a>>) -> Self {
        self.messages.extend(msg.map(|m| (m, Gate::Touched(touched, key))));
        self
    }

    /// Each of `msgs`, gated as `message`.
    pub fn messages(mut self, touched: &'a Touched, key: &'a str, msgs: impl IntoIterator<Item = Message<'a>>) -> Self {
        self.messages.extend(msgs.into_iter().map(|m| (m, Gate::Touched(touched, key))));
        self
    }

    /// A line shown at once, whatever was edited ("Checking…", a status).
    pub fn note(mut self, msg: Option<Message<'a>>) -> Self {
        self.messages.extend(msg.map(|m| (m, Gate::Always)));
        self
    }

    /// Draws the group around the field `add` draws, and returns the field's response.
    pub fn show(self, ui: &mut Ui, add: impl FnOnce(&mut Ui) -> Response) -> Response {
        let FieldGroup { label, optional, help, messages } = self;
        if let Some(text) = label.as_deref() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(text).size(type_scale::BODY).color(color::text()));
                if let Some(o) = optional {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(o).size(type_scale::SMALL).color(color::weak()));
                    });
                }
            });
        }
        let r = add(ui);
        // The field carries the label's text as its name; the label stays a node of its own.
        if let Some(text) = label.as_deref() {
            access::name_field(&r, text);
        }
        let mut shown = false;
        for (m, gate) in messages {
            if gate.shows(&m) {
                shown = true;
                // Already gated: shown as is.
                form_message::field(ui, &Touched::all(), "", Some(m));
            }
        }
        if let (false, Some(help)) = (shown, help) {
            ui.add_space(2.0 - ui.spacing().item_spacing.y);
            ui.add(egui::Label::new(RichText::new(help).size(type_scale::SMALL).color(color::weak())).wrap());
        }
        ui.add_space(AFTER);
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::widgets::text_field::TextField;
    use eframe::egui::accesskit::Role;
    use eframe::egui::vec2;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    const HELP: &str = "Letters, digits and slashes.";
    const ERROR: &str = "Not a valid branch name.";

    /// A group with help and an error, shown with `touched`, or with an info note.
    fn harness(touched: Touched, info: bool) -> Harness<'static, String> {
        crate::ui::snapshot_check::builder().with_size(vec2(400.0, 160.0)).build_ui_state(
            move |ui, text: &mut String| {
                let mut g = FieldGroup::new("Branch name").help(HELP).message(&touched, "name", Some(Message::error(ERROR)));
                if info {
                    g = g.note(Some(Message::info("Checking...")));
                }
                g.show(ui, |ui| ui.add(TextField::new(text)));
            },
            String::new(),
        )
    }

    fn has(h: &Harness<'_, String>, text: &str) -> bool {
        h.query_by_label(text).is_some()
    }

    #[test]
    fn help_shows_while_an_error_waits() {
        let mut h = harness(Touched::default(), false);
        h.run();
        assert!(has(&h, HELP));
        assert!(!has(&h, ERROR), "untouched");
    }

    #[test]
    fn a_touched_error_replaces_the_help() {
        let mut t = Touched::default();
        t.mark("name");
        let mut h = harness(t, false);
        h.run();
        assert!(has(&h, ERROR));
        assert!(!has(&h, HELP));
    }

    #[test]
    fn info_shows_at_once() {
        let mut h = harness(Touched::default(), true);
        h.run();
        assert!(has(&h, "Checking..."));
        assert!(!has(&h, HELP));
    }

    #[test]
    fn the_label_names_the_field() {
        let mut h = harness(Touched::default(), false);
        h.run();
        assert!(h.query_all_by_label("Branch name").any(|n| n.role() == Role::Label), "the visible label");
        assert!(h.query_all_by_label("Branch name").any(|n| n.role() == Role::TextInput), "the field it names");
    }
}
