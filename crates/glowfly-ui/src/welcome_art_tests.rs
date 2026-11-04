//! The welcome header's firefly (`welcome_art.rs`) in every built-in theme and the Garden twins:
//! which drawing each gets, the band keeping its text readable, every mark at 3:1 on the band
//! and none brown, the gold leading the trail, the art inside the band, and the same contrast
//! read back from painted pixels.

use super::*;
use crate::ui::snapshot_check::{SKIP_VAR, skip_requested};
use crate::ui::theme::{self, color};
use contrast::{brownish, ratio};
use glowfly_core::settings::BuiltinTheme;

/// Every built-in palette and the Garden twins, by name.
fn palettes() -> Vec<(String, Palette)> {
    let all = BuiltinTheme::ALL.into_iter().map(|t| (format!("{t:?}"), *theme::builtin(t)));
    all.chain([("GardenDark".into(), theme::GARDEN_DARK), ("GardenLight".into(), theme::GARDEN_LIGHT)]).collect()
}

/// The art's rect at the header's widest, on a 104 px band.
fn art_rect() -> Rect {
    Rect::from_min_size(pos2(0.0, 0.0), vec2(320.0, 104.0))
}

/// Every colour the drawing paints as a mark (not the band, the knock-out rings or the wings).
fn marks(s: &ArtStyle) -> Vec<(&'static str, Color32)> {
    let mut v = vec![("gold", s.gold), ("rose", s.rose), ("ice", s.ice)];
    if s.glows() {
        v.push(("lantern", LANTERN_TONES[0].1));
    }
    v
}

#[test]
fn only_the_high_contrast_palettes_get_the_high_contrast_drawing() {
    for (name, p) in palettes() {
        assert_eq!(art_style(&p).kind == ArtKind::HighContrast, p.high_contrast, "{name}");
    }
    assert_eq!(art_style(&theme::DARK).kind, ArtKind::Dark);
    assert_eq!(art_style(&theme::LIGHT).kind, ArtKind::Light);
}

#[test]
fn the_band_keeps_text_readable() {
    for (name, p) in palettes() {
        let s = art_style(&p);
        let text_min = if s.kind == ArtKind::HighContrast { 7.0 } else { 4.5 };
        assert!(ratio(p.text, s.band) >= text_min, "{name}: title on the band");
        assert!(ratio(p.weak, s.band) >= 4.5, "{name}: subtitle on the band");
        assert_eq!(s.rule.is_some(), s.kind == ArtKind::HighContrast, "{name}: a rule under the band only in high contrast");
    }
}

#[test]
fn every_mark_reads_on_the_band_and_none_is_brown() {
    for (name, p) in palettes() {
        let s = art_style(&p);
        for (what, c) in marks(&s) {
            assert!(ratio(c, s.band) >= 3.0, "{name}: {what} {c:?} at {:.2}:1", ratio(c, s.band));
        }
        let mut painted = marks(&s);
        if s.glows() {
            painted.extend(LANTERN_TONES.iter().map(|&(_, c)| ("lantern tone", c)));
            painted.extend(s.lantern_halo.iter().map(|&c| ("lantern halo", c)));
            for hue in [ROSE, ICE] {
                painted.extend(HEAD_HALO.iter().map(|&(_, k)| ("head halo", contrast::tint(s.band, hue, k, true))));
            }
        }
        for (what, c) in painted {
            assert!(!brownish(c), "{name}: {what} {c:?} reads brown");
        }
    }
}

/// The lantern's halo lifts the band toward the text, so it carries no hue of its own.
#[test]
fn the_lantern_halo_is_a_neutral_lift() {
    for (name, p) in palettes() {
        let s = art_style(&p);
        if !s.glows() {
            continue;
        }
        let band_chroma = contrast::oklch(s.band).1;
        for c in s.lantern_halo {
            assert!(!brownish(c), "{name}: {c:?} reads brown");
            assert!(contrast::oklch(c).1 <= band_chroma + 0.01, "{name}: {c:?} is more coloured than the band");
        }
    }
}

/// On the dark themes the firefly leads: the trail sits 70% of the way to its hues under a
/// full-strength gold. The light themes draw all three opaque at about 3.6 to 4:1 (the orange
/// can't go darker without turning brown) and high contrast all at full strength; there the
/// lantern's size leads instead.
#[test]
fn the_gold_leads_the_trail_on_the_dark_themes() {
    for (name, p) in palettes().into_iter().filter(|(_, p)| art_style(p).kind == ArtKind::Dark) {
        let s = art_style(&p);
        let gold = ratio(s.gold, s.band);
        assert!(gold >= ratio(s.rose, s.band) && gold >= ratio(s.ice, s.band), "{name}: gold {gold:.2}");
    }
}

#[test]
fn the_art_stays_inside_the_band() {
    let r = art_rect();
    let m = Map::new(r);
    let halo = lantern_ellipse(&m, LANTERN_HALO[0].0);
    let bottom = halo.iter().map(|p| p.y).fold(f32::MIN, f32::max);
    assert!(bottom - r.top() <= 100.0, "the lantern's halo reaches {bottom}");
    let (c, radius) = head(&m, 1);
    let top = c.y - radius * HEAD_HALO[0].0;
    assert!(top - r.top() >= 4.0, "the ice head's halo starts at {top}");
    let (ice, _) = head(&m, 1);
    assert!((r.right() - ice.x - 24.0).abs() < 0.01, "the ice head ends 24 px from the right edge");
    let wings: Vec<Pos2> = [false, true].into_iter().flat_map(|mirror| wing(&m, mirror)).collect();
    let (top, bottom) = wings.iter().fold((f32::MAX, f32::MIN), |(t, b), p| (t.min(p.y), b.max(p.y)));
    assert!(top - r.top() > 44.7 + 2.0 && bottom - r.top() <= 100.0, "the wings span {top}..{bottom}, clear of the rose lane");
    let lantern = m.at(LANTERN_AT);
    assert!(wings.iter().all(|p| p.x > lantern.x), "both wings fan forward, along the trail");
}

/// The drawing as painted into `art_rect` in `p`: the image and the style.
fn rendered(p: &Palette) -> (image::RgbaImage, ArtStyle) {
    let pal = *p;
    let s = art_style(p);
    let mut h = crate::ui::snapshot_check::builder().with_size(art_rect().size()).build_ui(move |ui| {
        color::set_palette(&pal);
        let r = art_rect();
        ui.painter().rect_filled(r, 0.0, s.band);
        paint(&ui.painter().with_clip_rect(r), r, &s);
    });
    h.run();
    let img = h.render().expect("render");
    (img, s)
}

/// The pixel at (x, y), opaque.
fn px(img: &image::RgbaImage, x: f32, y: f32) -> Color32 {
    let p = img.get_pixel(x as u32, y as u32);
    Color32::from_rgb(p[0], p[1], p[2])
}

/// The strongest contrast on the band in the column at `x`, within 2 px of row `y`.
fn strongest(img: &image::RgbaImage, band: Color32, x: f32, y: f32) -> (f64, Color32) {
    (-2..=2)
        .map(|d| px(img, x, y + d as f32))
        .map(|c| (ratio(c, band), c))
        .fold((0.0, Color32::BLACK), |a, b| if b.0 > a.0 { b } else { a })
}

#[test]
fn the_painted_lanes_read_on_the_band() {
    if skip_requested(std::env::var(SKIP_VAR).ok().as_deref()) {
        eprintln!("note: {SKIP_VAR} is set; not rendering the art");
        return;
    }
    let m = Map::new(art_rect());
    // A straight stretch of each lane, clear of its commits and heads.
    let gold = m.at((340.0, 760.0));
    let rose = m.at((612.0, 575.0));
    let ice = m.at((792.0, 300.0));
    let mut failures = Vec::new();
    for (name, p) in palettes() {
        let (img, s) = rendered(&p);
        let band = px(&img, art_rect().right() - 2.0, art_rect().bottom() - 2.0);
        if band != s.band {
            failures.push(format!("{name}: band painted {band:?}, not {:?}", s.band));
        }
        for (what, at) in [("gold", gold), ("rose", rose), ("ice", ice)] {
            let (r, c) = strongest(&img, band, at.x, at.y);
            if r < 3.0 {
                failures.push(format!("{name}: the {what} lane paints {c:?} at {r:.2}:1"));
            }
            if brownish(c) {
                failures.push(format!("{name}: the {what} lane paints brown {c:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
