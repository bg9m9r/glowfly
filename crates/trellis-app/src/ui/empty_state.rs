//! A calm placeholder for a pane with nothing to draw (a binary file's diff, say): an outline
//! icon, a title and a one-line hint, centred and set about a third of the way down the space
//! left, with an optional button under it. The icons are painted as shapes in the weak colour.

use super::theme::color;
use eframe::egui::{
    self, Align, Color32, CornerRadius, FontId, Layout, Painter, Pos2, Rect, Response, RichText, Stroke, StrokeKind, Ui, UiBuilder, pos2,
    vec2,
};
use trellis_core::lfs::pointer::size_text;

/// The width the hint wraps at.
const HINT_WIDTH: f32 = 360.0;
/// The icon's square.
const ICON: f32 = 40.0;
/// The least space above the block, on a short pane.
const MIN_TOP: f32 = 24.0;

pub(super) const BINARY_TITLE: &str = "Binary file";
pub(super) const BINARY_HINT: &str = "Changes inside binary files aren't shown.";
pub(super) const WHITESPACE_TITLE: &str = "Only whitespace changed";
pub(super) const WHITESPACE_HINT: &str = "Ignore whitespace is hiding the changes in this file.";
pub(super) const SHOW_WHITESPACE: &str = "Show whitespace changes";
pub(super) const NO_CHANGES_TITLE: &str = "No changes to show";
pub(super) const NO_CHANGES_HINT: &str = "Only the file's mode changed, or the file is empty.";

/// A binary file's diff (and its File view in the file history).
pub(super) fn binary_file(ui: &mut Ui) {
    empty_state(ui, EmptyIcon::Binary, BINARY_TITLE, BINARY_HINT, None);
}

/// A binary file's diff with its sizes known (a file that isn't an image preview).
pub(super) fn binary_sizes(ui: &mut Ui, old: Option<u64>, new: Option<u64>) {
    empty_state(ui, EmptyIcon::Binary, BINARY_TITLE, &sizes_hint(old, new), None);
}

/// "12.4 KB before, 14.1 KB after", "Added, 14.1 KB" or "Deleted, 12.4 KB"; the plain hint
/// without either size.
pub(super) fn sizes_hint(old: Option<u64>, new: Option<u64>) -> String {
    match (old, new) {
        (Some(a), Some(b)) => format!("{} before, {} after", size_text(a), size_text(b)),
        (None, Some(b)) => format!("Added, {}", size_text(b)),
        (Some(a), None) => format!("Deleted, {}", size_text(a)),
        (None, None) => BINARY_HINT.to_owned(),
    }
}

/// A diff whose changes Ignore whitespace hides all of; true when its button asks to show them.
pub(super) fn whitespace_only(ui: &mut Ui) -> bool {
    empty_state(ui, EmptyIcon::Whitespace, WHITESPACE_TITLE, WHITESPACE_HINT, Some(SHOW_WHITESPACE)).is_some_and(|r| r.clicked())
}

/// A diff without hunks for another reason: a mode change or an empty file.
pub(super) fn no_changes(ui: &mut Ui) {
    empty_state(ui, EmptyIcon::File, NO_CHANGES_TITLE, NO_CHANGES_HINT, None);
}

/// What the icon shows: a page outline with a mark of what's (not) inside.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmptyIcon {
    /// A page of bits: a binary file.
    Binary,
    /// A page of dotted rows: only whitespace changed.
    Whitespace,
    /// A page of plain rows: nothing changed in the text.
    File,
}

/// Draws the empty state in the space left in `ui`. With `action`, a button
/// of that text sits under the hint; its response is returned.
pub fn empty_state(ui: &mut Ui, icon: EmptyIcon, title: &str, hint: &str, action: Option<&str>) -> Option<Response> {
    let area = ui.available_rect_before_wrap();
    let width = HINT_WIDTH.min(area.width() - 32.0).max(120.0);
    let hint_h = ui.painter().layout(hint.to_owned(), body_font(ui), Color32::PLACEHOLDER, width).size().y;
    let line_h = ui.text_style_height(&egui::TextStyle::Body);
    let button_h = if action.is_some() { 16.0 + ui.spacing().interact_size.y.max(28.0) } else { 0.0 };
    let block_h = ICON + 16.0 + line_h + 8.0 + hint_h + button_h;
    let free = if area.height().is_finite() { area.height() - block_h } else { 0.0 };
    let top = area.top() + (free / 3.0).max(MIN_TOP);
    let block = Rect::from_min_size(pos2(area.center().x - width / 2.0, top), vec2(width, block_h.max(0.0)));
    let mut clicked = None;
    ui.scope_builder(UiBuilder::new().max_rect(block).layout(Layout::top_down(Align::Center)), |ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(ICON, ICON), egui::Sense::hover());
        paint(ui.painter(), rect.center(), icon, color::weak());
        ui.add_space(16.0);
        ui.label(RichText::new(title).strong().color(color::text()));
        ui.add_space(8.0);
        ui.add(egui::Label::new(RichText::new(hint).color(color::weak())).wrap());
        if let Some(text) = action {
            ui.add_space(16.0);
            clicked = Some(ui.button(text));
        }
    });
    clicked
}

fn body_font(ui: &Ui) -> FontId {
    egui::TextStyle::Body.resolve(ui.style())
}

/// A 30 x 38 page with its top-right corner folded, centred on `c`, and its mark.
pub(super) fn paint(p: &Painter, c: Pos2, icon: EmptyIcon, col: Color32) {
    let s = Stroke::new(1.6_f32, col);
    let (l, r, t, b, fold) = (c.x - 15.0, c.x + 15.0, c.y - 19.0, c.y + 19.0, 9.0);
    let outline = vec![pos2(l, t), pos2(r - fold, t), pos2(r, t + fold), pos2(r, b), pos2(l, b)];
    p.add(egui::Shape::closed_line(outline, s));
    p.add(egui::Shape::line(vec![pos2(r - fold, t), pos2(r - fold, t + fold), pos2(r, t + fold)], s));
    let thin = Stroke::new(1.4_f32, col);
    match icon {
        EmptyIcon::Binary => {
            // Two rows of bits: a 1 is an upright bar, a 0 a narrow ring.
            for (row, bits) in [(c.y - 3.0, [true, false, true]), (c.y + 9.0, [false, true, true])] {
                for (i, one) in bits.into_iter().enumerate() {
                    let x = c.x - 7.0 + i as f32 * 7.0;
                    if one {
                        p.line_segment([pos2(x, row - 4.0), pos2(x, row + 4.0)], thin);
                    } else {
                        let ring = Rect::from_center_size(pos2(x, row), vec2(5.0, 8.0));
                        p.rect_stroke(ring, CornerRadius::same(3), thin, StrokeKind::Middle);
                    }
                }
            }
        }
        EmptyIcon::Whitespace => {
            // Rows of spaces, as an editor marks them: dots.
            for (row, n) in [(c.y - 4.0, 5), (c.y + 3.0, 3), (c.y + 10.0, 4)] {
                for i in 0..n {
                    p.circle_filled(pos2(l + 7.0 + i as f32 * 4.0, row), 1.1, col);
                }
            }
        }
        EmptyIcon::File => {
            for (row, len) in [(c.y - 4.0, 16.0), (c.y + 3.0, 12.0), (c.y + 10.0, 14.0)] {
                p.line_segment([pos2(l + 7.0, row), pos2(l + 7.0 + len, row)], thin);
            }
        }
    }
}
