//! Keys drawn as key caps: each chord's parts as caps joined by "+", alternatives joined by
//! "or". Used by the shortcut sheet and Preferences > Keyboard.

use crate::keymap::Chord;
use crate::ui::theme::color;
use crate::ui::theme::tokens::radius;
use crate::ui::theme::type_scale;
use eframe::egui::{
    self, Color32, CornerRadius, FontId, Galley, Rect, Response, Sense, Stroke, StrokeKind, Ui, WidgetInfo, WidgetType, pos2, vec2,
};
use std::sync::Arc;

const CAP_FONT: f32 = type_scale::SMALL;
/// Horizontal and vertical padding inside a key cap.
const CAP_PAD: egui::Vec2 = vec2(10.0, 4.0);
/// Space between a cap and the "+" or "or" next to it.
const GAP: f32 = 4.0;

/// One group of caps, laid out and ready to paint.
pub struct Caps {
    /// (is a cap, its text).
    pieces: Vec<(bool, Arc<Galley>)>,
    pub size: egui::Vec2,
    dim: bool,
}

impl Caps {
    /// The caps for `chords`; `dim` draws them faint (keys that can't be changed). No chords
    /// gives a weak "No key".
    pub fn new(ui: &Ui, chords: &[Chord], dim: bool) -> Self {
        let font = FontId::proportional(CAP_FONT);
        let text = |s: &str, c: Color32| ui.painter().layout_no_wrap(s.to_owned(), font.clone(), c);
        let fg = if dim { color::faint() } else { color::text() };
        let mut pieces = Vec::new();
        if chords.is_empty() {
            pieces.push((false, text("No key", color::weak())));
        }
        for (n, chord) in chords.iter().enumerate() {
            if n > 0 {
                pieces.push((false, text("or", color::weak())));
            }
            for (k, part) in chord.parts().into_iter().enumerate() {
                if k > 0 {
                    pieces.push((false, text("+", color::weak())));
                }
                pieces.push((true, text(part, fg)));
            }
        }
        let width = pieces.iter().map(|(cap, g)| piece_width(*cap, g)).sum::<f32>() + GAP * pieces.len().saturating_sub(1) as f32;
        let height = pieces.iter().map(|(_, g)| g.size().y).fold(0.0, f32::max) + CAP_PAD.y;
        Self { pieces, size: vec2(width, height), dim }
    }

    /// Paints the caps as one widget, labelled `label` for screen readers and tests.
    pub fn paint(self, ui: &mut Ui, label: &str, sense: Sense) -> Response {
        let (rect, resp) = ui.allocate_exact_size(self.size, sense);
        let kind = if sense.senses_click() { WidgetType::Button } else { WidgetType::Label };
        resp.widget_info(|| WidgetInfo::labeled(kind, ui.is_enabled(), label));
        if !ui.is_rect_visible(rect) {
            return resp;
        }
        let hovered = sense.senses_click() && resp.hovered();
        let painter = ui.painter();
        let border = if hovered { color::primary_border() } else { color::faint() };
        let fill = if hovered { color::hover() } else { color::raised() };
        let fg = if self.dim { color::faint() } else { color::text() };
        let mut x = rect.left();
        for (cap, g) in self.pieces {
            let w = piece_width(cap, &g);
            let r = Rect::from_min_size(pos2(x, rect.top()), vec2(w, rect.height()));
            if cap {
                painter.rect(r, CornerRadius::same(radius::SM), fill, Stroke::new(1.0_f32, border), StrokeKind::Inside);
            }
            let at = r.center() - g.size() / 2.0;
            painter.galley(at, g, fg);
            x += w + GAP;
        }
        if sense.senses_click() { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
    }
}

fn piece_width(cap: bool, g: &Galley) -> f32 {
    g.size().x + if cap { CAP_PAD.x } else { 0.0 }
}
