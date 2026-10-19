//! The fixed rows of a list that recreates merges: a label row (a "label" chip and the commit
//! it marks), a reset row (a "reset" chip and where the next line starts), and the merge dot a
//! merge row shows in place of the grip.

use super::commit_row::{paint_back, paint_chip};
use super::rows::{BUTTONS_W, GRIP_X, SHA_X, Shade};
use crate::ui::theme::color;
use crate::ui::widgets::paint_cut_text;
use eframe::egui::{Color32, FontId, Rect, Ui, pos2};

fn paint_fixed(ui: &Ui, rect: Rect, chip: &str, chip_color: Color32, text: &str, shade: Shade) {
    paint_back(ui, rect, shade, false);
    paint_chip(ui, rect, chip, chip_color);
    let x = rect.left() + SHA_X;
    let max_w = rect.right() - BUTTONS_W - 16.0 - x;
    paint_cut_text(ui.painter(), pos2(x, rect.center().y), text, FontId::proportional(13.0), color::weak(), max_w);
}

/// A label row: "at-1a2b3c4: marks 1a2b3c4 Summary for a later step".
pub(super) fn paint_label(ui: &Ui, rect: Rect, text: &str, shade: Shade) {
    paint_fixed(ui, rect, "label", color::weak(), text, shade);
}

/// A reset row: "Continue from …".
pub(super) fn paint_reset(ui: &Ui, rect: Rect, text: &str, shade: Shade) {
    paint_fixed(ui, rect, "reset", color::primary(), text, shade);
}

/// A merge row's dot where other rows have their grip, like the graph's merge nodes.
pub(super) fn paint_merge_mark(ui: &Ui, rect: Rect, dropped: bool) {
    let c = if dropped { color::faint() } else { color::purple() };
    ui.painter().circle_filled(pos2(rect.left() + GRIP_X, rect.center().y), 5.0, c);
}
