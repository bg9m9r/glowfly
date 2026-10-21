//! A binary file's body: a changed image shows Before and After side by side, each image
//! fitted in its half over a checkerboard (so transparency shows) with its size under it; a
//! missing side shows a dashed "Added" or "Deleted" card and a side that can't be decoded says
//! so with its size. Any other binary file shows the binary empty state with both sizes. While
//! the sides load (or when they failed) the plain binary empty state shows.

use super::empty_state::{self, EmptyIcon};
use crate::state::AppState;
use crate::state::diff_preview;
use crate::ui::image_paint::{checkerboard, load};
use crate::ui::theme::color;
use crate::ui::theme::tokens::radius;
use crate::ui::theme::type_scale;
use eframe::egui::{self, Align, Color32, Context, CornerRadius, Layout, Rect, RichText, Stroke, TextureHandle, Ui, UiBuilder, pos2, vec2};
use std::sync::Arc;
use trellis_core::lfs::pointer::size_text;
use trellis_core::preview_image::{PreviewImage, is_image_path};
use trellis_core::repo::{BinarySide, BinarySides};

/// Around the two columns, and between them.
const PAD: f32 = 16.0;
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
}

impl PreviewTextures {
    fn sync(&mut self, ctx: &Context, sides: &Arc<BinarySides>) {
        if self.key.as_ref().is_some_and(|k| Arc::ptr_eq(k, sides)) {
            return;
        }
        fn image(s: &Option<BinarySide>) -> Option<&PreviewImage> {
            s.as_ref().and_then(|s| s.image.as_ref())
        }
        self.old = image(&sides.old).map(|i| load(ctx, "diff-image-old", i));
        self.new = image(&sides.new).map(|i| load(ctx, "diff-image-new", i));
        self.key = Some(sides.clone());
    }

    fn clear(&mut self) {
        if self.key.is_some() {
            *self = Self::default();
        }
    }
}

/// Draws the binary diff on screen's body.
pub(super) fn show(ui: &mut Ui, st: &AppState, tex: &mut PreviewTextures) {
    let (Some(sides), Some((target, _))) = (diff_preview::shown(st), st.diff.as_ref()) else {
        tex.clear();
        empty_state::binary_file(ui);
        return;
    };
    let any_image = sides.old.iter().chain(&sides.new).any(|s| s.image.is_some());
    if !any_image && !is_image_path(target.path()) {
        tex.clear();
        empty_state::binary_sizes(ui, sides.old.as_ref().map(|s| s.size), sides.new.as_ref().map(|s| s.size));
        return;
    }
    tex.sync(ui.ctx(), sides);
    let area = ui.available_rect_before_wrap();
    let col_w = ((area.width() - 2.0 * PAD - GAP) / 2.0).max(40.0);
    let line_h = 18.0;
    let card_h = (area.height() - 2.0 * PAD - 2.0 * (line_h + 8.0)).clamp(CARD_MIN_H, CARD_MAX_H);
    let columns = [(BEFORE, &sides.old, &tex.old, ADDED), (AFTER, &sides.new, &tex.new, DELETED)];
    for (i, (title, side, texture, missing)) in columns.into_iter().enumerate() {
        let left = area.left() + PAD + i as f32 * (col_w + GAP);
        let col = Rect::from_min_size(pos2(left, area.top() + PAD), vec2(col_w, line_h + 8.0 + card_h + 8.0 + line_h));
        ui.scope_builder(UiBuilder::new().max_rect(col).layout(Layout::top_down(Align::Min)), |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.label(RichText::new(title).font(type_scale::font(type_scale::SMALL)).color(color::weak()));
            let (card, _) = ui.allocate_exact_size(vec2(col_w, card_h), egui::Sense::hover());
            match (side, texture) {
                (None, _) => placeholder(ui, card, missing),
                (Some(s), Some(t)) => {
                    let img = s.image.as_ref().expect("a texture comes from an image");
                    image_card(ui, card, t, img);
                    caption(ui, &format!("{} \u{d7} {} px \u{b7} {}", img.width, img.height, size_text(s.size)));
                }
                (Some(s), None) => undecodable(ui, card, s.size),
            }
        });
    }
    ui.allocate_rect(area, egui::Sense::hover());
}

fn caption(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).font(type_scale::font(type_scale::SMALL)).color(color::weak()));
}

/// The image fitted in `card`, centred over a checkerboard.
fn image_card(ui: &Ui, card: Rect, t: &TextureHandle, img: &PreviewImage) {
    let p = ui.painter();
    p.rect_filled(card, CornerRadius::same(radius::MD), color::raised());
    let room = card.shrink(PAD);
    let (w, h) = (img.width.max(1) as f32, img.height.max(1) as f32);
    let scale = (room.width() / w).min(room.height() / h).min(MAX_ZOOM);
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
    ui.painter().rect_filled(card, CornerRadius::same(radius::MD), color::raised());
    centred(ui, card, |ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(40.0, 40.0), egui::Sense::hover());
        empty_state::paint(ui.painter(), rect.center(), EmptyIcon::Binary, color::weak());
        ui.add_space(8.0);
        ui.label(RichText::new(CANT_PREVIEW).color(color::text()));
        ui.label(RichText::new(size_text(size)).font(type_scale::font(type_scale::SMALL)).color(color::weak()));
    });
}

/// Draws `add` centred in `card`, about 90 px tall.
fn centred(ui: &mut Ui, card: Rect, add: impl FnOnce(&mut Ui)) {
    let block = Rect::from_center_size(card.center(), vec2(card.width(), 90.0_f32.min(card.height())));
    let mut child = ui.new_child(UiBuilder::new().max_rect(block).layout(Layout::top_down(Align::Center)));
    add(&mut child);
}
