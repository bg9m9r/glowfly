//! The loading spinner: egui's arc, drawn the same, but asking for its next frame only every
//! `redraw::REDRAW` and only while it can be seen. Views use this, never `egui::Spinner`
//! (`tests/view_checks/repaints.rs`).

use super::redraw;
use eframe::egui::emath::lerp;
use eframe::egui::{Color32, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Widget, WidgetInfo, WidgetType, vec2};

/// The arc's stroke width and how many points it is drawn through.
const STROKE: f32 = 3.0;
const POINTS: usize = 20;

/// A spinner: square, sized to the style's `interact_size` unless `size` says.
#[derive(Default)]
#[must_use = "put the spinner in a ui with `ui.add(spinner)`, or paint it with `paint_at`"]
pub struct Spinner {
    size: Option<f32>,
    color: Option<Color32>,
}

impl Spinner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Both its width and height.
    pub fn size(mut self, size: f32) -> Self {
        self.size = Some(size);
        self
    }

    /// Its colour; the strong text colour without.
    pub fn color(mut self, color: impl Into<Color32>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// Paints the spinner in `rect` and, while it can be seen, asks for a frame after
    /// `redraw::REDRAW`.
    pub fn paint_at(&self, ui: &Ui, rect: Rect) {
        if !ui.is_rect_visible(rect) {
            return;
        }
        redraw::next_frame(ui, rect);
        let color = self.color.unwrap_or_else(|| ui.visuals().strong_text_color());
        let radius = rect.height() / 2.0 - 2.0;
        let time = ui.input(|i| i.time);
        let start = time * std::f64::consts::TAU;
        let end = start + 240f64.to_radians() * time.sin();
        let points: Vec<Pos2> = (0..POINTS)
            .map(|i| {
                let (sin, cos) = lerp(start..=end, i as f64 / POINTS as f64).sin_cos();
                rect.center() + radius * vec2(cos as f32, sin as f32)
            })
            .collect();
        ui.painter().add(Shape::line(points, Stroke::new(STROKE, color)));
    }
}

impl Widget for Spinner {
    fn ui(self, ui: &mut Ui) -> Response {
        let size = self.size.unwrap_or_else(|| ui.style().spacing.interact_size.y);
        let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
        response.widget_info(|| WidgetInfo::new(WidgetType::ProgressIndicator));
        self.paint_at(ui, rect);
        response
    }
}

#[cfg(test)]
#[path = "spinner_tests.rs"]
mod tests;
