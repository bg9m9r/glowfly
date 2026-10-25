//! The one way view tests compare against an approved image. With `GLOWFLY_SKIP_SNAPSHOTS=1`
//! (CI on macOS and Linux, where the images were not rendered and wgpu may have no adapter) the
//! comparison is skipped with a note, and the rest of each test (driving the harness, checking
//! messages) still runs. Every harness comes from [`builder`], so harnesses reuse wgpu devices
//! instead of each making its own.

use eframe::egui;
use eframe::egui::mutex::RwLock;
use egui_kittest::wgpu::{WgpuTestRenderer, create_render_state, default_wgpu_setup};
use egui_kittest::{Harness, HarnessBuilder, LazyRenderer, SnapshotError, TestRenderer};
use std::sync::{Arc, Mutex};

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
/// rendering crawl.
pub(crate) fn builder<S>() -> HarnessBuilder<S> {
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
    /// `Harness::snapshot`, unless snapshots are skipped, passing when no more than
    /// `TOLERATED_PIXELS` differ.
    fn check_snapshot(&mut self, name: &str);
}

impl<S> CheckSnapshot for Harness<'_, S> {
    #[track_caller]
    fn check_snapshot(&mut self, name: &str) {
        if skip_requested(std::env::var(SKIP_VAR).ok().as_deref()) {
            eprintln!("note: {SKIP_VAR} is set; not comparing snapshot {name}");
            return;
        }
        match self.try_snapshot(name) {
            Ok(()) => {}
            Err(SnapshotError::Diff { diff, .. }) if within_tolerance(diff) => {
                eprintln!("note: snapshot {name} differs in {diff} pixels, within the tolerated {TOLERATED_PIXELS}");
            }
            Err(err) => panic!("{err}"),
        }
    }
}

#[test]
fn tolerance() {
    assert!(within_tolerance(0));
    assert!(within_tolerance(14), "the CI renderer's usual difference");
    assert!(within_tolerance(TOLERATED_PIXELS));
    assert!(!within_tolerance(TOLERATED_PIXELS + 1));
}

#[test]
fn skip_values() {
    for v in ["1", "true", "yes"] {
        assert!(skip_requested(Some(v)), "{v}");
    }
    for v in [None, Some(""), Some("0"), Some("false"), Some(" ")] {
        assert!(!skip_requested(v), "{v:?}");
    }
}
