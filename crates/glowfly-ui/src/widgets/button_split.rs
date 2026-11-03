//! A split button: an action and its menu caret joined into one shape with two targets (the
//! commit box's Commit and its options), laid out as a segment bar (`segment_bar::lay_out`).
//! Each half paints its kind's look over its own shape through `button_look::paint_button`, so
//! only the hovered half lights, and a disabled action never does; the whole shape takes one
//! edge and one 1 px divider that always shows, and each half's focus ring is inset on the half's
//! own corners, so it never reaches the other half. The caret half stays latched while its menu
//! is `open`, can carry a tone (`menu_tone`: tinted fill, caret in the tone's colour) so a choice
//! in the menu stays visible while it is closed, and can be clicked while the action itself is
//! disabled. The caller opens the menu, as the toolbar's split buttons do.

use super::button_look::{self, InsetRing, Press, Target};
use super::buttons::{Kind, StyledButton};
use super::segment_bar::{Seg, lay_out};
use crate::ui::access;
use crate::ui::theme::color;
use crate::ui::theme::sizes::ControlSize;
use crate::ui::theme::surfaces::Tone;
use crate::ui::theme::tokens::{BORDER_W, radius};
use eframe::egui::{CornerRadius, CursorIcon, Response, Stroke, StrokeKind, Ui, pos2};

/// The caret half's width.
pub const CARET_W: f32 = 28.0;
/// How far the divider is shaded from the action's fill, in a kind without an edge.
const DIVIDER_SHADE: f32 = 0.25;

/// A split button; show it with `show`.
#[must_use = "show it with .show(ui)"]
pub struct SplitButton<'a> {
    button: StyledButton<'a>,
    menu_name: &'a str,
    open: bool,
    menu_enabled: bool,
    menu_tone: Option<Tone>,
}

/// `button` with a menu caret named `menu_name` joined to its right.
pub fn split<'a>(button: StyledButton<'a>, menu_name: &'a str) -> SplitButton<'a> {
    SplitButton { button, menu_name, open: false, menu_enabled: true, menu_tone: None }
}

impl<'a> SplitButton<'a> {
    /// Both halves' size (the action's height, text and corners; Regular by default). Compact
    /// is for quiet splits only.
    pub fn size(mut self, size: ControlSize) -> Self {
        self.button = self.button.size(size);
        self
    }

    /// Whether the menu is open: the caret half stays latched.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Whether the caret can be clicked (true by default, even while the action is disabled).
    #[allow(dead_code)]
    fn menu_enabled(mut self, enabled: bool) -> Self {
        self.menu_enabled = enabled;
        self
    }

    /// Tints the caret half with tone `t`, and draws the caret in it.
    pub fn menu_tone(mut self, t: Option<Tone>) -> Self {
        self.menu_tone = t;
        self
    }

    /// Draws both halves; returns the action's and the caret's responses.
    pub fn show(self, ui: &mut Ui) -> (Response, Response) {
        let SplitButton { button, menu_name, open, menu_enabled, menu_tone } = self;
        let (kind, size, r) = (button.kind(), button.control_size(), button.radius());
        let main_on = button.live(ui);
        let menu_on = menu_enabled && ui.is_enabled();
        let (galley, main_size) = button.layout(ui);
        let segs = [Seg { w: main_size.x, enabled: main_on }, Seg { w: CARET_W, enabled: menu_on }];
        let (whole, mut halves) = lay_out(ui, main_size.y, 0.0, &segs, false);
        let caret = halves.pop().expect("the caret");
        let main = halves.pop().expect("the action");
        button.name(&main, main_on);
        access::name_button(&caret, menu_on, menu_name);
        if ui.is_rect_visible(whole) {
            let p = color::current();
            let painter = ui.painter();
            let press = |r: &Response, on: bool, latched: bool| {
                if on { Press::of(r.hovered(), r.is_pointer_button_down_on(), latched) } else { Press::Rest }
            };
            let main_look = button_look::look(kind, main_on, press(&main, main_on, false));
            let main_t = Target { rect: main.rect, corners: radius::segment(r, true, false), edge: None };
            button_look::paint_button(painter, main_t, size, main_look, false);
            let caret_press = press(&caret, menu_on, open);
            // While the action is disabled the caret keeps the disabled fill, with state layers
            // and full text so it still reads as clickable.
            let mut look = button_look::look(kind, menu_on && main_on, caret_press);
            if menu_on && !main_on {
                look.fill = button_look::layer_on(look.fill, caret_press);
                look.ink = p.text;
            }
            if let Some(t) = menu_tone.filter(|_| menu_on) {
                look.fill = button_look::layer_on(p.tone_tint(t).0, caret_press);
                look.ink = p.tone(t);
            }
            let caret_t = Target { rect: caret.rect, corners: radius::segment(r, false, true), edge: None };
            button_look::paint_button(painter, caret_t, size, look, false);
            // One edge around the whole shape, then one divider inside it.
            let rest = button_look::look(kind, main_on, Press::Rest);
            let edge = rest.edge;
            if edge.width > 0.0 {
                painter.rect_stroke(whole, CornerRadius::same(r), edge, StrokeKind::Inside);
            }
            // The divider is the edge's colour, as a segment bar's; a kind without an edge shades
            // its fill instead.
            let line = if !kind.framed() {
                p.border
            } else if edge.width > 0.0 {
                edge.color
            } else {
                p.shade_fill(rest.fill, DIVIDER_SHADE)
            };
            let x = caret.rect.left().round() + 0.5;
            let inset = if edge.width > 0.0 { edge.width } else { 0.0 };
            painter.line_segment([pos2(x, whole.top() + inset), pos2(x, whole.bottom() - inset)], Stroke::new(BORDER_W, line));
            button.paint_content(painter, main.rect, galley, main_look.ink);
            super::caret(painter, caret.rect.center(), look.ink);
            let primary = kind == Kind::Primary && main_on;
            let main_ring = InsetRing::on(primary, main_look.fill);
            button_look::paint_focus(painter, main_t, size, main.has_focus(), main_ring);
            let caret_ring = InsetRing::on(primary && menu_on && menu_tone.is_none(), look.fill);
            button_look::paint_focus(painter, caret_t, size, caret.has_focus(), caret_ring);
        }
        let main = button.explain(main, main_on);
        let caret = if menu_on { caret.on_hover_cursor(CursorIcon::PointingHand) } else { caret };
        (main, caret)
    }
}
