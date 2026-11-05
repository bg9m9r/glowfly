//! A binary file's body: a changed image shows Before and After side by side, each image
//! fitted in its half over a checkerboard (so transparency shows) with its size under it; a
//! missing side shows a dashed "Added" or "Deleted" card and a side that can't be decoded says
//! so with its size. An SVG (binary, or text with the header's switch on Preview) shows the same
//! way, drawn by `preview_svg`, with no zoom limit: it fills its card; a side too large to draw
//! or unreadable says so, a drawing that names files outside itself says they aren't shown,
//! and while the sides load the cards stand empty. A font (TrueType, OpenType or a collection)
//! shows its specimen the same way, drawn in each side's own font at one shared scale
//! (`preview_font`), with the same too-large, unreadable and loading cards. Any other binary file shows its bytes in the hex view (`hex`) when both
//! sides are within the cap, or else the binary empty state with both sizes (and, over the cap,
//! why). While
//! the sides load (or when they failed) the plain binary empty state shows.

use super::empty_state::{self, EmptyIcon};
use super::preview_font;
use crate::image_paint::{checkerboard, load, load_smooth};
use crate::theme::color;
use crate::theme::tokens::radius;
use crate::theme::type_scale;
use eframe::egui::{self, Align, Color32, Context, CornerRadius, Layout, Rect, RichText, Stroke, TextureHandle, Ui, UiBuilder, pos2, vec2};
use glowfly_core::i18n::Lang;
use glowfly_core::lfs::pointer::size_text;
use glowfly_core::preview_font::{FONT_MAX_BYTES, is_font_path};
use glowfly_core::preview_image::{PreviewImage, is_image_path};
use glowfly_core::preview_svg::{SVG_MAX_BYTES, is_svg_path};
use glowfly_core::repo::{BinarySide, BinarySides, HEX_MAX_BYTES};
use glowfly_model::state::AppState;
use glowfly_model::state::diff_preview;
use std::sync::Arc;

/// Around the two columns, and between them; also inside a card, around what it shows.
pub(super) const PAD: f32 = 16.0;
const GAP: f32 = 16.0;
/// Never drawn larger than this many times its own size.
const MAX_ZOOM: f32 = 4.0;
/// The card's least and greatest height.
const CARD_MIN_H: f32 = 120.0;
const CARD_MAX_H: f32 = 560.0;

pub(super) const BEFORE: &str = "Before";
pub(super) const AFTER: &str = "After";
pub(super) const ADDED: &str = "Added";
pub(super) const DELETED: &str = "Deleted";
pub(super) const CANT_PREVIEW: &str = "Can't preview this image";

/// The before and after textures, made from the sides on screen and kept until they change.
#[derive(Default)]
pub struct PreviewTextures {
    /// The sides the textures were made from (held, so a new allocation can't look the same).
    key: Option<Arc<BinarySides>>,
    old: Option<TextureHandle>,
    new: Option<TextureHandle>,
    /// The hex view's rows, for a binary file that isn't an image.
    hex: super::hex::HexLines,
}

impl PreviewTextures {
    fn sync(&mut self, ctx: &Context, sides: &Arc<BinarySides>) {
        if self.key.as_ref().is_some_and(|k| Arc::ptr_eq(k, sides)) {
            return;
        }
        fn image(s: &Option<BinarySide>) -> Option<&PreviewImage> {
            s.as_ref().and_then(|s| s.image.as_ref())
        }
        // A font's specimen is anti-aliased text: always smooth.
        let texture = |s: &Option<BinarySide>, name: &str| {
            let font = s.as_ref().is_some_and(|s| s.font.is_some());
            image(s).map(|i| if font { load_smooth(ctx, name, i) } else { load(ctx, name, i) })
        };
        self.old = texture(&sides.old, "diff-image-old");
        self.new = texture(&sides.new, "diff-image-new");
        self.key = Some(sides.clone());
    }

    /// Drops the textures, keeping the hex rows (the hex view calls this every frame).
    fn clear(&mut self) {
        if self.key.is_some() {
            (self.key, self.old, self.new) = (None, None, None);
        }
    }
}

/// Draws the binary diff on screen's body.
pub(super) fn show(ui: &mut Ui, st: &AppState, tex: &mut PreviewTextures) {
    let t = st.settings.lang();
    let path = st.session.diff.view.as_ref().map(|(target, _)| target.path());
    let svg = path.is_some_and(is_svg_path);
    let font_path = path.is_some_and(is_font_path);
    // What an undrawable side's card says: the font's or the SVG's words and cap.
    let words =
        |font: bool| if font { (FONT_MAX_BYTES, t.tr("diff.font.cant_preview")) } else { (SVG_MAX_BYTES, t.tr("diff.svg.cant_preview")) };
    let (limit, cant) = words(font_path);
    let (Some(sides), Some((target, _))) = (diff_preview::shown(st), st.session.diff.view.as_ref()) else {
        tex.clear();
        match st.session.diff_preview.as_ref().and_then(|p| p.sides.as_ref()) {
            // An SVG's or a font's cards stand empty while its sides load, and say so when they failed.
            Some(Err(_)) if svg || font_path => columns(ui, |ui, _, card| cant_draw(ui, card, t, None, limit, cant)),
            _ if svg || font_path => columns(ui, |ui, _, card| {
                ui.painter().rect_filled(card, CornerRadius::same(radius::MD), color::raised());
            }),
            _ => empty_state::binary_file(ui),
        }
        return;
    };
    let font = font_path || sides.old.iter().chain(&sides.new).any(|s| s.font.is_some());
    let (limit, cant) = words(font);
    let any_image = sides.old.iter().chain(&sides.new).any(|s| s.image.is_some());
    if !any_image && !is_image_path(target.path()) && !svg && !font {
        tex.clear();
        if sides.hex().is_some() {
            return super::hex::show(ui, st, sides, &mut tex.hex);
        }
        let too_large = sides.old.iter().chain(&sides.new).any(|s| s.size > HEX_MAX_BYTES);
        let note = too_large.then(|| st.settings.lang().tr("diff.hex.too_large"));
        empty_state::binary_sizes(ui, sides.old.as_ref().map(|s| s.size), sides.new.as_ref().map(|s| s.size), note);
        return;
    }
    tex.sync(ui.ctx(), sides);
    let textures = [&tex.old, &tex.new];
    fn specimen(s: &Option<BinarySide>) -> Option<&PreviewImage> {
        s.as_ref().filter(|s| s.font.is_some()).and_then(|s| s.image.as_ref())
    }
    let specimens = [specimen(&sides.old), specimen(&sides.new)];
    columns(ui, |ui, i, card| {
        let (side, missing) = if i == 0 { (&sides.old, ADDED) } else { (&sides.new, DELETED) };
        match (side, textures[i]) {
            (None, _) => placeholder(ui, card, missing),
            (Some(s @ BinarySide { font: Some(face), image: Some(img), .. }), Some(tx)) => {
                let scale = preview_font::shared_scale(card, specimens);
                preview_font::card(ui, card, tx, img, scale);
                preview_font::caption(ui, t, face, img, s.size);
            }
            (Some(s), Some(tx)) => {
                let img = s.image.as_ref().expect("a texture comes from an image");
                image_card(ui, card, tx, img, if svg { f32::INFINITY } else { MAX_ZOOM });
                let text = format!("{} \u{d7} {} px \u{b7} {}", img.width, img.height, size_text(s.size));
                if s.linked {
                    linked_caption(ui, t, &text);
                } else {
                    caption(ui, &text);
                }
            }
            (Some(s), None) if svg || font => cant_draw(ui, card, t, Some(s.size), limit, cant),
            (Some(s), None) => undecodable(ui, card, s.size),
        }
    });
}

/// Lays out the Before and After columns, each a caption over a card, and calls `card` for
/// each (0 Before, 1 After) to fill its card and add what goes under it.
fn columns(ui: &mut Ui, mut card: impl FnMut(&mut Ui, usize, Rect)) {
    let area = ui.available_rect_before_wrap();
    let col_w = ((area.width() - 2.0 * PAD - GAP) / 2.0).max(40.0);
    let line_h = 18.0;
    let card_h = (area.height() - 2.0 * PAD - 2.0 * (line_h + 8.0)).clamp(CARD_MIN_H, CARD_MAX_H);
    for (i, title) in [BEFORE, AFTER].into_iter().enumerate() {
        let left = area.left() + PAD + i as f32 * (col_w + GAP);
        let col = Rect::from_min_size(pos2(left, area.top() + PAD), vec2(col_w, line_h + 8.0 + card_h + 8.0 + line_h));
        ui.scope_builder(UiBuilder::new().max_rect(col).layout(Layout::top_down(Align::Min)), |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.label(RichText::new(title).font(type_scale::font(type_scale::SMALL)).color(color::weak()));
            let (rect, _) = ui.allocate_exact_size(vec2(col_w, card_h), egui::Sense::hover());
            card(ui, i, rect);
        });
    }
    ui.allocate_rect(area, egui::Sense::hover());
}

/// The caption of an SVG that names files outside itself: the size, then "Linked files not
/// shown", weak, with why on hover.
fn linked_caption(ui: &mut Ui, t: Lang, text: &str) {
    let note = format!("{text} \u{b7} {}", t.tr("diff.svg.linked"));
    let text = RichText::new(note).font(type_scale::font(type_scale::SMALL)).color(color::weak());
    ui.label(text).on_hover_text(t.tr("diff.svg.linked.tip"));
}

/// The card of an SVG or font side that can't be drawn: too large (over `limit`), else
/// unreadable (saying `cant`), with its size when known.
fn cant_draw(ui: &mut Ui, card: Rect, t: Lang, size: Option<u64>, limit: u64, cant: &str) {
    let too_large = size.is_some_and(|s| s > limit);
    let title = if too_large { t.tr("diff.svg.too_large") } else { cant };
    let line = match size {
        Some(s) if too_large => Some(t.trf("diff.svg.too_large.size", &[("size", &size_text(s)), ("limit", &size_text(limit))])),
        Some(s) => Some(size_text(s)),
        None => None,
    };
    unpreviewable(ui, card, title, line.as_deref());
}

fn caption(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).font(type_scale::font(type_scale::SMALL)).color(color::weak()));
}

/// The image fitted in `card`, centred over a checkerboard, at most `max_zoom` times its size.
fn image_card(ui: &Ui, card: Rect, t: &TextureHandle, img: &PreviewImage, max_zoom: f32) {
    let p = ui.painter();
    p.rect_filled(card, CornerRadius::same(radius::MD), color::raised());
    let room = card.shrink(PAD);
    let (w, h) = (img.width.max(1) as f32, img.height.max(1) as f32);
    let scale = (room.width() / w).min(room.height() / h).min(max_zoom);
    let rect = Rect::from_center_size(room.center(), vec2(w * scale, h * scale));
    checkerboard(p, rect);
    p.image(t.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
}

/// A dashed card naming what happened to the side that has no file.
fn placeholder(ui: &mut Ui, card: Rect, text: &str) {
    let p = ui.painter();
    let s = Stroke::new(1.0_f32, color::faint());
    let r = card.shrink(0.5);
    let corners = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()];
    for k in 0..4 {
        let (a, b) = (corners[k], corners[(k + 1) % 4]);
        let len = (b - a).length();
        let dir = (b - a) / len;
        let mut d = 0.0;
        while d < len {
            p.line_segment([a + dir * d, a + dir * (d + 6.0).min(len)], s);
            d += 10.0;
        }
    }
    centred(ui, card, |ui| {
        ui.label(RichText::new(text).color(color::weak()));
    });
}

/// The binary page, "Can't preview this image" and the size.
fn undecodable(ui: &mut Ui, card: Rect, size: u64) {
    unpreviewable(ui, card, CANT_PREVIEW, Some(&size_text(size)));
}

/// A card with the binary page, `title` and, weak under it, `line`.
fn unpreviewable(ui: &mut Ui, card: Rect, title: &str, line: Option<&str>) {
    ui.painter().rect_filled(card, CornerRadius::same(radius::MD), color::raised());
    centred(ui, card, |ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(40.0, 40.0), egui::Sense::hover());
        empty_state::paint(ui.painter(), rect.center(), EmptyIcon::Binary, color::weak());
        ui.add_space(8.0);
        ui.label(RichText::new(title).color(color::text()));
        if let Some(line) = line {
            ui.label(RichText::new(line).font(type_scale::font(type_scale::SMALL)).color(color::weak()));
        }
    });
}

/// Draws `add` centred in `card`, about 90 px tall.
fn centred(ui: &mut Ui, card: Rect, add: impl FnOnce(&mut Ui)) {
    let block = Rect::from_center_size(card.center(), vec2(card.width(), 90.0_f32.min(card.height())));
    let mut child = ui.new_child(UiBuilder::new().max_rect(block).layout(Layout::top_down(Align::Center)));
    add(&mut child);
}
