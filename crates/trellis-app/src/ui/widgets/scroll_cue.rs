//! The scroll cue: a thin scroll bar that is always visible (4 px at rest, 8 px on hover, in the
//! text colour at low opacity, with 8 px kept free beside it so it never covers row text), and a
//! fade where a vertical scroll area cuts its content, from transparent to the surface colour:
//! at the top once scrolled down, at the bottom while more is below. Dialog bodies, the
//! Preferences page and the file history's File view use it.

use eframe::egui::{self, Color32, Mesh, Rect, Ui, pos2, scroll_area::ScrollAreaOutput, style::ScrollStyle};

/// How tall a fade is, at most (and never more than a third of the view).
pub const FADE_H: f32 = 24.0;

/// The cue's scroll bar: floating, always visible, in the text colour.
pub fn style() -> ScrollStyle {
    ScrollStyle {
        floating: true,
        bar_width: 8.0,
        floating_width: 4.0,
        floating_allocated_width: 8.0,
        handle_min_length: 24.0,
        bar_inner_margin: 2.0,
        bar_outer_margin: 0.0,
        foreground_color: true,
        dormant_background_opacity: 0.0,
        active_background_opacity: 0.0,
        interact_background_opacity: 0.3,
        dormant_handle_opacity: 0.35,
        active_handle_opacity: 0.5,
        interact_handle_opacity: 0.7,
    }
}

/// Which edges of a vertical view cut its content: (top, bottom). Half a pixel of slack keeps
/// rounding from drawing a fade on content that fits.
pub fn cut_edges(offset_y: f32, view_h: f32, content_h: f32) -> (bool, bool) {
    (offset_y > 0.5, offset_y + view_h < content_h - 0.5)
}

/// Paints the fades over `out`'s view where its content is cut, in `surface`.
pub fn paint_fades<R>(ui: &Ui, out: &ScrollAreaOutput<R>, surface: Color32) {
    let rect = out.inner_rect;
    let (top, bottom) = cut_edges(out.state.offset.y, rect.height(), out.content_size.y);
    let h = FADE_H.min(rect.height() / 3.0);
    if h <= 0.0 {
        return;
    }
    let painter = ui.painter().with_clip_rect(rect);
    if top {
        let band = Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + h));
        painter.add(gradient(band, surface, Color32::TRANSPARENT));
    }
    if bottom {
        let band = Rect::from_min_max(pos2(rect.left(), rect.bottom() - h), rect.max);
        painter.add(gradient(band, Color32::TRANSPARENT, surface));
    }
}

/// A vertical gradient over `rect`, `top` at its top edge and `bottom` at its bottom edge.
fn gradient(rect: Rect, top: Color32, bottom: Color32) -> Mesh {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    mesh
}

/// Shows `area` with the cue's bar and, where its content is cut, the fades in `surface`.
pub fn show<R>(ui: &mut Ui, area: egui::ScrollArea, surface: Color32, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.scope(|ui| {
        ui.spacing_mut().scroll = style();
        let out = area.show(ui, add);
        paint_fades(ui, &out, surface);
        out.inner
    })
    .inner
}

#[cfg(test)]
mod tests {
    use super::cut_edges;

    #[test]
    fn fades_mark_only_the_cut_edges() {
        assert_eq!(cut_edges(0.0, 100.0, 100.0), (false, false));
        assert_eq!(cut_edges(0.0, 100.0, 300.0), (false, true));
        assert_eq!(cut_edges(50.0, 100.0, 300.0), (true, true));
        assert_eq!(cut_edges(200.0, 100.0, 300.0), (true, false));
        // Sub-pixel slack draws no fade.
        assert_eq!(cut_edges(0.2, 100.0, 100.3), (false, false));
    }
}
