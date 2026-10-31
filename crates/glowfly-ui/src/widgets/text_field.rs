//! A dialog's text field: 36 px tall with single-line text centred, 10 px of side padding, the
//! faint accent-tinted `color::field_fill()`, and `theme::controls::Field`'s edge and focus ring
//! (the only code that draws them), with a soft accent halo outside the ring while focused
//! (`tokens::focus_halo`). A multiline field gets the same fill, edge and padding, and
//! keeps its rows. Ids, hints, focus, masking and char limits behave as the plain `TextEdit`.
//! Labels, help and messages around a field are `field_group`'s.

use crate::ui::theme::tokens::{self, radius};
use crate::ui::theme::{color, controls};
use crate::ui::widgets::buttons::StyledButton;
use eframe::egui::{self, Align, Id, Key, Margin, Response, Ui, WidgetText, vec2};

/// A field's height, and a multiline field's least height.
pub const HEIGHT: f32 = 36.0;
/// The padding at the text's left and right.
const PAD_X: i8 = 10;
/// The padding above and below a multiline field's text (a single-line field is centred).
const PAD_Y: i8 = 8;

/// A text field to add with `ui.add`; fills the row unless given a `width`.
pub struct TextField<'t> {
    text: &'t mut dyn egui::TextBuffer,
    multiline: Option<usize>,
    id: Option<Id>,
    hint: Option<WidgetText>,
    width: Option<f32>,
    password: bool,
    char_limit: Option<usize>,
}

impl<'t> TextField<'t> {
    /// A single-line field.
    pub fn new(text: &'t mut dyn egui::TextBuffer) -> Self {
        Self { text, multiline: None, id: None, hint: None, width: None, password: false, char_limit: None }
    }

    /// A multiline field `rows` lines tall (at least `HEIGHT`).
    pub fn multiline(text: &'t mut dyn egui::TextBuffer, rows: usize) -> Self {
        Self { multiline: Some(rows), ..Self::new(text) }
    }

    pub fn id(mut self, id: Id) -> Self {
        self.id = Some(id);
        self
    }

    /// Weak text shown while the field is empty.
    pub fn hint(mut self, hint: impl Into<WidgetText>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// The whole field's width, padding included; the default fills the row.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Masks what is typed.
    pub fn password(mut self, password: bool) -> Self {
        self.password = password;
        self
    }

    pub fn char_limit(mut self, limit: usize) -> Self {
        self.char_limit = Some(limit);
        self
    }
}

impl egui::Widget for TextField<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let TextField { text, multiline, id, hint, width, password, char_limit } = self;
        let rows = multiline.unwrap_or(1);
        let edit = if multiline.is_some() { egui::TextEdit::multiline(text) } else { egui::TextEdit::singleline(text) };
        let margin = Margin::symmetric(PAD_X, PAD_Y);
        let inner_w = width.map_or(f32::INFINITY, |w| (w - margin.sum().x).max(0.0));
        let align = if multiline.is_some() { Align::Min } else { Align::Center };
        let mut edit = edit
            .margin(margin)
            .min_size(vec2(0.0, HEIGHT))
            .vertical_align(align)
            .background_color(color::field_fill())
            .desired_width(inner_w)
            .desired_rows(rows)
            .password(password);
        if let Some(id) = id {
            edit = edit.id(id);
        }
        if let Some(hint) = hint {
            edit = edit.hint_text(hint);
        }
        if let Some(n) = char_limit {
            edit = edit.char_limit(n);
        }
        let mut r = ui.add(controls::Field(edit));
        // egui reports the text's rect; the field is the whole box.
        r.rect += margin;
        if r.has_focus() {
            tokens::focus_halo(ui.painter(), r.rect, radius::MD);
        }
        r
    }
}

/// `field` and `button` (Browse…) on one row: the button as tall as the field, the field
/// filling the rest. Returns their responses.
pub fn with_button(ui: &mut Ui, field: TextField<'_>, button: StyledButton<'_>) -> (Response, Response) {
    ui.horizontal(|ui| {
        let button = button.height(HEIGHT);
        let w = ui.available_width() - button.desired_width(ui) - ui.spacing().item_spacing.x;
        let r = ui.add(field.width(w));
        (r, ui.add(button))
    })
    .inner
}

/// Whether the single-line field `r` gave up the focus to Enter this frame (a submit). Read it
/// before focus is requested again, or the lost focus is gone.
pub fn entered(ui: &Ui, r: &Response) -> bool {
    r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter))
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::accesskit::Role;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    #[derive(Default)]
    struct Seen {
        single: f32,
        multi: f32,
        entered: Vec<bool>,
    }

    fn harness() -> Harness<'static, (String, String, Seen)> {
        crate::ui::snapshot_check::builder().with_size(vec2(400.0, 200.0)).build_ui_state(
            |ui, (a, b, seen): &mut (String, String, Seen)| {
                let r = ui.add(TextField::new(a).id(Id::new("one")).hint("Name"));
                crate::ui::access::name_field(&r, "One");
                seen.single = r.rect.height();
                seen.entered.push(entered(ui, &r));
                let r = ui.add(TextField::multiline(b, 3));
                seen.multi = r.rect.height();
            },
            Default::default(),
        )
    }

    #[test]
    fn single_line_is_36_px_and_multiline_taller() {
        let mut h = harness();
        h.run();
        let seen = &h.state().2;
        assert_eq!(seen.single, HEIGHT);
        assert!(seen.multi > HEIGHT, "{}", seen.multi);
    }

    #[test]
    fn keeps_the_id_and_role_of_a_text_edit() {
        let mut h = harness();
        h.run();
        let node = h.get_by_label("One");
        assert_eq!(node.role(), Role::TextInput);
        node.focus();
        h.run();
        assert!(h.ctx.memory(|m| m.has_focus(Id::new("one"))), "the field keeps the id it was given");
    }

    #[test]
    fn entered_only_on_the_frame_enter_left_it() {
        let mut h = harness();
        h.run();
        h.get_by_label("One").focus();
        h.run();
        h.get_by_label("One").type_text("x");
        h.run();
        h.state_mut().2.entered.clear();
        h.press_key(Key::Enter);
        h.run();
        let seen = &h.state().2.entered;
        assert_eq!(seen.iter().filter(|e| **e).count(), 1, "{seen:?}");
        h.state_mut().2.entered.clear();
        h.run();
        assert!(h.state().2.entered.iter().all(|e| !e));
    }
}
