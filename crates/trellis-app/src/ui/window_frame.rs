//! The window frame Trellis draws itself: the viewport builder for each `Chrome`, the viewport
//! command for each `WindowCmd` (sent by the shell), and the overlay drawn above every dialog on
//! Windows: the window buttons, the resize edges and corners, a 1 px border, and a drag zone
//! over the title bar while a dialog blocks it. Everything here pushes `AppMsg::Window`; only
//! the shell sends viewport commands.

use super::theme::color;
use super::title_bar;
use super::window_buttons::{self, BUTTON_W};
use crate::msg::{AppMsg, Msg};
use crate::state::AppState;
use crate::state::window_chrome::{Chrome, ResizeEdge, WindowCmd};
use eframe::egui::viewport::ResizeDirection;
use eframe::egui::{
    self, Area, CursorIcon, Id, LayerId, Order, PointerButton, Rect, Sense, Stroke, StrokeKind, ViewportBuilder, pos2, vec2,
};

/// The resize edges' thickness and the corners' size, in screen points at 100% zoom.
const EDGE: f32 = 5.0;
const CORNER: f32 = 12.0;

/// `b` set up for `chrome`: no system frame on Windows, a transparent full-size title bar on
/// macOS, the system frame elsewhere.
pub fn viewport(chrome: Chrome, b: ViewportBuilder) -> ViewportBuilder {
    match chrome {
        Chrome::Windows => b.with_decorations(false),
        Chrome::MacOs => b.with_fullsize_content_view(true).with_title_shown(false).with_titlebar_shown(false),
        Chrome::System => b,
    }
}

/// The viewport command that carries out `cmd`.
pub(super) fn viewport_cmd(cmd: WindowCmd) -> egui::ViewportCommand {
    match cmd {
        WindowCmd::StartDrag => egui::ViewportCommand::StartDrag,
        WindowCmd::BeginResize(e) => egui::ViewportCommand::BeginResize(direction(e)),
        WindowCmd::Minimize => egui::ViewportCommand::Minimized(true),
        WindowCmd::SetMaximized(on) => egui::ViewportCommand::Maximized(on),
    }
}

fn direction(e: ResizeEdge) -> ResizeDirection {
    match e {
        ResizeEdge::North => ResizeDirection::North,
        ResizeEdge::South => ResizeDirection::South,
        ResizeEdge::East => ResizeDirection::East,
        ResizeEdge::West => ResizeDirection::West,
        ResizeEdge::NorthEast => ResizeDirection::NorthEast,
        ResizeEdge::NorthWest => ResizeDirection::NorthWest,
        ResizeEdge::SouthEast => ResizeDirection::SouthEast,
        ResizeEdge::SouthWest => ResizeDirection::SouthWest,
    }
}

fn cursor(e: ResizeEdge) -> CursorIcon {
    match e {
        ResizeEdge::North => CursorIcon::ResizeNorth,
        ResizeEdge::South => CursorIcon::ResizeSouth,
        ResizeEdge::East => CursorIcon::ResizeEast,
        ResizeEdge::West => CursorIcon::ResizeWest,
        ResizeEdge::NorthEast => CursorIcon::ResizeNorthEast,
        ResizeEdge::NorthWest => CursorIcon::ResizeNorthWest,
        ResizeEdge::SouthEast => CursorIcon::ResizeSouthEast,
        ResizeEdge::SouthWest => CursorIcon::ResizeSouthWest,
    }
}

/// The eight resize zones of `screen`: `corner`-sized squares in the corners, and `edge`-thick
/// strips along the sides between them.
pub(super) fn resize_zones(screen: Rect, edge: f32, corner: f32) -> [(ResizeEdge, Rect); 8] {
    let (l, r, t, b) = (screen.left(), screen.right(), screen.top(), screen.bottom());
    let rect = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(pos2(x0, y0), pos2(x1, y1));
    [
        (ResizeEdge::NorthWest, rect(l, t, l + corner, t + corner)),
        (ResizeEdge::NorthEast, rect(r - corner, t, r, t + corner)),
        (ResizeEdge::SouthWest, rect(l, b - corner, l + corner, b)),
        (ResizeEdge::SouthEast, rect(r - corner, b - corner, r, b)),
        (ResizeEdge::North, rect(l + corner, t, r - corner, t + edge)),
        (ResizeEdge::South, rect(l + corner, b - edge, r - corner, b)),
        (ResizeEdge::West, rect(l, t + corner, l + edge, b - corner)),
        (ResizeEdge::East, rect(r - edge, t + corner, r, b - corner)),
    ]
}

/// The overlay above every dialog (call last): only on Windows, where Trellis draws the frame.
pub(super) fn overlay(ctx: &egui::Context, st: &AppState, msgs: &mut Vec<Msg>) {
    if !st.view.chrome.window_buttons() {
        return;
    }
    let screen = ctx.screen_rect();
    let window = st.view.window;
    if !window.maximized && !window.fullscreen {
        let border = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("window-border")));
        border.rect_stroke(screen, 0.0, Stroke::new(1.0_f32, color::border()), StrokeKind::Inside);
        if st.view.chrome.resize_edges() {
            resize_edges(ctx, screen, msgs);
        }
    }
    let t = st.settings.lang();
    let buttons_w = 3.0 * BUTTON_W;
    Area::new(Id::new("window-buttons"))
        .order(Order::Tooltip)
        .fixed_pos(pos2(screen.right() - buttons_w, screen.top()))
        .constrain(false)
        .show(ctx, |ui| window_buttons::show(ui, t, window.maximized, msgs));
    // A dialog blocks the bar: keep its empty space for moving the window.
    let Some((bar, layer)) = title_bar::place(ctx) else { return };
    if ctx.memory(|m| m.allows_interaction(layer)) {
        return;
    }
    let zone = Rect::from_min_max(bar.min, pos2(bar.right() - buttons_w, bar.bottom()));
    Area::new(Id::new("window-drag-over-dialog")).order(Order::Tooltip).fixed_pos(zone.min).constrain(false).show(ctx, |ui| {
        let (_, r) = ui.allocate_exact_size(zone.size(), Sense::CLICK | Sense::DRAG);
        if r.double_clicked() {
            msgs.push(Msg::App(AppMsg::Window(WindowCmd::SetMaximized(!window.maximized))));
        } else if r.drag_started_by(PointerButton::Primary) {
            msgs.push(Msg::App(AppMsg::Window(WindowCmd::StartDrag)));
        }
    });
}

/// The edges and corners: each shows its resize cursor and starts an OS resize on a press.
/// Their size is in screen points, so they keep it when the user zooms.
fn resize_edges(ctx: &egui::Context, screen: Rect, msgs: &mut Vec<Msg>) {
    let zoom = ctx.zoom_factor();
    for (edge, rect) in resize_zones(screen, EDGE / zoom, CORNER / zoom) {
        Area::new(Id::new(("window-resize", edge))).order(Order::Tooltip).fixed_pos(rect.min).constrain(false).show(ctx, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let (_, r) = ui.allocate_exact_size(rect.size(), Sense::DRAG);
            if r.hovered() || r.dragged() {
                ctx.set_cursor_icon(cursor(edge));
            }
            if r.is_pointer_button_down_on() && ctx.input(|i| i.pointer.primary_pressed()) {
                msgs.push(Msg::App(AppMsg::Window(WindowCmd::BeginResize(edge))));
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_frame_sets_up_its_viewport() {
        let b = viewport(Chrome::Windows, ViewportBuilder::default());
        assert_eq!(b.decorations, Some(false));
        assert_eq!((b.fullsize_content_view, b.title_shown, b.titlebar_shown), (None, None, None));
        let b = viewport(Chrome::MacOs, ViewportBuilder::default());
        assert_eq!((b.fullsize_content_view, b.title_shown, b.titlebar_shown), (Some(true), Some(false), Some(false)));
        assert_eq!(b.decorations, None, "macOS keeps its frame");
        let b = viewport(Chrome::System, ViewportBuilder::default());
        assert_eq!((b.decorations, b.fullsize_content_view, b.title_shown, b.titlebar_shown), (None, None, None, None));
    }

    #[test]
    fn every_window_action_has_its_command() {
        use egui::ViewportCommand as V;
        assert_eq!(viewport_cmd(WindowCmd::StartDrag), V::StartDrag);
        assert_eq!(viewport_cmd(WindowCmd::Minimize), V::Minimized(true));
        assert_eq!(viewport_cmd(WindowCmd::SetMaximized(true)), V::Maximized(true));
        assert_eq!(viewport_cmd(WindowCmd::SetMaximized(false)), V::Maximized(false));
        for (e, d) in [
            (ResizeEdge::North, ResizeDirection::North),
            (ResizeEdge::South, ResizeDirection::South),
            (ResizeEdge::East, ResizeDirection::East),
            (ResizeEdge::West, ResizeDirection::West),
            (ResizeEdge::NorthEast, ResizeDirection::NorthEast),
            (ResizeEdge::NorthWest, ResizeDirection::NorthWest),
            (ResizeEdge::SouthEast, ResizeDirection::SouthEast),
            (ResizeEdge::SouthWest, ResizeDirection::SouthWest),
        ] {
            assert_eq!(viewport_cmd(WindowCmd::BeginResize(e)), V::BeginResize(d), "{e:?}");
        }
    }

    #[test]
    fn the_zones_cover_the_border_without_overlapping() {
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(1440.0, 880.0));
        let zones = resize_zones(screen, 5.0, 12.0);
        for (i, (a, ra)) in zones.iter().enumerate() {
            assert!(ra.width() > 0.0 && ra.height() > 0.0, "{a:?}");
            assert!(screen.contains_rect(*ra), "{a:?}");
            for (b, rb) in &zones[i + 1..] {
                assert!(!ra.intersects(*rb) || ra.intersect(*rb).area() == 0.0, "{a:?} and {b:?} overlap");
            }
        }
        let at = |x: f32, y: f32| zones.iter().find(|(_, r)| r.contains(pos2(x, y))).map(|(e, _)| *e);
        assert_eq!(at(1.0, 400.0), Some(ResizeEdge::West));
        assert_eq!(at(1438.0, 878.0), Some(ResizeEdge::SouthEast));
        assert_eq!(at(11.0, 11.0), Some(ResizeEdge::NorthWest));
        assert_eq!(at(700.0, 2.0), Some(ResizeEdge::North));
        assert_eq!(at(700.0, 877.0), Some(ResizeEdge::South));
        assert_eq!(at(1437.0, 300.0), Some(ResizeEdge::East));
        assert_eq!(at(1430.0, 5.0), Some(ResizeEdge::NorthEast));
        assert_eq!(at(5.0, 870.0), Some(ResizeEdge::SouthWest));
        assert_eq!(at(700.0, 400.0), None, "the middle is the window's own");
        assert_eq!(at(6.0, 400.0), None, "5 pt edges");
    }
}
