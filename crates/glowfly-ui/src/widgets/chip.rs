//! The only chip and badge painter: a filled shape of one of the two chip sizes
//! (`theme::sizes::ChipSize`): an 18 px Badge pill for counts and status letters, or a 22 px
//! Chip for refs, labels, identities and the like. Each takes its height, side padding, text
//! step, leading glyph size and gap, and corners from its size; the text sits on the size's
//! padding and is centred by `sizes::text_pos`. A [`Chip`] paints with a painter (`paint`) or in
//! its own rect in a layout (`show`), may lead with a dot or a painted glyph and may cut its
//! text with "…" to a width. A view that lays out its own content (several runs of text) paints
//! the shape with [`frame`] in a rect from [`rect_at`] and [`width`]. The clickable quiet chip
//! (a commit's SHA, parents and committer) is [`quiet_chip`].

use super::button_look::{Press, Target, paint_target};
use super::truncate::{cut_galley, cut_job};
use crate::ui::theme::color;
use crate::ui::theme::ref_chip::ChipLook;
pub use crate::ui::theme::sizes::ChipSize;
use crate::ui::theme::sizes::{self, ControlSize};
use crate::ui::theme::type_scale;
use eframe::egui::{
    self, Color32, CornerRadius, FontId, Galley, Painter, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, WidgetText, pos2, vec2,
};
use std::sync::Arc;

/// A chip's colours: its fill, an optional 1 px edge inside its outline, and its text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub fill: Color32,
    pub edge: Option<Color32>,
    pub ink: Color32,
}

impl Look {
    /// `ink` text on `fill`, without an edge.
    pub fn new(fill: Color32, ink: Color32) -> Self {
        Self { fill, edge: None, ink }
    }

    /// This look with a 1 px `edge`.
    pub fn edged(self, edge: Color32) -> Self {
        Self { edge: Some(edge), ..self }
    }
}

impl From<ChipLook> for Look {
    /// A ref chip's look; an outlined one (no fill) is transparent inside its edge. Its halo is
    /// painted apart, with [`paint_halo`].
    fn from(l: ChipLook) -> Self {
        Self { fill: l.fill.unwrap_or(Color32::TRANSPARENT), edge: l.edge, ink: l.text }
    }
}

/// What a chip shows before its text, in the size's glyph slot and gap.
#[derive(Clone, Copy)]
pub enum Lead<'a> {
    /// A filled dot in a colour.
    Dot(Color32),
    /// A shape painted by the caller, given the slot's centre and the size's glyph size.
    Paint(&'a dyn Fn(&Painter, Pos2, f32)),
}

/// The width of a chip of `size` holding `content_w` of content: its padding either side, at
/// least the size's least width.
pub fn width(size: ChipSize, content_w: f32) -> f32 {
    let m = size.metrics();
    (content_w + 2.0 * m.pad_x).max(m.min_w)
}

/// The rect of a chip of `size`, `w` wide, its left edge at `x`, centred on the line at `mid`
/// (its top on a whole point, as `sizes::text_pos` places text).
pub fn rect_at(size: ChipSize, x: f32, mid: f32, w: f32) -> Rect {
    let h = size.metrics().h;
    Rect::from_min_size(pos2(x, (mid - h / 2.0).floor()), vec2(w, h))
}

/// The corners of a chip of `size`.
fn corners(size: ChipSize) -> CornerRadius {
    CornerRadius::same(size.metrics().radius)
}

/// The halo round a ref chip (the checked-out branch, a detached HEAD): its gap from the chip.
const HALO_GAP: f32 = 1.5;
/// The halo's ring width.
const HALO_W: f32 = 1.5;
/// How far a halo reaches past its chip on every side: its gap and ring (a 22 px chip's halo is
/// 28 px tall).
pub const HALO_ROOM: f32 = HALO_GAP + HALO_W;

/// Paints the halo round a chip of `size` in `rect`: a `HALO_W` ring `HALO_GAP` outside it, its
/// corners the chip's plus `HALO_ROOM`.
pub fn paint_halo(p: &Painter, rect: Rect, size: ChipSize, colour: Color32) {
    let r = CornerRadius::same(size.metrics().radius + HALO_ROOM as u8);
    p.rect(rect.expand(HALO_ROOM), r, Color32::TRANSPARENT, Stroke::new(HALO_W, colour), StrokeKind::Inside);
}

/// Paints a chip's shape in `rect`: the fill and, when `look` has one, its edge.
pub fn frame(p: &Painter, rect: Rect, size: ChipSize, look: Look) {
    match look.edge {
        Some(edge) => p.rect(rect, corners(size), look.fill, Stroke::new(1.0_f32, edge), StrokeKind::Inside),
        None => p.rect_filled(rect, corners(size), look.fill),
    };
}

/// Where content `content_w` wide starts in a chip `rect` of `size`: its padding in from the
/// left, or centred when the chip is not as wide as its content and padding (a one-letter badge
/// at its least width, or a status letter's fixed square).
fn content_left(rect: Rect, size: ChipSize, content_w: f32) -> f32 {
    let pad = size.metrics().pad_x;
    if (content_w + 2.0 * pad - rect.width()).abs() > 0.5 { rect.center().x - content_w / 2.0 } else { rect.left() + pad }
}

/// A chip: `text` in its size's text step, a look, an optional lead
/// and an optional widest.
pub struct Chip<'a> {
    size: ChipSize,
    text: String,
    look: Look,
    lead: Option<Lead<'a>>,
    max_w: f32,
    trail: f32,
    latched: bool,
}

impl<'a> Chip<'a> {
    /// `text` as a chip of `size`, in the text colour on the raised fill.
    pub fn new(size: ChipSize, text: impl Into<String>) -> Self {
        let look = Look::new(color::raised(), color::text());
        Self { size, text: text.into(), look, lead: None, max_w: f32::INFINITY, trail: 0.0, latched: false }
    }

    pub fn look(mut self, look: impl Into<Look>) -> Self {
        self.look = look.into();
        self
    }

    pub fn lead(mut self, lead: Lead<'a>) -> Self {
        self.lead = Some(lead);
        self
    }

    /// The chip's widest: past it, the text is cut with "…".
    pub fn max_w(mut self, max_w: f32) -> Self {
        self.max_w = max_w;
        self
    }

    /// Room kept after the text, inside the right padding, for a control the caller paints
    /// there (a picker chip's ×).
    pub fn trail(mut self, w: f32) -> Self {
        self.trail = w;
        self
    }

    /// A `button` chip holds the latched layer while its menu is open.
    pub fn latched(mut self, latched: bool) -> Self {
        self.latched = latched;
        self
    }

    fn font(&self) -> FontId {
        type_scale::font(self.size.metrics().text)
    }

    /// The lead's slot and gap.
    fn lead_w(&self) -> f32 {
        let m = self.size.metrics();
        if self.lead.is_some() { m.glyph + m.gap } else { 0.0 }
    }

    fn galley(&self, ctx: &egui::Context) -> Arc<Galley> {
        let room = (self.max_w - 2.0 * self.size.metrics().pad_x - self.lead_w() - self.trail).max(0.0);
        cut_galley(ctx, cut_job(self.text.clone(), self.font(), self.look.ink), room)
    }

    /// The chip's width.
    pub fn width(&self, ctx: &egui::Context) -> f32 {
        width(self.size, self.lead_w() + self.galley(ctx).size().x + self.trail)
    }

    /// Paints the chip with its left edge at `x`, centred on the line at `mid`; returns its rect.
    pub fn paint(&self, p: &Painter, x: f32, mid: f32) -> Rect {
        let galley = self.galley(p.ctx());
        let rect = rect_at(self.size, x, mid, width(self.size, self.lead_w() + galley.size().x + self.trail));
        self.paint_in(p, rect, galley);
        rect
    }

    /// Paints the chip in its own rect in `ui`'s layout, named by its text for accessibility;
    /// returns its hover response.
    pub fn show(&self, ui: &mut Ui) -> Response {
        let galley = self.galley(ui.ctx());
        let w = width(self.size, self.lead_w() + galley.size().x + self.trail);
        let (rect, resp) = ui.allocate_exact_size(vec2(w, self.size.metrics().h), Sense::hover());
        if ui.is_rect_visible(rect) {
            self.paint_in(ui.painter(), rect, galley);
        }
        let text = self.text.clone();
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &text));
        resp
    }

    /// The chip as a clickable target in its own rect in `ui`'s layout (the signature badge): the
    /// quiet hover and pressed layers over its look, the inset focus ring, the pointing hand,
    /// named by its text; returns its click response.
    pub fn button(&self, ui: &mut Ui) -> Response {
        let galley = self.galley(ui.ctx());
        let w = width(self.size, self.lead_w() + galley.size().x + self.trail);
        let (rect, resp) = ui.allocate_exact_size(vec2(w, self.size.metrics().h), Sense::click());
        self.button_look(ui, rect, galley, resp)
    }

    /// `button` at `rect`, a place the caller fixed (a section header's filter chip), under
    /// `id`; the text is cut to the rect's width.
    pub fn button_at(&self, ui: &mut Ui, id: egui::Id, rect: Rect) -> Response {
        let resp = ui.interact(rect, id, Sense::click());
        let fitted = Chip { lead: self.lead, text: self.text.clone(), max_w: rect.width(), ..*self };
        let galley = fitted.galley(ui.ctx());
        self.button_look(ui, rect, galley, resp)
    }

    fn button_look(&self, ui: &Ui, rect: Rect, galley: Arc<Galley>, resp: Response) -> Response {
        if ui.is_rect_visible(rect) {
            let p = ui.painter();
            self.paint_in(p, rect, galley);
            let press = Press::of(resp.hovered(), resp.is_pointer_button_down_on(), self.latched);
            // Smaller than the hit area, as a Compact control is: quiet layers, the ring inset.
            let target = Target { rect, corners: corners(self.size), edge: None };
            paint_target(p, target, ControlSize::Compact, press, resp.has_focus());
        }
        let text = self.text.clone();
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &text));
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    }

    /// Paints the chip in `rect`, a size the caller fixed (a status letter's square); its
    /// content centres when the rect is not its natural width.
    pub fn paint_in_rect(&self, p: &Painter, rect: Rect) {
        self.paint_in(p, rect, self.galley(p.ctx()));
    }

    fn paint_in(&self, p: &Painter, rect: Rect, galley: Arc<Galley>) {
        let m = self.size.metrics();
        frame(p, rect, self.size, self.look);
        let mut x = content_left(rect, self.size, self.lead_w() + galley.size().x + self.trail);
        let text_top = sizes::text_pos(rect, &galley).y;
        if let Some(lead) = self.lead {
            // The glyph centres on the text's line, as icons do beside text.
            let c = pos2(x + m.glyph / 2.0, text_top + galley.size().y / 2.0);
            match lead {
                Lead::Dot(col) => {
                    p.circle_filled(c, (m.glyph * 0.6 / 2.0).round(), col);
                }
                Lead::Paint(paint) => paint(p, c, m.glyph),
            }
            x += m.glyph + m.gap;
        }
        p.galley(pos2(x, text_top), galley, self.look.ink);
    }
}

/// A quiet chip that can be clicked (a commit's SHA, its parents, its committer): `text` (the
/// Chip size's step unless it sets its own font, in its own colour or the text colour) on the
/// toolbar fill, with no outline, named `name` for accessibility. It is a quiet target over that
/// fill (`button_look::paint_target`): the hover layer, the pressed layer while held, and the
/// inset accent ring when it has the keyboard focus; and it shows the pointing hand. Callers
/// attach the tooltip and any menu.
pub fn quiet_chip(ui: &mut Ui, text: impl Into<WidgetText>, name: &str) -> Response {
    let size = ChipSize::Chip;
    let m = size.metrics();
    let galley = text.into().into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, type_scale::font(m.text));
    let (rect, resp) = ui.allocate_exact_size(vec2(width(size, galley.size().x), m.h), Sense::click());
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        frame(p, rect, size, Look::new(color::toolbar(), color::text()));
        let press = Press::of(resp.hovered(), resp.is_pointer_button_down_on(), false);
        // Smaller than the hit area, as a Compact control is: quiet layers, the ring inset.
        paint_target(p, Target { rect, corners: corners(size), edge: None }, ControlSize::Compact, press, resp.has_focus());
        let pos = pos2(rect.left() + m.pad_x, sizes::text_pos(rect, &galley).y);
        p.galley(pos, galley, color::text());
    }
    let name = name.to_owned();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &name));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

#[cfg(test)]
#[path = "chip_tests.rs"]
mod tests;
