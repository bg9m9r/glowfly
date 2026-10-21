//! The welcome's Start step: three large cards (open, clone, start a repository), each one
//! button with a painted icon on a tinted tile, its title and a line of description. Side by
//! side when each card gets `CARD_MIN_W`, else stacked as full-width rows.

use super::theme::color;
use super::theme::contrast;
use super::welcome_art;
use crate::msg::Msg;
use crate::onboarding::{OnboardingMsg, StartChoice};
use crate::ui::theme::tokens::{self, radius};
use crate::ui::theme::type_scale;
use eframe::egui::{
    self, Align, Color32, CornerRadius, FontId, Layout, Painter, Pos2, Rect, Response, RichText, Sense, Shape, Stroke, StrokeKind, Ui,
    UiBuilder, pos2, vec2,
};
use glowfly_core::i18n::Lang;

/// Between two cards side by side, and between stacked rows.
const GAP: f32 = 12.0;
const ROW_GAP: f32 = 8.0;
/// Each card's least width side by side; narrower, the cards stack.
const CARD_MIN_W: f32 = 180.0;
/// A card's padding, its icon tile, and the gaps after the tile and the title.
const PAD: f32 = 16.0;
const TILE: f32 = 40.0;
const TILE_GAP: f32 = 12.0;
const TITLE_GAP: f32 = 4.0;
/// The least height side by side and stacked.
const MIN_H: f32 = 132.0;
const ROW_MIN_H: f32 = 64.0;
const RADIUS: u8 = radius::LG;

/// The three cards; a click (or Enter or Space on a focused card) finishes with its choice.
pub(super) fn choices(ui: &mut Ui, t: Lang, msgs: &mut Vec<Msg>) {
    let cards = [
        (StartChoice::Open, t.tr("common.open_repo.button"), t.tr("welcome.start.open.hint")),
        (StartChoice::Clone, t.tr("common.clone_repo.button"), t.tr("welcome.start.clone.hint")),
        (StartChoice::Init, t.tr("common.init_repo.button"), t.tr("welcome.start.init.hint")),
    ];
    let avail = ui.available_width();
    let side = (avail - 2.0 * GAP) / 3.0;
    let wide = side >= CARD_MIN_W;
    let title_font = type_scale::strong(ui, type_scale::BODY);
    let text_w = if wide { side - 2.0 * PAD } else { avail - 2.0 * PAD - TILE - TILE_GAP };
    let height = |text: &str, font: &FontId| ui.painter().layout(text.to_owned(), font.clone(), Color32::PLACEHOLDER, text_w).size().y;
    let hint_font = type_scale::font(type_scale::SMALL);
    let texts = cards.map(|(_, title, hint)| height(title, &title_font) + TITLE_GAP + height(hint, &hint_font));
    let tallest = texts.iter().copied().fold(0.0_f32, f32::max);
    if wide {
        let h = (PAD + TILE + TILE_GAP + tallest + PAD).max(MIN_H);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for (entry, text_h) in cards.into_iter().zip(texts) {
                card(ui, Fit { size: vec2(side, h), wide: true, text_h }, entry, &title_font, msgs);
            }
        });
    } else {
        let h = (PAD + TILE.max(tallest) + PAD).max(ROW_MIN_H);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = ROW_GAP;
            for (entry, text_h) in cards.into_iter().zip(texts) {
                card(ui, Fit { size: vec2(avail, h), wide: false, text_h }, entry, &title_font, msgs);
            }
        });
    }
}

/// How one card is laid out: its size, side by side (`wide`) or stacked, and its texts' height.
struct Fit {
    size: egui::Vec2,
    wide: bool,
    text_h: f32,
}

/// One card: the tile on top (side by side) or on the left (stacked), the title and the hint.
fn card(ui: &mut Ui, fit: Fit, (choice, title, hint): (StartChoice, &str, &str), title_font: &FontId, msgs: &mut Vec<Msg>) {
    let (rect, resp) = ui.allocate_exact_size(fit.size, Sense::click());
    paint_card(ui.painter(), rect, &resp);
    let (tile_rect, text_rect) = if fit.wide {
        let tile = Rect::from_min_size(rect.min + vec2(PAD, PAD), vec2(TILE, TILE));
        (tile, Rect::from_min_max(pos2(rect.left() + PAD, tile.bottom() + TILE_GAP), rect.max - vec2(PAD, PAD)))
    } else {
        // Stacked, the texts are centred beside the tile.
        let tile = Rect::from_min_size(pos2(rect.left() + PAD, rect.center().y - TILE / 2.0), vec2(TILE, TILE));
        let top = rect.center().y - fit.text_h / 2.0;
        (tile, Rect::from_min_max(pos2(tile.right() + TILE_GAP, top), rect.max - vec2(PAD, PAD)))
    };
    tile(ui.painter(), tile_rect, welcome_art::high_contrast(&color::current()));
    paint_choice_icon(ui.painter(), tile_rect.center(), choice, color::accent());
    // The texts are drawn over the card but don't take its clicks.
    let mut col = ui.new_child(UiBuilder::new().max_rect(text_rect).layout(Layout::top_down(Align::Min)));
    col.spacing_mut().item_spacing.y = TITLE_GAP;
    let title_galley = col.painter().layout(title.to_owned(), title_font.clone(), color::text(), text_rect.width());
    let (title_at, _) = col.allocate_exact_size(title_galley.size(), Sense::hover());
    col.painter().galley(title_at.min, title_galley, color::text());
    let hint_text = RichText::new(hint).size(type_scale::SMALL).color(color::weak());
    col.add(egui::Label::new(hint_text).wrap().selectable(false));
    crate::ui::access::name_button(&resp, true, title);
    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
        msgs.push(Msg::Onboarding(OnboardingMsg::Finish(Some(choice))));
    }
}

/// The card's fill and edge for its state, and the focus ring.
fn paint_card(p: &Painter, rect: Rect, resp: &Response) {
    let radius = CornerRadius::same(RADIUS);
    let (fill, edge) = if resp.is_pointer_button_down_on() {
        (color::selected(), color::accent())
    } else if resp.hovered() {
        (color::hover(), color::accent())
    } else {
        (color::raised(), color::border())
    };
    p.rect(rect, radius, fill, Stroke::new(1.0_f32, edge), StrokeKind::Inside);
    if resp.has_focus() {
        tokens::focus_ring(p, rect, RADIUS);
    }
}

/// The icon's rounded tile, tinted toward the accent; in high contrast the panel with an accent
/// edge.
fn tile(p: &Painter, rect: Rect, hc: bool) {
    let radius = CornerRadius::same(RADIUS);
    if hc {
        p.rect(rect, radius, color::panel(), Stroke::new(1.0_f32, color::accent()), StrokeKind::Inside);
    } else {
        p.rect_filled(rect, radius, contrast::mix(color::raised(), color::accent(), 0.14));
    }
}

/// A 24 px outline for `choice`, centred on `c`: a folder opened, one with an arrow coming down
/// into it, and one with a plus inside.
fn paint_choice_icon(p: &Painter, c: Pos2, choice: StartChoice, col: Color32) {
    let s = Stroke::new(1.6_f32, col);
    let line = |pts: Vec<Pos2>| p.add(Shape::line(pts, s));
    match choice {
        StartChoice::Open => {
            let (l, r, t, b) = (c.x - 11.0, c.x + 10.0, c.y - 8.0, c.y + 8.0);
            // The back with its tab, then the front flap tipped forward.
            line(vec![pos2(l, b), pos2(l, t), pos2(l + 7.0, t), pos2(l + 9.5, t + 3.0), pos2(r - 2.0, t + 3.0), pos2(r - 2.0, t + 7.0)]);
            p.add(Shape::closed_line(vec![pos2(l, b), pos2(l + 4.0, t + 7.0), pos2(r + 2.0, t + 7.0), pos2(r - 2.0, b)], s));
        }
        StartChoice::Clone => {
            let (l, r, t, b) = (c.x - 11.0, c.x + 11.0, c.y - 4.0, c.y + 10.0);
            folder(p, l, r, t, b, s);
            p.line_segment([pos2(c.x, c.y - 12.0), pos2(c.x, c.y + 5.0)], s);
            line(vec![pos2(c.x - 4.0, c.y + 1.0), pos2(c.x, c.y + 5.0), pos2(c.x + 4.0, c.y + 1.0)]);
        }
        StartChoice::Init => {
            let (l, r, t, b) = (c.x - 11.0, c.x + 11.0, c.y - 8.0, c.y + 9.0);
            folder(p, l, r, t, b, s);
            let mid = pos2(c.x, (t + 3.0 + b) / 2.0);
            p.line_segment([mid - vec2(4.5, 0.0), mid + vec2(4.5, 0.0)], s);
            p.line_segment([mid - vec2(0.0, 4.5), mid + vec2(0.0, 4.5)], s);
        }
    }
}

/// A closed folder: its tab on the top left, its body from just under the tab's top.
fn folder(p: &Painter, l: f32, r: f32, t: f32, b: f32, s: Stroke) {
    p.add(Shape::line(vec![pos2(l, t + 3.0), pos2(l, t), pos2(l + 7.0, t), pos2(l + 9.5, t + 3.0)], s));
    p.rect_stroke(Rect::from_min_max(pos2(l, t + 3.0), pos2(r, b)), CornerRadius::same(2), s, StrokeKind::Middle);
}
