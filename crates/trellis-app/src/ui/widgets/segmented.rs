//! A segmented control: a 28 px track of options side by side, the chosen one raised on the
//! panel fill. Each option is named by its label and marked selected for screen readers and
//! tests. A segment can be disabled with a reason (`Segment::why`): it senses hover only, never
//! takes a click or shows the pointing hand, paints its text `faint` (`weak` when it is the
//! raised one) instead of egui's fade, and shows the reason on hover. A segment's id comes from
//! the track and its index, so a label that changes (a count) keeps its id.

use super::super::theme::color;
use crate::ui::theme::type_scale;
use eframe::egui::{self, CornerRadius, CursorIcon, FontId, Rect, Sense, Stroke, StrokeKind, Ui, WidgetInfo, WidgetType, pos2, vec2};

/// The track's height and corners; a segment sits 2 px inside it.
const HEIGHT: f32 = 28.0;
const RADIUS: u8 = 6;
const INSET: f32 = 2.0;
/// Room either side of a label.
const PAD_X: f32 = 12.0;
const FONT: f32 = type_scale::BODY;

/// One option of a segmented control.
#[derive(Clone, Copy)]
pub struct Segment<'a, T> {
    pub value: T,
    pub label: &'a str,
    /// What it shows, on hover while enabled.
    pub tip: &'a str,
    /// Why it can't be picked: `Some` disables it, explained on hover.
    pub why: Option<&'a str>,
}

impl<'a, T> Segment<'a, T> {
    pub fn new(value: T, label: &'a str, tip: &'a str) -> Self {
        Self { value, label, tip, why: None }
    }

    /// Disables it with `why`, or leaves it enabled with `None`.
    pub fn why(self, why: Option<&'a str>) -> Self {
        Self { why, ..self }
    }
}

/// Draws `options` (value, label, tooltip) left to right with `current` raised; returns the
/// value of an option other than `current` that was clicked.
pub fn segmented<T: Copy + PartialEq>(ui: &mut Ui, current: T, options: &[(T, &str, &str)]) -> Option<T> {
    let options: Vec<Segment<'_, T>> = options.iter().map(|&(value, label, tip)| Segment::new(value, label, tip)).collect();
    segmented_opts(ui, current, &options)
}

/// Draws `options` left to right with `current` raised; returns the value of an enabled option
/// other than `current` that was clicked.
pub fn segmented_opts<T: Copy + PartialEq>(ui: &mut Ui, current: T, options: &[Segment<'_, T>]) -> Option<T> {
    let font = FontId::proportional(FONT);
    let widths: Vec<f32> = options
        .iter()
        .map(|o| ui.painter().layout_no_wrap(o.label.to_owned(), font.clone(), color::text()).size().x + 2.0 * PAD_X)
        .collect();
    let total = widths.iter().sum::<f32>() + 2.0 * INSET;
    let (track, track_resp) = ui.allocate_exact_size(vec2(total, HEIGHT), Sense::hover());
    ui.painter().rect_filled(track, CornerRadius::same(RADIUS), color::raised());
    let mut picked = None;
    let mut x = track.left() + INSET;
    for (i, (o, w)) in options.iter().zip(widths).enumerate() {
        let rect = Rect::from_min_size(pos2(x, track.top() + INSET), vec2(w, HEIGHT - 2.0 * INSET));
        x += w;
        let selected = o.value == current;
        let enabled = o.why.is_none() && ui.is_enabled();
        let id = track_resp.id.with(("segment", i));
        let resp = ui.interact(rect, id, if enabled { Sense::click() } else { Sense::hover() });
        let label = o.label;
        resp.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, enabled, selected, label));
        let p = ui.painter();
        let inner = CornerRadius::same(RADIUS - 1);
        let fg = match (selected, enabled) {
            (true, _) => {
                p.rect(rect, inner, color::panel(), Stroke::new(1.0_f32, color::input_border()), StrokeKind::Inside);
                if enabled { color::strong() } else { color::weak() }
            }
            (false, false) => color::faint(),
            (false, true) if resp.hovered() => {
                p.rect_filled(rect, inner, color::hover());
                color::text()
            }
            (false, true) => color::weak(),
        };
        if resp.has_focus() {
            p.rect_stroke(rect, inner, Stroke::new(1.5_f32, color::accent()), StrokeKind::Inside);
        }
        p.text(rect.center(), egui::Align2::CENTER_CENTER, label, font.clone(), fg);
        let hover = if enabled { o.tip } else { o.why.unwrap_or(o.tip) };
        let resp = resp.on_hover_text(hover).on_disabled_hover_text(hover);
        if enabled && !selected {
            let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
            if resp.clicked() {
                picked = Some(o.value);
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

    #[test]
    fn a_disabled_segment_is_named_and_inert() {
        let mut h = Harness::builder().with_size(vec2(400.0, 60.0)).build_ui_state(
            |ui, picked: &mut Vec<u8>| {
                let options = [
                    Segment::new(1u8, "One", "The first").why(Some("Not now")),
                    Segment::new(2, "Two", "The second"),
                    Segment::new(3, "Three", "The third").why(Some("Never")),
                ];
                if let Some(v) = segmented_opts(ui, 3u8, &options) {
                    picked.push(v);
                }
            },
            Vec::new(),
        );
        h.run();
        let one = h.get_by_label("One");
        assert!(one.is_disabled());
        assert_eq!(one.toggled(), Some(Toggled::False));
        let three = h.get_by_label("Three");
        assert!(three.is_disabled(), "the raised one can be disabled too");
        assert_eq!(three.toggled(), Some(Toggled::True));
        assert!(!h.get_by_label("Two").is_disabled());
        h.get_by_label("One").click();
        h.run();
        h.get_by_label("Two").click();
        h.run();
        assert_eq!(h.state(), &[2], "only the enabled segment picks");
    }

    #[test]
    fn a_disabled_ui_disables_every_segment() {
        let mut h = Harness::builder().with_size(vec2(300.0, 60.0)).build_ui_state(
            |ui, picked: &mut Vec<u8>| {
                ui.add_enabled_ui(false, |ui| {
                    if let Some(v) = segmented(ui, 1u8, &[(1, "One", "The first"), (2, "Two", "The second")]) {
                        picked.push(v);
                    }
                });
            },
            Vec::new(),
        );
        h.run();
        assert!(h.get_by_label("One").is_disabled());
        assert!(h.get_by_label("Two").is_disabled());
        h.get_by_label("Two").click();
        h.run();
        assert!(h.state().is_empty());
    }
}
