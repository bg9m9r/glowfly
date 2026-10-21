//! The window on Mica, Dark and Light: a busy stand-in wallpaper (diagonal stripes of the
//! `MICA_*` bands) is painted first, then the app with the backdrop its palette's tint gives.
//! The title bar, toolbar and sidebar show the stripes faintly through their tint; everything
//! else stays opaque.

use super::backdrop::{MICA_DARK, MICA_LIGHT, mica_tint};
use super::snapshot_check::CheckSnapshot;
use super::snapshot_tests::{NOW, remove_fixture, with_repo};
use super::theme::{self, DARK, LIGHT, color, color::Palette};
use super::{UiCache, avatars::Avatars, draw};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::window_chrome::Chrome;
use crate::state::window_native::Backdrop;
use eframe::egui::{self, Color32, LayerId, Shape, Stroke, pos2, vec2};
use egui_kittest::Harness;
use glowfly_core::repo::ChangeCounts;
use glowfly_core::settings::ThemeChoice;
use glowfly_core::spell::Speller;

/// Stripes of `bands` across the whole screen, on the layer the panels paint on, before them.
fn paint_wallpaper(ctx: &egui::Context, bands: &[Color32; 9]) {
    let screen = ctx.screen_rect();
    let p = ctx.layer_painter(LayerId::background());
    let (w, h) = (48.0, screen.height());
    let mut x = screen.left() - h;
    let mut i = 0;
    while x < screen.right() + w {
        let pts = vec![pos2(x, screen.top()), pos2(x + w, screen.top()), pos2(x + w + h, screen.bottom()), pos2(x + h, screen.bottom())];
        p.add(Shape::convex_polygon(pts, bands[i % bands.len()], Stroke::NONE));
        x += w;
        i += 1;
    }
}

/// The app at the window's opening size in palette `p`, over the stand-in wallpaper.
fn harness(mut st: AppState, p: Palette, bands: &'static [Color32; 9]) -> Harness<'static, (AppState, Vec<Msg>)> {
    st.view.now = NOW;
    st.view.utc_offset = 0;
    let mut cache: Option<UiCache> = None;
    Harness::builder().with_size(vec2(1440.0, 880.0)).build_state(
        move |ctx, (st, msgs): &mut (AppState, Vec<Msg>)| {
            color::set_palette(&p);
            theme::apply_style(ctx, &p);
            ctx.set_theme(theme::egui_theme(&p));
            let cache = cache.get_or_insert_with(|| UiCache {
                avatars: Avatars::default(),
                speller: Speller::new(),
                diff_rows: Default::default(),
                history_text: Default::default(),
                blame_rows: Default::default(),
                diff_images: Default::default(),
                markdown: Default::default(),
                history_preview: Default::default(),
            });
            if let Some(snap) = st.snap.as_mut() {
                snap.changes = ChangeCounts::of(&snap.staged, &snap.unstaged);
            }
            paint_wallpaper(ctx, bands);
            draw(ctx, st, cache, msgs);
        },
        (st, Vec::new()),
    )
}

fn mica(name: &str, theme: ThemeChoice, p: Palette, bands: &'static [Color32; 9]) {
    let (mut st, dir) = with_repo(theme, name);
    st.view.chrome = Chrome::Windows;
    st.settings.reduce_motion = true;
    let tint = mica_tint(&p).expect("the built-in palettes have a tint");
    st.view.backdrop = Backdrop::Mica { chrome: tint.chrome, side: tint.side };
    let mut h = harness(st, p, bands);
    for _ in 0..4 {
        h.step();
    }
    h.run();
    h.check_snapshot(name);
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn mica_snapshots() {
    mica("mica_dark", ThemeChoice::Dark, DARK, &MICA_DARK);
    mica("mica_light", ThemeChoice::Light, LIGHT, &MICA_LIGHT);
}
