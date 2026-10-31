//! The welcome screen's mark: the app icon beside the "Glowfly" wordmark, centred as one row.

use super::theme::{color, type_scale};
use eframe::egui::{Align, ColorImage, Context, Id, Image, Layout, RichText, TextureHandle, TextureOptions, Ui, vec2};

/// The icon's side next to the wordmark, a little taller than its capitals.
const ICON: f32 = 64.0;
/// Between the icon and the wordmark.
const GAP: f32 = 12.0;

/// The icon as a texture, decoded once per context (the bundled 256 px PNG).
fn icon(ctx: &Context) -> TextureHandle {
    let id = Id::new("app-mark-icon");
    if let Some(t) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return t;
    }
    let data = super::app_icon();
    let image = ColorImage::from_rgba_unmultiplied([data.width as usize, data.height as usize], &data.rgba);
    let t = ctx.load_texture("app-mark-icon", image, TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, t.clone()));
    t
}

/// The icon and the wordmark, centred in `ui`'s width.
pub fn show(ui: &mut Ui) {
    let font = type_scale::strong(ui, type_scale::WORDMARK);
    let word = ui.painter().layout_no_wrap("Glowfly".to_owned(), font.clone(), color::text());
    let size = vec2(ICON + GAP + word.size().x, ICON.max(word.size().y));
    let texture = icon(ui.ctx());
    ui.allocate_ui_with_layout(size, Layout::left_to_right(Align::Center), |ui| {
        ui.add(Image::new(&texture).fit_to_exact_size(vec2(ICON, ICON)));
        ui.add_space(GAP - ui.spacing().item_spacing.x);
        ui.label(RichText::new("Glowfly").font(font).color(color::text()));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_icon_is_decoded_once() {
        let ctx = Context::default();
        let (a, b) = (icon(&ctx), icon(&ctx));
        assert_eq!(a.id(), b.id());
        assert_eq!(a.size(), [256, 256]);
    }
}
