//! Field sizes: the padding of inline (Regular) and dialog (Large) fields and where a
//! single-line field's text sits; `SizedField`.

use super::controls::Field;
use super::sizes::{self, ControlSize};
use eframe::egui::{self, FontId, Margin, Response, Ui, vec2};

/// A field's padding at the text's left and right, in both field sizes.
pub const FIELD_PAD_X: f32 = 10.0;

/// The least padding above and below a field's text. A single-line field sets its text where
/// `sizes::text_pos` would (`line_field_margin`), so this is only its least room (and grows the
/// field with a larger font); a multiline field's rows start below it.
pub(super) const fn field_pad_y(size: ControlSize, multiline: bool) -> f32 {
    match (size, multiline) {
        (ControlSize::Large, _) => 8.0,
        (_, true) => 6.0,
        (_, false) => 4.0,
    }
}

/// A field's padding in `size`, with `trailing` px right of the text instead of the side
/// padding (room for a button painted inside the field).
pub fn field_margin(size: ControlSize, multiline: bool, trailing: Option<f32>) -> Margin {
    let (x, y) = (FIELD_PAD_X as i8, field_pad_y(size, multiline) as i8);
    Margin { left: x, right: trailing.map_or(x, |t| t as i8), top: y, bottom: y }
}

/// A single-line field's padding in `size` for text `row_h` tall: the text's top where
/// `sizes::text_pos` puts it in the field (a whole point at or above the centre), never under
/// the least padding; `trailing` as `field_margin`.
pub fn line_field_margin(size: ControlSize, row_h: f32, trailing: Option<f32>) -> Margin {
    let least = field_pad_y(size, false);
    let top = sizes::centred_on(size.metrics().h / 2.0, row_h).max(least);
    Margin { top: top as i8, ..field_margin(size, false, trailing) }
}

/// The text's width in a field `outer` px wide (with the side padding).
pub fn field_text_width(outer: f32) -> f32 {
    (outer - 2.0 * FIELD_PAD_X).max(0.0)
}

/// A [`Field`] at a control size: Regular (28 px: the inline fields, such as filters, find, the
/// commit message and Preferences forms) or Large (36 px: dialog fields). Its padding comes
/// from the size, never from the view, and a single-line field sets its text where
/// `sizes::text_pos` would. `width` is the whole field's; the response's rect is the whole
/// field, padding included (egui's own is the text's).
pub struct SizedField<'t> {
    edit: egui::TextEdit<'t>,
    size: ControlSize,
    multiline: bool,
    trailing: Option<f32>,
    width: Option<f32>,
    font: Option<FontId>,
}

impl<'t> SizedField<'t> {
    /// `edit` at `size`, Regular or Large; it fills the row unless given a `width`.
    pub fn new(size: ControlSize, edit: egui::TextEdit<'t>) -> Self {
        debug_assert!(matches!(size, ControlSize::Regular | ControlSize::Large), "a field is Regular or Large, not {size:?}");
        Self { edit, size, multiline: false, trailing: None, width: None, font: None }
    }

    /// `edit` is multiline: its rows start at the top padding.
    pub fn multiline(mut self) -> Self {
        self.multiline = true;
        self
    }

    /// The whole field's width, padding included.
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    /// Keeps `w` px right of the text instead of the side padding, for a button painted inside
    /// the field.
    pub fn trailing_room(mut self, w: f32) -> Self {
        self.trailing = Some(w);
        self
    }

    /// The edit's font; set it here, not on the TextEdit, so the text is centred for its height.
    pub fn font(mut self, f: FontId) -> Self {
        self.font = Some(f);
        self
    }
}

impl egui::Widget for SizedField<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let SizedField { edit, size, multiline, trailing, width, font } = self;
        let margin = if multiline {
            field_margin(size, true, trailing)
        } else {
            let line = font.clone().unwrap_or_else(|| egui::FontSelection::Default.resolve(ui.style()));
            line_field_margin(size, ui.fonts(|f| f.row_height(&line)), trailing)
        };
        let inner = width.map_or(f32::INFINITY, |w| (w - margin.sum().x).max(0.0));
        let mut edit = edit.margin(margin).min_size(vec2(0.0, size.metrics().h)).desired_width(inner);
        if let Some(f) = font {
            edit = edit.font(f);
        }
        let mut r = ui.add(Field(edit));
        r.rect += margin;
        r
    }
}
