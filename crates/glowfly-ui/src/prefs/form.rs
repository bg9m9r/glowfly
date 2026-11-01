//! The pieces every Preferences form is built from: a row (a right-aligned label, then the
//! controls with their help underneath, stacked when the content column is narrow), titled
//! sections, labelled checkbox rows, text fields and combos that fit the column, help and
//! warning lines, and the page's primary button.

use crate::ui::theme::{color, controls, type_scale};
use crate::ui::widgets::{self, buttons};
use eframe::egui::{self, Align2, Color32, RichText, Sense, Stroke, Ui, pos2, vec2};

/// The label column, the gap after it, the widest the field column gets, and the least room
/// it keeps beside the label before the row stacks.
pub const LABEL_W: f32 = 240.0;
pub const GAP: f32 = 18.0;
pub const ROW_W: f32 = 480.0;
const MIN_FIELD_W: f32 = 220.0;

/// How a row fits `avail` points: side by side, or the label over the controls.
#[derive(Clone, Copy, Debug, PartialEq)]
struct RowLayout {
    pub stacked: bool,
    pub field_w: f32,
}

fn row_layout(avail: f32) -> RowLayout {
    let field_w = (avail - LABEL_W - GAP).min(ROW_W);
    if field_w < MIN_FIELD_W {
        RowLayout { stacked: true, field_w: avail.clamp(0.0, ROW_W) }
    } else {
        RowLayout { stacked: false, field_w }
    }
}

/// One form row: right-aligned label, then the control(s) and help text underneath. With too
/// little room beside the label, the label goes on its own line above the controls.
pub fn row(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    let lay = row_layout(ui.available_width());
    if lay.stacked {
        if !label.is_empty() {
            ui.label(RichText::new(label).font(type_scale::font(type_scale::BODY)).color(color::text()));
        }
        ui.vertical(|ui| {
            ui.set_max_width(lay.field_w);
            add(ui);
        });
    } else {
        ui.horizontal_top(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(LABEL_W, 24.0), Sense::hover());
            ui.painter().text(
                pos2(r.right(), r.center().y),
                Align2::RIGHT_CENTER,
                label,
                type_scale::font(type_scale::BODY),
                color::text(),
            );
            // The field column starts GAP after the label, item spacing included.
            ui.add_space((GAP - ui.spacing().item_spacing.x).max(0.0));
            ui.vertical(|ui| {
                ui.set_max_width(lay.field_w);
                add(ui);
            });
        });
    }
    ui.add_space(18.0);
}

/// Where the field column starts, from the left of `ui`: after the label, or at the left edge
/// when rows stack.
pub fn field_left(ui: &Ui) -> f32 {
    if row_layout(ui.available_width()).stacked { 0.0 } else { LABEL_W + GAP }
}

/// The field column's width in `ui`.
pub fn field_width(ui: &Ui) -> f32 {
    row_layout(ui.available_width()).field_w
}

/// `w`, or less when `ui` has less room.
pub fn fit(ui: &Ui, w: f32) -> f32 {
    w.min(ui.available_width())
}

/// A titled section: its header in the shared section style over a rule as wide as the form.
pub fn section(ui: &mut Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(type_scale::section_text(ui, title));
    let w = fit(ui, LABEL_W + GAP + ROW_W);
    let (r, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
    ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(1.0_f32, color::border()));
    ui.add_space(10.0);
}

/// A row holding one checkbox with its short text after the box and, under it, its help.
pub fn check_row(ui: &mut Ui, label: &str, value: &mut bool, check: &str, help_text: Option<&str>) -> egui::Response {
    let mut out = None;
    row(ui, label, |ui| {
        out = Some(controls::checkbox(ui, value, check));
        if let Some(h) = help_text {
            help(ui, h);
        }
    });
    out.expect("the row draws its contents")
}

pub fn combo<T: PartialEq + Copy>(ui: &mut Ui, id: &str, value: &mut T, options: &[(T, &str)]) {
    let current = options.iter().find(|(v, _)| v == value).map(|(_, l)| *l).unwrap_or("");
    egui::ComboBox::from_id_salt(id).selected_text(current).width(fit(ui, 230.0)).show_ui(ui, |ui| {
        for (v, label) in options {
            ui.selectable_value(value, *v, *label);
        }
    });
}

pub fn help(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).size(type_scale::SMALL).color(color::weak())).wrap());
}

pub fn warn(ui: &mut Ui, text: &str) {
    ui.horizontal_top(|ui| {
        warn_icon(ui);
        ui.add(egui::Label::new(RichText::new(text).size(type_scale::SMALL).color(color::weak())).wrap());
    });
}

/// `warn` with `then` (a link, say) inline after the text, wrapping with it.
pub fn warn_then(ui: &mut Ui, text: &str, then: impl FnOnce(&mut Ui)) {
    ui.horizontal_wrapped(|ui| {
        warn_icon(ui);
        // Not selectable: a wrapped label's rect spans its rows, the end of the last one too,
        // and would take the pointer from `then`.
        ui.add(egui::Label::new(RichText::new(text).size(type_scale::SMALL).color(color::weak())).selectable(false));
        then(ui);
    });
}

/// The warning triangle in front of a warning line.
fn warn_icon(ui: &mut Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(12.0, 14.0), Sense::hover());
    let c = r.center();
    ui.painter().add(egui::Shape::convex_polygon(
        vec![pos2(c.x, c.y - 5.0), pos2(c.x + 5.5, c.y + 4.5), pos2(c.x - 5.5, c.y + 4.5)],
        Color32::TRANSPARENT,
        Stroke::new(1.3_f32, color::warn()),
    ));
}

pub fn caption(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        ui.label(RichText::new(text).size(type_scale::SMALL).color(color::weak()));
    });
    ui.add_space(4.0);
}

pub fn text_field(ui: &mut Ui, value: &mut String, hint: &str) -> egui::Response {
    let w = fit(ui, 320.0);
    controls::field(ui, egui::TextEdit::singleline(value).hint_text(hint).desired_width(w).margin(vec2(8.0, 5.0)))
}

/// The widest a disabled primary's reason gets beside it before it is cut.
const REASON_W: f32 = 280.0;

/// A page's primary button; while disabled, `why` shows as a weak line beside it (pass None when
/// the page already shows that problem above the button). Returns the button's response.
pub fn primary_button(ui: &mut Ui, text: &str, enabled: bool, why: Option<&str>) -> egui::Response {
    let button = buttons::primary(text).enabled(enabled).why(why);
    let Some(why) = why.filter(|_| !enabled) else { return ui.add(button) };
    let with_reason = |ui: &mut Ui| {
        let r = ui.add(button);
        widgets::reason::beside(ui, why, REASON_W);
        r
    };
    // Beside it: in the caller's row, or in a row of its own.
    if ui.layout().is_horizontal() { with_reason(ui) } else { ui.horizontal(with_reason).inner }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_fit_the_content_column() {
        assert_eq!(row_layout(900.0), RowLayout { stacked: false, field_w: ROW_W });
        assert_eq!(row_layout(600.0), RowLayout { stacked: false, field_w: 600.0 - LABEL_W - GAP });
        // Under 478 px the field column would be narrower than 220 px: the row stacks.
        assert!(!row_layout(478.0).stacked);
        assert_eq!(row_layout(477.0), RowLayout { stacked: true, field_w: 477.0 });
        assert_eq!(row_layout(300.0), RowLayout { stacked: true, field_w: 300.0 });
        for avail in [0.0, 100.0, 477.0, 478.0, 700.0, 2000.0] {
            let lay = row_layout(avail);
            assert!(lay.field_w <= ROW_W && lay.field_w <= avail, "{avail}: {lay:?}");
        }
    }
}
