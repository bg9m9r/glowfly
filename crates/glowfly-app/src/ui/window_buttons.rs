//! Minimize, Maximize (Restore while maximized) and Close, flush with the window's top-right
//! corner when Glowfly draws the frame. The glyphs are painted, and the buttons never take the
//! keyboard focus (the window keeps its own keys for these). While Windows tracks the Maximize
//! button for Snap Layouts, egui sees no pointer over it: its hover and press then come from
//! the OS (`CaptionButton`), and its click reaches the shell from there.

use super::theme::color;
use crate::msg::{AppMsg, Msg};
use crate::state::window_chrome::WindowCmd;
use crate::state::window_native::CaptionButton;
use eframe::egui::{self, Color32, Painter, Pos2, Rect, Sense, Stroke, WidgetInfo, WidgetType, pos2, vec2};
use glowfly_core::i18n::Lang;

pub(super) const BUTTON_W: f32 = 46.0;
pub(super) const BUTTON_H: f32 = 36.0;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Minimize,
    Maximize,
    Restore,
    Close,
}

impl Kind {
    fn name(self, t: Lang) -> &'static str {
        match self {
            Kind::Minimize => t.tr("title_bar.minimize"),
            Kind::Maximize => t.tr("title_bar.maximize"),
            Kind::Restore => t.tr("title_bar.restore"),
            Kind::Close => t.tr("title_bar.close"),
        }
    }

    fn msg(self) -> Msg {
        match self {
            Kind::Minimize => Msg::App(AppMsg::Window(WindowCmd::Minimize)),
            Kind::Maximize => Msg::App(AppMsg::Window(WindowCmd::SetMaximized(true))),
            Kind::Restore => Msg::App(AppMsg::Window(WindowCmd::SetMaximized(false))),
            // Like File > Quit.
            Kind::Close => Msg::App(AppMsg::Quit),
        }
    }
}

/// The three buttons, left to right from the cursor (the caller places them at the corner);
/// `native` is the Maximize button as the OS tracks it.
pub(super) fn show(ui: &mut egui::Ui, t: Lang, maximized: bool, native: CaptionButton, msgs: &mut Vec<Msg>) {
    let (row, _) = ui.allocate_exact_size(vec2(3.0 * BUTTON_W, BUTTON_H), Sense::hover());
    let kinds = [Kind::Minimize, if maximized { Kind::Restore } else { Kind::Maximize }, Kind::Close];
    for (i, kind) in kinds.into_iter().enumerate() {
        let rect = Rect::from_min_size(row.min + vec2(i as f32 * BUTTON_W, 0.0), vec2(BUTTON_W, BUTTON_H));
        // Click without FOCUSABLE: Tab never lands here.
        let r = ui.interact(rect, ui.id().with(("window-button", i)), Sense::CLICK);
        let name = kind.name(t);
        r.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, name));
        let (mut hovered, mut pressed) = (r.hovered(), r.is_pointer_button_down_on());
        if matches!(kind, Kind::Maximize | Kind::Restore) {
            hovered |= native.hovered;
            pressed |= native.drawn_pressed();
        }
        let close = kind == Kind::Close;
        let fill = match (close, pressed, hovered) {
            (true, true, _) => Some(color::danger().gamma_multiply(0.8)),
            (true, false, true) => Some(color::danger()),
            (false, true, _) => Some(color::selected()),
            (false, false, true) => Some(color::hover()),
            _ => None,
        };
        let p = ui.painter();
        if let Some(fill) = fill {
            p.rect_filled(rect, 0.0, fill);
        }
        let ink = if close && (hovered || pressed) { Color32::WHITE } else { color::text() };
        paint_glyph(p, rect.center(), kind, Stroke::new(1.0_f32, ink));
        // egui's click too: tests and accessibility tools press it without the OS.
        if r.on_hover_text(name).clicked() {
            msgs.push(kind.msg());
        }
    }
}

/// A 10 px glyph centred on `c`, on whole pixels so the 1 px lines stay sharp.
fn paint_glyph(p: &Painter, c: Pos2, kind: Kind, s: Stroke) {
    let px = p.ctx().pixels_per_point();
    let snap = |v: f32| ((v * px).round() + 0.5) / px;
    let c = pos2(snap(c.x), snap(c.y));
    let line = |a: Pos2, b: Pos2| {
        p.line_segment([a, b], s);
    };
    match kind {
        Kind::Minimize => line(pos2(c.x - 5.0, c.y), pos2(c.x + 5.0, c.y)),
        Kind::Maximize => {
            p.rect_stroke(Rect::from_center_size(c, vec2(10.0, 10.0)), 0.0, s, egui::StrokeKind::Middle);
        }
        Kind::Restore => {
            // The front square, and the parts of the one behind it (up and right) it doesn't cover.
            let front = Rect::from_min_max(pos2(c.x - 5.0, c.y - 3.0), pos2(c.x + 3.0, c.y + 5.0));
            p.rect_stroke(front, 0.0, s, egui::StrokeKind::Middle);
            let (l, t, r, b) = (front.left() + 2.0, front.top() - 2.0, front.right() + 2.0, front.bottom() - 2.0);
            line(pos2(l, front.top()), pos2(l, t));
            line(pos2(l, t), pos2(r, t));
            line(pos2(r, t), pos2(r, b));
            line(pos2(r, b), pos2(front.right(), b));
        }
        Kind::Close => {
            line(pos2(c.x - 5.0, c.y - 5.0), pos2(c.x + 5.0, c.y + 5.0));
            line(pos2(c.x - 5.0, c.y + 5.0), pos2(c.x + 5.0, c.y - 5.0));
        }
    }
}
