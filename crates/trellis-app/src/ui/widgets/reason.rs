//! Disabled actions that say why on screen, not only on hover, always in one weak line
//! (`color::weak()`, never red or amber), cut with "…" and whole on hover when it doesn't fit:
//!
//! - `beside`: after a button in a row (the conflict header's Save, the WIP banner, the
//!   preference pages' primaries);
//! - `line`: a whole-width row under a button, its height kept whether or not there is a reason
//!   (the commit box);
//! - `above` and `in_row`: a dialog footer's reason, in the row's free space before the dismiss
//!   button, or on its own row above the buttons when the row has less than `IN_ROW_MIN` free;
//! - `menu_button` (in `menu_reason.rs`, re-exported here): a menu item with its label greyed
//!   and a painted info mark when it is disabled, the reason in the shared tooltip on hover or
//!   keyboard focus, or "Set up…" after the label of an entry that opens the setting it needs.

use super::super::theme::{color, type_scale};
use super::truncate::truncate_label;
use eframe::egui::{Align, Direction, Layout, Response, RichText, Sense, TextStyle, TextWrapMode, Ui, UiBuilder, WidgetText, vec2};

pub use super::menu_reason::{ItemState, menu_button};

/// The reason line's text size, and the height a `line` always takes.
pub const LINE_SIZE: f32 = type_scale::SMALL;
pub const LINE_H: f32 = 18.0;
/// The least free space in a dialog footer's row for its reason to go there, and the space
/// between the reason and the dismiss button.
pub const IN_ROW_MIN: f32 = 140.0;
pub const ROW_GAP: f32 = 12.0;
/// The height of a dialog footer's buttons, which the reason in their row is centred on.
const FOOTER_ROW_H: f32 = 28.0;

fn text(why: &str) -> RichText {
    RichText::new(why).size(LINE_SIZE).color(color::weak())
}

/// `why` as a weak label after a button, cut to `max_w` (and the room left) with the whole text
/// on hover when cut.
pub fn beside(ui: &mut Ui, why: &str, max_w: f32) -> Response {
    truncate_label(ui, text(why), max_w.min(ui.available_width()))
}

/// A whole-width row `LINE_H` high with `why` in it, at the left (`Align::Min`) or centred; just
/// the space when there is no reason, so the caller's height doesn't change.
pub fn line(ui: &mut Ui, why: Option<&str>, align: Align) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), LINE_H), Sense::hover());
    let Some(why) = why else { return };
    let layout = match align {
        Align::Center => Layout::centered_and_justified(Direction::TopDown),
        _ => Layout::left_to_right(Align::Center),
    };
    let mut child = ui.new_child(UiBuilder::new().max_rect(rect).layout(layout));
    beside(&mut child, why, rect.width());
}

/// `why` on its own right-aligned row, above a dialog footer's buttons.
pub fn above(ui: &mut Ui, why: &str) {
    let w = ui.available_width();
    ui.allocate_ui_with_layout(vec2(w, LINE_H), Layout::right_to_left(Align::Center), |ui| {
        beside(ui, why, w);
    });
    ui.add_space(4.0);
}

/// Whether a footer row with `room` free takes its reason (else it goes `above`).
pub fn fits_in_row(room: f32) -> bool {
    room >= IN_ROW_MIN
}

/// Fills a row's `gap` (the free space before the dismiss button) with `why` right-aligned in
/// it, `ROW_GAP` before the next widget.
pub fn in_row(ui: &mut Ui, why: &str, gap: f32) {
    let spacing = ui.spacing().item_spacing.x;
    let max = (gap - ROW_GAP).max(0.0);
    let galley = WidgetText::from(text(why)).into_galley(ui, Some(TextWrapMode::Truncate), max, TextStyle::Body);
    let w = galley.size().x;
    ui.add_space((gap - ROW_GAP - w).max(0.0));
    // Centred on the buttons that follow, not on the row's height so far.
    ui.allocate_ui_with_layout(vec2(w, FOOTER_ROW_H), Layout::left_to_right(Align::Center), |ui| {
        truncate_label(ui, WidgetText::Galley(galley), max);
    });
    ui.add_space((ROW_GAP - spacing).max(0.0));
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;
    use trellis_core::i18n::Lang;

    #[test]
    fn rows_fit_their_reason_by_room_only() {
        assert!(fits_in_row(IN_ROW_MIN) && !fits_in_row(IN_ROW_MIN - 1.0));
    }

    #[test]
    fn menu_items_and_their_setup_state_are_reachable_from_here() {
        let mut h = Harness::new_ui(|ui| {
            menu_button(ui, "Open", "", ItemState::setup(Lang::En, "Opens the page"));
        });
        h.run();
        assert!(!h.get_by_label("Open").is_disabled());
    }

    #[test]
    fn a_line_keeps_its_height_without_a_reason() {
        let mut heights = Vec::new();
        for why in [None, Some("Enter a summary")] {
            let mut h = Harness::new_ui(|ui| {
                let top = ui.cursor().top();
                line(ui, why, Align::Center);
                heights.push(ui.cursor().top() - top);
            });
            h.run();
            assert_eq!(h.query_by_label("Enter a summary").is_some(), why.is_some());
        }
        assert!(heights.windows(2).all(|w| w[0] == w[1]), "{heights:?}");
    }
}
