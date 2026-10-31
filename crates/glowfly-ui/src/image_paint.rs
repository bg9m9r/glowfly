//! Drawing a decoded image, shared by the binary diff's before/after preview and the file
//! history's File view: its texture (sharp pixels for small images, smooth for others) and the
//! light and dark checkerboard under it, so transparent pixels show as such.

use super::theme::color;
use eframe::egui::{Color32, ColorImage, Context, Painter, Rect, TextureHandle, TextureOptions, pos2, vec2};
use glowfly_core::preview_image::PreviewImage;

/// The checkerboard's squares.
const CHECK: f32 = 8.0;
/// Images this small or smaller are drawn with sharp pixels.
const SHARP_MAX: u32 = 256;

/// `img`'s pixels uploaded as a texture called `name`.
pub(crate) fn load(ctx: &Context, name: &str, img: &PreviewImage) -> TextureHandle {
    let pixels = ColorImage::from_rgba_unmultiplied(img.shown, &img.rgba);
    let options = if img.width.max(img.height) <= SHARP_MAX { TextureOptions::NEAREST } else { TextureOptions::LINEAR };
    ctx.load_texture(name, pixels, options)
}

/// Light and dark squares under `rect`, so transparent pixels show as such.
pub(crate) fn checkerboard(p: &Painter, rect: Rect) {
    let (light, dark) = if color::is_dark() {
        (Color32::from_gray(0x5a), Color32::from_gray(0x46))
    } else {
        (Color32::from_gray(0xf4), Color32::from_gray(0xdc))
    };
    p.rect_filled(rect, 0.0, light);
    let p = p.with_clip_rect(rect.intersect(p.clip_rect()));
    let cols = (rect.width() / CHECK).ceil() as usize;
    let rows = (rect.height() / CHECK).ceil() as usize;
    for r in 0..rows {
        for c in (r % 2..cols).step_by(2) {
            let min = pos2(rect.left() + c as f32 * CHECK, rect.top() + r as f32 * CHECK);
            p.rect_filled(Rect::from_min_size(min, vec2(CHECK, CHECK)), 0.0, dark);
        }
    }
}
