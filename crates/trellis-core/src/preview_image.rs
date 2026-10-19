//! Decoding an image (PNG, JPEG, GIF, WebP or BMP) for the diff viewer's before/after preview
//! and the file history's File view: the image's own size, its format and its RGBA pixels,
//! downscaled when large so a texture of it stays small. Decoding is bounded (`MAX_SIDE`,
//! `MAX_ALLOC`) so a hostile or huge file is refused instead of eating memory; `decode_sized`
//! says why. An animation gives its first frame. CPU-only work: call it from a worker thread.

use image::{ImageError, ImageFormat, ImageReader, Limits};
use std::io::Cursor;
use std::sync::Arc;

/// The longest side a preview decodes; anything larger is refused.
pub const MAX_SIDE: u32 = 8192;
/// The most memory the decoder may allocate.
const MAX_ALLOC: u64 = 256 << 20;
/// The longest side of the pixels the diff keeps for drawing; larger images are scaled down.
pub const SHOWN_MAX: u32 = 1024;
/// The longest side of the pixels the File view keeps for drawing.
pub const FILE_VIEW_SHOWN_MAX: u32 = 2048;
/// The extensions `is_image_path` takes, compared ignoring ASCII case.
pub const IMAGE_EXTS: [&str; 6] = ["png", "jpg", "jpeg", "gif", "webp", "bmp"];

/// A decoded image: its original size, its format and the (possibly downscaled) pixels to draw.
#[derive(Clone)]
pub struct PreviewImage {
    /// The image's own width and height, in pixels.
    pub width: u32,
    pub height: u32,
    /// "PNG", "JPEG", "GIF", "WebP" or "BMP", found from the content.
    pub format: &'static str,
    /// The width and height of `rgba`, at most the asked-for size on the longer side.
    pub shown: [usize; 2],
    /// Unmultiplied RGBA, `shown[0] * shown[1] * 4` bytes.
    pub rgba: Arc<Vec<u8>>,
}

/// Sizes only: the pixels would flood the debug log.
impl std::fmt::Debug for PreviewImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [w, h] = self.shown;
        write!(f, "PreviewImage({} {}x{}, shown {w}x{h})", self.format, self.width, self.height)
    }
}

fn format_name(f: ImageFormat) -> Option<&'static str> {
    Some(match f {
        ImageFormat::Png => "PNG",
        ImageFormat::Jpeg => "JPEG",
        ImageFormat::Gif => "GIF",
        ImageFormat::WebP => "WebP",
        ImageFormat::Bmp => "BMP",
        _ => return None,
    })
}

fn too_large(w: u32, h: u32) -> String {
    format!("This image is too large to preview ({w} × {h})")
}

fn no_pixels(w: u32, h: u32) -> String {
    format!("This image has no pixels to preview ({w} × {h})")
}

/// `bytes` decoded as an image (the format guessed from the bytes), or `None` when they are
/// none of the five formats, are corrupt, or exceed the limits. Pixels at most `SHOWN_MAX` a side.
pub fn decode(bytes: &[u8]) -> Option<PreviewImage> {
    decode_sized(bytes, SHOWN_MAX).ok()
}

/// `bytes` decoded as an image with its pixels at most `shown_max` on the longer side. An
/// unknown format, too large an image, one without pixels or a broken file is an error that
/// says so.
pub fn decode_sized(bytes: &[u8], shown_max: u32) -> Result<PreviewImage, String> {
    let reader = || ImageReader::new(Cursor::new(bytes)).with_guessed_format().map_err(|e| e.to_string());
    let first = reader()?;
    let format = first.format().and_then(format_name).ok_or("this image format can't be previewed")?;
    let (width, height) = first.into_dimensions().map_err(|e| e.to_string())?;
    if width > MAX_SIDE || height > MAX_SIDE {
        return Err(too_large(width, height));
    }
    if width == 0 || height == 0 {
        // No GPU takes a texture without pixels.
        return Err(no_pixels(width, height));
    }
    let mut reader = reader()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_ALLOC);
    reader.limits(limits);
    let img = match reader.decode() {
        Ok(img) => img,
        Err(ImageError::Limits(_)) => return Err(too_large(width, height)),
        Err(e) => return Err(e.to_string()),
    };
    let (width, height) = (img.width(), img.height());
    if width == 0 || height == 0 {
        return Err(no_pixels(width, height));
    }
    let rgba = if width.max(height) > shown_max {
        let scale = shown_max as f64 / width.max(height) as f64;
        let w = ((width as f64 * scale).round() as u32).max(1);
        let h = ((height as f64 * scale).round() as u32).max(1);
        image::imageops::thumbnail(&img.to_rgba8(), w, h)
    } else {
        img.to_rgba8()
    };
    let shown = [rgba.width() as usize, rgba.height() as usize];
    Ok(PreviewImage { width, height, format, shown, rgba: Arc::new(rgba.into_raw()) })
}

/// Whether `path` names a file the preview can show, by its extension (`IMAGE_EXTS` in any
/// case). A guess at the file's type, not a path comparison, so case folding is right here.
pub fn is_image_path(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    let Some((stem, ext)) = name.rsplit_once('.') else { return false };
    !stem.is_empty() && IMAGE_EXTS.iter().any(|e| ext.eq_ignore_ascii_case(e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, Rgb, RgbImage, Rgba, RgbaImage};

    fn encode(img: impl Into<DynamicImage>, format: ImageFormat) -> Vec<u8> {
        let img = img.into();
        let mut out = Cursor::new(Vec::new());
        match format {
            // JPEG has no alpha channel.
            ImageFormat::Jpeg => img.to_rgb8().write_to(&mut out, format).unwrap(),
            _ => img.write_to(&mut out, format).unwrap(),
        }
        out.into_inner()
    }

    /// An image whose four left columns are transparent.
    fn rgba(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, y| Rgba([(x * 6) as u8, (y * 8) as u8, 128, if x < 4 { 0 } else { 255 }]))
    }

    #[test]
    fn each_format_decodes_with_its_size_and_name() {
        let cases = [
            (encode(rgba(40, 30), ImageFormat::Png), "PNG"),
            (encode(RgbImage::from_pixel(40, 30, Rgb([10, 200, 30])), ImageFormat::Jpeg), "JPEG"),
            (encode(rgba(40, 30), ImageFormat::Gif), "GIF"),
            (encode(rgba(40, 30), ImageFormat::WebP), "WebP"),
            (encode(rgba(40, 30), ImageFormat::Bmp), "BMP"),
        ];
        for (bytes, name) in cases {
            let p = decode_sized(&bytes, FILE_VIEW_SHOWN_MAX).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!((p.width, p.height, p.format, p.shown), (40, 30, name, [40, 30]), "{name}");
            assert_eq!(p.rgba.len(), 40 * 30 * 4, "{name}");
            assert!(decode(&bytes).is_some(), "{name} decodes for the diff too");
        }
    }

    #[test]
    fn transparency_is_kept() {
        let p = decode_sized(&encode(rgba(40, 30), ImageFormat::Png), FILE_VIEW_SHOWN_MAX).unwrap();
        assert_eq!(p.rgba[3], 0, "the first pixel is transparent");
        assert_eq!(p.rgba[4 * 39 + 3], 255);
    }

    #[test]
    fn large_images_are_scaled_down_for_drawing() {
        let img = RgbaImage::from_pixel(3000, 10, Rgba([0, 0, 255, 128]));
        let p = decode(&encode(img, ImageFormat::Png)).unwrap();
        assert_eq!((p.width, p.height), (3000, 10));
        assert!(p.shown[0] <= SHOWN_MAX as usize && p.shown[1] >= 1, "{p:?}");
        assert_eq!(p.rgba.len(), p.shown[0] * p.shown[1] * 4);
    }

    #[test]
    fn the_file_view_keeps_a_larger_size_and_the_aspect() {
        let p = decode_sized(&encode(RgbaImage::new(3000, 1000), ImageFormat::Png), FILE_VIEW_SHOWN_MAX).unwrap();
        assert_eq!((p.width, p.height), (3000, 1000), "the original size is reported");
        assert_eq!(p.shown, [2048, 683]);
        assert_eq!(p.rgba.len(), 2048 * 683 * 4);
    }

    #[test]
    fn oversized_images_are_refused_saying_so() {
        let wide = encode(RgbaImage::from_pixel(MAX_SIDE + 1, 1, Rgba([0, 0, 0, 255])), ImageFormat::Png);
        assert_eq!(decode_sized(&wide, FILE_VIEW_SHOWN_MAX).unwrap_err(), "This image is too large to preview (8193 × 1)");
        assert!(decode(&wide).is_none());
    }

    /// A GIF whose screen and only frame are `w` × `h` and interlaced, its pixel data empty.
    fn empty_gif(w: u16, h: u16) -> Vec<u8> {
        let mut b = b"GIF89a".to_vec();
        b.extend(w.to_le_bytes());
        b.extend(h.to_le_bytes());
        b.extend([0x80, 0, 0, 0, 0, 0, 255, 255, 255]); // a two-colour palette
        b.push(0x2C);
        b.extend([0, 0, 0, 0]);
        b.extend(w.to_le_bytes());
        b.extend(h.to_le_bytes());
        b.push(0x40); // interlaced
        b.extend([2, 1, 0x2C, 0]); // clear code, end code
        b.push(0x3B);
        b
    }

    #[test]
    fn images_without_pixels_are_refused() {
        // A texture with no pixels is fatal to the GPU, so these never reach a view.
        for (w, h) in [(0, 0), (5, 0), (0, 5)] {
            let err = decode_sized(&empty_gif(w, h), FILE_VIEW_SHOWN_MAX).unwrap_err();
            assert_eq!(err, format!("This image has no pixels to preview ({w} × {h})"), "{w}x{h}");
        }
    }

    #[test]
    fn garbage_and_truncated_files_are_errors() {
        assert_eq!(decode_sized(b"not an image at all", SHOWN_MAX).unwrap_err(), "this image format can't be previewed");
        assert!(decode(b"not an image at all").is_none());
        assert!(decode(b"\x89PNG\r\n\x1a\njunk after the magic").is_none());
        let png = encode(rgba(40, 30), ImageFormat::Png);
        assert!(decode_sized(&png[..png.len() / 2], SHOWN_MAX).is_err(), "a truncated PNG");
        assert!(decode_sized(&[], SHOWN_MAX).is_err());
    }

    #[test]
    fn the_format_comes_from_the_content() {
        // No name is involved at all: PNG bytes read as PNG whatever the file is called.
        assert_eq!(decode(&encode(rgba(8, 8), ImageFormat::Png)).unwrap().format, "PNG");
        assert_eq!(decode(&encode(rgba(8, 8), ImageFormat::Bmp)).unwrap().format, "BMP");
    }

    #[test]
    fn debug_leaves_out_the_pixels() {
        let p = decode(&encode(rgba(4, 3), ImageFormat::Png)).unwrap();
        let text = format!("{p:?}");
        assert_eq!(text, "PreviewImage(PNG 4x3, shown 4x3)");
    }

    #[test]
    fn image_paths_by_extension() {
        for p in ["a.png", "dir/b.JPG", "c.jpeg", "d.gif", "e.WebP", "f.bmp"] {
            assert!(is_image_path(p), "{p}");
        }
        for p in ["a.png.txt", "png", "dir.png/readme", ".gitignore", "logo.svg", "icon.ico", "data.bin", "dir/.png"] {
            assert!(!is_image_path(p), "{p}");
        }
    }
}
