//! A font side of the binary diff's before/after preview: the specimen `preview_font` drew,
//! tinted to the theme's text colour on the plain raised card (no checkerboard: a font has no
//! pixels of its own), left-aligned and centred vertically, and under it the face's name, its
//! format, its face count when it is a collection, and the file's size. Both sides share one
//! scale (`shared_scale`, never enlarging), so their letters compare at the same size.

use super::preview::PAD;
use crate::theme::color;
use crate::theme::tokens::radius;
use crate::theme::type_scale;
use eframe::egui::{CornerRadius, Label, Rect, RichText, TextureHandle, Ui, pos2, vec2};
use glowfly_core::i18n::Lang;
use glowfly_core::lfs::pointer::size_text;
use glowfly_core::preview_font::FontFace;
use glowfly_core::preview_image::PreviewImage;

/// The scale both specimens are drawn at in a card of `card`'s size: the smaller of the scales
/// that fit each present one in the card's padded room, and never above 1. Image sizes are
/// points, so a side rasterised below 2x still compares at the same size.
pub(super) fn shared_scale(card: Rect, imgs: [Option<&PreviewImage>; 2]) -> f32 {
    let room = card.shrink(PAD);
    imgs.into_iter()
        .flatten()
        .map(|img| {
            let (w, h) = (img.width.max(1) as f32, img.height.max(1) as f32);
            (room.width() / w).min(room.height() / h).min(1.0)
        })
        .fold(1.0, f32::min)
        .max(0.0)
}

/// The raised card with the specimen at `scale`, at the left of its padded room and centred
/// vertically, its white glyphs tinted to the text colour.
pub(super) fn card(ui: &Ui, card: Rect, tex: &TextureHandle, img: &PreviewImage, scale: f32) {
    let p = ui.painter();
    p.rect_filled(card, CornerRadius::same(radius::MD), color::raised());
    let room = card.shrink(PAD);
    let (w, h) = (img.width.max(1) as f32 * scale, img.height.max(1) as f32 * scale);
    let rect = Rect::from_min_size(pos2(room.left(), room.center().y - h / 2.0), vec2(w, h));
    p.image(tex.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), color::text());
}

/// "{family} {style} · {format}[ · {n} faces] · {size}", empty parts left out, cut to the
/// column with the whole text on hover.
pub(super) fn caption(ui: &mut Ui, t: Lang, face: &FontFace, img: &PreviewImage, size: u64) {
    let name = [face.family.as_str(), face.style.as_str()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
    let faces = (face.faces > 1).then(|| t.trn("diff.font.faces", face.faces as usize, &[]));
    let parts = [Some(name), Some(img.format.to_owned()), faces, Some(size_text(size))];
    let text = parts.into_iter().flatten().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" \u{b7} ");
    // A cut label shows its whole text on hover by itself.
    ui.add(Label::new(RichText::new(text).font(type_scale::font(type_scale::SMALL)).color(color::weak())).truncate());
}
