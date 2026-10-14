//! The keyboard shortcut sheet (Ctrl+/): a filter field over every binding in the table,
//! grouped, each row with its keys drawn as key caps. A reference only; rows don't run
//! anything.

use crate::keymap::{self, Binding};
use crate::ui::theme::color;
use eframe::egui::{
    self, Align, CornerRadius, FontId, Galley, Id, Layout, Rect, RichText, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, Ui, WidgetInfo,
    WidgetType, pos2, vec2,
};
use std::sync::Arc;

const WIDTH: f32 = 540.0;
const LIST_MAX_H: f32 = 460.0;
const CAP_FONT: f32 = 12.0;
/// Horizontal and vertical padding inside a key cap.
const CAP_PAD: egui::Vec2 = vec2(10.0, 4.0);
/// Space between a cap and the "+" or "or" next to it.
const GAP: f32 = 4.0;

/// Draws the sheet inside the dialog; `focus` gives the filter field the keyboard (not under a
/// credential prompt). Returns true when Close is clicked.
pub fn show(ui: &mut Ui, filter: &mut String, focus: bool) -> bool {
    ui.set_width(WIDTH);
    ui.heading("Keyboard shortcuts");
    ui.add_space(6.0);
    let field = TextEdit::singleline(filter)
        .id(Id::new("trellis-shortcut-filter"))
        .hint_text("Filter by action or key")
        .desired_width(f32::INFINITY);
    let r = ui.add(field);
    if focus {
        r.request_focus();
    }
    ui.add_space(4.0);
    let groups = keymap::filter(filter);
    ScrollArea::vertical().id_salt("trellis-shortcut-list").max_height(LIST_MAX_H).show(ui, |ui| {
        ui.set_width(ui.available_width());
        if groups.is_empty() {
            ui.add_space(8.0);
            ui.label(RichText::new("No shortcuts match").color(color::weak()));
        }
        for (group, rows) in &groups {
            ui.add_space(8.0);
            ui.label(RichText::new(group.title()).strong().size(12.5).color(color::weak()));
            for b in rows {
                row(ui, b);
            }
        }
    });
    ui.add_space(8.0);
    let mut close = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Esc to close").color(color::weak()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            close = ui.button("Close").clicked();
        });
    });
    close
}

/// One binding: the action and its context note on the left (wrapping when long), the key caps
/// right-aligned.
fn row(ui: &mut Ui, b: &Binding) {
    let caps = Caps::new(ui, b);
    ui.horizontal(|ui| {
        let left = (ui.available_width() - caps.size.x - 16.0).max(120.0);
        let wrap = Layout::left_to_right(Align::Center).with_main_wrap(true);
        ui.allocate_ui_with_layout(vec2(left, 0.0), wrap, |ui| {
            ui.label(RichText::new(b.label).color(color::text()));
            if let Some(note) = b.when.note() {
                ui.label(RichText::new(note).size(11.5).color(color::weak()));
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| caps.paint(ui, b));
    });
}

/// A binding's keys laid out as caps and separators: each chord's parts as caps joined by "+",
/// alternatives joined by "or".
struct Caps {
    /// (is a cap, its text).
    pieces: Vec<(bool, Arc<Galley>)>,
    size: egui::Vec2,
}

impl Caps {
    fn new(ui: &Ui, b: &Binding) -> Self {
        let font = FontId::proportional(CAP_FONT);
        let text = |s: &str, c| ui.painter().layout_no_wrap(s.to_owned(), font.clone(), c);
        let mut pieces = Vec::new();
        for (n, chord) in b.chords.iter().enumerate() {
            if n > 0 {
                pieces.push((false, text("or", color::weak())));
            }
            for (k, part) in chord.parts().into_iter().enumerate() {
                if k > 0 {
                    pieces.push((false, text("+", color::weak())));
                }
                pieces.push((true, text(part, color::text())));
            }
        }
        let width = pieces.iter().map(|(cap, g)| piece_width(*cap, g)).sum::<f32>() + GAP * pieces.len().saturating_sub(1) as f32;
        let height = pieces.iter().map(|(_, g)| g.size().y).fold(0.0, f32::max) + CAP_PAD.y;
        Self { pieces, size: vec2(width, height) }
    }

    /// Paints the caps as one widget, labelled with the keys' text for screen readers and tests.
    fn paint(self, ui: &mut Ui, b: &Binding) {
        let (rect, resp) = ui.allocate_exact_size(self.size, Sense::hover());
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, keymap::keys_text(b)));
        if !ui.is_rect_visible(rect) {
            return;
        }
        let painter = ui.painter();
        let mut x = rect.left();
        for (cap, g) in self.pieces {
            let w = piece_width(cap, &g);
            let r = Rect::from_min_size(pos2(x, rect.top()), vec2(w, rect.height()));
            if cap {
                painter.rect(r, CornerRadius::same(3), color::toolbar(), Stroke::new(1.0_f32, color::faint()), StrokeKind::Inside);
            }
            let at = r.center() - g.size() / 2.0;
            painter.galley(at, g, color::text());
            x += w + GAP;
        }
    }
}

fn piece_width(cap: bool, g: &Galley) -> f32 {
    g.size().x + if cap { CAP_PAD.x } else { 0.0 }
}
