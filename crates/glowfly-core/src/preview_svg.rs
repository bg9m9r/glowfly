//! Rendering an SVG for the diff viewer's before/after preview: the drawing parsed and drawn by
//! resvg into the same `PreviewImage` the image preview shows, its longer side scaled to
//! `SHOWN_MAX`. Nothing outside the file is reached: an `<image>` that names a path or a URL
//! is skipped (only `data:` images inside the file are drawn), text is set only in the bundled
//! fonts (`bundled_fonts`), never the system's, and SVG has no scripts that resvg would run.
//! CPU-only work: call it from a worker thread.

use crate::bundled_fonts;
use crate::preview_image::{PreviewImage, SHOWN_MAX};
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{ImageHrefResolver, Options, Tree, fontdb};
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
/// refused setting `linked`.
fn options(linked: Arc<AtomicBool>) -> Options<'static> {
    Options {
        resources_dir: None,
        font_family: "Noto Sans".to_owned(),
        fontdb: fonts(),
        image_href_resolver: ImageHrefResolver {
            resolve_data: ImageHrefResolver::default_data_resolver(),
            resolve_string: Box::new(move |_, _| {
                linked.store(true, Ordering::Relaxed);
                None
            }),
        },
        ..Options::default()
    }
}

/// An SVG drawn: the picture, and whether it names files outside itself that were left out.
#[derive(Clone, Debug)]
pub struct SvgRender {
    pub image: PreviewImage,
    pub linked: bool,
}

/// `bytes` rendered as an SVG, its longer side scaled to `SHOWN_MAX` pixels. The image's
/// `width` and `height` are the drawing's own size in CSS pixels, rounded.
pub fn render(bytes: &[u8]) -> Result<SvgRender, SvgError> {
    let linked = Arc::new(AtomicBool::new(false));
    let tree = Tree::from_data(bytes, &options(linked.clone())).map_err(|e| {
        tracing::debug!("svg preview: {e}");
        SvgError::Unreadable
    })?;
    let size = tree.size();
    let (w, h) = (size.width(), size.height());
    if !(w.is_finite() && h.is_finite()) || w <= 0.0 || h <= 0.0 {
        return Err(SvgError::Empty);
    }
    let scale = SHOWN_MAX as f32 / w.max(h);
    let px = |v: f32| ((v * scale).round() as u32).clamp(1, SHOWN_MAX);
    let mut pixmap = Pixmap::new(px(w), px(h)).ok_or(SvgError::Empty)?;
    resvg::render(&tree, Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    let shown = [pixmap.width() as usize, pixmap.height() as usize];
    let rgba: Vec<u8> = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    let own = |v: f32| (v.round() as u32).max(1);
    let image = PreviewImage { width: own(w), height: own(h), format: "SVG", shown, rgba: Arc::new(rgba) };
    Ok(SvgRender { image, linked: linked.load(Ordering::Relaxed) })
}

/// Whether `path` names an SVG file, by its `.svg` extension in any case.
pub fn is_svg_path(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.rsplit_once('.').is_some_and(|(stem, ext)| !stem.is_empty() && ext.eq_ignore_ascii_case("svg"))
}

#[cfg(test)]
#[path = "preview_svg_tests.rs"]
mod tests;
