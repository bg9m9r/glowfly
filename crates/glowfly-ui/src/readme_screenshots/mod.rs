//! The README's screenshots (`docs/screenshots/`), rendered by tests from a made-up demo
//! repository (`demo.rs`) the way the view snapshots are, so no window opens and no real data
//! shows. `UPDATE_SCREENSHOTS=1 cargo test -p glowfly-ui readme_screenshots` writes the PNGs;
//! without it each test still renders its view and checks the stored image is there at the same
//! size (the pixels are left alone, so an unrelated view change doesn't fail them). With
//! `GLOWFLY_SKIP_SNAPSHOTS` set (CI on macOS and Linux) nothing is rendered.

mod demo;
mod demo_files;
mod pull_request;
mod shots;

use crate::snapshot_check::{SKIP_VAR, crop_image, skip_requested};
use crate::snapshot_fixtures::harness_scaled;
use crate::theme;
use eframe::egui;
use egui_kittest::Harness;
use glowfly_model::msg::Msg;
use glowfly_model::state::AppState;
use std::path::Path;

/// Where the screenshots are kept: the repository's `docs/screenshots`.
const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/screenshots");

/// The variable that asks for the screenshots to be written.
const UPDATE_VAR: &str = "UPDATE_SCREENSHOTS";

/// Every screenshot is drawn at two pixels per point, sharp on high-density screens.
const PPP: f32 = 2.0;

/// The most a screenshot may weigh.
const MAX_BYTES: u64 = 450 * 1024;

type H = Harness<'static, (AppState, Vec<Msg>)>;

/// A harness drawing `st` in Graphite (`dark`) or Porcelain, `size` points at `PPP`.
fn harness(st: AppState, dark: bool, size: egui::Vec2) -> H {
    harness_scaled(st, theme::view_palette(dark), size, PPP)
}

/// Whether `UPDATE_SCREENSHOTS` asks for new images.
fn updating() -> bool {
    std::env::var(UPDATE_VAR).is_ok_and(|v| !matches!(v.trim(), "" | "0" | "false" | "no" | "off"))
}

/// Renders `h`'s last frame, cut to `crop` (points), as `docs/screenshots/<name>.png`, or
/// checks the stored one is there at the same size.
fn shoot(h: &mut H, name: &str, crop: Option<egui::Rect>) {
    if skip_requested(std::env::var(SKIP_VAR).ok().as_deref()) {
        eprintln!("note: {SKIP_VAR} is set; not rendering screenshot {name}");
        return;
    }
    let image = h.render().unwrap_or_else(|e| panic!("could not render {name}: {e}"));
    let image = image::DynamicImage::ImageRgba8(crop_image(image, crop, PPP)).to_rgb8();
    let path = Path::new(DIR).join(format!("{name}.png"));
    if updating() {
        std::fs::create_dir_all(DIR).unwrap();
        write_png(&image, &path);
        let bytes = std::fs::metadata(&path).unwrap().len();
        println!("wrote {} ({}x{}, {} KB)", path.display(), image.width(), image.height(), bytes / 1024);
        assert!(bytes <= MAX_BYTES, "{name}.png is {} KB, over {} KB: make the window smaller", bytes / 1024, MAX_BYTES / 1024);
        return;
    }
    let stored = image::image_dimensions(&path).unwrap_or_else(|e| {
        panic!("no screenshot {} ({e}): run `{UPDATE_VAR}=1 cargo test -p glowfly-ui readme_screenshots`", path.display())
    });
    assert_eq!(
        stored,
        image.dimensions(),
        "{name}.png is no longer the size it renders at: run `{UPDATE_VAR}=1 cargo test -p glowfly-ui readme_screenshots`"
    );
}

/// Writes `image` as a PNG compressed as far as the encoder goes.
fn write_png(image: &image::RgbImage, path: &Path) {
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    let file = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    let encoder = PngEncoder::new_with_quality(file, CompressionType::Best, FilterType::Adaptive);
    image.write_with_encoder(encoder).unwrap();
}
