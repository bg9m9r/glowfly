//! A split button: an action and its menu caret joined into one pill (the commit box's Commit
//! and its options), parted by a 1 px line. Each half has its own hover, press and focus; the
//! caret half stays latched while its menu is `open`, can carry a tone (`menu_tone`: tinted
//! fill, caret in the tone's colour) so a choice in the menu stays visible while it is closed,
//! and can be clicked while the action itself is disabled. The caller opens the menu, as the
//! toolbar's split buttons do.

use super::button_look::{self, Press};
use super::buttons::{Kind, StyledButton};
use crate::ui::access;
use crate::ui::theme::color;
use crate::ui::theme::surfaces::Tone;
use crate::ui::theme::tokens::{self, radius};
use eframe::egui::{CursorIcon, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

/// The caret half's width.
pub const CARET_W: f32 = 28.0;
/// How far the divider is shaded from the action's fill.
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
    /// Whether the menu is open: the caret half stays latched.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Whether the caret can be clicked (true by default, even while the action is disabled).
    #[allow(dead_code)]
    pub fn menu_enabled(mut self, enabled: bool) -> Self {
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
        let kind = button.kind();
        let main_on = button.is_enabled() && ui.is_enabled();
        let main = button.show_shaped(ui, radius::left(radius::MD), false);
        ui.add_space(-ui.spacing().item_spacing.x);
        let menu_enabled = menu_enabled && ui.is_enabled();
        let sense = if menu_enabled { Sense::click() } else { Sense::hover() };
        let (rect, caret) = ui.allocate_exact_size(vec2(CARET_W, main.rect.height()), sense);
        access::name_button(&caret, menu_enabled, menu_name);
        if ui.is_rect_visible(rect) {
            let p = color::current();
            let press = if menu_enabled { Press::of(caret.hovered(), caret.is_pointer_button_down_on(), open) } else { Press::Rest };
            // While the action is disabled the caret keeps the disabled fill, with state layers
            // and full text so it still reads as clickable.
            let mut look = button_look::look(kind, menu_enabled && main_on, press);
            let layered = |fill| match press {
                Press::Rest => fill,
                Press::Hover => p.hover_on(fill),
                Press::Pressed => p.pressed_on(fill),
                Press::Latched => p.latched_on(fill),
            };
            if menu_enabled && !main_on {
                look.fill = layered(look.fill);
                look.ink = p.text;
            }
            if let Some(t) = menu_tone.filter(|_| menu_enabled) {
                look.fill = layered(p.tone_tint(t).0);
                look.ink = p.tone(t);
            }
            let painter = ui.painter();
            painter.rect(rect, radius::right(radius::MD), look.fill, look.edge, StrokeKind::Inside);
            let main_fill = button_look::look(kind, main_on, Press::Rest).fill;
            let line = if kind == Kind::Quiet { p.border } else { p.shade_fill(main_fill, DIVIDER_SHADE) };
            let x = rect.left().round() + 0.5;
            let inset = if look.edge.width > 0.0 { 1.0 } else { 0.0 };
            painter.line_segment([pos2(x, rect.top() + inset), pos2(x, rect.bottom() - inset)], Stroke::new(1.0_f32, line));
            super::caret(painter, rect.center(), look.ink);
            if caret.has_focus() {
                tokens::focus_ring(painter, rect, radius::MD);
            }
        }
        let caret = if menu_enabled { caret.on_hover_cursor(CursorIcon::PointingHand) } else { caret };
        (main, caret)
    }
}
