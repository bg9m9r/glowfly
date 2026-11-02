//! The shared look of an edge-to-edge list row in each state (`RowState`): the selection fill with
//! an accent bar along the left edge, the lifted hover (the hover fill with a soft shadow inside
//! the bottom edge), the pressed row (one layer stronger, flat), and the inset focus ring on the
//! row's square corners. The graph, the sidebar's rows and headers, the file lists, the file
//! history list, the dashboard and the Preferences pages all use it; a row inset in a padded pane
//! is a `quiet_target` on `radius::SM` instead. In the
//! graph and the file lists the fill is `selected` while the row's pane has the keyboard focus,
//! and the neutral `tokens::selected_inactive` otherwise.

use crate::ui::theme::{color, tokens};
use eframe::egui::{Color32, Painter, Rect, Response, pos2, vec2};

/// The width of a selected row's accent bar.
pub const ACCENT_BAR_W: f32 = 3.0;

/// What a row is doing this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RowState {
    pub selected: bool,
    /// Its pane has the keyboard focus: the selection takes `selected`, else `selected_inactive`.
    pub active: bool,
    pub hovered: bool,
    /// The pointer is held down on it.
    pub down: bool,
    /// It has the keyboard focus itself.
    pub focused: bool,
}

impl RowState {
    /// The state of row `r` in a pane that is always active, `hovered` as the caller decides
    /// (the whole row, or not while a button on it is hovered).
    pub fn of(r: &Response, selected: bool, hovered: bool) -> Self {
        Self { selected, active: true, hovered, down: r.is_pointer_button_down_on(), focused: r.has_focus() }
    }
}

/// Paints a row's selection or hover in `rect`: the selection wins; a plain row paints nothing.
pub fn paint(p: &Painter, rect: Rect, selected: bool, hovered: bool) {
    paint_in(p, rect, selected, true, hovered);
}

/// `paint` for a row of a pane that takes the keyboard focus: its selection is filled with
/// `selected` while the pane has the focus (`active`), else with `selected_inactive`.
pub fn paint_in(p: &Painter, rect: Rect, selected: bool, active: bool, hovered: bool) {
    paint_state(p, rect, RowState { selected, active, hovered, down: false, focused: false });
}

/// Paints row `rect` in state `s`: the selection (fill and accent bar) wins; else a pressed row
/// takes the hover fill with the hover layer over it and no shadow, a hovered one the lifted
/// look. The inset focus ring goes on last, over the selection too.
pub fn paint_state(p: &Painter, rect: Rect, s: RowState) {
    let pal = color::current();
    if s.selected {
        let fill = if s.active { pal.selected } else { tokens::selected_inactive(&pal) };
        p.rect_filled(rect, 0.0, fill);
        p.rect_filled(Rect::from_min_size(rect.left_top(), vec2(ACCENT_BAR_W, rect.height())), 0.0, pal.accent);
    } else if s.down {
        p.rect_filled(rect, 0.0, pal.hover);
        p.rect_filled(rect, 0.0, pal.hover_layer());
    } else if s.hovered {
        // Lifted: the hover fill with a soft shadow inside the bottom edge.
        p.rect_filled(rect, 0.0, pal.hover);
        let alpha = if pal.dark { 70 } else { 26 };
        let line = |from: f32, a: u8| {
            let r = Rect::from_min_max(pos2(rect.left(), rect.bottom() - from), pos2(rect.right(), rect.bottom() - from + 1.0));
            p.rect_filled(r, 0.0, Color32::from_black_alpha(a));
        };
        line(1.0, alpha);
        line(2.0, alpha / 2);
    }
    if s.focused {
        tokens::focus_ring_inset(p, rect, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{self, Id, LayerId, Order, Shape};

    const ROW: Rect = Rect { min: egui::Pos2 { x: 10.0, y: 20.0 }, max: egui::Pos2 { x: 210.0, y: 56.0 } };

    /// The rects `paint_state` draws for one row, as (rect, fill, stroke colour).
    fn shapes(s: RowState) -> Vec<(Rect, Color32, Color32)> {
        color::set_dark(true);
        let ctx = egui::Context::default();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            let p = Painter::new(ctx.clone(), LayerId::new(Order::Background, Id::new("rows")), Rect::EVERYTHING);
            paint_state(&p, ROW, s);
        });
        out.shapes
            .iter()
            .filter_map(|c| match &c.shape {
                Shape::Rect(r) => Some((r.rect, r.fill, r.stroke.color)),
                _ => None,
            })
            .collect()
    }

    /// The filled rects `paint` draws for one row, as (rect, colour).
    fn fills(selected: bool, hovered: bool) -> Vec<(Rect, Color32)> {
        let s = RowState { selected, active: true, hovered, ..Default::default() };
        shapes(s).into_iter().filter(|(_, f, _)| *f != Color32::TRANSPARENT).map(|(r, f, _)| (r, f)).collect()
    }

    /// Whether `shapes` hold an accent ring.
    fn ringed(shapes: &[(Rect, Color32, Color32)]) -> bool {
        shapes.iter().any(|(_, _, stroke)| *stroke == color::current().accent)
    }

    #[test]
    fn selected_rows_get_the_fill_and_a_left_accent_bar() {
        let f = fills(true, true);
        let pal = color::current();
        assert!(f.contains(&(ROW, pal.selected)), "the fill spans the row: {f:?}");
        let bar = f.iter().find(|(_, c)| *c == pal.accent).expect("an accent bar");
        assert_eq!(bar.0, Rect::from_min_size(ROW.left_top(), vec2(ACCENT_BAR_W, ROW.height())));
        assert!(!f.iter().any(|(_, c)| *c == pal.hover), "the selection wins over the hover");
    }

    #[test]
    fn hovered_rows_are_lifted() {
        let f = fills(false, true);
        let pal = color::current();
        assert_eq!(f[0], (ROW, pal.hover));
        let lines: Vec<_> = f[1..].iter().collect();
        assert_eq!(lines.len(), 2, "two shadow lines: {f:?}");
        for (i, (r, c)) in lines.iter().enumerate() {
            assert_eq!(r.height(), 1.0);
            assert_eq!(r.width(), ROW.width());
            assert!(r.bottom() <= ROW.bottom() && r.top() >= ROW.bottom() - 2.0, "inside the bottom edge: {r:?}");
            assert_eq!((c.r(), c.g(), c.b()), (0, 0, 0), "black alpha");
            assert!(c.a() > 0, "line {i} shows");
        }
    }

    #[test]
    fn a_plain_row_paints_nothing() {
        assert!(fills(false, false).is_empty());
    }

    #[test]
    fn a_focused_row_takes_the_inset_ring_selected_or_not() {
        for selected in [false, true] {
            let s = RowState { selected, active: true, focused: true, ..Default::default() };
            assert!(ringed(&shapes(s)), "selected {selected}: {:?}", shapes(s));
        }
        assert!(!ringed(&shapes(RowState { hovered: true, ..Default::default() })), "no ring without the focus");
    }

    #[test]
    fn a_pressed_row_is_flat_and_stronger_than_hovered() {
        let pal = color::current();
        let down = shapes(RowState { hovered: true, down: true, ..Default::default() });
        assert_eq!(down.len(), 2, "the hover fill and its layer, no shadow lines: {down:?}");
        assert!(down.iter().all(|(r, _, _)| *r == ROW), "both across the row: {down:?}");
        assert_eq!(down[1].1, pal.hover_layer(), "one layer over the hover fill");
        let selected = shapes(RowState { selected: true, active: true, hovered: true, down: true, ..Default::default() });
        assert!(selected.iter().all(|(_, f, _)| *f != pal.hover_layer() && *f != pal.hover), "the selection beats the press");
    }
}
