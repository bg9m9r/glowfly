//! Rendering an SVG for the diff viewer's before/after preview: the drawing parsed and drawn by
//! resvg into the same `PreviewImage` the image preview shows, its longer side scaled to
//! `SHOWN_MAX`. Nothing outside the file is reached: an `<image>` that names a path or a URL
//! is skipped (only `data:` images inside the file are drawn), text is set only in the bundled
//! fonts (`bundled_fonts`), never the system's, and SVG has no scripts that resvg would run.
//! An `<image>`, `<use>` or `<feImage>` naming another file, here or in an SVG embedded as a
//! `data:` image, marks the render `linked`. A drawing whose filters or layers would cost too
//! much to draw is refused (`preview_svg_budget`).
//! CPU-only work: call it from a worker thread.

use crate::bundled_fonts;
use crate::preview_image::{PreviewImage, SHOWN_MAX};
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{self, ImageHrefResolver, ImageKind, Options, Tree, fontdb};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

/// The largest SVG file the preview renders; a larger one shows its sizes only.
pub const SVG_MAX_BYTES: u64 = 4 << 20;

/// What `render` gives back: the drawing, or why it couldn't be drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SvgError {
    /// The file isn't SVG that can be read (broken XML, no size, not UTF-8).
    Unreadable,
    /// The drawing has no area to draw.
    Empty,
    /// Its filters or layers would take too long or too much memory to draw.
    TooCostly,
    /// It was still drawing when the runner stopped waiting, or a newer request took its place.
    Abandoned,
}

/// The bundled fonts, loaded once and shared by every render.
fn fonts() -> Arc<fontdb::Database> {
    static DB: OnceLock<Arc<fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        for data in [bundled_fonts::NOTO_SANS, bundled_fonts::NOTO_SANS_SEMIBOLD, bundled_fonts::JETBRAINS_MONO] {
            db.load_font_source(fontdb::Source::Binary(Arc::new(data)));
        }
        db.set_serif_family("Noto Sans");
        db.set_sans_serif_family("Noto Sans");
        db.set_cursive_family("Noto Sans");
        db.set_fantasy_family("Noto Sans");
        db.set_monospace_family("JetBrains Mono");
        Arc::new(db)
    })
    .clone()
}

/// The options every render uses: the bundled fonts, and `<image>` hrefs other than `data:`
/// resolved to nothing (usvg's own resolver would read the path from the disk), each one
/// refused setting `linked`. An SVG embedded as a `data:` image is parsed with these same
/// options (usvg's own nested parse refuses its hrefs without a word), so its references
/// count too.
fn options(linked: Arc<AtomicBool>) -> Options<'static> {
    let data = ImageHrefResolver::default_data_resolver();
    let nested = linked.clone();
    Options {
        resources_dir: None,
        font_family: "Noto Sans".to_owned(),
        fontdb: fonts(),
        image_href_resolver: ImageHrefResolver {
            resolve_data: Box::new(move |mime, bytes, opts| {
                // Parsed once, here: usvg's own nested parse would call back into this
                // resolver for the SVGs inside it, doubling the work at every level.
                if mime == "image/svg+xml" || (mime == "text/plain" && !is_raster(&bytes)) {
                    parse(&bytes, opts, &nested).ok().map(ImageKind::SVG)
                } else {
                    data(mime, bytes, opts)
                }
            }),
            resolve_string: Box::new(move |_, _| {
                linked.store(true, Ordering::Relaxed);
                None
            }),
        },
        ..Options::default()
    }
}

/// `bytes` parsed with `opts`, setting `linked` when they name other files.
fn parse(bytes: &[u8], opts: &Options, linked: &AtomicBool) -> Result<Tree, usvg::Error> {
    if names_other_files(bytes) {
        linked.store(true, Ordering::Relaxed);
    }
    Tree::from_data(bytes, opts)
}

/// Whether `bytes` start like a PNG, JPEG, GIF or WebP image (as usvg's data resolver tells
/// them from an SVG).
fn is_raster(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x89, b'P', b'N', b'G'])
        || bytes.starts_with(&[0xff, 0xd8])
        || bytes.starts_with(b"GIF8")
        || (bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(&b"WEBP"[..]))
}

/// Whether an `<image>`, `<use>` or `<feImage>` in `bytes` has an `href` (or `xlink:href`)
/// that is neither a fragment of this file (`#id`) nor a `data:` URL. usvg drops a `<use>` of
/// another file without asking a resolver, so the text is read for it.
fn names_other_files(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else { return false };
    let mut rest = text;
    while let Some(at) = rest.find("href") {
        let (before, after) = rest.split_at(at);
        rest = &after[4..];
        let tag = before.rfind('<').map(|lt| &before[lt + 1..]).unwrap_or("");
        let name = tag.split(|c: char| c.is_whitespace() || c == '/' || c == '>').next().unwrap_or("");
        let name = name.rsplit(':').next().unwrap_or(name);
        if !matches!(name, "image" | "use" | "feImage") || tag.contains('>') {
            continue;
        }
        let value = rest.trim_start().strip_prefix('=').map(str::trim_start);
        let Some(value) = value.and_then(|v| v.strip_prefix(['"', '\''])) else { continue };
        let value = value.trim_start();
        if !(value.starts_with('#') || value.get(..5).is_some_and(|p| p.eq_ignore_ascii_case("data:"))) {
            return true;
        }
    }
    false
}

/// An SVG drawn: the picture, and whether it names files outside itself that were left out.
#[derive(Clone, Debug)]
pub struct SvgRender {
    pub image: PreviewImage,
    pub linked: bool,
}

/// `bytes` rendered as an SVG, its longer side scaled to `SHOWN_MAX` pixels (or a half or a
/// quarter of that when its filters would cost too much at full size). The image's
/// `width` and `height` are the drawing's own size in CSS pixels, rounded.
pub fn render(bytes: &[u8]) -> Result<SvgRender, SvgError> {
    let linked = Arc::new(AtomicBool::new(false));
    let tree = parse(bytes, &options(linked.clone()), &linked).map_err(|e| {
        tracing::debug!("svg preview: {e}");
        SvgError::Unreadable
    })?;
    let size = tree.size();
    let (w, h) = (size.width(), size.height());
    if !(w.is_finite() && h.is_finite()) || w <= 0.0 || h <= 0.0 {
        return Err(SvgError::Empty);
    }
    // Filters cost more the larger they are drawn: over budget, the drawing is tried at half
    // and a quarter of the size (an honest picture, only softer) before it is refused.
    let full = SHOWN_MAX as f32 / w.max(h);
    let scale =
        [full, full / 2.0, full / 4.0].into_iter().find(|&s| crate::preview_svg_budget::within(&tree, s)).ok_or(SvgError::TooCostly)?;
    let px = |v: f32| ((v * scale).round() as u32).clamp(1, SHOWN_MAX);
    let mut pixmap = Pixmap::new(px(w), px(h)).ok_or(SvgError::Empty)?;
    resvg::render(&tree, Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    let shown = [pixmap.width() as usize, pixmap.height() as usize];
    let rgba = unmultiplied(&pixmap);
    let own = |v: f32| (v.round() as u32).max(1);
    let image = PreviewImage { width: own(w), height: own(h), format: "SVG", shown, rgba: Arc::new(rgba) };
    Ok(SvgRender { image, linked: linked.load(Ordering::Relaxed) })
}

/// `pixmap`'s pixels as unmultiplied RGBA, as `PreviewImage::rgba` holds them (the font
/// preview's renderer uses it too).
pub(crate) fn unmultiplied(pixmap: &Pixmap) -> Vec<u8> {
    pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect()
}

/// Whether `path` names an SVG file, by its `.svg` extension in any case.
pub fn is_svg_path(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.rsplit_once('.').is_some_and(|(stem, ext)| !stem.is_empty() && ext.eq_ignore_ascii_case("svg"))
}

#[cfg(test)]
#[path = "preview_svg_tests.rs"]
mod tests;
