//! The File view's image preview as a texture: uploaded once per loaded version, and dropped
//! (freeing the GPU memory) at the end of the first frame that draws no image.

use super::image_paint;
use crate::state::history::FileTextData;
use eframe::egui::{self, TextureHandle};
use std::sync::Arc;
use trellis_core::preview_image::PreviewImage;

#[derive(Default)]
pub struct PreviewTexture {
    /// The text the texture was made from; held so its address can't be reused by another.
    data: Option<Arc<FileTextData>>,
    tex: Option<TextureHandle>,
    /// `get` was called this frame.
    used: bool,
}

impl PreviewTexture {
    /// The texture of `img` (the preview of `data`), uploaded again only for another `data`.
    pub(in crate::ui) fn get(&mut self, ctx: &egui::Context, data: &Arc<FileTextData>, img: &PreviewImage) -> &TextureHandle {
        self.used = true;
        if !self.data.as_ref().is_some_and(|d| Arc::ptr_eq(d, data)) {
            self.tex = None;
            self.data = Some(data.clone());
        }
        self.tex.get_or_insert_with(|| image_paint::load(ctx, "history-preview", img))
    }

    /// Called once a frame, after drawing: drops the texture when no image was drawn.
    pub(in crate::ui) fn end_frame(&mut self) {
        if !self.used {
            self.tex = None;
            self.data = None;
        }
        self.used = false;
    }

    #[cfg(test)]
    fn id(&self) -> Option<egui::TextureId> {
        self.tex.as_ref().map(TextureHandle::id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::history::Preview;
    use trellis_core::repo::LfsState;

    fn data() -> (Arc<FileTextData>, PreviewImage) {
        let mut png = Vec::new();
        image::RgbaImage::new(4, 3).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let image = trellis_core::preview_image::decode(&png).unwrap();
        let preview = Some(Preview::Image { image: image.clone(), size: png.len() as u64 });
        (Arc::new(FileTextData { lines: Vec::new(), spans: None, lfs: LfsState::None, preview }), image)
    }

    #[test]
    fn loads_once_per_text_and_drops_it_after_a_frame_without_one() {
        let ctx = egui::Context::default();
        let mut c = PreviewTexture::default();
        let (a, img) = data();
        let first = c.get(&ctx, &a, &img).id();
        c.end_frame();
        assert_eq!(c.get(&ctx, &a, &img).id(), first, "the same text keeps its texture across frames");
        c.end_frame();
        let (b, img_b) = data();
        assert_ne!(c.get(&ctx, &b, &img_b).id(), first, "another text gets a new one");
        c.end_frame();
        assert!(c.id().is_some(), "kept while drawn");
        c.end_frame();
        assert_eq!(c.id(), None, "a frame without the image drops it");
    }
}
