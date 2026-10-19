//! Decoding a PNG or JPEG for the diff viewer's before/after preview: the image's own size and
//! its RGBA pixels, downscaled when large so a texture of it stays small. Decoding is bounded
//! (`MAX_SIDE`, `MAX_ALLOC`) so a hostile or huge file gives `None` instead of eating memory.

use std::io::Cursor;
use std::sync::Arc;

/// The longest side a preview decodes; anything larger gives `None`.
pub const MAX_SIDE: u32 = 8192;
/// The most memory the decoder may allocate.
const MAX_ALLOC: u64 = 256 << 20;
/// The longest side of the pixels kept for drawing; larger images are scaled down to it.
pub const SHOWN_MAX: u32 = 1024;

/// A decoded image: its original size and the (possibly downscaled) pixels to draw.
#[derive(Clone)]
pub struct PreviewImage {
    /// The image's own width and height, in pixels.
    pub width: u32,
    pub height: u32,
    /// The width and height of `rgba`, at most `SHOWN_MAX` on the longer side.
    pub shown: [usize; 2],
    /// Unmultiplied RGBA, `shown[0] * shown[1] * 4` bytes.
    pub rgba: Arc<Vec<u8>>,
}

/// Sizes only: the pixels would flood the debug log.
impl std::fmt::Debug for PreviewImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PreviewImage({}x{}, shown {}x{})", self.width, self.height, self.shown[0], self.shown[1])
    }
}

/// `bytes` decoded as a PNG or JPEG (the format guessed from the bytes), or `None` when they
/// are neither, are corrupt, or exceed the limits.
pub fn decode(bytes: &[u8]) -> Option<PreviewImage> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format().ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_ALLOC);
    reader.limits(limits);
    let img = reader.decode().ok()?;
    let (width, height) = (img.width(), img.height());
    let rgba = if width.max(height) > SHOWN_MAX {
        let scale = SHOWN_MAX as f64 / width.max(height) as f64;
        let w = ((width as f64 * scale).round() as u32).max(1);
        let h = ((height as f64 * scale).round() as u32).max(1);
        image::imageops::thumbnail(&img.to_rgba8(), w, h)
    } else {
        img.to_rgba8()
    };
    let shown = [rgba.width() as usize, rgba.height() as usize];
    Some(PreviewImage { width, height, shown, rgba: Arc::new(rgba.into_raw()) })
}

/// Whether `path` names a file the preview can show, by its extension (png, jpg, jpeg in any
/// case). A guess at the file's type, not a path comparison, so case folding is right here.
pub fn is_image_path(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    let Some((stem, ext)) = name.rsplit_once('.') else { return false };
    !stem.is_empty() && ["png", "jpg", "jpeg"].iter().any(|e| ext.eq_ignore_ascii_case(e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgba, RgbaImage};

    fn encode(img: &RgbaImage, format: ImageFormat) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        match format {
            // JPEG has no alpha channel.
            ImageFormat::Jpeg => image::DynamicImage::ImageRgba8(img.clone()).to_rgb8().write_to(&mut out, format).unwrap(),
            _ => img.write_to(&mut out, format).unwrap(),
        }
        out.into_inner()
    }

    #[test]
    fn decodes_png_and_jpeg_at_their_size() {
        let img = RgbaImage::from_pixel(4, 3, Rgba([200, 10, 10, 255]));
        for format in [ImageFormat::Png, ImageFormat::Jpeg] {
            let p = decode(&encode(&img, format)).expect("decodes");
            assert_eq!((p.width, p.height, p.shown), (4, 3, [4, 3]), "{format:?}");
            assert_eq!(p.rgba.len(), 4 * 3 * 4);
        }
    }

    #[test]
    fn large_images_are_scaled_down_for_drawing() {
        let img = RgbaImage::from_pixel(3000, 10, Rgba([0, 0, 255, 128]));
        let p = decode(&encode(&img, ImageFormat::Png)).unwrap();
        assert_eq!((p.width, p.height), (3000, 10));
        assert!(p.shown[0] <= SHOWN_MAX as usize && p.shown[1] >= 1, "{p:?}");
        assert_eq!(p.rgba.len(), p.shown[0] * p.shown[1] * 4);
    }

    #[test]
    fn garbage_and_oversized_images_give_none() {
        assert!(decode(b"not an image at all").is_none());
        assert!(decode(b"\x89PNG\r\n\x1a\njunk after the magic").is_none());
        let wide = RgbaImage::from_pixel(MAX_SIDE + 1, 1, Rgba([0, 0, 0, 255]));
        assert!(decode(&encode(&wide, ImageFormat::Png)).is_none());
    }

    #[test]
    fn image_paths_by_extension() {
        for p in ["a.png", "dir/b.JPG", "c.jpeg"] {
            assert!(is_image_path(p), "{p}");
        }
        for p in ["a.png.txt", "png", "dir.png/readme", ".gitignore", "logo.svg", "data.bin", "dir/.png"] {
            assert!(!is_image_path(p), "{p}");
        }
    }
}
