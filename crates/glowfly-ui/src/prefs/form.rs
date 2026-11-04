//! The pieces every Preferences form is built from: a row (a right-aligned label, then the
//! controls with their help underneath, stacked when the content column is narrow), titled
//! sections, labelled checkbox rows, text fields and combos that fit the column, warning lines,
//! and the page's primary button. Help lines are in form_help.rs; the search's revealed row is
//! tinted by reveal.rs.

use crate::access;
use crate::theme::sizes::ControlSize;
use crate::theme::tokens::BORDER_W;
use crate::theme::{color, controls, type_scale};
use crate::widgets::{self, buttons};
use eframe::egui::{self, Color32, RichText, Sense, Stroke, Ui, pos2, vec2};

pub use super::form_help::help;

/// The widest the page's content gets (left-anchored however wide the window), the label
/// column, the gap after it, the widest the field column gets, and the least room it keeps
/// beside the label before the row stacks.
pub const CONTENT_W: f32 = 760.0;
pub const LABEL_W: f32 = 180.0;
pub const GAP: f32 = 18.0;
pub const ROW_W: f32 = CONTENT_W - LABEL_W - GAP;
const MIN_FIELD_W: f32 = 240.0;
/// A one-line row's height: the label sits centred in it.
const LINE_H: f32 = 24.0;

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
/// little room beside the label, the label goes on its own line above the controls. A label too
/// long for its column wraps inside it. The label names the fields, combos and boxes in the row
/// that have no name of their own. The row the search revealed is tinted (reveal.rs).
pub fn row(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    let tint = super::reveal::begin(ui, label);
    let top = ui.cursor().top();
    let lay = row_layout(ui.available_width());
    if lay.stacked {
        let named =
            (!label.is_empty()).then(|| ui.label(RichText::new(label).font(type_scale::font(type_scale::BODY)).color(color::text())).id);
        ui.vertical(|ui| {
            ui.set_max_width(lay.field_w);
            access::with_label(&ui.ctx().clone(), named, || add(ui));
        });
    } else {
        ui.horizontal_top(|ui| {
            let mut job = egui::text::LayoutJob::simple(label.to_owned(), type_scale::font(type_scale::BODY), color::text(), LABEL_W);
            job.halign = egui::Align::RIGHT;
            let galley = ui.painter().layout_job(job);
            let first = galley.rows.first().map_or(0.0, |r| r.height());
            let pad = ((LINE_H - first) / 2.0).max(0.0);
            let (r, resp) = ui.allocate_exact_size(vec2(LABEL_W, (galley.size().y + 2.0 * pad).max(LINE_H)), Sense::hover());
            ui.painter().galley(pos2(r.right(), r.top() + pad), galley, color::text());
            let named = (!label.is_empty()).then(|| {
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, label));
                resp.id
            });
            // The field column starts GAP after the label, item spacing included.
            ui.add_space((GAP - ui.spacing().item_spacing.x).max(0.0));
            ui.vertical(|ui| {
                ui.set_max_width(lay.field_w);
                access::with_label(&ui.ctx().clone(), named, || add(ui));
            });
        });
    }
    super::reveal::end(ui, tint, top);
    ui.add_space(18.0);
}

/// A page's opening sentence, above its first row.
pub fn intro(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).font(type_scale::font(type_scale::BODY)).color(color::text())).wrap());
    ui.add_space(14.0);
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
    let (r, _) = ui.allocate_exact_size(vec2(w, BORDER_W), Sense::hover());
    ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(BORDER_W, color::border()));
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

/// A combo of `options`, named by its row's label unless named after.
pub fn combo<T: PartialEq + Copy>(ui: &mut Ui, id: &str, value: &mut T, options: &[(T, &str)]) {
    combo_response(ui, id, value, options);
}

/// `combo`, returning the closed combo's response (to name it for screen readers).
pub fn combo_response<T: PartialEq + Copy>(ui: &mut Ui, id: &str, value: &mut T, options: &[(T, &str)]) -> egui::Response {
    let current = options.iter().find(|(v, _)| v == value).map(|(_, l)| *l).unwrap_or("");
    let r = egui::ComboBox::from_id_salt(id)
        .selected_text(current)
        .width(fit(ui, 230.0))
        .show_ui(ui, |ui| {
            for (v, label) in options {
                ui.selectable_value(value, *v, *label);
            }
        })
        .response;
    access::from_label(&r);
    r
}

/// A warning line under a row, broken between words at the help measure.
pub fn warn(ui: &mut Ui, text: &str) {
    let lines = super::form_help::wrap(text, super::form_help::MEASURE).join("\n");
    ui.horizontal_top(|ui| {
        warn_icon(ui);
        let r = ui.add(egui::Label::new(RichText::new(lines).size(type_scale::SMALL).color(color::weak())).wrap());
        // Screen readers get the text as written, without the line breaks.
        r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, text));
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

/// A Preferences text field's widest, padding included.
pub const FIELD_W: f32 = 336.0;

pub fn text_field(ui: &mut Ui, value: &mut String, hint: &str) -> egui::Response {
    let w = fit(ui, FIELD_W);
    ui.add(controls::SizedField::new(ControlSize::Regular, egui::TextEdit::singleline(value).hint_text(hint)).width(w))
}

/// The widest a disabled primary's reason gets beside it before it is cut.
const REASON_W: f32 = 280.0;

/// A page's primary button; while disabled, `why` shows as a weak line beside it (pass None when
/// the page already shows that problem above the button). Returns the button's response.
pub fn primary_button(ui: &mut Ui, text: &str, enabled: bool, why: Option<&str>) -> egui::Response {
    with_reason(ui, buttons::primary(text), enabled, why)
}

/// A secondary button at field height, with its reason beside it while disabled, as
/// `primary_button`: for an action in a field's row that isn't the page's main one.
pub fn secondary_button(ui: &mut Ui, text: &str, enabled: bool, why: Option<&str>) -> egui::Response {
    with_reason(ui, buttons::secondary(text).size(ControlSize::Regular), enabled, why)
}

fn with_reason(ui: &mut Ui, button: buttons::StyledButton<'_>, enabled: bool, why: Option<&str>) -> egui::Response {
    let button = button.enabled(enabled).why(why);
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
        assert_eq!(LABEL_W + GAP + ROW_W, CONTENT_W);
        assert_eq!(row_layout(600.0), RowLayout { stacked: false, field_w: 600.0 - LABEL_W - GAP });
        // Under 438 px the field column would be narrower than 240 px: the row stacks.
        assert!(!row_layout(438.0).stacked);
        assert_eq!(row_layout(437.0), RowLayout { stacked: true, field_w: 437.0 });
        assert_eq!(row_layout(300.0), RowLayout { stacked: true, field_w: 300.0 });
        for avail in [0.0, 100.0, 437.0, 438.0, 700.0, 2000.0] {
            let lay = row_layout(avail);
            assert!(lay.field_w <= ROW_W && lay.field_w <= avail, "{avail}: {lay:?}");
        }
    }
}
