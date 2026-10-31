//! The one way view tests compare against approved snapshots (`check_snapshot`). Every check
//! compares a text snapshot of the frame (`text.rs`, `compare.rs`); the pixel goldens listed in
//! `goldens.rs` are also compared as images, cropped as listed. With `GLOWFLY_SKIP_SNAPSHOTS=1`
//! (CI on macOS and Linux, where the images were not rendered, wgpu may have no adapter and the
//! platform's own labels differ) the comparisons are skipped with a note, and the rest of each
//! test (driving the harness, checking messages) still runs. Every harness comes from
//! [`builder`], so harnesses reuse wgpu devices instead of each making its own.

use eframe::egui;
use eframe::egui::mutex::RwLock;
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state, default_wgpu_setup};
use egui_kittest::{Harness, HarnessBuilder, LazyRenderer, SnapshotError, SnapshotOptions, TestRenderer};
use std::sync::{Arc, Mutex};

mod compare;
mod goldens;
mod tests;
mod text;

pub(crate) use goldens::Crop;

pub(crate) const SKIP_VAR: &str = "GLOWFLY_SKIP_SNAPSHOTS";

/// How many pixels may differ from the approved image before a comparison fails. The approved
/// images were rendered on a developer machine; the Windows CI runner's renderer anti-aliases a
/// handful of edge pixels differently (12 to 14 per image), which says nothing about the layout.
/// A real change (a moved widget, a new label, a wrong colour) touches far more pixels than this.
pub(crate) const TOLERATED_PIXELS: i32 = 40;

/// Whether a comparison that found `diff` differing pixels still passes.
pub(crate) fn within_tolerance(diff: i32) -> bool {
    diff <= TOLERATED_PIXELS
}

/// Whether `value` (the variable's) asks for snapshots to be skipped: set to anything but
/// empty, `0` or `false`.
pub(crate) fn skip_requested(value: Option<&str>) -> bool {
    value.is_some_and(|v| !matches!(v.trim(), "" | "0" | "false"))
}

/// `Harness::builder()` for view tests: when a test takes a snapshot, its harness renders on a
/// wgpu device it borrows from a pool rather than one of its own. Making a device (an instance,
/// the adapters, the software rasterizer kittest prefers) took about a sixth of a view test's
/// time. Each device serves one harness at a time: sharing one between the test threads made
/// rendering crawl. The test's thread starts in Garden dark, the palette view snapshots draw
/// in unless a test picks another.
pub(crate) fn builder<S>() -> HarnessBuilder<S> {
    crate::ui::theme::color::set_palette(&crate::ui::theme::GARDEN_DARK);
    crate::ui::theme::fonts::use_test_cjk();
    Harness::builder().renderer(LazyRenderer::new(Pooled::take))
}

/// Devices no harness is using.
static POOL: Mutex<Vec<egui_wgpu::RenderState>> = Mutex::new(Vec::new());

/// kittest's wgpu renderer on a pooled device, which goes back to the pool when the harness is
/// dropped.
struct Pooled {
    renderer: WgpuTestRenderer,
    device: Option<egui_wgpu::RenderState>,
}

impl Pooled {
    /// A renderer of its own (egui's textures are per context) on a free device, made the way
    /// kittest makes its default one.
    fn take() -> Self {
        let free = POOL.lock().unwrap_or_else(|e| e.into_inner()).pop();
        let device = free.unwrap_or_else(|| create_render_state(default_wgpu_setup()));
        let renderer = egui_wgpu::Renderer::new(&device.device, device.target_format, None, 1, false);
        let state = egui_wgpu::RenderState { renderer: Arc::new(RwLock::new(renderer)), ..device.clone() };
        Self { renderer: WgpuTestRenderer::from_render_state(state), device: Some(device) }
    }
}

impl Drop for Pooled {
    fn drop(&mut self) {
        if let Some(device) = self.device.take() {
            POOL.lock().unwrap_or_else(|e| e.into_inner()).push(device);
        }
    }
}

impl TestRenderer for Pooled {
    fn handle_delta(&mut self, delta: &egui::TexturesDelta) {
        self.renderer.handle_delta(delta);
    }

    fn render(&mut self, ctx: &egui::Context, output: &egui::FullOutput) -> Result<image::RgbaImage, String> {
        self.renderer.render(ctx, output)
    }
}

pub(crate) trait CheckSnapshot {
    /// Compares the last frame's text snapshot, and for a golden its image (no more than
    /// `TOLERATED_PIXELS` may differ), unless snapshots are skipped.
    fn check_snapshot(&mut self, name: &str);
}

impl<S> CheckSnapshot for Harness<'_, S> {
    #[track_caller]
    fn check_snapshot(&mut self, name: &str) {
        if skip_requested(std::env::var(SKIP_VAR).ok().as_deref()) {
            eprintln!("note: {SKIP_VAR} is set; not comparing snapshot {name}");
            return;
        }
        let text = text::of(self);
        let dir = std::path::Path::new(compare::DIR);
        let text_result = compare::check(dir, name, &text, compare::updating());
        if let Some(crop) = goldens::crop(name) {
            let image = self.render().unwrap_or_else(|e| panic!("could not render {name}: {e}"));
            let image = crop_image(image, crop_rect(self, crop), self.ctx.pixels_per_point());
            match egui_kittest::try_image_snapshot_options(&image, name, &SnapshotOptions::default()) {
                Ok(()) => {}
                Err(SnapshotError::Diff { diff, .. }) if within_tolerance(diff) => {
                    eprintln!("note: snapshot {name} differs in {diff} pixels, within the tolerated {TOLERATED_PIXELS}");
                }
                Err(err) => panic!("{err}"),
            }
        }
        if let Err(err) = text_result {
            panic!("{err}");
        }
    }
}

/// The part of `h`'s window (in points) that `crop` keeps, or None for all of it.
fn crop_rect<S>(h: &Harness<'_, S>, crop: Crop) -> Option<egui::Rect> {
    let screen = h.ctx.screen_rect();
    match crop {
        Crop::Window => None,
        Crop::Top(height) => Some(egui::Rect::from_min_size(screen.min, egui::vec2(screen.width(), height))),
        Crop::TopLeft(width, height) => Some(egui::Rect::from_min_size(screen.min, egui::vec2(width, height))),
        Crop::Dialog => {
            let rect =
                text::dialog_rects(h).into_iter().reduce(|a, b| a.union(b)).unwrap_or_else(|| panic!("no dialog is open to crop to"));
            Some(rect.expand(DIALOG_ROOM))
        }
    }
}

/// Room kept around a cropped dialog for its shadow.
const DIALOG_ROOM: f32 = 24.0;

/// `image` cut to `rect` (in points, at `ppp` pixels per point), kept inside the image.
pub(crate) fn crop_image(image: image::RgbaImage, rect: Option<egui::Rect>, ppp: f32) -> image::RgbaImage {
    let Some(rect) = rect else { return image };
    let (w, h) = image.dimensions();
    let x0 = ((rect.min.x * ppp).floor().max(0.0) as u32).min(w);
    let y0 = ((rect.min.y * ppp).floor().max(0.0) as u32).min(h);
    let x1 = ((rect.max.x * ppp).ceil().max(0.0) as u32).clamp(x0, w);
    let y1 = ((rect.max.y * ppp).ceil().max(0.0) as u32).clamp(y0, h);
    image::imageops::crop_imm(&image, x0, y0, x1 - x0, y1 - y0).to_image()
}
