//! Disabled actions that say why on screen, not only on hover, always in one weak line
//! (`color::weak()`, never red or amber), cut with "…" and whole on hover when it doesn't fit:
//!
//! - `beside`: after a button in a row (the conflict header's Save, the WIP banner, the
//!   preference pages' primaries);
//! - `line`: a whole-width row under a button, its height kept whether or not there is a reason
//!   (the selection summary);
//! - `wrapped`: `line` for a reason that wraps onto as many `LINE_H` rows as it needs (`lines`)
//!   and is never cut (the commit box);
//! - `above` and `in_row`: a dialog footer's reason, in the row's free space before the dismiss
//!   button, or on its own row above the buttons when the row has less than `IN_ROW_MIN` free;
//! - `menu_button` (in `menu_reason.rs`, re-exported here): a menu item with its label greyed
//!   and a painted info mark when it is disabled, the reason in the shared tooltip on hover or
//!   keyboard focus, or "Set up…" after the label of an entry that opens the setting it needs.

use super::super::theme::{color, sizes, type_scale};
use super::truncate::truncate_label;
use eframe::egui::text::LayoutJob;
use eframe::egui::{
    self, Align, Direction, Galley, Layout, Response, RichText, Sense, TextStyle, TextWrapMode, Ui, UiBuilder, WidgetText, pos2, vec2,
};
use std::sync::Arc;

pub use super::menu_reason::{ItemState, menu_button};

/// The reason line's text size, and the height a `line` always takes.
const LINE_SIZE: f32 = type_scale::SMALL;
pub const LINE_H: f32 = 18.0;
/// The least free space in a dialog footer's row for its reason to go there, and the space
/// between the reason and the dismiss button.
const IN_ROW_MIN: f32 = 140.0;
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

/// `why` wrapped to `width` and centred, in the reason style.
fn wrapped_galley(ui: &Ui, why: &str, width: f32) -> Arc<Galley> {
    let mut job = LayoutJob::simple(why.to_owned(), type_scale::font(LINE_SIZE), color::weak(), width.max(0.0));
    job.halign = Align::Center;
    ui.fonts(|f| f.layout_job(job))
}

/// How many lines `why` takes wrapped to `width` (one, with no reason): `wrapped`'s rows.
pub fn lines(ui: &Ui, why: Option<&str>, width: f32) -> usize {
    why.map_or(1, |why| wrapped_galley(ui, why, width).rows.len().max(1))
}

/// A whole-width block of `lines` rows `LINE_H` high with `why` wrapped and centred in it, never
/// cut; just the space when there is no reason, so the caller's height doesn't change.
pub fn wrapped(ui: &mut Ui, why: Option<&str>, lines: usize) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), LINE_H * lines.max(1) as f32), Sense::hover());
    let Some(why) = why else { return };
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), why));
    let galley = wrapped_galley(ui, why, rect.width());
    // A centred job lays its rows out around x = 0.
    let at = pos2(rect.center().x, sizes::centred_top(rect, galley.size().y));
    ui.painter().galley(at, galley, color::weak());
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
    use egui_kittest::kittest::Queryable;
    use glowfly_core::i18n::Lang;

    #[test]
    fn rows_fit_their_reason_by_room_only() {
        assert!(fits_in_row(IN_ROW_MIN) && !fits_in_row(IN_ROW_MIN - 1.0));
    }

    #[test]
    fn menu_items_and_their_setup_state_are_reachable_from_here() {
        let mut h = crate::ui::snapshot_check::builder().build_ui(|ui| {
            menu_button(ui, "Open", "", ItemState::setup(Lang::En, "Opens the page"));
        });
        h.run();
        assert!(!h.get_by_label("Open").is_disabled());
    }

    #[test]
    fn a_long_reason_wraps_whole_onto_more_lines() {
        let reason = "Noch ist nichts bereitgestellt. Stelle zuerst eine Datei oder einen Hunk bereit.";
        let mut seen = Vec::new();
        let mut h = crate::ui::snapshot_check::builder().build_ui(|ui| {
            crate::ui::theme::apply_theme(ui.ctx());
            assert_eq!(lines(ui, None, 300.0), 1, "no reason keeps one line");
            assert_eq!(lines(ui, Some("Enter a summary"), 300.0), 1);
            let n = lines(ui, Some(reason), 300.0);
            let g = wrapped_galley(ui, reason, 300.0);
            assert!(!g.elided, "never cut");
            assert!(g.size().y <= LINE_H * n as f32, "{} rows fit {n} lines", g.rows.len());
            let top = ui.cursor().top();
            wrapped(ui, Some(reason), n);
            seen.push((n, ui.cursor().top() - top));
        });
        h.run();
        assert!(h.query_by_label(reason).is_some(), "named by the whole reason");
        drop(h);
        let (n, h_px) = seen[0];
        assert_eq!(n, 2, "the German reason takes two lines at 300 px");
        assert!(h_px >= 2.0 * LINE_H, "{h_px}");
    }

    #[test]
    fn a_line_keeps_its_height_without_a_reason() {
        let mut heights = Vec::new();
        for why in [None, Some("Enter a summary")] {
            let mut h = crate::ui::snapshot_check::builder().build_ui(|ui| {
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
