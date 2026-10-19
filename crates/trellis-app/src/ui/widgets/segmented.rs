//! A segmented control: a 28 px track of options side by side, the chosen one raised on the
//! panel fill. Each option is named by its label and marked selected for screen readers and
//! tests.

use super::super::theme::color;
use eframe::egui::{self, CornerRadius, CursorIcon, FontId, Rect, Sense, Stroke, StrokeKind, Ui, WidgetInfo, WidgetType, pos2, vec2};

/// The track's height and corners; a segment sits 2 px inside it.
const HEIGHT: f32 = 28.0;
const RADIUS: u8 = 6;
const INSET: f32 = 2.0;
/// Room either side of a label.
const PAD_X: f32 = 12.0;
const FONT: f32 = 13.0;

/// Draws `options` (value, label, tooltip) left to right with `current` raised; returns the
/// value of an option other than `current` that was clicked.
pub fn segmented<T: Copy + PartialEq>(ui: &mut Ui, current: T, options: &[(T, &str, &str)]) -> Option<T> {
    let font = FontId::proportional(FONT);
    let widths: Vec<f32> = options
        .iter()
        .map(|(_, label, _)| ui.painter().layout_no_wrap((*label).to_owned(), font.clone(), color::text()).size().x + 2.0 * PAD_X)
        .collect();
    let total = widths.iter().sum::<f32>() + 2.0 * INSET;
    let (track, _) = ui.allocate_exact_size(vec2(total, HEIGHT), Sense::hover());
    ui.painter().rect_filled(track, CornerRadius::same(RADIUS), color::toolbar());
    let mut picked = None;
    let mut x = track.left() + INSET;
    for (&(value, label, tip), w) in options.iter().zip(widths) {
        let rect = Rect::from_min_size(pos2(x, track.top() + INSET), vec2(w, HEIGHT - 2.0 * INSET));
        x += w;
        let selected = value == current;
        let id = ui.id().with(("segment", label));
        let resp = ui.interact(rect, id, Sense::click());
        resp.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, true, selected, label));
        let p = ui.painter();
        let inner = CornerRadius::same(RADIUS - 1);
        let fg = if selected {
            p.rect(rect, inner, color::panel(), Stroke::new(1.0_f32, color::input_border()), StrokeKind::Inside);
            color::strong()
        } else {
            if resp.hovered() {
                p.rect_filled(rect, inner, color::hover());
            }
            if resp.hovered() { color::text() } else { color::weak() }
        };
        if resp.has_focus() {
            p.rect_stroke(rect, inner, Stroke::new(1.5_f32, color::accent()), StrokeKind::Inside);
        }
        p.text(rect.center(), egui::Align2::CENTER_CENTER, label, font.clone(), fg);
        let resp = resp.on_hover_text(tip);
        if !selected {
            let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
            if resp.clicked() {
                picked = Some(value);
            }
        }
    }
    picked
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::accesskit::Toggled;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    #[test]
    fn a_click_on_another_option_picks_it() {
        let mut h = Harness::builder().with_size(vec2(300.0, 60.0)).build_ui_state(
            |ui, picked: &mut Vec<u8>| {
                if let Some(v) = segmented(ui, 1u8, &[(1, "One", "The first"), (2, "Two", "The second")]) {
                    picked.push(v);
                }
            },
            Vec::new(),
        );
        h.run();
        assert_eq!(h.get_by_label("One").toggled(), Some(Toggled::True));
        assert_eq!(h.get_by_label("Two").toggled(), Some(Toggled::False));
        let (one, two) = (h.get_by_label("One").bounding_box().unwrap(), h.get_by_label("Two").bounding_box().unwrap());
        assert!(one.x1 <= two.x0 + 0.5, "in reading order");
        assert!((one.y1 - one.y0 - (HEIGHT - 2.0 * INSET) as f64).abs() < 0.5);
        h.get_by_label("One").click();
        h.run();
        h.get_by_label("Two").click();
        h.run();
        assert_eq!(h.state(), &[2], "only the other option counts");
    }
}
